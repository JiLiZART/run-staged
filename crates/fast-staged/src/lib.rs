pub mod config;
mod error;
pub mod git;
pub use error::AppError;

pub async fn run() -> Result<(), AppError> {
    // Phase 2: Load config (auto-discover, no --config flag yet — Phase 6 adds CLI)
    let config = config::load_config(None)?;

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

    Ok(())
}
