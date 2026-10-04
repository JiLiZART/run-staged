//! Fixture-based end-to-end harness for the `run-staged` binary.
//!
//! A [`Repo`] is a throwaway git repository built from three layers
//! (`committed/`, `staged/`, `unstaged/`) in an environment that ignores the
//! developer's git config. [`Repo::runner`] runs a binary inside it with
//! `fixture-tool` first on PATH. Setup and inspection use the `git` CLI on
//! purpose: the harness must not share code (gix) with what it checks.

use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::OnceLock,
    thread,
    time::{Duration, Instant, SystemTime},
};

mod normalize;
pub use normalize::normalize;

/// Builds a [`Repo`] from `<crate>/tests/fixtures/<name>/` of the calling crate.
#[macro_export]
macro_rules! fixture_repo {
    ($name:expr) => {
        $crate::Repo::from_fixture(
            ::std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join($name),
        )
    };
}

/// Path of the `fixture-tool` binary, built on first use.
pub fn fixture_tool_path() -> &'static Path {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from(env!("CARGO")));
        // Ask cargo where the artifact is: target layouts differ across cargo versions.
        let mut cmd = Command::new(cargo);
        cmd.args([
            "build",
            "--quiet",
            "--message-format=json",
            "-p",
            "run-staged-testkit",
            "--bin",
            "fixture-tool",
        ]);
        if !cfg!(debug_assertions) {
            cmd.arg("--release");
        }
        let out = cmd
            .stderr(Stdio::inherit())
            .output()
            .expect("run cargo build for fixture-tool");
        assert!(out.status.success(), "building fixture-tool failed");
        let bin = String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
            .filter(|m| m["reason"] == "compiler-artifact" && m["target"]["name"] == "fixture-tool")
            .find_map(|m| m["executable"].as_str().map(PathBuf::from))
            .expect("cargo reported no fixture-tool executable");
        assert!(bin.is_file(), "fixture-tool not found at {}", bin.display());
        bin
    })
}

/// Declarative description of a repository. See [`Repo::builder`].
#[derive(Default)]
pub struct RepoBuilder {
    committed: BTreeMap<String, Vec<u8>>,
    staged: BTreeMap<String, Vec<u8>>,
    unstaged: BTreeMap<String, Vec<u8>>,
    extra: BTreeMap<String, Vec<u8>>,
    config: Vec<(String, String)>,
}

impl RepoBuilder {
    pub fn committed(mut self, path: &str, content: impl AsRef<[u8]>) -> Self {
        self.committed
            .insert(path.into(), content.as_ref().to_vec());
        self
    }
    pub fn staged(mut self, path: &str, content: impl AsRef<[u8]>) -> Self {
        self.staged.insert(path.into(), content.as_ref().to_vec());
        self
    }
    pub fn unstaged(mut self, path: &str, content: impl AsRef<[u8]>) -> Self {
        self.unstaged.insert(path.into(), content.as_ref().to_vec());
        self
    }
    /// A file written after all layers and never added to git (e.g. a hook).
    pub fn file(mut self, path: &str, content: impl AsRef<[u8]>) -> Self {
        self.extra.insert(path.into(), content.as_ref().to_vec());
        self
    }
    pub fn git_config(mut self, key: &str, value: &str) -> Self {
        self.config.push((key.into(), value.into()));
        self
    }

    pub fn build(self) -> Repo {
        let tmp = tempfile::Builder::new()
            .prefix("run-staged-fixture-")
            .tempdir()
            .expect("tempdir");
        let base = fs::canonicalize(tmp.path()).expect("canonicalize tempdir");
        let home = base.join("home");
        let root = base.join("repo");
        fs::create_dir_all(home.join(".config")).unwrap();
        fs::create_dir_all(&root).unwrap();
        fs::write(home.join(".gitconfig"), "").unwrap();

        let keep = std::env::var_os("RUN_STAGED_KEEP_FIXTURES").is_some_and(|v| v == "1");
        let tmp = if keep {
            let _ = tmp.keep();
            eprintln!("run-staged-testkit: keeping fixture at {}", base.display());
            None
        } else {
            Some(tmp)
        };

        let repo = Repo {
            _tmp: tmp,
            root,
            home,
            index_file: None,
        };
        repo.git(&["-c", "init.defaultBranch=main", "init", "-q"]);
        for (k, v) in &self.config {
            repo.git(&["config", k, v]);
        }
        if !self.committed.is_empty() {
            write_layer(&repo.root, &self.committed);
            repo.git(&["add", "-A"]);
            repo.git(&["commit", "-q", "-m", "initial"]);
        }
        if !self.staged.is_empty() {
            write_layer(&repo.root, &self.staged);
            repo.git(&["add", "-A"]);
        }
        write_layer(&repo.root, &self.unstaged);
        write_layer(&repo.root, &self.extra);
        repo
    }
}

