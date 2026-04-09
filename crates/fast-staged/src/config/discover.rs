use std::path::{Path, PathBuf};

use crate::AppError;

use super::{parse_json, parse_toml, types::Config, types::ConfigSource};

/// Priority-ordered list of non-package.json config file names.
const CONFIG_FILES: &[&str] = &[
    ".fast-staged.toml",
    ".fast-staged.json",
    "fast-staged.toml",
    "fast-staged.json",
];

/// Find the config file by walking up from `start` to the git root.
/// Checks all 5 formats in priority order per directory before moving to parent.
/// Stops at git root (directory containing `.git/`).
///
/// Returns `Err(AppError::ConfigNotFound)` if no config is found.
pub fn find_config_file(start: &Path) -> Result<PathBuf, AppError> {
    let mut current = start.to_path_buf();

    loop {
        // Check standard config files in priority order
        for name in CONFIG_FILES {
            let candidate = current.join(name);
            if candidate.exists() {
                return Ok(candidate);
            }
        }

        // Check package.json — only if it contains a "fast-staged" key
        let pkg = current.join("package.json");
        if pkg.exists()
            && std::fs::read_to_string(&pkg)
                .ok()
                .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                .and_then(|v| v.get("fast-staged").cloned())
                .is_some()
        {
            return Ok(pkg);
        }

        // Stop if we've reached the git root (directory containing .git/)
        if current.join(".git").exists() {
            break;
        }

        // Walk up to parent
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => break,
        }
    }

    Err(AppError::ConfigNotFound)
}

/// Parse the config at the given path, using the provided source variant.
fn parse_config_at(path: &Path, source: ConfigSource) -> Result<Config, AppError> {
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if filename == "package.json" {
        let content = std::fs::read_to_string(path).map_err(AppError::Io)?;
        let pkg: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| AppError::ConfigInvalid(format!("{}: {}", path.display(), e)))?;
        let fs_value = pkg.get("fast-staged").ok_or_else(|| {
            AppError::ConfigInvalid(format!("{}: missing 'fast-staged' key", path.display()))
        })?;
        parse_json::parse_package_json_value(fs_value.clone(), source)
    } else if filename.ends_with(".toml") {
        let mut config = parse_toml::parse_toml_file(path)?;
        config.source = source;
        Ok(config)
    } else {
        let mut config = parse_json::parse_json_file(path)?;
        config.source = source;
        Ok(config)
    }
}

