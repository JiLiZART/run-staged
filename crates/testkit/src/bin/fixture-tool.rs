//! Portable stand-in for linters and formatters in fixture tests.
//!
//! Usage: `fixture-tool [options] [--] [files...]`
//!
//! Every invocation appends one JSON line `{"args": [...], "cwd": "<rel>"}` to
//! `<git-dir>/fixture-tool.log`, then applies the options in this order:
//! `--copy-git-file`, sleep, file edits, stdout/stderr text, exit code.
//!
//! `--copy-git-file <name>` copies `<git-dir>/<name>` to
//! `<git-dir>/fixture-tool.<name>` so tests can see what existed mid-run.

use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process, thread,
    time::Duration,
};

#[derive(Default)]
struct Opts {
    sleep_ms: u64,
    append: Option<String>,
    replace: Option<String>,
    delete: bool,
    stdout: Option<String>,
    stderr: Option<String>,
    copy_git_file: Option<String>,
    exit: i32,
    files: Vec<String>,
}

fn parse(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = |name: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match a.as_str() {
            "--sleep" => {
                o.sleep_ms = val("--sleep")?
                    .parse()
                    .map_err(|e| format!("--sleep: {e}"))?
            }
            "--append" => o.append = Some(val("--append")?),
            "--replace" => o.replace = Some(val("--replace")?),
            "--delete" => o.delete = true,
            "--stdout" => o.stdout = Some(val("--stdout")?),
            "--stderr" => o.stderr = Some(val("--stderr")?),
            "--copy-git-file" => o.copy_git_file = Some(val("--copy-git-file")?),
            "--exit" => o.exit = val("--exit")?.parse().map_err(|e| format!("--exit: {e}"))?,
            "--" => o.files.extend(it.by_ref().cloned()),
            _ => o.files.push(a.clone()),
        }
    }
    Ok(o)
}

/// Finds the work tree root and git dir by walking up from `start` (honors `GIT_DIR`).
fn find_repo(start: &Path) -> Option<(PathBuf, PathBuf)> {
    if let Some(git_dir) = env::var_os("GIT_DIR") {
        let git_dir = start.join(git_dir);
        let root = git_dir.parent()?.to_path_buf();
        return Some((root, git_dir));
    }
    start
        .ancestors()
        .find(|d| d.join(".git").is_dir())
        .map(|d| (d.to_path_buf(), d.join(".git")))
}

fn log(args: &[String], copy_git_file: Option<&str>) {
    let cwd = env::current_dir().expect("cwd");
    let Some((root, git_dir)) = find_repo(&cwd) else {
        return;
    };
    if let Some(name) = copy_git_file {
        let _ = fs::copy(
            git_dir.join(name),
            git_dir.join(format!("fixture-tool.{name}")),
        );
    }
    let rel = cwd
        .strip_prefix(&root)
        .unwrap_or(&cwd)
        .to_string_lossy()
        .replace('\\', "/");
    let mut line = serde_json::json!({ "args": args, "cwd": rel }).to_string();
    line.push('\n');
    // One O_APPEND write per record, so parallel invocations never interleave.
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(git_dir.join("fixture-tool.log"))
        .expect("open fixture-tool.log");
    f.write_all(line.as_bytes())
        .expect("write fixture-tool.log");
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let o = match parse(&args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("fixture-tool: {e}");
            process::exit(64);
        }
    };
    log(&args, o.copy_git_file.as_deref());
    if o.sleep_ms > 0 {
        thread::sleep(Duration::from_millis(o.sleep_ms));
    }
    for f in &o.files {
        if o.delete {
            fs::remove_file(f).unwrap_or_else(|e| panic!("delete {f}: {e}"));
            continue;
        }
        if let Some(text) = &o.replace {
            fs::write(f, text).unwrap_or_else(|e| panic!("replace {f}: {e}"));
        }
        if let Some(text) = &o.append {
            let mut h = fs::OpenOptions::new()
                .append(true)
                .open(f)
                .unwrap_or_else(|e| panic!("append {f}: {e}"));
            h.write_all(text.as_bytes()).expect("append");
        }
    }
    if let Some(t) = &o.stdout {
        println!("{t}");
    }
    if let Some(t) = &o.stderr {
        eprintln!("{t}");
    }
    process::exit(o.exit);
}

#[cfg(test)]
mod tests {
    use super::parse;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn parses_options_and_files() {
        let o = parse(&s(&[
            "--sleep", "5", "--append", "x", "--exit", "3", "a.js", "b.js",
        ]))
        .unwrap();
        assert_eq!(o.sleep_ms, 5);
        assert_eq!(o.append.as_deref(), Some("x"));
        assert_eq!(o.exit, 3);
        assert_eq!(o.files, s(&["a.js", "b.js"]));
    }

    #[test]
    fn double_dash_ends_options() {
        let o = parse(&s(&["--delete", "--", "--exit"])).unwrap();
        assert!(o.delete);
        assert_eq!(o.files, s(&["--exit"]));
        assert_eq!(o.exit, 0);
    }

    #[test]
    fn missing_value_is_error() {
        assert!(parse(&s(&["--stderr"])).is_err());
    }
}
