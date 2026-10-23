use axum_template::engine::Engine;
use minijinja::Environment;
use serde::Serialize;

use common::models::{AppInfo, AppStatus, GithubUser, HealthStatus, RestartPolicy, SystemMetrics};

pub type TemplateEngine = Engine<Environment<'static>>;

pub fn create_template_engine() -> TemplateEngine {
    let mut env = Environment::new();
    env.add_template("dashboard.html", include_str!("templates/dashboard.html"))
        .expect("Failed to compile dashboard.html template");

    Engine::new(env)
}

#[derive(Debug, Serialize)]
pub struct DashboardData {
    pub platform_name: String,
    pub apps_ram_mb: String,
    pub platform_ram_mb: String,
    pub platform_vmsize_mb: String,
    pub binary_size_mb: String,
    pub running_apps: usize,
    pub total_apps: usize,
    pub uptime_formatted: String,
    pub apps: Vec<AppViewData>,
    pub current_user: Option<GithubUser>,
    pub has_oauth_config: bool,
}

#[derive(Debug, Serialize)]
pub struct AppViewData {
    pub id: String,
    pub name: String,
    pub status: String,
    pub status_label: String,
    pub is_running: bool,
    pub is_building: bool,
    pub health_status: String,
    pub health_detail: Option<String>,
    pub domain: Option<String>,
    pub port: Option<u16>,
    pub git_repo: Option<String>,
    pub git_branch: Option<String>,
    pub git_commit_short: Option<String>,
    pub command: String,
    pub ram_mb: Option<String>,
    pub uptime_formatted: Option<String>,
    pub restart_count: u32,
    pub has_git: bool,
    pub restart_policy: String,
    pub health_check_url: Option<String>,
    pub env_vars: Vec<(String, String)>,
    pub created_at_formatted: String,
}

pub fn prepare_dashboard_data(
    metrics: &SystemMetrics,
    apps: &[AppInfo],
    current_user: Option<&GithubUser>,
    has_oauth_config: bool,
) -> DashboardData {
    let total_apps_rss_bytes: u64 = apps.iter().filter_map(|app| app.rss_bytes).sum();
    let apps_ram_mb = format!(
        "{:.1}",
        (total_apps_rss_bytes as f64) / (1024.0 * 1024.0)
    );
    let platform_ram_mb = apps_ram_mb.clone();
    let platform_vmsize_mb = format!(
        "{:.1}",
        (metrics.platform_vmsize_bytes as f64) / (1024.0 * 1024.0)
    );
    let binary_size_mb = format!(
        "{:.2}",
        (metrics.binary_size_bytes as f64) / (1024.0 * 1024.0)
    );

    let app_views = apps
        .iter()
        .map(|app| {
            let (status, status_label) = if app.is_building {
                ("building".to_string(), "Building...".to_string())
            } else {
                match &app.status {
                    AppStatus::Running { pid, .. } => {
                        ("running".to_string(), format!("Running (PID {})", pid))
                    }
                    AppStatus::Stopped => ("stopped".to_string(), "Stopped".to_string()),
                    AppStatus::Starting => ("starting".to_string(), "Starting...".to_string()),
                    AppStatus::Stopping => ("stopping".to_string(), "Stopping...".to_string()),
                    AppStatus::Crashed { exit_code, .. } => (
                        "crashed".to_string(),
                        format!("Crashed (exit: {:?})", exit_code),
                    ),
                }
            };

            let (health_status, health_detail) = match &app.health {
                HealthStatus::Healthy => ("healthy".to_string(), None),
                HealthStatus::Degraded(detail) => ("degraded".to_string(), Some(detail.clone())),
                HealthStatus::Unhealthy(err) => ("unhealthy".to_string(), Some(err.clone())),
                HealthStatus::Unknown => ("unknown".to_string(), None),
            };

            let ram_mb = app
                .rss_bytes
                .map(|b| format!("{:.1}", (b as f64) / (1024.0 * 1024.0)));
            let uptime_formatted = app.uptime_secs.map(format_duration);

            let (git_repo, git_branch, git_commit_short) = match &app.config.git {
                Some(g) => {
                    let commit_short = g.last_commit_hash.as_ref().map(|h| {
                        let len = h.len().min(7);
                        h[..len].to_string()
                    });
                    (
                        Some(g.repo_url.clone()),
                        Some(g.branch.clone()),
                        commit_short,
                    )
                }
                None => (None, None, None),
            };

            let command = if app.config.args.is_empty() {
                app.config.executable.clone()
            } else {
                format!("{} {}", app.config.executable, app.config.args.join(" "))
            };

            let is_running = matches!(app.status, AppStatus::Running { .. });
            let has_git = app.config.git.is_some();

            let restart_policy = match app.config.restart_policy {
                RestartPolicy::Always => "Always".to_string(),
                RestartPolicy::OnFailure => "On Failure".to_string(),
                RestartPolicy::Never => "Never".to_string(),
            };

            let mut env_vars: Vec<(String, String)> = app.config.env.clone().into_iter().collect();
            env_vars.sort_by(|a, b| a.0.cmp(&b.0));

            let created_at_formatted = format_timestamp(app.config.created_at);

            AppViewData {
                id: app.config.id.clone(),
                name: app.config.name.clone(),
                status,
                status_label,
                is_running,
                is_building: app.is_building,
                health_status,
                health_detail,
                domain: app.config.domain.clone(),
                port: app.config.port,
                git_repo,
                git_branch,
                git_commit_short,
                command,
                ram_mb,
                uptime_formatted,
                restart_count: app.restart_count,
                has_git,
                restart_policy,
                health_check_url: app.config.health_check_url.clone(),
                env_vars,
                created_at_formatted,
            }
        })
        .collect();

    DashboardData {
        platform_name: "LaunchLite".to_string(),
        apps_ram_mb,
        platform_ram_mb,
        platform_vmsize_mb,
        binary_size_mb,
        running_apps: metrics.running_apps,
        total_apps: metrics.total_apps,
        uptime_formatted: format_duration(metrics.platform_uptime_secs),
        apps: app_views,
        current_user: current_user.cloned(),
        has_oauth_config,
    }
}

fn format_duration(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if days > 0 {
        format!("{}d {}h", days, hours)
    } else if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
}

fn format_timestamp(timestamp: u64) -> String {
    if timestamp == 0 {
        return "Unknown".to_string();
    }
    // Approximate readable date/time (or UTC date string)
    let days_since_epoch = timestamp / 86400;
    let year = 1970 + days_since_epoch / 365;
    format!("Epoch +{}s (~{})", timestamp, year)
}
