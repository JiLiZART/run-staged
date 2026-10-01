use std::{path::Path, sync::OnceLock};

use regex::Regex;

/// Makes captured output stable for snapshots:
/// CRLF → LF, repo path → `<repo>` (with `/` separators after it),
/// durations like `12ms` / `1.50s` → `<duration>`.
pub fn normalize(text: &str, repo_root: &Path) -> String {
    static DURATION: OnceLock<Regex> = OnceLock::new();
    static REPO_PATH: OnceLock<Regex> = OnceLock::new();

    let mut s = text.replace("\r\n", "\n");
    let root = repo_root.to_string_lossy();
    // macOS temp dirs appear both as /var/... and /private/var/...
    let mut roots = vec![root.to_string()];
    if let Some(stripped) = root.strip_prefix("/private") {
        roots.push(stripped.to_string());
    }
    for r in &roots {
        s = s.replace(r.as_str(), "<repo>");
    }
    let repo_path = REPO_PATH.get_or_init(|| Regex::new(r"<repo>[^\s:)]*").unwrap());
    s = repo_path
        .replace_all(&s, |c: &regex::Captures| c[0].replace('\\', "/"))
        .into_owned();
    let duration = DURATION.get_or_init(|| Regex::new(r"\b\d+(\.\d+)?(ms|s)\b").unwrap());
    duration.replace_all(&s, "<duration>").into_owned()
}

#[cfg(test)]
mod tests {
    use super::normalize;
    use std::path::Path;

    #[test]
    fn durations() {
        let root = Path::new("/tmp/x");
        assert_eq!(
            normalize("✓ default/*.js: fixture-tool (12ms, 1 file)", root),
            "✓ default/*.js: fixture-tool (<duration>, 1 file)"
        );
        assert_eq!(normalize("3 files, 1.50s", root), "3 files, <duration>");
    }

    #[test]
    fn repo_path_and_separators() {
        let root = Path::new(r"C:\Temp\fx\repo");
        assert_eq!(
            normalize(r"error at C:\Temp\fx\repo\.git\run-staged.index", root),
            "error at <repo>/.git/run-staged.index"
        );
    }

    #[test]
    fn private_var_alias() {
        let root = Path::new("/private/var/folders/x/repo");
        assert_eq!(normalize("/var/folders/x/repo/a.js", root), "<repo>/a.js");
    }

    #[test]
    fn crlf() {
        assert_eq!(normalize("a\r\nb\r\n", Path::new("/r")), "a\nb\n");
    }

    #[test]
    fn plain_counts_untouched() {
        assert_eq!(normalize("1 file", Path::new("/r")), "1 file");
    }
}
