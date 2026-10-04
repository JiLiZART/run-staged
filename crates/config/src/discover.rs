use std::path::{Path, PathBuf};

use crate::ConfigError;

use super::{parse_json, parse_toml, types::Config, types::ConfigSource};

/// Priority-ordered list of non-package.json config file names.
const CONFIG_FILES: &[&str] = &[
    ".run-staged.toml",
    ".run-staged.json",
    "run-staged.toml",
    "run-staged.json",
];

/// Find the config file by walking up from `start` to the git root.
/// Checks all 5 formats in priority order per directory before moving to parent.
/// Stops at git root (directory containing `.git/`).
///
/// Returns `Err(ConfigError::NotFound)` if no config is found.
pub fn find_config_file(start: &Path) -> Result<PathBuf, ConfigError> {
    let mut current = start.to_path_buf();

    loop {
        // Check standard config files in priority order
        for name in CONFIG_FILES {
            let candidate = current.join(name);
            if candidate.exists() {
                return Ok(candidate);
            }
        }

        // Check package.json — only if it contains a "run-staged" key
        let pkg = current.join("package.json");
        if pkg.exists()
            && std::fs::read_to_string(&pkg)
                .ok()
                .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                .and_then(|v| v.get("run-staged").cloned())
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

    Err(ConfigError::NotFound)
}

/// Parse the config at the given path, using the provided source variant.
fn parse_config_at(path: &Path, source: ConfigSource) -> Result<Config, ConfigError> {
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if filename == "package.json" {
        let content = std::fs::read_to_string(path).map_err(ConfigError::Io)?;
        let pkg: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| ConfigError::Invalid(format!("{}: {}", path.display(), e)))?;
        let fs_value = pkg.get("run-staged").ok_or_else(|| {
            ConfigError::Invalid(format!("{}: missing 'run-staged' key", path.display()))
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

/// Load config from an explicit path (--config flag) or auto-discover from `start`.
///
/// - If `explicit_path` is `Some`: bypass discovery, load that file directly.
///   Returns `Err(ConfigError::NotFound)` if the file does not exist.
/// - If `explicit_path` is `None`: discover config walking up from `start`.
pub fn load_config(start: &Path, explicit_path: Option<&Path>) -> Result<Config, ConfigError> {
    if let Some(path) = explicit_path {
        if !path.exists() {
            return Err(ConfigError::NotFound);
        }
        let source = ConfigSource::Explicit(path.to_path_buf());
        parse_config_at(path, source)
    } else {
        let config_path = find_config_file(start)?;

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
        let base = std::env::temp_dir().join(format!("run-staged-test-{suffix}"));
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
    fn test_finds_run_staged_toml_with_dot() {
        let dir = make_temp_dir("t01");
        make_git_root(&dir);
        write_file(
            &dir,
            ".run-staged.toml",
            "[patterns]\n\"*.rs\" = [\"cargo fmt\"]\n",
        );
        let result = find_config_file(&dir).unwrap();
        assert_eq!(result, dir.join(".run-staged.toml"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_priority_dotjson_over_toml_without_dot() {
        let dir = make_temp_dir("t02");
        make_git_root(&dir);
        // Both .run-staged.json (priority 2) and run-staged.toml (priority 3) exist
        write_file(
            &dir,
            ".run-staged.json",
            "{\"patterns\": {\"*.rs\": [\"cargo fmt\"]}}",
        );
        write_file(
            &dir,
            "run-staged.toml",
            "[patterns]\n\"*.rs\" = [\"cargo fmt\"]\n",
        );
        let result = find_config_file(&dir).unwrap();
        assert_eq!(result, dir.join(".run-staged.json"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_finds_run_staged_json_without_dot() {
        let dir = make_temp_dir("t03");
        make_git_root(&dir);
        write_file(
            &dir,
            "run-staged.json",
            "{\"patterns\": {\"*.rs\": [\"cargo fmt\"]}}",
        );
        let result = find_config_file(&dir).unwrap();
        assert_eq!(result, dir.join("run-staged.json"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_package_json_with_run_staged_key_is_found() {
        let dir = make_temp_dir("t04");
        make_git_root(&dir);
        write_file(
            &dir,
            "package.json",
            "{\"name\": \"myapp\", \"run-staged\": {\"patterns\": {\"*.js\": [\"eslint\"]}}}",
        );
        let result = find_config_file(&dir).unwrap();
        assert_eq!(result, dir.join("package.json"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_package_json_without_run_staged_key_is_skipped() {
        let dir = make_temp_dir("t05");
        make_git_root(&dir);
        write_file(&dir, "package.json", "{\"name\": \"myapp\"}");
        let result = find_config_file(&dir);
        assert!(
            matches!(result, Err(ConfigError::NotFound)),
            "Expected ConfigNotFound, got: {result:?}"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_old_mvp_file_names_are_not_discovered() {
        let dir = make_temp_dir("t05-mvp-names");
        make_git_root(&dir);
        write_file(
            &dir,
            "fast-staged.toml",
            "[g]\npatterns = { \"*\" = [\"x\"] }\n",
        );
        write_file(&dir, ".fast-staged.json", "{}");
        write_file(
            &dir,
            "package.json",
            "{\"fast-staged\": {\"patterns\": {\"*\": [\"x\"]}}}",
        );
        let result = find_config_file(&dir);
        assert!(
            matches!(result, Err(ConfigError::NotFound)),
            "Expected NotFound, got: {result:?}"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_walks_up_to_parent_directory() {
        let parent = make_temp_dir("t06-parent");
        make_git_root(&parent);
        write_file(
            &parent,
            ".run-staged.toml",
            "[patterns]\n\"*.rs\" = [\"cargo fmt\"]\n",
        );
        let child = parent.join("subdir");
        fs::create_dir_all(&child).expect("create subdir");

        let result = find_config_file(&child).unwrap();
        assert_eq!(result, parent.join(".run-staged.toml"));
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
            ".run-staged.toml",
            "[patterns]\n\"*.rs\" = [\"cargo fmt\"]\n",
        );
        let parent = grandparent.join("repo");
        fs::create_dir_all(&parent).expect("create repo");
        make_git_root(&parent); // git root is here
        let child = parent.join("src");
        fs::create_dir_all(&child).expect("create src");

        let result = find_config_file(&child);
        assert!(
            matches!(result, Err(ConfigError::NotFound)),
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
            matches!(result, Err(ConfigError::NotFound)),
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
            path.parent().unwrap(),
            "myconfig.toml",
            "[patterns]\n\"*.rs\" = [\"cargo fmt\"]\n",
        );
        let config = load_config(Path::new("."), Some(&path)).unwrap();
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
        let result = load_config(Path::new("."), Some(&path));
        assert!(
            matches!(result, Err(ConfigError::NotFound)),
            "Expected ConfigNotFound, got: {result:?}"
        );
    }

    #[test]
    fn test_load_config_none_discovers_from_start() {
        let dir = make_temp_dir("t-start");
        make_git_root(&dir);
        fs::create_dir_all(dir.join("sub")).unwrap();
        write_file(
            &dir,
            "run-staged.toml",
            "[g]\npatterns = { \"*.rs\" = [\"x\"] }\n",
        );
        let config = load_config(&dir.join("sub"), None).unwrap();
        assert_eq!(config.groups[0].name, "g");
        fs::remove_dir_all(&dir).ok();
    }
}
