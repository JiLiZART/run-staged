use std::process;

#[tokio::main]
async fn main() {
    // Minimal flag detection — Phase 6 replaces this with full clap parsing.
    let args: Vec<String> = std::env::args().collect();
    let unstaged = args.iter().any(|a| a == "--unstaged");
    let allow_empty = args.iter().any(|a| a == "--allow-empty");
    let quiet = args.iter().any(|a| a == "--quiet");
    if let Err(e) = fast_staged::run(unstaged, allow_empty, quiet).await {
        eprintln!("error: {e}");
        process::exit(1);
    }
}
