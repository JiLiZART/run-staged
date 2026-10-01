//! Self-tests for the harness. They run `fixture-tool` (and `git`) through the
//! same runner that `apps/run-staged` tests use for the real binary.

use std::{process::Command, thread, time::Duration};

use run_staged_testkit::{Invocation, Repo, fixture_repo, fixture_tool_path};

fn tool(repo: &Repo) -> run_staged_testkit::Runner<'_> {
    repo.runner().program(fixture_tool_path())
}

// ── fixture-tool ─────────────────────────────────────────────────────────────

#[test]
fn fixture_tool_path_is_stable() {
    let a = fixture_tool_path();
    let b = fixture_tool_path();
    assert_eq!(a, b);
    assert!(a.is_file());
}

#[test]
fn formatter_stand_in_appends_and_logs() {
    let repo = Repo::builder().staged("a.js", "let a;\n").build();
    let r = tool(&repo).args(["--append", "// fmt", "a.js"]).run();
    assert!(r.success(), "{r:?}");
    assert!(repo.worktree_content("a.js").unwrap().ends_with("// fmt"));
    assert_eq!(
        repo.invocations(),
        vec![Invocation {
            args: vec!["--append".into(), "// fmt".into(), "a.js".into()],
            cwd: String::new(),
        }]
    );
}

#[test]
fn failing_linter_stand_in() {
    let repo = Repo::builder().staged("a.js", "x").build();
    let r = tool(&repo)
        .args(["--stderr", "lint error", "--exit", "1"])
        .run();
    assert_eq!(r.code, Some(1));
    assert!(r.stderr.contains("lint error"));
}

#[test]
fn replace_delete_stdout() {
    let repo = Repo::builder()
        .staged("a.js", "old")
        .staged("b.js", "b")
        .build();
    let r = tool(&repo)
        .args(["--replace", "new", "--stdout", "hi", "a.js"])
        .run();
    assert!(r.success());
    assert_eq!(r.stdout.trim(), "hi");
    assert_eq!(repo.worktree_content("a.js").as_deref(), Some("new"));
    assert!(tool(&repo).args(["--delete", "b.js"]).run().success());
    assert!(repo.worktree_content("b.js").is_none());
}

#[test]
fn sleep_option_sleeps() {
    let repo = Repo::builder().staged("a", "a").build();
    let start = std::time::Instant::now();
    assert!(tool(&repo).args(["--sleep", "200"]).run().success());
    assert!(start.elapsed() >= Duration::from_millis(200));
}

#[test]
fn parallel_invocations_keep_log_lines_intact() {
    let repo = Repo::builder().staged("a", "a").build();
    let handles: Vec<_> = (0..20)
        .map(|i| {
            let root = repo.root().to_path_buf();
            thread::spawn(move || {
                Command::new(fixture_tool_path())
                    .arg(format!("file-{i}"))
                    .current_dir(root)
                    .status()
                    .unwrap()
            })
        })
        .collect();
    for h in handles {
        // The files do not exist; no edit option was given, so this is fine.
        assert!(h.join().unwrap().success());
    }
    let inv = repo.invocations();
    assert_eq!(inv.len(), 20);
    let mut args: Vec<_> = inv.into_iter().map(|i| i.args[0].clone()).collect();
    args.sort();
    args.dedup();
    assert_eq!(args.len(), 20);
}

// ── repository builder ───────────────────────────────────────────────────────

#[test]
fn staged_over_committed() {
    let repo = fixture_repo!("staged-over-committed");
    assert_eq!(repo.git(&["show", "HEAD:a.js"]), "committed\n");
    assert_eq!(repo.index_content("a.js").as_deref(), Some("staged\n"));
    assert_eq!(repo.staged_paths(), vec!["a.js"]);
}

#[test]
fn partially_staged_file() {
    let repo = fixture_repo!("partial");
    assert_eq!(repo.index_content("a.js").as_deref(), Some("v2\n"));
    assert_eq!(repo.worktree_content("a.js").as_deref(), Some("v3\n"));
}

#[test]
fn repository_without_commits() {
    let repo = fixture_repo!("no-commits");
    let head = Command::new("git")
        .args(["rev-parse", "--verify", "-q", "HEAD"])
        .current_dir(repo.root())
        .output()
        .unwrap();
    assert!(!head.status.success(), "expected no commits");
    assert_eq!(repo.staged_paths(), vec!["a.txt", "src/b.txt"]);
}

