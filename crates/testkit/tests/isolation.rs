//! Own test binary: it mutates the process environment, which is only safe
//! while no other test runs in the same process.

use run_staged_testkit::Repo;

#[test]
fn user_git_config_ignored() {
    let hostile = tempfile::tempdir().unwrap();
    let cfg = hostile.path().join("gitconfig");
    std::fs::write(
        &cfg,
        "[core]\n\tautocrlf = true\n\thooksPath = /nonexistent\n",
    )
    .unwrap();
    // SAFETY: single test in this binary; no other thread reads the env.
    unsafe {
        std::env::set_var("GIT_CONFIG_GLOBAL", &cfg);
        std::env::set_var("HOME", hostile.path());
        std::env::set_var("GIT_INDEX_FILE", "/nonexistent/index");
    }
    let repo = Repo::builder().staged("a", "a").build();
    for key in ["core.autocrlf", "core.hooksPath"] {
        let r = repo
            .runner()
            .program("git")
            .args(["config", "--get", key])
            .run();
        assert_eq!(r.code, Some(1), "{key} leaked into fixture: {r:?}");
    }
    assert_eq!(repo.staged_paths(), vec!["a"]);
}
