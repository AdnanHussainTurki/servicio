use crate::error::CoreError;
use crate::worker::WorkerSpec;
use std::path::Path;
use std::process::ExitStatus;
use tokio::io::AsyncRead;
use tokio::process::{Child, Command};

/// A live child process with its stdout/stderr pipes detached for the caller to read.
pub struct Spawned {
    child: Child,
    pub stdout: Option<Box<dyn AsyncRead + Unpin + Send>>,
    pub stderr: Option<Box<dyn AsyncRead + Unpin + Send>>,
}

impl std::fmt::Debug for Spawned {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Spawned").field("pid", &self.child.id()).finish_non_exhaustive()
    }
}

impl Spawned {
    /// The OS process id, if still available.
    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }

    /// Wait for the process to exit.
    pub async fn wait(&mut self) -> Result<ExitStatus, CoreError> {
        Ok(self.child.wait().await?)
    }

    /// Ask the process to stop (graceful). Unix: SIGTERM via kill(); Windows impl added later.
    pub async fn terminate(&mut self) -> Result<(), CoreError> {
        // start_kill sends SIGKILL on Unix today; a SIGTERM-then-SIGKILL refinement
        // lands with the platform-signals work in a later phase.
        self.child.start_kill()?;
        Ok(())
    }
}

/// Abstraction over "how do I start a process from a spec". Lets the supervisor be
/// tested against fakes and swapped per-platform later.
pub trait ProcessSpawner: Send + Sync + 'static {
    fn spawn(&self, spec: &WorkerSpec) -> Result<Spawned, CoreError>;
}

/// Real implementation backed by tokio::process.
pub struct TokioProcess;

impl ProcessSpawner for TokioProcess {
    fn spawn(&self, spec: &WorkerSpec) -> Result<Spawned, CoreError> {
        if !spec.working_dir.exists() {
            return Err(CoreError::MissingWorkingDir(spec.working_dir.display().to_string()));
        }
        let mut cmd = Command::new(&spec.command);
        cmd.args(&spec.args)
            .current_dir(&spec.working_dir)
            // Inherit the daemon env (PATH/HOME propagate to children), then overlay the
            // worker's own vars. Augment PATH so project-local + Homebrew/nvm tools resolve
            // even when the daemon was launched with a minimal PATH (e.g. from Finder).
            .env("PATH", augmented_path(&spec.working_dir, spec.env.get("PATH")))
            .envs(&spec.env)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        let mut child = cmd.spawn().map_err(|e| CoreError::Spawn(e.to_string()))?;
        let stdout = child.stdout.take().map(|s| Box::new(s) as Box<dyn AsyncRead + Unpin + Send>);
        let stderr = child.stderr.take().map(|s| Box::new(s) as Box<dyn AsyncRead + Unpin + Send>);
        Ok(Spawned { child, stdout, stderr })
    }
}

/// Build a PATH including project-local tool dirs + common install locations + the inherited
/// PATH, so worker commands resolve even under a minimal daemon PATH. A user-provided PATH in
/// the worker's env wins outright.
fn augmented_path(working_dir: &Path, user_path: Option<&String>) -> String {
    if let Some(p) = user_path {
        return p.clone();
    }
    let mut parts: Vec<String> = Vec::new();
    parts.push(working_dir.join("node_modules/.bin").display().to_string());
    parts.push(working_dir.join("vendor/bin").display().to_string());
    for d in ["/opt/homebrew/bin", "/opt/homebrew/sbin", "/usr/local/bin", "/usr/local/sbin"] {
        parts.push(d.to_string());
    }
    if let Some(home) = std::env::var_os("HOME") {
        parts.push(Path::new(&home).join(".cargo/bin").display().to_string());
        parts.push(Path::new(&home).join(".local/bin").display().to_string());
    }
    match std::env::var("PATH") {
        Ok(p) if !p.is_empty() => parts.push(p),
        _ => parts.push("/usr/bin:/bin:/usr/sbin:/sbin".to_string()),
    }
    parts.join(":")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worker::{RestartPolicy, RunMode};
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use tokio::io::{AsyncBufReadExt, BufReader};

    fn spec(cmd: &str, args: &[&str]) -> WorkerSpec {
        WorkerSpec {
            name: "t".into(),
            command: cmd.into(),
            args: args.iter().map(|s| s.to_string()).collect(),
            working_dir: PathBuf::from("/"),
            env: BTreeMap::new(),
            run_mode: RunMode::Daemon { concurrency: 1 },
            restart: RestartPolicy::default(),
            autostart: false,
            enabled: true,
            group: None,
            tags: Vec::new(),
            display_name: None,
        }
    }

    #[tokio::test]
    async fn spawns_and_captures_stdout_then_exits_zero() {
        let mut spawned = TokioProcess.spawn(&spec("echo", &["hello"])).unwrap();
        let mut lines = BufReader::new(spawned.stdout.take().unwrap()).lines();
        let first = lines.next_line().await.unwrap().unwrap();
        assert_eq!(first, "hello");
        let status = spawned.wait().await.unwrap();
        assert!(status.success());
    }

    #[tokio::test]
    async fn nonzero_exit_is_reported_as_failure() {
        let mut spawned = TokioProcess.spawn(&spec("sh", &["-c", "exit 3"])).unwrap();
        let status = spawned.wait().await.unwrap();
        assert!(!status.success());
        assert_eq!(status.code(), Some(3));
    }

    #[tokio::test]
    async fn missing_working_dir_is_rejected_before_spawn() {
        let mut s = spec("echo", &["x"]);
        s.working_dir = PathBuf::from("/no/such/dir/servicio-xyz");
        let err = TokioProcess.spawn(&s).unwrap_err();
        assert!(matches!(err, CoreError::MissingWorkingDir(_)));
    }

    #[tokio::test]
    async fn resolves_project_local_binary_via_augmented_path() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("node_modules/.bin");
        std::fs::create_dir_all(&bin).unwrap();
        let tool = bin.join("mytool");
        std::fs::write(&tool, "#!/bin/sh\necho ok\n").unwrap();
        #[cfg(unix)]
        { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap(); }
        let spec = WorkerSpec {
            name: "t".into(), command: "mytool".into(), args: vec![],
            working_dir: dir.path().to_path_buf(), env: std::collections::BTreeMap::new(),
            run_mode: crate::worker::RunMode::Daemon { concurrency: 1 },
            restart: crate::worker::RestartPolicy::default(), autostart: false, enabled: true,
            group: None, tags: vec![], display_name: None,
        };
        let mut sp = TokioProcess.spawn(&spec).unwrap();
        let mut out = String::new();
        use tokio::io::AsyncReadExt;
        sp.stdout.take().unwrap().read_to_string(&mut out).await.unwrap();
        assert!(out.contains("ok"), "got: {out}");
    }

    #[test]
    fn augmented_path_includes_local_and_brew() {
        let p = augmented_path(std::path::Path::new("/proj"), None);
        assert!(p.contains("/proj/node_modules/.bin"));
        assert!(p.contains("/opt/homebrew/bin"));
        assert_eq!(augmented_path(std::path::Path::new("/proj"), Some(&"/custom".to_string())), "/custom");
    }
}
