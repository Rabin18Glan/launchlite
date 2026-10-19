use crate::models::RestartPolicy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildStatus {
    Queued,
    Cloning,
    Detecting,
    Building,
    Success,
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildJob {
    pub id: String,
    pub app_id: String,
    pub repo_url: String,
    pub branch: String,
    pub commit_hash: Option<String>,
    pub status: BuildStatus,
    pub started_at: u64,
    pub completed_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployRepoRequest {
    pub name: String,
    pub id: Option<String>,
    pub repo_url: String,
    #[serde(default = "default_branch")]
    pub branch: String,
    pub build_cmd: Option<String>,
    pub run_cmd: Option<String>,
    pub health_check_url: Option<String>,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub restart_policy: RestartPolicy,
}

fn default_branch() -> String {
    "main".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectType {
    Rust,
    Node,
    Python,
    Go,
    Static,
    Custom,
    Unknown,
}
