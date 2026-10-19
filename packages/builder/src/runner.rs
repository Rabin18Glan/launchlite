use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::info;

use crate::detector::detect_project;
use crate::git::clone_or_pull;
use common::{AppConfig, BuildJob, BuildStatus, GitDeploymentConfig, LogBuffer, current_timestamp};
use db::Database;

pub struct BuildRunner {
    data_dir: String,
    db: Database,
}

pub struct BuildResult {
    pub executable: String,
    pub args: Vec<String>,
    pub working_dir: String,
    pub commit_hash: String,
}

impl BuildRunner {
    pub fn new(data_dir: &str, db: Database) -> Self {
        Self {
            data_dir: data_dir.to_string(),
            db,
        }
    }

    pub fn get_repo_dir(&self, app_id: &str) -> PathBuf {
        PathBuf::from(&self.data_dir).join("repos").join(app_id)
    }

    pub async fn run_build(
        &self,
        app: &AppConfig,
        git: &GitDeploymentConfig,
        auth_token: Option<&str>,
        logs: &LogBuffer,
    ) -> Result<BuildResult, String> {
        let repo_dir = self.get_repo_dir(&app.id);
        let build_id = format!("build-{}", current_timestamp());

        let mut job = BuildJob {
            id: build_id.clone(),
            app_id: app.id.clone(),
            repo_url: git.repo_url.clone(),
            branch: git.branch.clone(),
            commit_hash: None,
            status: BuildStatus::Cloning,
            started_at: current_timestamp(),
            completed_at: None,
        };

        let _ = self.db.save_build(&job);

        logs.push(
            "build",
            format!(
                "Cloning/updating repository {} (branch: {})...",
                git.repo_url, git.branch
            ),
        );
        info!(app = %app.id, branch = %git.branch, "Cloning or pulling repository");

        let commit_hash =
            match clone_or_pull(&git.repo_url, &git.branch, &repo_dir, auth_token).await {
                Ok(hash) => {
                    logs.push("build", format!("Successfully fetched commit: {}", hash));
                    job.commit_hash = Some(hash.clone());
                    job.status = BuildStatus::Detecting;
                    let _ = self.db.save_build(&job);
                    hash
                }
                Err(e) => {
                    logs.push("build", format!("Git error: {}", e));
                    job.status = BuildStatus::Failed(e.clone());
                    job.completed_at = Some(current_timestamp());
                    let _ = self.db.save_build(&job);
                    return Err(e);
                }
            };

        // Auto-detect or use user custom build/run commands
        let detected = detect_project(&repo_dir);
        let build_cmd = git.build_cmd.clone().or(detected.build_cmd);
        let run_cmd = git.run_cmd.clone().unwrap_or(detected.run_cmd);

        logs.push(
            "build",
            format!(
                "Detected project type: {:?}. Run command: {}",
                detected.project_type, run_cmd
            ),
        );

        // Execute build command if present
        if let Some(cmd_str) = build_cmd {
            logs.push("build", format!("Running build command: {}", cmd_str));
            job.status = BuildStatus::Building;
            let _ = self.db.save_build(&job);

            let mut build_proc = Command::new("sh")
                .arg("-c")
                .arg(&cmd_str)
                .current_dir(&repo_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| format!("Failed to spawn build command: {}", e))?;

            if let Some(stdout) = build_proc.stdout.take() {
                let logs_clone = logs.clone();
                tokio::spawn(async move {
                    let mut reader = BufReader::new(stdout).lines();
                    while let Ok(Some(line)) = reader.next_line().await {
                        logs_clone.push("build", line);
                    }
                });
            }

            if let Some(stderr) = build_proc.stderr.take() {
                let logs_clone = logs.clone();
                tokio::spawn(async move {
                    let mut reader = BufReader::new(stderr).lines();
                    while let Ok(Some(line)) = reader.next_line().await {
                        logs_clone.push("build", line);
                    }
                });
            }

            let status = build_proc.wait().await.map_err(|e| e.to_string())?;
            if !status.success() {
                let err_msg = format!("Build command failed with code: {:?}", status.code());
                logs.push("build", err_msg.clone());
                job.status = BuildStatus::Failed(err_msg.clone());
                job.completed_at = Some(current_timestamp());
                let _ = self.db.save_build(&job);
                return Err(err_msg);
            }
            logs.push("build", "Build step completed successfully!".to_string());
        }

        job.status = BuildStatus::Success;
        job.completed_at = Some(current_timestamp());
        let _ = self.db.save_build(&job);

        // Parse run command into executable + arguments
        let parts: Vec<&str> = run_cmd.split_whitespace().collect();
        let (executable, args) = if parts.is_empty() {
            ("./app".to_string(), vec![])
        } else {
            (
                parts[0].to_string(),
                parts[1..].iter().map(|s| s.to_string()).collect(),
            )
        };

        Ok(BuildResult {
            executable,
            args,
            working_dir: repo_dir.to_str().unwrap().to_string(),
            commit_hash,
        })
    }
}
