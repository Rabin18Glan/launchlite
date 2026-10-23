pub mod app;
pub mod auth;
pub mod build;

pub use app::*;
pub use auth::*;
pub use build::*;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub platform_rss_bytes: u64,
    #[serde(default)]
    pub apps_rss_bytes: u64,
    pub platform_vmsize_bytes: u64,
    pub platform_uptime_secs: u64,
    pub total_apps: usize,
    pub running_apps: usize,
    pub binary_size_bytes: u64,
}
