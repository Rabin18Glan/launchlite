use std::collections::VecDeque;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::oneshot;
use tracing::{error, info, warn};

use crate::models::{AppConfig, LogLine, current_timestamp};

pub const MAX_LOG_LINES: usize = 500;

#[derive(Debug, Clone)]
pub struct LogBuffer {
    capacity: usize,
    lines: Arc<Mutex<VecDeque<LogLine>>>,
}

impl LogBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            lines: Arc::new(Mutex::new(VecDeque::with_capacity(capacity))),
        }
    }

    pub fn push(&self, stream: &str, message: String) {
        let mut lines = self.lines.lock().unwrap();
        if lines.len() >= self.capacity {
            lines.pop_front();
        }
        lines.push_back(LogLine {
            timestamp: current_timestamp(),
            stream: stream.to_string(),
            message,
        });
    }

    pub fn get_lines(&self) -> Vec<LogLine> {
        let lines = self.lines.lock().unwrap();
        lines.iter().cloned().collect()
    }
}

pub enum ProcessExitReason {
    StoppedCleanly,
    Exited(Option<i32>),
    Crashed(String),
}

pub struct SpawnResult {
    pub pid: u32,
    pub stop_tx: oneshot::Sender<()>,
    pub exit_rx: oneshot::Receiver<ProcessExitReason>,
}

pub fn spawn_application(config: &AppConfig, logs: LogBuffer) -> Result<SpawnResult, String> {
    let mut cmd = Command::new(&config.executable);
    cmd.args(&config.args);

    if let Some(ref dir) = config.working_dir {
        cmd.current_dir(dir);
    }

    for (k, v) in &config.env {
        cmd.env(k, v);
    }

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.kill_on_drop(false);

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn {}: {}", config.executable, e))?;
    let pid = child
        .id()
        .ok_or_else(|| "Failed to get child PID".to_string())?;

    logs.push("system", format!("Process started with PID {}", pid));

    let (stop_tx, mut stop_rx) = oneshot::channel::<()>();
    let (exit_tx, exit_rx) = oneshot::channel::<ProcessExitReason>();

    // Stream stdout
    if let Some(stdout) = child.stdout.take() {
        let logs_clone = logs.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                logs_clone.push("stdout", line);
            }
        });
    }

    // Stream stderr
    if let Some(stderr) = child.stderr.take() {
        let logs_clone = logs.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                logs_clone.push("stderr", line);
            }
        });
    }

    // Supervise process lifecycle
    let app_id = config.id.clone();
    let logs_clone = logs.clone();
    tokio::spawn(async move {
        tokio::select! {
            _ = &mut stop_rx => {
                info!(app = %app_id, pid = pid, "Received stop signal, sending SIGTERM");
                logs_clone.push("system", format!("Sending SIGTERM to PID {}", pid));

                unsafe {
                    libc::kill(pid as i32, libc::SIGTERM);
                }

                // Wait up to 5 seconds for graceful shutdown
                let graceful_exit = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
                match graceful_exit {
                    Ok(Ok(status)) => {
                        info!(app = %app_id, pid = pid, ?status, "Process exited gracefully");
                        logs_clone.push("system", format!("Process exited gracefully with status {:?}", status));
                    }
                    _ => {
                        warn!(app = %app_id, pid = pid, "Graceful stop timed out after 5s, sending SIGKILL");
                        logs_clone.push("system", format!("SIGTERM timed out, sending SIGKILL to PID {}", pid));
                        unsafe {
                            libc::kill(pid as i32, libc::SIGKILL);
                        }
                        let _ = child.wait().await;
                        logs_clone.push("system", "Process terminated with SIGKILL".to_string());
                    }
                }
                let _ = exit_tx.send(ProcessExitReason::StoppedCleanly);
            }
            status_res = child.wait() => {
                match status_res {
                    Ok(status) => {
                        let code = status.code();
                        info!(app = %app_id, pid = pid, code = ?code, "Process exited");
                        logs_clone.push("system", format!("Process exited with code {:?}", code));
                        if status.success() {
                            let _ = exit_tx.send(ProcessExitReason::Exited(code));
                        } else {
                            let _ = exit_tx.send(ProcessExitReason::Crashed(format!("Exited with code {:?}", code)));
                        }
                    }
                    Err(e) => {
                        error!(app = %app_id, pid = pid, error = %e, "Process error during wait");
                        logs_clone.push("system", format!("Process error: {}", e));
                        let _ = exit_tx.send(ProcessExitReason::Crashed(e.to_string()));
                    }
                }
            }
        }
    });

    Ok(SpawnResult {
        pid,
        stop_tx,
        exit_rx,
    })
}
