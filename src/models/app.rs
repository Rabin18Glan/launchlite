use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RestartPolicy {
    #[default]
    Never,
    Always,
    OnFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AppStatus {
    Stopped,
    Starting,
    Running {
        pid: u32,
        started_at: u64,
    },
    Stopping,
    Crashed {
        exit_code: Option<i32>,
        crashed_at: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "detail", rename_all = "snake_case")]
pub enum HealthStatus {
    Healthy,
    Degraded(String),
    Unhealthy(String),
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitDeploymentConfig {
    pub repo_url: String,
    pub branch: String,
    pub build_cmd: Option<String>,
    pub run_cmd: Option<String>,
    pub last_commit_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub id: String,
    pub name: String,
    pub executable: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub working_dir: Option<String>,
    #[serde(default)]
    pub restart_policy: RestartPolicy,
    #[serde(default)]
    pub health_check_url: Option<String>,
    #[serde(default = "default_health_interval")]
    pub health_check_interval_secs: u64,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub git: Option<GitDeploymentConfig>,
    #[serde(default = "current_timestamp")]
    pub created_at: u64,
}

fn default_health_interval() -> u64 {
    10
}

pub fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAppRequest {
    pub id: Option<String>,
    pub name: String,
    pub executable: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub working_dir: Option<String>,
    #[serde(default)]
    pub restart_policy: RestartPolicy,
    #[serde(default)]
    pub health_check_url: Option<String>,
    #[serde(default = "default_health_interval")]
    pub health_check_interval_secs: u64,
    #[serde(default = "default_true")]
    pub auto_start: bool,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub git: Option<GitDeploymentConfig>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppInfo {
    pub config: AppConfig,
    pub status: AppStatus,
    pub health: HealthStatus,
    pub rss_bytes: Option<u64>,
    pub uptime_secs: Option<u64>,
    pub restart_count: u32,
    pub is_building: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogLine {
    pub timestamp: u64,
    pub stream: String, // "stdout", "stderr", "system", "build"
    pub message: String,
}