#[test]
fn inline_fixture_with_git_config() {
    let repo = Repo::builder()
        .git_config("core.hooksPath", ".githooks")
        .file(".githooks/pre-commit", "#!/bin/sh\n")
        .staged("a", "a")
        .build();
    assert_eq!(repo.git(&["config", "core.hooksPath"]).trim(), ".githooks");
    assert!(repo.root().join(".githooks/pre-commit").is_file());
    // Extra files are never added to git.
    assert_eq!(repo.staged_paths(), vec!["a"]);
}

#[test]
fn tempdir_removed_on_drop() {
    let root = {
        let repo = Repo::builder().staged("a", "a").build();
        repo.root().to_path_buf()
    };
    if std::env::var_os("RUN_STAGED_KEEP_FIXTURES").is_none() {
        assert!(!root.exists());
    }
}

#[test]
fn parallel_repos_do_not_collide() {
    let a = fixture_repo!("partial");
    let b = fixture_repo!("partial");
    a.write("a.js", "changed");
    assert_ne!(a.root(), b.root());
    assert_eq!(b.worktree_content("a.js").as_deref(), Some("v3\n"));
}

// ── runner ───────────────────────────────────────────────────────────────────

#[test]
fn run_in_subdirectory() {
    let repo = Repo::builder().staged("packages/web/a.js", "a").build();
    assert!(tool(&repo).dir("packages/web").run().success());
    assert_eq!(repo.invocations()[0].cwd, "packages/web");
}

#[test]
#[should_panic(expected = "timeout")]
fn hung_binary_is_killed() {
    let repo = Repo::builder().staged("a", "a").build();
    tool(&repo)
        .args(["--sleep", "60000"])
        .timeout(Duration::from_secs(1))
        .run();
}

#[test]
#[should_panic(expected = "spawn git")]
fn run_without_git() {
    let repo = Repo::builder().staged("a", "a").build();
    repo.runner().program("git").args(["status"]).no_git().run();
}

#[test]
fn fixture_tool_is_first_on_path() {
    let repo = Repo::builder().staged("a", "a").build();
    // Resolved through the child's PATH, not an absolute path.
    let r = repo
        .runner()
        .program("fixture-tool")
        .args(["--exit", "7"])
        .run();
    assert_eq!(r.code, Some(7));
}

// ── lock helpers ─────────────────────────────────────────────────────────────

#[test]
fn lock_released_during_run() {
    let repo = Repo::builder().staged("a", "a").build();
    let lock = repo.git_dir().join("index.lock");
    let r = tool(&repo)
        .args(["--sleep", "1500"])
        .hold_index_lock(Some(Duration::from_secs(1)))
        .run();
    assert!(r.success());
    assert!(!r.lock_touched);
    assert!(!lock.exists());
}

#[test]
fn lock_held_for_whole_run_is_reported_untouched() {
    let repo = Repo::builder().staged("a", "a").build();
    let r = tool(&repo).hold_index_lock(None).run();
    assert!(!r.lock_touched);
    assert!(
        !repo.git_dir_has("index.lock"),
        "harness removes its lock afterwards"
    );
}

#[test]
fn commit_a_style_index() {
    let mut repo = Repo::builder()
        .committed("a.js", "v1")
        .staged("a.js", "v2")
        .build();
    repo.enable_commit_a_index();
    assert!(repo.git_dir_has("index.lock"));
    assert_eq!(repo.index_path(), repo.git_dir().join("index.lock"));
    // Processes run by the harness see GIT_INDEX_FILE.
    let r = repo.runner().program("git").args(["ls-files", "-s"]).run();
    assert!(r.stdout.contains("a.js"), "{r:?}");
    assert_eq!(repo.index_content("a.js").as_deref(), Some("v2"));
}

// ── assertions ───────────────────────────────────────────────────────────────

#[test]
fn assert_staged_content() {
    let repo = Repo::builder()
        .committed("a.js", "a")
        .staged("a.js", "ugly")
        .build();
    assert!(
        tool(&repo)
            .args(["--replace", "pretty", "a.js"])
            .run()
            .success()
    );
    repo.git(&["add", "a.js"]);
    assert_eq!(repo.index_content("a.js").as_deref(), Some("pretty"));
    assert!(!repo.git_dir_has("run-staged.index"));
}
