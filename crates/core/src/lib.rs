//! Orchestrates one run-staged invocation:
//! config → git listing → matching → backup → execute → restore or stage → cleanup.
//!
//! [`run`] returns `Err` only for problems before any command runs. Once
//! commands have run, it returns a [`RunReport`] whose `error` carries a
//! command failure, a restore failure, or a staging failure, so the caller can
//! still print every outcome and the summary.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use run_staged_config::ConfigError;
use run_staged_exec::{CommandOutcome, ExecOptions, PlannedInvocation};
use run_staged_git::{GitError, Repo};
use run_staged_matcher::{MatchError, MatchedGroup};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Config(#[from] ConfigError),

    #[error(transparent)]
    Match(#[from] MatchError),

    #[error(transparent)]
    Git(#[from] GitError),

    #[error("command `{command}` failed with exit code {exit_code}")]
    CommandFailed { command: String, exit_code: i32 },

    #[error("failed to stage task changes: {source}\nthe index backup was kept at {}", backup.display())]
    StageFailed { backup: PathBuf, source: GitError },

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Writes the git `pre-commit` hook; see [`run_staged_git::install_hook`].
pub fn install_hook(cwd: &Path) -> Result<PathBuf, AppError> {
    Ok(run_staged_git::install_hook(cwd)?)
}

/// Inputs of one run. Paths are as the user gave them; `cwd` is the process cwd.
#[derive(Clone, Default)]
pub struct RunOptions {
    pub cwd: PathBuf,
    pub config_path: Option<PathBuf>,
    pub unstaged: bool,
    pub allow_empty: bool,
    /// Value of `GIT_INDEX_FILE`, if set.
    pub index_file: Option<PathBuf>,
    /// Per-command progress events and color forcing, passed to the executor.
    pub exec: ExecOptions,
}

/// What a run will do, announced by [`StepEvent::Planned`] before the backup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// `true` in `--unstaged` mode (only the tasks step runs).
    pub unstaged: bool,
    /// Distinct files matched by at least one group.
    pub file_count: usize,
    /// Groups with matched files, in declaration order.
    pub groups: Vec<PlanGroup>,
    /// Every invocation that may run, with the ids exec events carry.
    pub invocations: Vec<PlannedInvocation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanGroup {
    pub name: String,
    /// Patterns that matched at least one file.
    pub patterns: Vec<String>,
    /// Distinct files matched in this group.
    pub file_count: usize,
}

/// The run steps reported through [`StepEvent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Backup,
    Tasks,
    Stage,
    Cleanup,
}

#[derive(Debug)]
pub enum StepEvent<'a> {
    /// The run's shape, emitted once after matching and before any step.
    Planned(&'a Plan),
    Started(Step),
    Finished(Step),
    Failed(Step),
    /// All command outcomes, emitted right before `Tasks` finishes or fails.
    Outcomes(&'a [CommandOutcome]),
}

/// What happened in a run that got as far as matching files.
#[derive(Debug, Default)]
pub struct RunReport {
    /// `false` when the run exited early (e.g. nothing to do with `--allow-empty`).
    pub ran: bool,
    pub outcomes: Vec<CommandOutcome>,
    /// Distinct files matched by at least one group.
    pub file_count: usize,
    pub elapsed: Duration,
    /// Partially staged files a command modified; they were not re-staged.
    pub partial_warnings: Vec<PathBuf>,
    /// Command failure, restore failure, or staging failure.
    pub error: Option<AppError>,
}

pub async fn run(
    opts: RunOptions,
    on_step: &mut dyn FnMut(StepEvent<'_>),
) -> Result<RunReport, AppError> {
    let start = Instant::now();
    let config_path = opts.config_path.as_ref().map(|p| opts.cwd.join(p));
    let config = run_staged_config::load_config(&opts.cwd, config_path.as_deref())?;
    let repo = Repo::open(&opts.cwd, opts.index_file.clone())?;

    let files = match if opts.unstaged {
        repo.unstaged_files()
    } else {
        repo.staged_files()
    } {
        Ok(f) => f,
        Err(GitError::NoStagedFiles) if opts.allow_empty => return Ok(RunReport::default()),
        Err(e) => return Err(e.into()),
    };
    if opts.unstaged && files.is_empty() && opts.allow_empty {
        return Ok(RunReport::default());
    }
    let matched = match run_staged_matcher::match_files(&config, &files) {
        Ok(m) => m,
        Err(MatchError::NoFilesMatched) if opts.allow_empty => return Ok(RunReport::default()),
        Err(e) => return Err(e.into()),
    };

    let mut report = RunReport {
        ran: true,
        file_count: distinct_files(&matched, |_| true).len(),
        ..Default::default()
    };
    let plan = Plan {
        unstaged: opts.unstaged,
        file_count: report.file_count,
        groups: matched
            .iter()
            .map(|g| PlanGroup {
                name: g.group.name.clone(),
                patterns: g.patterns.iter().map(|p| p.pattern.clone()).collect(),
                file_count: distinct_files(std::slice::from_ref(g), |_| true).len(),
            })
            .collect(),
        invocations: run_staged_exec::plan(&matched, repo.work_dir()),
    };
    on_step(StepEvent::Planned(&plan));

    if opts.unstaged {
        report.outcomes = run_tasks(&matched, &config, repo.work_dir(), &opts.exec, on_step).await;
        report.error = first_failure(&report.outcomes);
        report.elapsed = start.elapsed();
        return Ok(report);
    }

    // Snapshot what commands may change, before they run. Only files matched by
    // a group that is not read-only are ever re-staged.
    let candidates: Vec<PathBuf> = distinct_files(&matched, |g| !g.group.read_only)
        .into_iter()
        .collect();
    let partial = repo.partially_staged(&candidates)?;
    let before = repo.worktree_ids(&candidates)?;

    step(on_step, Step::Backup, || repo.backup().map(drop))?;

    report.outcomes = run_tasks(&matched, &config, repo.work_dir(), &opts.exec, on_step).await;

    if let Some(err) = first_failure(&report.outcomes) {
        report.error = Some(err);
        on_step(StepEvent::Started(Step::Cleanup));
        match repo.restore() {
            Ok(()) => on_step(StepEvent::Finished(Step::Cleanup)),
            Err(e) => {
                on_step(StepEvent::Failed(Step::Cleanup));
                report.error = Some(e.into());
            }
        }
        report.elapsed = start.elapsed();
        return Ok(report);
    }

    on_step(StepEvent::Started(Step::Stage));
    let staged = stage_changes(&repo, &candidates, &before, &partial).map(|warnings| {
        report.partial_warnings = warnings;
    });
    if let Err(source) = staged {
        on_step(StepEvent::Failed(Step::Stage));
        report.error = Some(AppError::StageFailed {
            backup: repo.backup_path(),
            source,
        });
        report.elapsed = start.elapsed();
        return Ok(report);
    }
    on_step(StepEvent::Finished(Step::Stage));

    if let Err(e) = step(on_step, Step::Cleanup, || repo.cleanup()) {
        report.error = Some(e);
    }
    report.elapsed = start.elapsed();
    Ok(report)
}

/// Stages files commands changed, except partially staged ones, which are
/// returned as warnings when a command modified them.
fn stage_changes(
    repo: &Repo,
    candidates: &[PathBuf],
    before: &[Option<run_staged_git::ObjectId>],
    partial: &[PathBuf],
) -> Result<Vec<PathBuf>, GitError> {
    let after = repo.worktree_ids(candidates)?;
    let mut changed = Vec::new();
    let mut deleted = Vec::new();
    let mut warnings = Vec::new();
    for ((path, b), a) in candidates.iter().zip(before).zip(&after) {
        if b == a {
            continue;
        }
        if partial.contains(path) {
            warnings.push(path.clone());
        } else if a.is_some() {
            changed.push(path.clone());
        } else {
            deleted.push(path.clone());
        }
    }
    repo.stage(&changed, &deleted)?;
    Ok(warnings)
}

async fn run_tasks(
    matched: &[MatchedGroup<'_>],
    config: &run_staged_config::Config,
    work_dir: &Path,
    exec: &ExecOptions,
    on_step: &mut dyn FnMut(StepEvent<'_>),
) -> Vec<CommandOutcome> {
    on_step(StepEvent::Started(Step::Tasks));
    let outcomes = run_staged_exec::execute_all_with(matched, config, work_dir, exec).await;
    on_step(StepEvent::Outcomes(&outcomes));
    if outcomes.iter().any(CommandOutcome::is_failure) {
        on_step(StepEvent::Failed(Step::Tasks));
    } else {
        on_step(StepEvent::Finished(Step::Tasks));
    }
    outcomes
}

fn step<T, E: Into<AppError>>(
    on_step: &mut dyn FnMut(StepEvent<'_>),
    s: Step,
    f: impl FnOnce() -> Result<T, E>,
) -> Result<T, AppError> {
    on_step(StepEvent::Started(s));
    match f() {
        Ok(v) => {
            on_step(StepEvent::Finished(s));
            Ok(v)
        }
        Err(e) => {
            on_step(StepEvent::Failed(s));
            Err(e.into())
        }
    }
}

fn distinct_files(
    matched: &[MatchedGroup<'_>],
    include_group: impl Fn(&MatchedGroup<'_>) -> bool,
) -> BTreeSet<PathBuf> {
    matched
        .iter()
        .filter(|g| include_group(g))
        .flat_map(|g| g.patterns.iter().flat_map(|p| p.files.iter().cloned()))
        .collect()
}

/// The typed error for the first failed or cancelled invocation.
fn first_failure(outcomes: &[CommandOutcome]) -> Option<AppError> {
    outcomes.iter().find_map(|o| match o {
        CommandOutcome::Failure {
            command, exit_code, ..
        } => Some(AppError::CommandFailed {
            command: command.clone(),
            exit_code: *exit_code,
        }),
        CommandOutcome::Cancelled { command, .. } => Some(AppError::CommandFailed {
            command: command.clone(),
            exit_code: -1,
        }),
        CommandOutcome::Success { .. } => None,
    })
}
