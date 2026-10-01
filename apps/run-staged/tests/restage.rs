//! Spec: git-staging — "Stage task modifications" and "Partially staged files".

mod common;
use common::*;

const FORMAT: &str = "[fmt]\npatterns = { \"*.js\" = [\"fixture-tool --append ' // fmt'\"] }\n";

#[test]
fn formatter_fix_is_committed() {
    let repo = Repo::builder()
        .committed("a.js", "old")
        .staged("a.js", "new")
        .file("run-staged.toml", FORMAT)
        .build();
    assert_ok(&run_staged(&repo).run());
    assert_eq!(repo.index_content("a.js").as_deref(), Some("new // fmt"));
    let status = repo.git(&["status", "--porcelain", "--untracked-files=no"]);
    assert_eq!(status, "M  a.js\n", "worktree clean after re-staging");
}

#[test]
fn read_only_group_does_not_stage() {
    let repo = Repo::builder()
        .committed("a.js", "old")
        .staged("a.js", "new")
        .file(
            "run-staged.toml",
            "[lint]\nread_only = true\npatterns = { \"*.js\" = [\"fixture-tool --append ' // lint-touched'\"] }\n",
        )
        .build();
    assert_ok(&run_staged(&repo).run());
    assert_eq!(repo.index_content("a.js").as_deref(), Some("new"));
    assert_eq!(
        repo.worktree_content("a.js").as_deref(),
        Some("new // lint-touched")
    );
}

#[test]
fn file_in_read_only_and_writable_group_is_staged() {
    let repo = Repo::builder()
        .committed("a.js", "old")
        .staged("a.js", "new")
        .file(
            "run-staged.toml",
            "[lint]\nread_only = true\npatterns = { \"*.js\" = [\"fixture-tool\"] }\n\
             [fmt]\npatterns = { \"*.js\" = [\"fixture-tool --append ' // fmt'\"] }\n",
        )
        .build();
    assert_ok(&run_staged(&repo).run());
    assert_eq!(repo.index_content("a.js").as_deref(), Some("new // fmt"));
}

#[test]
fn failed_run_does_not_stage() {
    let repo = Repo::builder()
        .committed("a.js", "old")
        .staged("a.js", "new")
        .file(
            "run-staged.toml",
            "[fmt]\nexecution_order = \"sequential\"\n\
             patterns = { \"*.js\" = [\"fixture-tool --append ' // fmt'\", \"fixture-tool --exit 1\"] }\n",
        )
        .build();
    assert_fails(&run_staged(&repo).run(), 1);
    assert_eq!(repo.index_content("a.js").as_deref(), Some("new"));
    assert!(!repo.git_dir_has("run-staged.index"));
}

#[test]
fn deleted_by_task_is_removed_from_index() {
    let repo = Repo::builder()
        .committed("gen.js", "old")
        .staged("gen.js", "generated")
        .file(
            "run-staged.toml",
            "[clean]\npatterns = { \"gen.js\" = [\"fixture-tool --delete\"] }\n",
        )
        .build();
    assert_ok(&run_staged(&repo).run());
    assert_eq!(repo.index_content("gen.js"), None);
}

#[test]
fn unchanged_files_are_not_touched() {
    let repo = Repo::builder()
        .committed("a.js", "old")
        .staged("a.js", "new")
        .file(
            "run-staged.toml",
            "[lint]\npatterns = { \"*.js\" = [\"fixture-tool\"] }\n",
        )
        .build();
    let index_before = std::fs::read(repo.git_dir().join("index")).unwrap();
    assert_ok(&run_staged(&repo).run());
    assert_eq!(
        std::fs::read(repo.git_dir().join("index")).unwrap(),
        index_before
    );
}

#[test]
fn unstaged_hunk_protected() {
    let repo = Repo::builder()
        .committed("a.js", "line1\n")
        .staged("a.js", "line1\nstaged\n")
        .unstaged("a.js", "line1\nstaged\nunstaged\n")
        .file("run-staged.toml", FORMAT)
        .build();
    let r = run_staged(&repo).run();
    assert_ok(&r);
    assert_eq!(
        repo.index_content("a.js").as_deref(),
        Some("line1\nstaged\n")
    );
    assert!(
        r.stderr.contains("a.js") && r.stderr.contains("not staged"),
        "{}",
        r.stderr
    );
}

#[test]
fn unstaged_mode_stages_nothing() {
    let repo = Repo::builder()
        .committed("a.js", "old")
        .file("run-staged.toml", FORMAT)
        .build();
    repo.write("a.js", "changed");
    let index_before = std::fs::read(repo.git_dir().join("index")).unwrap();
    assert_ok(&run_staged(&repo).args(["--unstaged"]).run());
    assert_eq!(
        std::fs::read(repo.git_dir().join("index")).unwrap(),
        index_before
    );
}

#[test]
fn runs_without_git_on_path() {
    let repo = Repo::builder()
        .committed("a.js", "old")
        .staged("a.js", "new")
        .file("run-staged.toml", FORMAT)
        .build();
    let r = run_staged(&repo).no_git().run();
    assert_ok(&r);
    assert_eq!(repo.index_content("a.js").as_deref(), Some("new // fmt"));
    assert!(!repo.git_dir_has("run-staged.index"));
}
