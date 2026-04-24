use std::process;

#[tokio::main]
async fn main() {
    // Minimal flag detection — Phase 6 replaces this with full clap parsing.
    let unstaged = std::env::args().any(|a| a == "--unstaged");
    if let Err(e) = fast_staged::run(unstaged).await {
        eprintln!("error: {e}");
        process::exit(1);
    }
}
