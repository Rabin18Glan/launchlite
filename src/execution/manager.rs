use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, mpsc, oneshot};
use tracing::{error, info};

use crate::builder::runner::BuildRunner;
use crate::db::Database;
use crate::execution::health::check_health;
use crate::execution::process::{LogBuffer, MAX_LOG_LINES, ProcessExitReason, spawn_application};
use crate::metrics::{get_binary_size, get_process_memory, get_self_memory, is_process_alive};
use crate::models::{
    AppConfig, AppInfo, AppStatus, ImportSummary, PlatformExport, CreateAppRequest, DeployRepoRequest, GitDeploymentConfig,
    HealthStatus, LogLine, RestartPolicy, SystemMetrics, current_timestamp,
};

pub struct AppRuntime {
    pub config: AppConfig,
    pub status: AppStatus,
    pub health: HealthStatus,
    pub logs: LogBuffer,
    pub stop_tx: Option<oneshot::Sender<()>>,
    pub restart_count: u32,
    pub started_at: Option<u64>,
    pub is_building: bool,
}

pub struct ExecutionManager {
    apps: RwLock<HashMap<String, AppRuntime>>,
    db: Database,
    builder: BuildRunner,
    platform_started_at: u64,
    restart_tx: mpsc::UnboundedSender<String>,
}

impl ExecutionManager {
    pub fn new(db: Database, data_dir: &str) -> (Arc<Self>, mpsc::UnboundedReceiver<String>) {
        let (restart_tx, restart_rx) = mpsc::unbounded_channel();
        let builder = BuildRunner::new(data_dir, db.clone());
        let manager = Arc::new(Self {
            apps: RwLock::new(HashMap::new()),
            db,
            builder,
            platform_started_at: current_timestamp(),
            restart_tx,
        });
        (manager, restart_rx)
    }

    pub fn get_db(&self) -> &Database {
        &self.db
    }

    pub async fn init(
        self: &Arc<Self>,
        mut restart_rx: mpsc::UnboundedReceiver<String>,
    ) -> Result<(), String> {
        let saved_apps = self.db.get_all_apps().map_err(|e| e.to_string())?;
        info!("Loading {} apps from database", saved_apps.len());

        for config in saved_apps {
            let id = config.id.clone();
            let auto_start = config.auto_start;
            let runtime = AppRuntime {
                config,
                status: AppStatus::Stopped,
                health: HealthStatus::Unknown,
                logs: LogBuffer::new(MAX_LOG_LINES),
                stop_tx: None,
                restart_count: 0,
                started_at: None,
                is_building: false,
            };

            self.apps.write().await.insert(id.clone(), runtime);

            if auto_start {
                let self_clone = Arc::clone(self);
                tokio::spawn(async move {
                    if let Err(e) = self_clone.start_app(&id).await {
                        error!(app = %id, error = %e, "Auto-start failed");
                    }
                });
            }
        }

        let self_clone = Arc::clone(self);
        tokio::spawn(async move {
            while let Some(id) = restart_rx.recv().await {
                tokio::time::sleep(Duration::from_millis(500)).await;
                if let Err(e) = self_clone.start_app(&id).await {
                    error!(app = %id, error = %e, "Auto-restart failed");
                }
            }
        });

        let self_clone = Arc::clone(self);
        tokio::spawn(async move {
            self_clone.run_health_loop().await;
        });

        Ok(())
    }

    async fn allocate_port(&self, preferred: Option<u16>) -> u16 {
        if let Some(p) = preferred {
            return p;
        }

        let apps = self.apps.read().await;
        let used_ports: Vec<u16> = apps.values().filter_map(|r| r.config.port).collect();

        let mut candidate = 10000;
        while used_ports.contains(&candidate) {
            candidate += 1;
        }
        candidate
    }

