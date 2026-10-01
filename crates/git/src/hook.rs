//! `run-staged --install`: writes a `pre-commit` hook without husky.

use std::path::{Path, PathBuf};

use crate::{GitError, error::git_err};

/// Marker line that identifies a hook written by run-staged.
pub const HOOK_MARKER: &str = "# run-staged";

/// Prefers the project's npm install, then falls back to PATH. Git for Windows
/// runs hooks with its bundled `sh`, so one script covers every platform.
const HOOK_SCRIPT: &str = "#!/bin/sh
# run-staged
if [ -x ./node_modules/.bin/run-staged ]; then exec ./node_modules/.bin/run-staged \"$@\"; fi
exec run-staged \"$@\"
";

/// Writes the `pre-commit` hook into `core.hooksPath` (relative to the work
/// tree) or `<git-dir>/hooks`, and returns its path. An existing hook is only
/// replaced when it carries [`HOOK_MARKER`].
pub fn install_hook(cwd: &Path) -> Result<PathBuf, GitError> {
    let repo =
        gix::discover_with_environment_overrides(cwd).map_err(|_| GitError::NotGitRepository)?;
    let work_dir = repo.workdir().ok_or(GitError::Bare)?.to_path_buf();
    let hooks_dir = match repo.config_snapshot().trusted_path("core.hooksPath") {
        Some(path) => work_dir.join(path.map_err(git_err)?),
        None => repo.common_dir().join("hooks"),
    };
    let hook = hooks_dir.join("pre-commit");

    match std::fs::read(&hook) {
        Ok(existing) => {
            let ours = String::from_utf8_lossy(&existing)
                .lines()
                .any(|l| l.trim() == HOOK_MARKER);
            if !ours {
                return Err(GitError::HookExists { path: hook });
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }

    std::fs::create_dir_all(&hooks_dir)?;
    std::fs::write(&hook, HOOK_SCRIPT)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(hook)
}
