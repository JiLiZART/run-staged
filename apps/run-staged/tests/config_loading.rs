//! Spec: config-loading.

mod common;
use common::*;

const OK: &str = "[g]\npatterns = { \"*.rs\" = [\"fixture-tool\"] }\n";

#[test]
fn higher_priority_file_wins() {
    let repo = Repo::builder()
        .staged("a.rs", "a")
        .file(
            ".run-staged.toml",
            "[dot]\npatterns = { \"*.rs\" = [\"fixture-tool --stdout from-dot\"] }\n",
        )
        .file(
            "run-staged.json",
            r#"{"*.rs": ["fixture-tool --stdout from-json"]}"#,
        )
        .build();
    let r = run_staged(&repo).run();
    assert_ok(&r);
    assert_eq!(calls(&repo), vec![vec!["--stdout", "from-dot", "a.rs"]]);
}

#[test]
fn walk_up_to_parent_directory() {
    let repo = Repo::builder()
        .staged("pkg/web/a.rs", "a")
        .file("run-staged.toml", OK)
        .build();
    let r = run_staged(&repo).dir("pkg/web").run();
    assert_ok(&r);
    assert_eq!(calls(&repo), vec![vec!["pkg/web/a.rs"]]);
}

#[test]
fn package_json_without_run_staged_key_is_ignored() {
    let repo = Repo::builder()
        .staged("pkg/a.rs", "a")
        .file("pkg/package.json", r#"{"name": "x"}"#)
        .file("run-staged.toml", OK)
        .build();
    let r = run_staged(&repo).dir("pkg").run();
    assert_ok(&r);
    assert_eq!(calls(&repo).len(), 1);
}

#[test]
fn old_mvp_file_names_are_not_discovered() {
    let repo = Repo::builder()
        .staged("a.rs", "a")
        .file("fast-staged.toml", OK)
        .build();
    let r = run_staged(&repo).run();
    assert!(assert_fails(&r, 1).contains("configuration file not found"));
}

#[test]
fn no_config_found() {
    let repo = Repo::builder().staged("a.rs", "a").build();
    let r = run_staged(&repo).run();
    assert_eq!(
        assert_fails(&r, 1).trim(),
        "error: configuration file not found"
    );
}

#[test]
fn explicit_config_bypasses_discovery() {
    let repo = Repo::builder()
        .staged("a.rs", "a")
        .file("nested/custom.toml", OK)
        .build();
    let r = run_staged(&repo)
        .args(["--config", "nested/custom.toml"])
        .run();
    assert_ok(&r);
    assert_eq!(calls(&repo).len(), 1);
}

#[test]
fn explicit_config_is_relative_to_cwd() {
    let repo = Repo::builder()
        .staged("sub/a.rs", "a")
        .file("sub/cfg.toml", OK)
        .build();
    let r = run_staged(&repo)
        .dir("sub")
        .args(["--config", "cfg.toml"])
        .run();
    assert_ok(&r);
}

#[test]
fn explicit_config_missing() {
    let repo = Repo::builder()
        .staged("a.rs", "a")
        .file("run-staged.toml", OK)
        .build();
    let r = run_staged(&repo).args(["--config", "nope.toml"]).run();
    assert_eq!(
        assert_fails(&r, 1).trim(),
        "error: configuration file not found"
    );
}

#[test]
fn named_groups_in_toml() {
    let repo = Repo::builder()
        .staged("a.rs", "a")
        .staged("b.md", "b")
        .file(
            "run-staged.toml",
            "[lint]\npatterns = { \"*.rs\" = [\"fixture-tool\"] }\n",
        )
        .build();
    let r = run_staged(&repo).run();
    assert_ok(&r);
    assert!(
        r.stdout.contains("✓ lint/*.rs: fixture-tool"),
        "{}",
        r.stdout
    );
    assert_eq!(calls(&repo), vec![vec!["a.rs"]]);
}

#[test]
fn named_group_without_patterns_is_skipped() {
    let repo = Repo::builder()
        .staged("a.rs", "a")
        .file(
            "run-staged.toml",
            "[empty]\ntimeout = \"1s\"\n[also_empty]\npatterns = {}\n[g]\npatterns = { \"*.rs\" = [\"fixture-tool\"] }\n",
        )
        .build();
    let r = run_staged(&repo).run();
    assert_ok(&r);
    assert_eq!(calls(&repo).len(), 1);
}

#[test]
fn declaration_order_preserved() {
    let repo = Repo::builder()
        .committed("m.txt", "old")
        .staged("m.txt", "")
        .file(
            "run-staged.toml",
            "[zeta]\npatterns = { \"m.txt\" = [\"fixture-tool --sleep 300 --append z\"] }\n\
             [alpha]\npatterns = { \"m.txt\" = [\"fixture-tool --append a\"] }\n",
        )
        .build();
    assert_ok(&run_staged(&repo).run());
    assert_eq!(repo.worktree_content("m.txt").as_deref(), Some("za"));
}

#[test]
fn group_overrides_root() {
    let repo = Repo::builder()
        .staged("a.rs", "a")
        .staged("b.md", "b")
        .file(
            "run-staged.toml",
            "timeout = \"1s\"\ncontinue_on_error = true\n\
             [long]\ntimeout = \"10s\"\npatterns = { \"*.rs\" = [\"fixture-tool --sleep 1500\"] }\n\
             [inherits]\npatterns = { \"*.md\" = [\"fixture-tool --sleep 1500\"] }\n",
        )
        .build();
    let r = run_staged(&repo).run();
    let stderr = assert_fails(&r, 1);
    assert!(
        r.stdout.contains("✓ long/*.rs"),
        "group timeout wins:\n{}",
        r.stdout
    );
    assert!(
        stderr.contains("timed out after 1s"),
        "root timeout inherited:\n{stderr}"
    );
}

#[test]
fn root_value_inherited() {
    let repo = Repo::builder()
        .committed("m.txt", "old")
        .staged("m.txt", "")
        .file(
            "run-staged.toml",
            "execution_order = \"sequential\"\n\
             [g]\npatterns = { \"m.txt\" = [\"fixture-tool --sleep 300 --append 1\", \"fixture-tool --append 2\"] }\n",
        )
        .build();
    assert_ok(&run_staged(&repo).run());
    assert_eq!(repo.worktree_content("m.txt").as_deref(), Some("12"));
}

#[test]
fn flat_json_mapping() {
    let repo = Repo::builder()
        .staged("a.rs", "a")
        .file("run-staged.json", r#"{"*.rs": ["fixture-tool"]}"#)
        .build();
    let r = run_staged(&repo).run();
    assert_ok(&r);
    assert!(
        r.stdout.contains("✓ default/*.rs: fixture-tool"),
        "{}",
        r.stdout
    );
}

#[test]
fn package_json_section() {
    let repo = Repo::builder()
        .staged("a.ts", "a")
        .file(
            "package.json",
            r#"{"name": "x", "run-staged": {"*.ts": ["fixture-tool"]}}"#,
        )
        .build();
    assert_ok(&run_staged(&repo).run());
    assert_eq!(calls(&repo), vec![vec!["a.ts"]]);
}

#[test]
fn syntax_error() {
    let repo = Repo::builder()
        .staged("a.rs", "a")
        .file("run-staged.toml", "[g\npatterns = ")
        .build();
    let r = run_staged(&repo).run();
    let stderr = assert_fails(&r, 1);
    assert!(stderr.contains("invalid configuration"), "{stderr}");
    assert!(stderr.contains("run-staged.toml"), "{stderr}");
}

#[test]
fn bad_enum_value() {
    let repo = Repo::builder()
        .staged("a.rs", "a")
        .file(
            "run-staged.toml",
            "[g]\nexecution_order = \"random\"\npatterns = { \"*.rs\" = [\"x\"] }\n",
        )
        .build();
    let stderr = assert_fails(&run_staged(&repo).run(), 1).to_string();
    assert!(
        stderr
            .contains(r#"unknown execution_order: "random", expected "parallel" or "sequential""#),
        "{stderr}"
    );
}

#[test]
fn typo_in_toml_group_key() {
    let repo = Repo::builder()
        .staged("a.rs", "a")
        .file(
            "run-staged.toml",
            "[g]\ncontinue_on_eror = true\npatterns = { \"*.rs\" = [\"x\"] }\n",
        )
        .build();
    let stderr = assert_fails(&run_staged(&repo).run(), 1).to_string();
    assert!(stderr.contains("invalid configuration"), "{stderr}");
}

#[test]
fn structured_json_with_only_new_keys() {
    let repo = Repo::builder()
        .committed("a.js", "old")
        .staged("a.js", "new")
        .file(
            "run-staged.json",
            r#"{"read_only": true, "lint": {"patterns": {"*.js": ["fixture-tool --append x"]}}}"#,
        )
        .build();
    assert_ok(&run_staged(&repo).run());
    assert_eq!(
        repo.index_content("a.js").as_deref(),
        Some("new"),
        "read-only: not re-staged"
    );
}

// ── lint-staged configs (add-lint-staged-compat) ────────────────────────────

#[test]
fn lintstagedrc_with_group_pattern_matches_nested_file() {
    let repo = Repo::builder()
        .staged("packages/diir/subdir/somedir/file.test.ts", "t")
        .staged("README.md", "r")
        .file(
            ".lintstagedrc.json",
            r#"{"*.(js|ts|tsx|jsx)": "fixture-tool"}"#,
        )
        .build();
    let r = run_staged(&repo).run();
    assert_ok(&r);
    assert_eq!(
        calls(&repo),
        vec![vec!["packages/diir/subdir/somedir/file.test.ts"]]
    );
}

#[test]
fn js_lint_staged_config_is_rejected() {
    let repo = Repo::builder()
        .staged("a.ts", "a")
        .file(
            "lint-staged.config.js",
            "export default { '*.ts': 'eslint' }",
        )
        .build();
    let r = run_staged(&repo).run();
    let err = assert_fails(&r, 1);
    assert!(
        err.contains("lint-staged.config.js") && err.contains("not supported"),
        "{err}"
    );
}

#[test]
fn unbalanced_group_is_invalid_pattern() {
    let repo = Repo::builder()
        .staged("a.ts", "a")
        .file(
            "run-staged.toml",
            "[patterns]\n\"*.(js|ts\" = [\"fixture-tool\"]\n",
        )
        .build();
    let r = run_staged(&repo).run();
    let err = assert_fails(&r, 1);
    assert!(
        err.contains("invalid configuration: invalid glob pattern \"*.(js|ts\""),
        "{err}"
    );
}