    pub async fn create_app(self: &Arc<Self>, req: CreateAppRequest) -> Result<AppInfo, String> {
        let id = req.id.unwrap_or_else(|| {
            let rand_suffix: u32 = rand_simple();
            format!(
                "{}-{}",
                req.name.to_lowercase().replace(' ', "-"),
                rand_suffix
            )
        });

        let executable = req.executable.unwrap_or_else(|| "./app".to_string());
        let port = if req.domain.is_some() || req.port.is_some() {
            Some(self.allocate_port(req.port).await)
        } else {
            None
        };

        let mut env = req.env;
        if let Some(p) = port {
            env.insert("PORT".to_string(), p.to_string());
        }

        let health_url = req
            .health_check_url
            .or_else(|| port.map(|p| format!("http://127.0.0.1:{}/", p)));

        let config = AppConfig {
            id: id.clone(),
            name: req.name,
            executable,
            args: req.args,
            env,
            working_dir: req.working_dir,
            restart_policy: req.restart_policy,
            health_check_url: health_url,
            health_check_interval_secs: req.health_check_interval_secs,
            auto_start: req.auto_start,
            domain: req.domain.map(|d| d.trim().to_lowercase()),
            port,
            git: req.git,
            created_at: current_timestamp(),
        };

        self.db
            .save_app(&config)
            .map_err(|e| format!("DB error: {}", e))?;

        let runtime = AppRuntime {
            config,
            status: AppStatus::Stopped,
            health: HealthStatus::Unknown,
            logs: LogBuffer::new(MAX_LOG_LINES),
            stop_tx: None,
            restart_count: 0,
            started_at: None,
            is_building: false,
        };

        self.apps.write().await.insert(id.clone(), runtime);

        if req.auto_start {
            self.start_app(&id).await?;
        }

        self.get_app(&id)
            .await
            .ok_or_else(|| "Failed to retrieve created app".to_string())
    }

    pub async fn deploy_git_repo(
        self: &Arc<Self>,
        req: DeployRepoRequest,
        auth_token: Option<String>,
    ) -> Result<AppInfo, String> {
        let id = req.id.unwrap_or_else(|| {
            let rand_suffix: u32 = rand_simple();
            format!(
                "{}-{}",
                req.name.to_lowercase().replace(' ', "-").replace('/', "-"),
                rand_suffix
            )
        });

        let port = Some(self.allocate_port(req.port).await);
        let mut env = req.env;
        if let Some(p) = port {
            env.insert("PORT".to_string(), p.to_string());
        }

        let health_url = req
            .health_check_url
            .or_else(|| port.map(|p| format!("http://127.0.0.1:{}/", p)));

        let git_config = GitDeploymentConfig {
            repo_url: req.repo_url.clone(),
            branch: req.branch.clone(),
            build_cmd: req.build_cmd,
            run_cmd: req.run_cmd,
            last_commit_hash: None,
        };

        let app_config = AppConfig {
            id: id.clone(),
            name: req.name,
            executable: "./placeholder".to_string(),
            args: vec![],
            env,
            working_dir: None,
            restart_policy: req.restart_policy,
            health_check_url: health_url,
            health_check_interval_secs: 10,
            auto_start: true,
            domain: req.domain.map(|d| d.trim().to_lowercase()),
            port,
            git: Some(git_config),
            created_at: current_timestamp(),
        };

        self.db
            .save_app(&app_config)
            .map_err(|e| format!("DB error: {}", e))?;

        let logs = LogBuffer::new(MAX_LOG_LINES);
        let runtime = AppRuntime {
            config: app_config.clone(),
            status: AppStatus::Starting,
            health: HealthStatus::Unknown,
            logs: logs.clone(),
            stop_tx: None,
            restart_count: 0,
            started_at: None,
            is_building: true,
        };

        self.apps.write().await.insert(id.clone(), runtime);

        let self_clone = Arc::clone(self);
        let id_clone = id.clone();
        tokio::spawn(async move {
            self_clone
                .perform_build_and_launch(&id_clone, auth_token)
                .await;
        });

        self.get_app(&id)
            .await
            .ok_or_else(|| "Failed to retrieve app".to_string())
    }

    pub async fn redeploy_app(
        self: &Arc<Self>,
        id: &str,
        auth_token: Option<String>,
    ) -> Result<(), String> {
        {
            let mut apps = self.apps.write().await;
            let runtime = apps
                .get_mut(id)
                .ok_or_else(|| format!("App '{}' not found", id))?;
            if runtime.config.git.is_none() {
                return Err("App does not have a Git repository attached".to_string());
            }
            if runtime.is_building {
                return Err("App is already building".to_string());
            }
            runtime.is_building = true;
        }

        let self_clone = Arc::clone(self);
        let id_owned = id.to_string();
        tokio::spawn(async move {
            self_clone
                .perform_build_and_launch(&id_owned, auth_token)
                .await;
        });

        Ok(())
    }

