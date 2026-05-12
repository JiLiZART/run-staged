use std::{path::PathBuf, process};

use clap::Parser;

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
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // D-04: resolve relative --config paths against CWD (not git root) before
    // handing to lib::run. Absolute paths pass through unchanged.
    let config_path = cli.config.map(|p| {
        if p.is_relative() {
            std::env::current_dir()
                .expect("cannot determine current directory")
                .join(p)
        } else {
            p
        }
    });

    if let Err(e) = fast_staged::run(config_path, cli.unstaged, cli.allow_empty, cli.quiet).await {
        eprintln!("error: {e}");
        process::exit(1);
    }
}
