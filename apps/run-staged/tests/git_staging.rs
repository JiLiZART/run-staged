//! Spec: git-staging (existing requirements; re-staging and lock behavior
//! live in restage.rs and locks.rs).

mod common;
use common::*;

const LINT_ALL: &str = "[g]\npatterns = { \"*\" = [\"fixture-tool\"] }\n";

#[test]
fn outside_a_repository() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("run-staged.toml"), LINT_ALL).unwrap();
    let repo = Repo::builder().build();
    let r = run_staged(&repo).dir(dir.path()).run();
    assert_eq!(assert_fails(&r, 1).trim(), "error: not a git repository");
}

#[test]
fn committed_files_are_not_relinted() {
    let repo = Repo::builder()
        .committed("src/main.rs", "fn main() {}")
        .staged("README.md", "hi")
        .file("run-staged.toml", LINT_ALL)
        .build();
    assert_ok(&run_staged(&repo).run());
    assert_eq!(calls(&repo), vec![vec!["README.md"]]);
}

#[test]
fn deleted_file_excluded() {
    let repo = Repo::builder()
        .committed("old.rs", "x")
        .staged("new.rs", "y")
        .file("run-staged.toml", LINT_ALL)
        .build();
    repo.git(&["rm", "-q", "old.rs"]);
    assert_ok(&run_staged(&repo).run());
    assert_eq!(calls(&repo), vec![vec!["new.rs"]]);
}

#[test]
fn fresh_repository() {
    let repo = Repo::builder()
        .staged("a.txt", "a")
        .file("run-staged.toml", LINT_ALL)
        .build();
    assert_ok(&run_staged(&repo).run());
    assert_eq!(calls(&repo), vec![vec!["a.txt"]]);
}

#[test]
fn nothing_staged() {
    let repo = Repo::builder()
        .committed("a", "a")
        .file("run-staged.toml", LINT_ALL)
        .build();
    let r = run_staged(&repo).run();
    assert_eq!(
        assert_fails(&r, 1).trim(),
        "error: no staged files — run 'git add' to stage files"
    );
}

#[test]
fn nothing_staged_with_allow_empty() {
    let repo = Repo::builder()
        .committed("a", "a")
        .file("run-staged.toml", LINT_ALL)
        .build();
    let r = run_staged(&repo).args(["--allow-empty"]).run();
    assert_ok(&r);
    assert_eq!((r.stdout.as_str(), r.stderr.as_str()), ("", ""));
}

#[test]
fn unstaged_files_used() {
    let repo = Repo::builder()
        .committed("a.rs", "a")
        .committed("b.rs", "b")
        .file("run-staged.toml", LINT_ALL)
        .build();
    repo.write("a.rs", "changed");
    let r = run_staged(&repo).args(["--unstaged"]).run();
    assert_ok(&r);
    assert_eq!(calls(&repo), vec![vec!["a.rs"]]);
}

#[test]
fn no_backup_in_unstaged_mode() {
    let repo = Repo::builder()
        .committed("a.rs", "a")
        .file(
            "run-staged.toml",
            "[g]\npatterns = { \"*\" = [\"fixture-tool --copy-git-file run-staged.index --exit 1\"] }\n",
        )
        .build();
    repo.write("a.rs", "changed");
    let index_before = std::fs::read(repo.git_dir().join("index")).unwrap();
    assert_fails(&run_staged(&repo).args(["--unstaged"]).run(), 1);
    assert!(!repo.git_dir_has("fixture-tool.run-staged.index"));
    assert!(!repo.git_dir_has("run-staged.index"));
    assert_eq!(
        std::fs::read(repo.git_dir().join("index")).unwrap(),
        index_before
    );
}

#[test]
fn backup_written_before_commands() {
    let repo = Repo::builder()
        .committed("a.rs", "a")
        .staged("a.rs", "b")
        .file(
            "run-staged.toml",
            "[g]\npatterns = { \"*\" = [\"fixture-tool --copy-git-file run-staged.index\"] }\n",
        )
        .build();
    let index_before = std::fs::read(repo.git_dir().join("index")).unwrap();
    assert_ok(&run_staged(&repo).run());
    assert_eq!(
        std::fs::read(repo.git_dir().join("fixture-tool.run-staged.index")).unwrap(),
        index_before,
        "the backup existed mid-run and equals the pre-run index"
    );
}

#[test]
fn no_match_no_backup() {
    let repo = Repo::builder()
        .staged("a.md", "a")
        .file(
            "run-staged.toml",
            "[g]\npatterns = { \"*.rs\" = [\"fixture-tool\"] }\n",
        )
        .build();
    assert_fails(&run_staged(&repo).run(), 1);
    assert!(!repo.git_dir_has("run-staged.index"));
}

#[test]
fn failure_restores_and_cleans_up() {
    let repo = Repo::builder()
        .committed("a.js", "1")
        .staged("a.js", "2")
        .unstaged("extra.txt", "not staged")
        .file(
            "run-staged.toml",
            "[g]\nexecution_order = \"sequential\"\n\
             patterns = { \"a.js\" = [\"git add extra.txt\", \"fixture-tool --exit 1\"] }\n",
        )
        .build();
    let index_before = std::fs::read(repo.git_dir().join("index")).unwrap();
    assert_fails(&run_staged(&repo).run(), 1);
    assert_eq!(
        std::fs::read(repo.git_dir().join("index")).unwrap(),
        index_before
    );
    assert_eq!(repo.staged_paths(), vec!["a.js"]);
    assert!(!repo.git_dir_has("run-staged.index"));
}

#[test]
fn success_cleans_up() {
    let repo = Repo::builder()
        .staged("a.js", "a")
        .file("run-staged.toml", LINT_ALL)
        .build();
    assert_ok(&run_staged(&repo).run());
    assert!(!repo.git_dir_has("run-staged.index"));
}
