//! Guard: no operation of this crate may start a `git` process.
//!
//! Fixtures are built with the real git first; then PATH is replaced by a
//! directory whose only `git` is a stub that records every call. Own test
//! binary because it mutates PATH.

#![cfg(unix)]

use std::{os::unix::fs::PermissionsExt, path::PathBuf};

use run_staged_git::Repo;
use run_staged_testkit::Repo as Fixture;

#[test]
fn no_operation_spawns_git() {
    let fx = Fixture::builder()
        .committed("a.js", "1")
        .committed("gone.js", "g")
        .staged("a.js", "2")
        .staged("p.js", "staged")
        .unstaged("p.js", "unstaged")
        .build();
    let stub_dir = tempfile::tempdir().unwrap();
    let marker = stub_dir.path().join("git-was-called");
    let stub = stub_dir.path().join("git");
    std::fs::write(
        &stub,
        format!("#!/bin/sh\necho \"$@\" >> '{}'\nexit 1\n", marker.display()),
    )
    .unwrap();
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
    // SAFETY: single test in this binary; no other thread reads the env.
    unsafe { std::env::set_var("PATH", stub_dir.path()) };

    let files = |v: &[&str]| v.iter().map(PathBuf::from).collect::<Vec<_>>();
    let repo = Repo::open(fx.root(), None).unwrap();
    repo.staged_files().unwrap();
    repo.unstaged_files().unwrap();
    let snap = repo.snapshot(&files(&["a.js", "p.js"])).unwrap();
    repo.changes(&snap).unwrap();
    repo.backup().unwrap();
    std::fs::write(fx.root().join("a.js"), "3").unwrap();
    std::fs::remove_file(fx.root().join("gone.js")).unwrap();
    repo.stage(&files(&["a.js"]), &files(&["gone.js"])).unwrap();
    repo.restore().unwrap();
    repo.cleanup().unwrap();

    assert!(
        !marker.exists(),
        "git was started: {}",
        std::fs::read_to_string(&marker).unwrap_or_default()
    );
}