fn write_layer(root: &Path, files: &BTreeMap<String, Vec<u8>>) {
    for (rel, content) in files {
        let p = root.join(rel);
        if let Some(dir) = p.parent() {
            fs::create_dir_all(dir).unwrap();
        }
        fs::write(&p, content).unwrap_or_else(|e| panic!("write {}: {e}", p.display()));
    }
}

fn read_layer(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(base, &p, out);
            } else if p.file_name().is_some_and(|n| n != ".gitkeep") {
                let rel = p
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(rel, fs::read(&p).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// A throwaway git repository. Deleted on drop unless `RUN_STAGED_KEEP_FIXTURES=1`.
pub struct Repo {
    _tmp: Option<tempfile::TempDir>,
    root: PathBuf,
    home: PathBuf,
    index_file: Option<PathBuf>,
}

/// One recorded `fixture-tool` invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    pub args: Vec<String>,
    pub cwd: String,
}

impl Repo {
    pub fn builder() -> RepoBuilder {
        RepoBuilder::default()
    }

    /// Builds a repository from a fixture directory with optional
    /// `committed/`, `staged/`, `unstaged/` layers.
    pub fn from_fixture(dir: impl AsRef<Path>) -> Repo {
        let dir = dir.as_ref();
        assert!(dir.is_dir(), "fixture not found: {}", dir.display());
        RepoBuilder {
            committed: read_layer(&dir.join("committed")),
            staged: read_layer(&dir.join("staged")),
            unstaged: read_layer(&dir.join("unstaged")),
            ..RepoBuilder::default()
        }
        .build()
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn git_dir(&self) -> PathBuf {
        self.root.join(".git")
    }

    /// The index file the binary and accessors use (`GIT_INDEX_FILE` when set).
    pub fn index_path(&self) -> PathBuf {
        self.index_file
            .clone()
            .unwrap_or_else(|| self.git_dir().join("index"))
    }

    /// Simulates `git commit -a`: copies the index to `.git/index.lock` and makes
    /// runs and accessors use it via `GIT_INDEX_FILE`.
    pub fn enable_commit_a_index(&mut self) {
        let lock = self.git_dir().join("index.lock");
        fs::copy(self.git_dir().join("index"), &lock).expect("copy index to index.lock");
        self.index_file = Some(lock);
    }

    fn isolated_env(&self, cmd: &mut Command) {
        for var in [
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_INDEX_FILE",
            "GIT_OBJECT_DIRECTORY",
            "GIT_CEILING_DIRECTORIES",
        ] {
            cmd.env_remove(var);
        }
        cmd.env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", self.home.join(".gitconfig"))
            .env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .env("XDG_CONFIG_HOME", self.home.join(".config"))
            .env("GIT_AUTHOR_NAME", "Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@example.com")
            .env("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z")
            .env("GIT_COMMITTER_NAME", "Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@example.com")
            .env("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z");
        if let Some(idx) = &self.index_file {
            cmd.env("GIT_INDEX_FILE", idx);
        }
    }

    /// The isolated environment (plus a PATH with `fixture-tool` first) as
    /// key/value pairs, for spawning processes the [`Runner`] can't, such as
    /// ones attached to a pseudo-terminal.
    pub fn isolated_env_vars(&self) -> Vec<(OsString, OsString)> {
        let mut cmd = Command::new("unused");
        self.isolated_env(&mut cmd);
        let tool_dir = fixture_tool_path().parent().unwrap().to_path_buf();
        let path = std::env::var_os("PATH").unwrap_or_default();
        let path =
            std::env::join_paths(std::iter::once(tool_dir).chain(std::env::split_paths(&path)))
                .expect("join PATH");
        cmd.get_envs()
            .filter_map(|(k, v)| Some((k.to_os_string(), v?.to_os_string())))
            .chain(std::iter::once((OsString::from("PATH"), path)))
            .collect()
    }

    /// Runs `git` in the repository with the isolated env; panics on failure.
    pub fn git(&self, args: &[&str]) -> String {
        let out = self.git_output(args);
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    fn git_output(&self, args: &[&str]) -> std::process::Output {
        let mut cmd = Command::new("git");
        cmd.args(args).current_dir(&self.root);
        self.isolated_env(&mut cmd);
        cmd.output().expect("spawn git")
    }

    /// Content of `path` in the index, or `None` if it is not in the index.
    pub fn index_content(&self, path: &str) -> Option<String> {
        let out = self.git_output(&["show", &format!(":{path}")]);
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    }

    pub fn worktree_content(&self, path: &str) -> Option<String> {
        fs::read_to_string(self.root.join(path)).ok()
    }

    /// Paths whose index entry differs from HEAD (all entries when HEAD is unborn).
    pub fn staged_paths(&self) -> Vec<String> {
        self.git(&["diff", "--cached", "--name-only", "--no-renames"])
            .lines()
            .map(str::to_owned)
            .collect()
    }

    pub fn git_dir_has(&self, name: &str) -> bool {
        self.git_dir().join(name).exists()
    }

    /// `fixture-tool` invocations in completion order.
    pub fn invocations(&self) -> Vec<Invocation> {
        let Ok(text) = fs::read_to_string(self.git_dir().join("fixture-tool.log")) else {
            return Vec::new();
        };
        text.lines()
            .map(|l| {
                let v: serde_json::Value = serde_json::from_str(l).expect("log line is JSON");
                Invocation {
                    args: v["args"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|a| a.as_str().unwrap().to_owned())
                        .collect(),
                    cwd: v["cwd"].as_str().unwrap().to_owned(),
                }
            })
            .collect()
    }

    pub fn write(&self, path: &str, content: impl AsRef<[u8]>) {
        write_layer(
            &self.root,
            &BTreeMap::from([(path.to_owned(), content.as_ref().to_vec())]),
        );
    }

    /// Starts configuring a run. Set the binary with [`Runner::program`], e.g.
    /// `.program(env!("CARGO_BIN_EXE_run-staged"))` in an `apps/run-staged` test
    /// (`env!` must expand in the test crate, where cargo sets that variable).
    pub fn runner(&self) -> Runner<'_> {
        Runner {
            repo: self,
            program: None,
            args: Vec::new(),
            dir: None,
            env: Vec::new(),
            no_git: false,
            timeout: Duration::from_secs(30),
            hold_lock: None,
            path_prepend: Vec::new(),
        }
    }
}

/// Configures and executes one process inside a [`Repo`].
pub struct Runner<'r> {
    repo: &'r Repo,
    program: Option<PathBuf>,
    args: Vec<OsString>,
    dir: Option<PathBuf>,
    env: Vec<(OsString, OsString)>,
    no_git: bool,
    timeout: Duration,
    hold_lock: Option<Option<Duration>>,
    path_prepend: Vec<PathBuf>,
}

/// Captured result of a [`Runner::run`].
#[derive(Debug)]
pub struct RunResult {
    /// Exit code; `None` when killed by a signal.
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    /// Whether the held index lock's bytes or mtime changed while the process ran.
    pub lock_touched: bool,
}

impl RunResult {
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }

    /// Exit code, stdout, and stderr normalized for an `insta` snapshot.
    pub fn snapshot(&self, repo: &Repo) -> String {
        format!(
            "exit: {:?}\n--- stdout\n{}--- stderr\n{}",
            self.code,
            normalize(&self.stdout, repo.root()),
            normalize(&self.stderr, repo.root()),
        )
    }
}

