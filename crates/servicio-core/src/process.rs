use crate::error::CoreError;
use crate::worker::WorkerSpec;
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
            .env_clear()
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
}
