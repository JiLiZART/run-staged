use std::{path::PathBuf, process::ExitCode};

use clap::Parser;
use run_staged_core::{RunOptions, Step, StepEvent};
use run_staged_output::{
    StepState, format_partial_warning, format_step, format_summary, render_results,
};

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

    let opts = RunOptions {
        cwd,
        config_path: cli.config,
        unstaged: cli.unstaged,
        allow_empty: cli.allow_empty,
        index_file: std::env::var_os("GIT_INDEX_FILE").map(PathBuf::from),
    };
    let tasks_label = if cli.unstaged {
        "running tasks for unstaged files"
    } else {
        "running tasks for staged files"
    };
    let quiet = cli.quiet;
    let mut on_step = |event: StepEvent<'_>| {
        let (state, step) = match event {
            StepEvent::Outcomes(outcomes) => {
                let rendered = render_results(outcomes, quiet);
                rendered.stdout.iter().for_each(|l| println!("{l}"));
                rendered.stderr.iter().for_each(|l| eprintln!("{l}"));
                return;
            }
            StepEvent::Started(s) => (StepState::Started, s),
            StepEvent::Finished(s) => (StepState::Finished, s),
            StepEvent::Failed(s) => (StepState::Failed, s),
        };
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

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => return fail(format!("cannot start async runtime: {e}")),
    };
    let report = match runtime.block_on(run_staged_core::run(opts, &mut on_step)) {
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

fn fail(message: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {message}");
    ExitCode::FAILURE
}