    async fn perform_build_and_launch(self: &Arc<Self>, id: &str, auth_token: Option<String>) {
        let (config, logs) = {
            let apps = self.apps.read().await;
            match apps.get(id) {
                Some(r) => (r.config.clone(), r.logs.clone()),
                None => return,
            }
        };

        let git = match &config.git {
            Some(g) => g.clone(),
            None => return,
        };

        logs.push("build", "Starting build pipeline...".to_string());

        let build_res = self
            .builder
            .run_build(&config, &git, auth_token.as_deref(), &logs)
            .await;

        match build_res {
            Ok(result) => {
                logs.push(
                    "build",
                    "Build successful! Preparing application launch...".to_string(),
                );

                let mut updated_config = config.clone();
                updated_config.executable = result.executable;
                updated_config.args = result.args;
                updated_config.working_dir = Some(result.working_dir);

                if let Some(ref mut g) = updated_config.git {
                    g.last_commit_hash = Some(result.commit_hash);
                }

                let _ = self.db.save_app(&updated_config);

                let _ = self.stop_app(id).await;
                tokio::time::sleep(Duration::from_millis(300)).await;

                {
                    let mut apps = self.apps.write().await;
                    if let Some(r) = apps.get_mut(id) {
                        r.config = updated_config;
                        r.is_building = false;
                    }
                }

                if let Err(e) = self.start_app(id).await {
                    error!(app = %id, error = %e, "Failed to launch app after build");
                    logs.push("system", format!("Failed to launch after build: {}", e));
                }
            }
            Err(e) => {
                error!(app = %id, error = %e, "Build pipeline failed");
                logs.push(
                    "build",
                    format!("Deployment halted due to build error: {}", e),
                );
                let mut apps = self.apps.write().await;
                if let Some(r) = apps.get_mut(id) {
                    r.is_building = false;
                    r.status = AppStatus::Crashed {
                        exit_code: None,
                        crashed_at: current_timestamp(),
                        error: Some(format!("Build failed: {}", e)),
                    };
                }
            }
        }
    }

    pub async fn start_app(self: &Arc<Self>, id: &str) -> Result<AppInfo, String> {
        let mut apps = self.apps.write().await;
        let runtime = apps
            .get_mut(id)
            .ok_or_else(|| format!("App '{}' not found", id))?;

        if let AppStatus::Running { .. } = runtime.status {
            return Err(format!("App '{}' is already running", id));
        }

        runtime.status = AppStatus::Starting;
        let config = runtime.config.clone();
        let logs = runtime.logs.clone();

        let spawn_res = spawn_application(&config, logs)?;
        let pid = spawn_res.pid;
        let started_at = current_timestamp();

        runtime.status = AppStatus::Running { pid, started_at };
        runtime.started_at = Some(started_at);
        runtime.health = HealthStatus::Unknown;
        runtime.stop_tx = Some(spawn_res.stop_tx);

        drop(apps);

        let self_clone = Arc::clone(self);
        let id_owned = id.to_string();
        tokio::spawn(async move {
            if let Ok(exit_reason) = spawn_res.exit_rx.await {
                self_clone.handle_process_exit(&id_owned, exit_reason).await;
            }
        });

        self.get_app(id)
            .await
            .ok_or_else(|| "App not found after start".to_string())
    }

