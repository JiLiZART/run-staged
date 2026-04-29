pub mod commands;
pub mod config;
mod error;
pub mod git;
pub mod matcher;
pub mod output;

pub use error::AppError;

/// Top-level orchestrator. Loads config, discovers staged files, matches them
/// to command groups, executes the commands, and ensures the staged diff is
/// restored on failure.
///
/// Flags:
/// - `unstaged`: operate on modified-but-not-staged files. Skips backup/restore/cleanup
///   because there is no staged diff to preserve (Phase 4 D-12).
/// - `allow_empty`: when true, exit silently with `Ok(())` if there are no staged files
///   OR if no staged files match any configured pattern (D-21, D-22, D-23). Skips
///   backup and cleanup entirely in the no-op path.
/// - `quiet`: suppress success summary lines. Failure boxes always print (D-12).
pub async fn run(unstaged: bool, allow_empty: bool, quiet: bool) -> Result<(), AppError> {
    let config = config::load_config(None)?;
    let git_ctx = git::GitContext::open()?;

    // Step 1: discover files. --allow-empty suppresses NoStagedFiles → Ok(()) silently.
    let files_result = if unstaged {
        git_ctx.unstaged_files()
    } else {
        git_ctx.staged_files()
    };
    let files = match files_result {
        Ok(f) => f,
        Err(AppError::NoStagedFiles) if allow_empty => return Ok(()),
        Err(e) => return Err(e),
    };

    // Step 2: --unstaged + --allow-empty: empty file list → silent ok (D-22).
    if unstaged && files.is_empty() && allow_empty {
        return Ok(());
    }

    // Step 3: match files to groups BEFORE backup. --allow-empty suppresses NoFilesMatched.
    // Per D-23, the no-op exit must never write the patch file — so matching has to run
    // before backup(). When match returns NoFilesMatched with --allow-empty, we return
    // Ok(()) immediately; no backup was taken so there is nothing to clean up.
    let matched = match matcher::match_files(&config, &files) {
        Ok(m) => m,
        Err(AppError::NoFilesMatched) if allow_empty => return Ok(()),
        Err(e) => return Err(e),
    };

    // Step 4: backup AFTER matching so the patch is only written when we actually run
    // commands. Skip in --unstaged mode (Phase 4 D-12 — no staged diff to preserve).
    if !unstaged {
        git_ctx.backup()?;
    }

    // Step 5: derive git_root (workdir) for the executor's path_format absolute mode.
    let git_root = git_ctx.work_dir();

    // Step 6: execute everything. Returns all outcomes regardless of failures.
    let outcomes = commands::execute_all(&matched, &config, git_root).await;

    // Step 7: print results. Success lines suppressed by quiet; failure boxes always.
    let _failure_count = output::print_results(&outcomes, quiet);

    // Step 8: branch on outcomes for restore/cleanup wiring (D-28, D-31).
    let first_failure = outcomes.iter().find(|o| o.is_failure());
    if let Some(failure) = first_failure {
        // Failure path.
        if !unstaged {
            // D-31: restore FIRST, cleanup ONLY if restore succeeded.
            if let Err(restore_err) = git_ctx.restore() {
                // D-30: do NOT call cleanup — the user needs the patch for manual recovery.
                let patch_path = git_ctx.git_dir().join("fast-staged.patch");
                return Err(AppError::RestoreFailed {
                    patch_path,
                    reason: Box::new(restore_err),
                });
            }
            git_ctx.cleanup()?;
        }
        // Build the typed error from the first failure for the binary's exit-code path.
        return Err(failure_to_app_error(failure));
    }

    // Success path: only cleanup, no restore.
    if !unstaged {
        git_ctx.cleanup()?;
    }
    Ok(())
}

/// Convert the first Failure CommandOutcome into a typed AppError so main.rs
/// can print it and exit non-zero (EXEC-03).
fn failure_to_app_error(outcome: &commands::CommandOutcome) -> AppError {
    use commands::CommandOutcome;
    match outcome {
        CommandOutcome::Failure {
            command, exit_code, ..
        } => AppError::CommandFailed {
            command: command.clone(),
            exit_code: *exit_code,
        },
        CommandOutcome::Cancelled { command, .. } => {
            // Reuse the existing CommandFailed variant for sibling-cancellation cases.
            // (Plan 07 owns Ctrl+C-driven cancellation; this is a sibling-failure cascade.)
            AppError::CommandFailed {
                command: command.clone(),
                exit_code: -1,
            }
        }
        CommandOutcome::Success { .. } => unreachable!("only failures should reach here"),
    }
}
