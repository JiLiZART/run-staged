//! Spec: git-staging — "Lock-free reads", "Locked index writes", and the
//! hook-provided index (`GIT_INDEX_FILE`, as during `git commit -a`).

use std::time::Duration;

mod common;
use common::*;

const FORMAT: &str = "[fmt]\npatterns = { \"*.js\" = [\"fixture-tool --append ' // fmt'\"] }\n";

fn formatter_repo() -> Repo {
    Repo::builder()
        .committed("a.js", "old")
        .staged("a.js", "new")
        .file("run-staged.toml", FORMAT)
        .build()
}

#[test]
fn index_locked_by_another_git_process() {
    let repo = Repo::builder()
        .committed("a.js", "old")
        .staged("a.js", "new")
        .file(
            "run-staged.toml",
            "[lint]\nread_only = true\npatterns = { \"*.js\" = [\"fixture-tool --append ' // x'\"] }\n",
        )
        .build();
    let r = run_staged(&repo).hold_index_lock(None).run();
    assert_ok(&r);
    assert!(
        !r.lock_touched,
        "run-staged must not touch another process's lock"
    );
    assert_eq!(repo.invocations().len(), 1, "commands ran");
}

#[test]
fn commands_may_use_git_during_the_run() {
    let repo = Repo::builder()
        .committed("a.js", "old")
        .staged("a.js", "new")
        .unstaged("extra.txt", "x")
        .file(
            "run-staged.toml",
            "[g]\nexecution_order = \"sequential\"\n\
             patterns = { \"a.js\" = [\"git status --short\", \"git add extra.txt\"] }\n",
        )
        .build();
    let r = run_staged(&repo).run();
    assert_ok(&r);
    let mut staged = repo.staged_paths();
    staged.sort();
    assert_eq!(
        staged,
        vec!["a.js", "extra.txt"],
        "git add inside a command worked"
    );
}

#[test]
fn busy_lock_released_in_time() {
    let repo = formatter_repo();
    let r = run_staged(&repo)
        .hold_index_lock(Some(Duration::from_secs(1)))
        .run();
    assert_ok(&r);
    assert_eq!(repo.index_content("a.js").as_deref(), Some("new // fmt"));
    assert!(!repo.git_dir_has("run-staged.index"));
}

#[test]
fn lock_never_released() {
    let repo = formatter_repo();
    let r = run_staged(&repo).hold_index_lock(None).run();
    let stderr = assert_fails(&r, 1).to_string();
    assert!(stderr.contains("index.lock"), "names the lock:\n{stderr}");
    assert!(
        stderr.contains("run-staged.index"),
        "names the backup:\n{stderr}"
    );
    assert!(repo.git_dir_has("run-staged.index"), "backup kept");
    assert!(!r.lock_touched);
    assert_eq!(repo.index_content("a.js").as_deref(), Some("new"));
}

#[test]
fn restore_failure_keeps_backup() {
    let repo = Repo::builder()
        .committed("a.js", "old")
        .staged("a.js", "new")
        .file(
            "run-staged.toml",
            "[g]\npatterns = { \"*.js\" = [\"fixture-tool --exit 1\"] }\n",
        )
        .build();
    let r = run_staged(&repo).hold_index_lock(None).run();
    let stderr = assert_fails(&r, 1).to_string();
    assert!(stderr.contains("failed to restore the index"), "{stderr}");
    assert!(
        stderr.contains("run-staged.index") && stderr.contains("index"),
        "{stderr}"
    );
    assert!(repo.git_dir_has("run-staged.index"));
}

#[test]
fn hook_provided_index() {
    let mut repo = Repo::builder()
        .committed("a.js", "old")
        .file("run-staged.toml", FORMAT)
        .build();
    // `git commit -a`: git stages into .git/index.lock and hands it to the hook.
    repo.enable_commit_a_index();
    repo.write("a.js", "new");
    repo.git(&["add", "a.js"]);
    let default_index = std::fs::read(repo.git_dir().join("index")).unwrap();

    let r = run_staged(&repo).run();
    assert_ok(&r);
    assert_eq!(repo.index_content("a.js").as_deref(), Some("new // fmt"));
    assert!(!repo.git_dir_has("index.lock.lock"));
    assert_eq!(
        std::fs::read(repo.git_dir().join("index")).unwrap(),
        default_index
    );
}
