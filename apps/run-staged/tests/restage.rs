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

#[test]
fn same_size_rewrite_is_staged() {
    let repo = Repo::builder()
        .committed("a.js", "old")
        .staged("a.js", "aaaa")
        .file(
            "run-staged.toml",
            "[fmt]\npatterns = { \"*.js\" = [\"fixture-tool --replace bbbb\"] }\n",
        )
        .build();
    assert_ok(&run_staged(&repo).run());
    assert_eq!(repo.index_content("a.js").as_deref(), Some("bbbb"));
}

#[test]
fn stale_stat_with_same_content_is_not_staged_or_partial() {
    let repo = Repo::builder()
        .committed("a.js", "old")
        .staged("a.js", "new")
        .file(
            "run-staged.toml",
            "[lint]\npatterns = { \"*.js\" = [\"fixture-tool\"] }\n",
        )
        .build();
    // Rewrite identical bytes: the index entry's stat no longer matches.
    repo.write("a.js", "new");
    let index_before = std::fs::read(repo.git_dir().join("index")).unwrap();
    let r = run_staged(&repo).run();
    assert_ok(&r);
    assert!(!r.stderr.contains("not staged"), "{}", r.stderr);
    assert_eq!(
        std::fs::read(repo.git_dir().join("index")).unwrap(),
        index_before
    );
}

#[test]
fn untouched_partial_file_has_no_warning() {
    let repo = Repo::builder()
        .committed("a.js", "line1\n")
        .staged("a.js", "line1\nstaged\n")
        .unstaged("a.js", "line1\nstaged\nunstaged\n")
        .file(
            "run-staged.toml",
            "[lint]\npatterns = { \"*.js\" = [\"fixture-tool\"] }\n",
        )
        .build();
    let r = run_staged(&repo).run();
    assert_ok(&r);
    assert!(!r.stderr.contains("not staged"), "{}", r.stderr);
    assert_eq!(
        repo.index_content("a.js").as_deref(),
        Some("line1\nstaged\n")
    );
}

#[test]
fn many_files_formatted_and_partial_ones_protected() {
    let mut b = Repo::builder().file("run-staged.toml", FORMAT);
    for i in 0..200 {
        b = b.committed(&format!("f{i}.js"), "old");
        b = b.staged(&format!("f{i}.js"), format!("new{i}"));
    }
    b = b.unstaged("f7.js", "new7 unstaged");
    let repo = b.build();
    let r = run_staged(&repo).run();
    assert_ok(&r);
    for i in (0..200).filter(|i| *i != 7) {
        assert_eq!(
            repo.index_content(&format!("f{i}.js")),
            Some(format!("new{i} // fmt"))
        );
    }
    assert_eq!(repo.index_content("f7.js").as_deref(), Some("new7"));
    assert!(r.stderr.contains("f7.js"), "{}", r.stderr);
}