    async fn handle_process_exit(self: &Arc<Self>, id: &str, reason: ProcessExitReason) {
        let mut apps = self.apps.write().await;
        let runtime = match apps.get_mut(id) {
            Some(r) => r,
            None => return,
        };

        runtime.stop_tx = None;

        let should_restart = match &reason {
            ProcessExitReason::StoppedCleanly => {
                runtime.status = AppStatus::Stopped;
                runtime.health = HealthStatus::Unknown;
                false
            }
            ProcessExitReason::Exited(code) => {
                let exit_code = *code;
                if exit_code == Some(0) {
                    runtime.status = AppStatus::Stopped;
                    runtime.health = HealthStatus::Unknown;
                    runtime.config.restart_policy == RestartPolicy::Always
                } else {
                    runtime.status = AppStatus::Crashed {
                        exit_code,
                        crashed_at: current_timestamp(),
                        error: Some(format!("Process exited with status code {:?}", exit_code)),
                    };
                    runtime.health = HealthStatus::Unhealthy("Process crashed".to_string());
                    matches!(
                        runtime.config.restart_policy,
                        RestartPolicy::Always | RestartPolicy::OnFailure
                    )
                }
            }
            ProcessExitReason::Crashed(err) => {
                runtime.status = AppStatus::Crashed {
                    exit_code: None,
                    crashed_at: current_timestamp(),
                    error: Some(err.clone()),
                };
                runtime.health = HealthStatus::Unhealthy(err.clone());
                matches!(
                    runtime.config.restart_policy,
                    RestartPolicy::Always | RestartPolicy::OnFailure
                )
            }
        };

        if should_restart {
            runtime.restart_count += 1;
            let restart_count = runtime.restart_count;
            runtime.logs.push(
                "system",
                format!(
                    "Restart policy triggered. Scheduling restart (count: {})...",
                    restart_count
                ),
            );
            drop(apps);
            let _ = self.restart_tx.send(id.to_string());
        }
    }

