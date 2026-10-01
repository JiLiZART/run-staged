//! Own test binary: mutates `GIT_DIR` / `GIT_WORK_TREE` in the process
//! environment, which is only safe with a single test per process.

use std::path::PathBuf;

use run_staged_git::Repo;
use run_staged_testkit::Repo as Fixture;

#[test]
fn git_dir_and_work_tree_from_environment() {
    let fx = Fixture::builder()
        .committed("a.js", "1")
        .staged("b.js", "2")
        .build();
    let elsewhere = tempfile::tempdir().unwrap();
    // SAFETY: single test in this binary; no other thread reads the env.
    unsafe {
        std::env::set_var("GIT_DIR", fx.git_dir());
        std::env::set_var("GIT_WORK_TREE", fx.root());
    }
    let repo = Repo::open(elsewhere.path(), None).expect("open via GIT_DIR");
    assert_eq!(
        std::fs::canonicalize(repo.work_dir()).unwrap(),
        std::fs::canonicalize(fx.root()).unwrap()
    );
    assert_eq!(repo.staged_files().unwrap(), vec![PathBuf::from("b.js")]);
}