impl<'r> Runner<'r> {
    /// The executable to run.
    pub fn program(mut self, program: impl Into<PathBuf>) -> Self {
        self.program = Some(program.into());
        self
    }
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }
    /// Working directory relative to the repository root.
    pub fn dir(mut self, rel: impl AsRef<Path>) -> Self {
        self.dir = Some(rel.as_ref().to_path_buf());
        self
    }
    pub fn env(mut self, key: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }
    /// Puts `dir` on PATH, after `fixture-tool` and before everything else
    /// (e.g. the directory of the `run-staged` binary for hook tests).
    pub fn path_prepend(mut self, dir: impl Into<PathBuf>) -> Self {
        self.path_prepend.push(dir.into());
        self
    }
    /// Removes every PATH entry that contains a `git` executable.
    pub fn no_git(mut self) -> Self {
        self.no_git = true;
        self
    }
    pub fn timeout(mut self, d: Duration) -> Self {
        self.timeout = d;
        self
    }
    /// Creates `<index>.lock` before the run; removes it after `release_after`, or never.
    pub fn hold_index_lock(mut self, release_after: Option<Duration>) -> Self {
        self.hold_lock = Some(release_after);
        self
    }

    fn path_env(&self) -> OsString {
        let tool_dir = fixture_tool_path().parent().unwrap().to_path_buf();
        let git_name = format!("git{}", std::env::consts::EXE_SUFFIX);
        let current = std::env::var_os("PATH").unwrap_or_default();
        let rest = std::env::split_paths(&current)
            .filter(|p| !(self.no_git && p.join(&git_name).is_file()));
        std::env::join_paths(
            std::iter::once(tool_dir)
                .chain(self.path_prepend.iter().cloned())
                .chain(rest),
        )
        .expect("join PATH")
    }

    pub fn run(self) -> RunResult {
        let program = self.program.clone().expect(
            "no program: call .program(env!(\"CARGO_BIN_EXE_run-staged\")) from the test crate",
        );
        let cwd = match &self.dir {
            Some(d) => self.repo.root.join(d),
            None => self.repo.root.clone(),
        };
        let mut cmd = Command::new(&program);
        cmd.args(&self.args)
            .current_dir(&cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        self.repo.isolated_env(&mut cmd);
        cmd.env("PATH", self.path_env());
        for (k, v) in &self.env {
            cmd.env(k, v);
        }

        let lock_path = PathBuf::from(format!("{}.lock", self.repo.index_path().display()));
        let lock_before = self.hold_lock.map(|_| {
            fs::write(&lock_path, b"held by run-staged-testkit").expect("create index lock");
            lock_meta(&lock_path)
        });
        let releaser = self.hold_lock.flatten().map(|after| {
            let p = lock_path.clone();
            thread::spawn(move || {
                thread::sleep(after);
                let _ = fs::remove_file(p);
            })
        });

        let mut child = cmd
            .spawn()
            .unwrap_or_else(|e| panic!("spawn {}: {e}", program.display()));
        let mut out_pipe = child.stdout.take().unwrap();
        let mut err_pipe = child.stderr.take().unwrap();
        let out_t = thread::spawn(move || {
            let mut s = String::new();
            let _ = out_pipe.read_to_string(&mut s);
            s
        });
        let err_t = thread::spawn(move || {
            let mut s = String::new();
            let _ = err_pipe.read_to_string(&mut s);
            s
        });

        let start = Instant::now();
        let status = loop {
            if let Some(st) = child.try_wait().expect("try_wait") {
                break st;
            }
            if start.elapsed() > self.timeout {
                let _ = child.kill();
                let _ = child.wait();
                panic!(
                    "{} did not exit within {:?} (timeout); killed",
                    program.display(),
                    self.timeout
                );
            }
            thread::sleep(Duration::from_millis(10));
        };
        let stdout = out_t.join().unwrap();
        let stderr = err_t.join().unwrap();

        // A lock our own thread already released can only be compared while it existed;
        // one that still exists must be byte- and mtime-identical.
        let lock_touched = lock_before
            .as_ref()
            .is_some_and(|before| lock_path.exists() && lock_meta(&lock_path) != *before);
        if let Some(t) = releaser {
            let _ = t.join();
        } else if lock_before.is_some() {
            let _ = fs::remove_file(&lock_path);
        }

        RunResult {
            code: status.code(),
            stdout,
            stderr,
            lock_touched,
        }
    }
}

fn lock_meta(p: &Path) -> Option<(Vec<u8>, SystemTime)> {
    let bytes = fs::read(p).ok()?;
    let mtime = fs::metadata(p).ok()?.modified().ok()?;
    Some((bytes, mtime))
}