    pub async fn stop_app(self: &Arc<Self>, id: &str) -> Result<AppInfo, String> {
        let stop_tx = {
            let mut apps = self.apps.write().await;
            let runtime = apps
                .get_mut(id)
                .ok_or_else(|| format!("App '{}' not found", id))?;

            match runtime.status {
                AppStatus::Running { .. } | AppStatus::Starting => {
                    runtime.status = AppStatus::Stopping;
                    runtime.stop_tx.take()
                }
                _ => return Err(format!("App '{}' is not running", id)),
            }
        };

        if let Some(tx) = stop_tx {
            let _ = tx.send(());
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
        self.get_app(id)
            .await
            .ok_or_else(|| "App not found after stop".to_string())
    }

    pub async fn restart_app(self: &Arc<Self>, id: &str) -> Result<AppInfo, String> {
        let is_running = {
            let apps = self.apps.read().await;
            let runtime = apps
                .get(id)
                .ok_or_else(|| format!("App '{}' not found", id))?;
            matches!(
                runtime.status,
                AppStatus::Running { .. } | AppStatus::Starting
            )
        };

        if is_running {
            let _ = self.stop_app(id).await;
            tokio::time::sleep(Duration::from_millis(300)).await;
        }

        self.start_app(id).await
    }

    pub async fn delete_app(self: &Arc<Self>, id: &str) -> Result<(), String> {
        let _ = self.stop_app(id).await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        self.db
            .delete_app(id)
            .map_err(|e| format!("DB error: {}", e))?;
        self.apps.write().await.remove(id);
        Ok(())
    }

    pub async fn get_app(&self, id: &str) -> Option<AppInfo> {
        let apps = self.apps.read().await;
        apps.get(id).map(|r| self.build_app_info(r))
    }

    pub async fn list_apps(&self) -> Vec<AppInfo> {
        let apps = self.apps.read().await;
        apps.values().map(|r| self.build_app_info(r)).collect()
    }

    pub async fn get_app_by_domain(&self, host: &str) -> Option<(String, u16)> {
        let domain_only = host.split(':').next()?.trim().to_lowercase();
        let apps = self.apps.read().await;

        for (id, runtime) in apps.iter() {
            if let Some(ref d) = runtime.config.domain {
                if d == &domain_only && matches!(runtime.status, AppStatus::Running { .. }) {
                    if let Some(port) = runtime.config.port {
                        return Some((id.clone(), port));
                    }
                }
            }
        }
        None
    }

    pub async fn generate_caddyfile(&self) -> String {
        let apps = self.apps.read().await;
        let mut lines = Vec::new();

        lines.push("# Auto-generated Caddyfile by LaunchLite".to_string());
        for runtime in apps.values() {
            if let (Some(domain), Some(port)) = (&runtime.config.domain, runtime.config.port) {
                lines.push(format!(
                    "{} {{\n    reverse_proxy 127.0.0.1:{}\n}}",
                    domain, port
                ));
            }
        }
        lines.join("\n\n")
    }

    pub async fn get_logs(&self, id: &str) -> Option<Vec<LogLine>> {
        let apps = self.apps.read().await;
        apps.get(id).map(|r| r.logs.get_lines())
    }

    pub async fn export_data(&self) -> PlatformExport {
        let apps = self.apps.read().await;
        let mut configs: Vec<AppConfig> = apps.values().map(|r| r.config.clone()).collect();
        configs.sort_by(|a, b| a.name.cmp(&b.name));

        PlatformExport {
            version: "1.0".to_string(),
            platform: "LaunchLite".to_string(),
            exported_at: current_timestamp(),
            apps: configs,
        }
    }

    pub async fn import_data(self: &Arc<Self>, export: PlatformExport) -> Result<ImportSummary, String> {
        let mut imported = 0;
        let mut updated = 0;
        let mut failed = 0;
        let mut errors = Vec::new();

        for config in export.apps {
            let app_id = config.id.clone();
            if let Err(e) = self.db.save_app(&config) {
                errors.push(format!("{}: DB error: {}", app_id, e));
                failed += 1;
                continue;
            }

            let mut apps = self.apps.write().await;
            if let Some(runtime) = apps.get_mut(&app_id) {
                runtime.config = config;
                updated += 1;
            } else {
                let runtime = AppRuntime {
                    config,
                    status: AppStatus::Stopped,
                    health: HealthStatus::Unknown,
                    logs: LogBuffer::new(MAX_LOG_LINES),
                    stop_tx: None,
                    restart_count: 0,
                    started_at: None,
                    is_building: false,
                };
                apps.insert(app_id, runtime);
                imported += 1;
            }
        }

        Ok(ImportSummary {
            total: imported + updated + failed,
            imported,
            updated,
            failed,
            errors,
        })
    }

    fn build_app_info(&self, runtime: &AppRuntime) -> AppInfo {
        let now = current_timestamp();
        let (rss_bytes, uptime_secs) = match runtime.status {
            AppStatus::Running { pid, started_at } => {
                let mem = get_process_memory(pid).map(|m| m.rss_bytes);
                let uptime = if now >= started_at {
                    Some(now - started_at)
                } else {
                    Some(0)
                };
                (mem, uptime)
            }
            _ => (None, None),
        };

        AppInfo {
            config: runtime.config.clone(),
            status: runtime.status.clone(),
            health: runtime.health.clone(),
            rss_bytes,
            uptime_secs,
            restart_count: runtime.restart_count,
            is_building: runtime.is_building,
        }
    }

    pub async fn get_system_metrics(&self) -> SystemMetrics {
        let mem = get_self_memory();
        let apps = self.apps.read().await;
        let total_apps = apps.len();
        let running_apps = apps
            .values()
            .filter(|r| matches!(r.status, AppStatus::Running { .. }))
            .count();
        let now = current_timestamp();

        SystemMetrics {
            platform_rss_bytes: mem.rss_bytes,
            platform_vmsize_bytes: mem.vm_size_bytes,
            platform_uptime_secs: if now >= self.platform_started_at {
                now - self.platform_started_at
            } else {
                0
            },
            total_apps,
            running_apps,
            binary_size_bytes: get_binary_size(),
        }
    }

    async fn run_health_loop(self: Arc<Self>) {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;

            let running_apps_to_check: Vec<(String, Option<String>, u32)> = {
                let apps = self.apps.read().await;
                apps.iter()
                    .filter_map(|(id, r)| match r.status {
                        AppStatus::Running { pid, .. } => {
                            Some((id.clone(), r.config.health_check_url.clone(), pid))
                        }
                        _ => None,
                    })
                    .collect()
            };

            for (app_id, health_url, pid) in running_apps_to_check {
                let health_result = if let Some(url) = health_url {
                    check_health(&url).await
                } else if is_process_alive(pid) {
                    HealthStatus::Healthy
                } else {
                    HealthStatus::Unhealthy("Process not responding".to_string())
                };

                let mut apps = self.apps.write().await;
                if let Some(r) = apps.get_mut(&app_id) {
                    if matches!(r.status, AppStatus::Running { .. }) {
                        r.health = health_result;
                    }
                }
            }
        }
    }
}

fn rand_simple() -> u32 {
    let now = current_timestamp();
    ((now ^ (now >> 7)) % 9000 + 1000) as u32
}
