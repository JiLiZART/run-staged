pub mod config;
mod error;
pub mod git;
pub mod matcher;
pub use error::AppError;

pub async fn run(unstaged: bool) -> Result<(), AppError> {
    // Phase 2: Load config (auto-discover, no --config flag yet — Phase 6 adds CLI)
    let config = config::load_config(None)?;

    // Phase 4: Git integration (per D-12 — after config, before execution)
    let git_ctx = git::GitContext::open()?;

    // GIT-05: --unstaged flag branches between staged and unstaged file lists.
    // Unlike staged_files(), unstaged_files() does NOT error on an empty result —
    // users running with --unstaged expect a clean worktree to be a valid state.
    let files = if unstaged {
        git_ctx.unstaged_files()?
    } else {
        git_ctx.staged_files()?
    };

    // Backup captures the HEAD-vs-index diff. Skip it in --unstaged mode since
    // the flag targets worktree-only changes (no staged diff to preserve).
    if !unstaged {
        git_ctx.backup()?;
    }

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
    let label = if unstaged {
        "Unstaged files"
    } else {
        "Staged files"
    };
    eprintln!("{}: {}", label, files.len());
    for f in &files {
        eprintln!("  {}", f.display());
    }

    // Phase 5 will add: match files to commands, execute
    // On success: cleanup patch (GIT-04) — only meaningful when backup ran.
    if !unstaged {
        git_ctx.cleanup()?;
    }

    // Note: On command failure (Phase 5), call git_ctx.restore()? then git_ctx.cleanup()?
    // That wiring happens in Phase 5 when execution logic is added.

    Ok(())
}