/// Load config from an explicit path (--config flag) or auto-discover from CWD.
///
/// - If `explicit_path` is `Some`: bypass discovery, load that file directly.
///   Returns `Err(AppError::ConfigNotFound)` if the file does not exist.
/// - If `explicit_path` is `None`: discover config from `std::env::current_dir()`.
pub fn load_config(explicit_path: Option<&Path>) -> Result<Config, AppError> {
    if let Some(path) = explicit_path {
        if !path.exists() {
            return Err(AppError::ConfigNotFound);
        }
        let source = ConfigSource::Explicit(path.to_path_buf());
        parse_config_at(path, source)
    } else {
        let cwd = std::env::current_dir().map_err(AppError::Io)?;
        let config_path = find_config_file(&cwd)?;

        // Determine the source type from the filename
        let filename = config_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        let source = if filename == "package.json" {
            ConfigSource::PackageJson(config_path.clone())
        } else if filename.ends_with(".toml") {
            ConfigSource::TomlFile(config_path.clone())
        } else {
            ConfigSource::JsonFile(config_path.clone())
        };

        parse_config_at(&config_path, source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Create a temp dir with a unique name and return its path.
    /// Caller is responsible for cleanup (or let OS clean up).
    fn make_temp_dir(suffix: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!("fast-staged-test-{suffix}"));
        fs::create_dir_all(&base).expect("failed to create temp dir");
        base
    }

    /// Write a file to a directory.
    fn write_file(dir: &Path, name: &str, content: &str) {
        fs::write(dir.join(name), content).expect("failed to write file");
    }

    /// Create a `.git/` directory to simulate a git repo root.
    fn make_git_root(dir: &Path) {
        fs::create_dir_all(dir.join(".git")).expect("failed to create .git");
    }

    // ── find_config_file tests ───────────────────────────────────────────────

    #[test]
    fn test_finds_fast_staged_toml_with_dot() {
        let dir = make_temp_dir("t01");
        make_git_root(&dir);
        write_file(
            &dir,
            ".fast-staged.toml",
            "[patterns]\n\"*.rs\" = [\"cargo fmt\"]\n",
        );
        let result = find_config_file(&dir).unwrap();
        assert_eq!(result, dir.join(".fast-staged.toml"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_priority_dotjson_over_toml_without_dot() {
        let dir = make_temp_dir("t02");
        make_git_root(&dir);
        // Both .fast-staged.json (priority 2) and fast-staged.toml (priority 3) exist
        write_file(
            &dir,
            ".fast-staged.json",
            "{\"patterns\": {\"*.rs\": [\"cargo fmt\"]}}",
        );
        write_file(
            &dir,
            "fast-staged.toml",
            "[patterns]\n\"*.rs\" = [\"cargo fmt\"]\n",
        );
        let result = find_config_file(&dir).unwrap();
        assert_eq!(result, dir.join(".fast-staged.json"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_finds_fast_staged_json_without_dot() {
        let dir = make_temp_dir("t03");
        make_git_root(&dir);
        write_file(
            &dir,
            "fast-staged.json",
            "{\"patterns\": {\"*.rs\": [\"cargo fmt\"]}}",
        );
        let result = find_config_file(&dir).unwrap();
        assert_eq!(result, dir.join("fast-staged.json"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_package_json_with_fast_staged_key_is_found() {
        let dir = make_temp_dir("t04");
        make_git_root(&dir);
        write_file(
            &dir,
            "package.json",
            "{\"name\": \"myapp\", \"fast-staged\": {\"patterns\": {\"*.js\": [\"eslint\"]}}}",
        );
        let result = find_config_file(&dir).unwrap();
        assert_eq!(result, dir.join("package.json"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_package_json_without_fast_staged_key_is_skipped() {
        let dir = make_temp_dir("t05");
        make_git_root(&dir);
        write_file(&dir, "package.json", "{\"name\": \"myapp\"}");
        let result = find_config_file(&dir);
        assert!(
            matches!(result, Err(AppError::ConfigNotFound)),
            "Expected ConfigNotFound, got: {result:?}"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_walks_up_to_parent_directory() {
        let parent = make_temp_dir("t06-parent");
        make_git_root(&parent);
        write_file(
            &parent,
            ".fast-staged.toml",
            "[patterns]\n\"*.rs\" = [\"cargo fmt\"]\n",
        );
        let child = parent.join("subdir");
        fs::create_dir_all(&child).expect("create subdir");

        let result = find_config_file(&child).unwrap();
        assert_eq!(result, parent.join(".fast-staged.toml"));
        fs::remove_dir_all(&parent).ok();
    }

    #[test]
    fn test_walk_stops_at_git_root() {
        // grandparent has .git and a config
        // parent is the git root boundary (has .git)
        // child is where we start — no config
        // Config in grandparent should NOT be found because walk stops at parent (.git)
        let grandparent = make_temp_dir("t07-grand");
        write_file(
            &grandparent,
            ".fast-staged.toml",
            "[patterns]\n\"*.rs\" = [\"cargo fmt\"]\n",
        );
        let parent = grandparent.join("repo");
        fs::create_dir_all(&parent).expect("create repo");
        make_git_root(&parent); // git root is here
        let child = parent.join("src");
        fs::create_dir_all(&child).expect("create src");

        let result = find_config_file(&child);
        assert!(
            matches!(result, Err(AppError::ConfigNotFound)),
            "Walk should have stopped at git root, but got: {result:?}"
        );
        fs::remove_dir_all(&grandparent).ok();
    }

    #[test]
    fn test_empty_directory_tree_returns_config_not_found() {
        let dir = make_temp_dir("t08");
        make_git_root(&dir);
        let result = find_config_file(&dir);
        assert!(
            matches!(result, Err(AppError::ConfigNotFound)),
            "Expected ConfigNotFound, got: {result:?}"
        );
        fs::remove_dir_all(&dir).ok();
    }

    // ── load_config tests ────────────────────────────────────────────────────

    #[test]
    fn test_load_config_explicit_toml_path() {
        let dir = make_temp_dir("t09");
        make_git_root(&dir);
        let path = dir.join("myconfig.toml");
        write_file(
            &path.parent().unwrap(),
            "myconfig.toml",
            "[patterns]\n\"*.rs\" = [\"cargo fmt\"]\n",
        );
        let config = load_config(Some(&path)).unwrap();
        assert!(
            matches!(config.source, ConfigSource::Explicit(_)),
            "Expected Explicit source, got: {:?}",
            config.source
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_config_explicit_nonexistent_returns_not_found() {
        let path = PathBuf::from("/nonexistent/path/config.toml");
        let result = load_config(Some(&path));
        assert!(
            matches!(result, Err(AppError::ConfigNotFound)),
            "Expected ConfigNotFound, got: {result:?}"
        );
    }

    #[test]
    fn test_load_config_none_discovers_from_cwd() {
        // We can't easily change CWD in tests, but we can verify that load_config(None)
        // either returns a Config (if project has a config) or Err(ConfigNotFound).
        // This test just verifies load_config(None) doesn't panic.
        let result = load_config(None);
        // Either Ok (found a config) or Err(ConfigNotFound) is acceptable
        match result {
            Ok(_) | Err(AppError::ConfigNotFound) => {}
            Err(e) => panic!("Unexpected error from load_config(None): {e}"),
        }
    }
}
