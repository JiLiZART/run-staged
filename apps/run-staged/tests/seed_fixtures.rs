//! One smoke test per checked-in fixture in `tests/fixtures/`. Output is
//! compared as a normalized snapshot, so these also pin the step lines and
//! the summary.

mod common;
use common::*;
use run_staged_testkit::fixture_repo;

#[test]
fn basic_js() {
    let repo = fixture_repo!("basic-js");
    let r = run_staged(&repo).run();
    assert_ok(&r);
    insta::assert_snapshot!(r.snapshot(&repo));
    for f in ["index.js", "util.ts"] {
        let staged = repo.index_content(f).unwrap();
        assert!(
            staged.ends_with("// formatted"),
            "{f} edit not staged: {staged:?}"
        );
    }
}

#[test]
fn rust_crate() {
    let repo = fixture_repo!("rust-crate");
    let r = run_staged(&repo).run();
    assert_ok(&r);
    let mut c = calls(&repo);
    c.sort();
    assert_eq!(c, vec![vec!["src/main.rs"], vec!["src/new_mod.rs"]]);
    insta::assert_snapshot!(r.snapshot(&repo));
}

#[test]
fn nested_config() {
    let repo = fixture_repo!("nested-config");
    let r = run_staged(&repo).dir("packages/web").run();
    assert_ok(&r);
    assert_eq!(
        calls(&repo),
        vec![vec!["--stdout", "web-config", "packages/web/src/app.ts"]]
    );
    insta::assert_snapshot!(r.snapshot(&repo));
}

#[test]
fn partial_stage() {
    let repo = fixture_repo!("partial-stage");
    let r = run_staged(&repo).run();
    assert_ok(&r);
    assert_eq!(
        repo.index_content("a.js").as_deref(),
        Some("line1\nstaged\n")
    );
    assert_eq!(
        repo.index_content("b.js").as_deref(),
        Some("full\n// formatted")
    );
    insta::assert_snapshot!(r.snapshot(&repo));
}

#[test]
fn no_commits() {
    let repo = fixture_repo!("no-commits");
    let r = run_staged(&repo).args(["--quiet"]).run();
    assert_ok(&r);
    let mut files: Vec<String> = calls(&repo).into_iter().flatten().collect();
    files.sort();
    assert_eq!(files, vec!["README.md", "run-staged.toml", "src/x.rs"]);
    insta::assert_snapshot!(r.snapshot(&repo));
}
