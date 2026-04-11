pub mod config;
mod error;
pub mod git;
pub use error::AppError;

pub async fn run() -> Result<(), AppError> {
    // Phase 2: Load config (auto-discover, no --config flag yet — Phase 6 adds CLI)
    let config = config::load_config(None)?;

    // Phase 4: Git integration (per D-12 — after config, before execution)
    let git_ctx = git::GitContext::open()?;
    let staged_files = git_ctx.staged_files()?;

    // Create backup before any commands run (GIT-02)
    git_ctx.backup()?;

    // Debug output until TUI is built (Phase 8)
    eprintln!("Loaded config from {:?}", config.source);
    eprintln!("Groups: {}", config.groups.len());
    for group in &config.groups {
        eprintln!(
            "  Group '{}': {} pattern(s)",
            group.name,
            group.patterns.len()
        );
    }
    eprintln!("Staged files: {}", staged_files.len());
    for f in &staged_files {
        eprintln!("  {}", f.display());
    }

    // Phase 5 will add: match files to commands, execute
    // On success: cleanup patch (GIT-04)
    git_ctx.cleanup()?;

    // Note: On command failure (Phase 5), call git_ctx.restore()? then git_ctx.cleanup()?
    // That wiring happens in Phase 5 when execution logic is added.

    Ok(())
}
