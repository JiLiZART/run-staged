use std::{
    io::IsTerminal,
    path::PathBuf,
    process::ExitCode,
    sync::{Arc, Mutex},
    time::Instant,
};

use clap::Parser;
use run_staged_core::{Plan, RunOptions, Step, StepEvent};
use run_staged_exec::{EventSink, ExecOptions};
use run_staged_output::{
    DEFAULT_BOX_WIDTH, StepState, format_partial_warning, format_step, format_summary,
    render_results,
};
use run_staged_tui::{Event, LiveView, PlanGroup, PlanInfo, StepKind, Theme, terminal_width};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Path to the config file (overrides auto-discovery)
    #[arg(long)]
    config: Option<PathBuf>,

    /// Suppress success summary output
    #[arg(long)]
    quiet: bool,

    /// Run on modified (not staged) files instead of staged files
    #[arg(long)]
    unstaged: bool,

    /// Exit 0 when no staged files or no patterns match
    #[arg(long)]
    allow_empty: bool,

    /// Install a git pre-commit hook that runs run-staged
    #[arg(long, conflicts_with_all = ["unstaged", "allow_empty"])]
    install: bool,
}

/// Exit code after Ctrl+C (128 + SIGINT).
const INTERRUPTED: u8 = 130;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(e) => return fail(format!("cannot determine current directory: {e}")),
    };

    if cli.install {
        return match run_staged_core::install_hook(&cwd) {
            Ok(path) => {
                println!("installed pre-commit hook at {}", path.display());
                ExitCode::SUCCESS
            }
            Err(e) => fail(e),
        };
    }

    let quiet = cli.quiet;
    let colored = color_allowed();
    // Failure boxes span the terminal they are printed to.
    let box_width = std::io::stderr()
        .is_terminal()
        .then(terminal_width)
        .flatten()
        .unwrap_or(DEFAULT_BOX_WIDTH);
    // The live panel replaces step lines and success lines on a terminal.
    let live = (std::io::stdout().is_terminal() && !quiet && !dumb_term()).then(|| {
        Arc::new(Mutex::new(Some(LiveView::start(Theme {
            color: !no_color(),
        }))))
    });
    let sender = live
        .as_ref()
        .and_then(|v| v.lock().ok()?.as_ref().map(LiveView::sender));
    let send = |e: Event| {
        if let Some(tx) = &sender {
            let _ = tx.send((Instant::now(), e));
        }
    };
    let exec_events: Option<EventSink> = sender.clone().map(|tx| {
        Arc::new(move |e| {
            let _ = tx.send((Instant::now(), Event::Exec(e)));
        }) as EventSink
    });

    let opts = RunOptions {
        cwd,
        config_path: cli.config,
        unstaged: cli.unstaged,
        allow_empty: cli.allow_empty,
        index_file: std::env::var_os("GIT_INDEX_FILE").map(PathBuf::from),
        exec: ExecOptions {
            events: exec_events,
            force_color: colored,
        },
    };
    let tasks_label = if cli.unstaged {
        "running tasks for unstaged files"
    } else {
        "running tasks for staged files"
    };

    // In live mode, failure boxes wait until the panel's final frame is drawn.
    let mut deferred: Vec<String> = Vec::new();
    let mut on_step = |event: StepEvent<'_>| {
        let (state, step) = match event {
            StepEvent::Planned(plan) => {
                send(Event::Plan(plan_info(plan)));
                return;
            }
            StepEvent::Outcomes(outcomes) => {
                if live.is_some() {
                    deferred.extend(render_results(outcomes, true, colored, box_width).stderr);
                } else {
                    let rendered = render_results(outcomes, quiet, colored, box_width);
                    rendered.stdout.iter().for_each(|l| println!("{l}"));
                    rendered.stderr.iter().for_each(|l| eprintln!("{l}"));
                }
                return;
            }
            StepEvent::Started(s) => (StepState::Started, s),
            StepEvent::Finished(s) => (StepState::Finished, s),
            StepEvent::Failed(s) => (StepState::Failed, s),
        };
        if live.is_some() {
            let kind = step_kind(step);
            send(match state {
                StepState::Started => Event::StepStarted(kind),
                StepState::Finished => Event::StepFinished(kind),
                StepState::Failed => Event::StepFailed(kind),
            });
            return;
        }
        if quiet {
            return;
        }
        let what = match step {
            Step::Backup => "backing up original state",
            Step::Tasks => tasks_label,
            Step::Stage => "staging changes from tasks",
            Step::Cleanup => "cleaning up temporary files",
        };
        println!("{}", format_step(state, what));
    };

    // One thread: commands are separate processes, so all the concurrency here
    // is waiting on pipes and exits. A worker per core only costs startup time.
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => return fail(format!("cannot start async runtime: {e}")),
    };
    if let Some(view) = &live {
        // Ctrl+C: finish the frame, restore the terminal, and exit like a shell would.
        let view = Arc::clone(view);
        runtime.spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                if let Some(v) = view.lock().ok().and_then(|mut v| v.take()) {
                    let _ = v.finish();
                }
                std::process::exit(INTERRUPTED.into());
            }
        });
    }
    let result = runtime.block_on(run_staged_core::run(opts, &mut on_step));
    if let Some(v) = live.as_ref().and_then(|v| v.lock().ok()?.take()) {
        let _ = v.finish();
    }
    deferred.iter().for_each(|l| eprintln!("{l}"));

    let report = match result {
        Ok(report) => report,
        Err(e) => return fail(e),
    };
    for path in &report.partial_warnings {
        eprintln!("{}", format_partial_warning(path));
    }
    if report.ran && !quiet {
        println!("{}", format_summary(report.file_count, report.elapsed));
    }
    match report.error {
        Some(e) => fail(e),
        None => ExitCode::SUCCESS,
    }
}

fn plan_info(plan: &Plan) -> PlanInfo {
    PlanInfo {
        unstaged: plan.unstaged,
        file_count: plan.file_count,
        groups: plan
            .groups
            .iter()
            .map(|g| PlanGroup {
                name: g.name.clone(),
                patterns: g.patterns.clone(),
                file_count: g.file_count,
            })
            .collect(),
        invocations: plan.invocations.clone(),
    }
}

fn step_kind(step: Step) -> StepKind {
    match step {
        Step::Backup => StepKind::Backup,
        Step::Tasks => StepKind::Tasks,
        Step::Stage => StepKind::Stage,
        Step::Cleanup => StepKind::Cleanup,
    }
}

/// Colored failure boxes and forced command colors: stderr is a terminal,
/// `NO_COLOR` is unset or empty, and `TERM` is not `dumb`.
fn color_allowed() -> bool {
    std::io::stderr().is_terminal() && !no_color() && !dumb_term()
}

fn no_color() -> bool {
    std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty())
}

fn dumb_term() -> bool {
    std::env::var_os("TERM").is_some_and(|v| v == "dumb")
}

fn fail(message: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {message}");
    ExitCode::FAILURE
}
