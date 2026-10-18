use crate::models::{AppInfo, AppStatus, GithubUser, HealthStatus, SystemMetrics};
use axum::response::Html;

pub fn render_dashboard(
    metrics: &SystemMetrics,
    apps: &[AppInfo],
    current_user: Option<&GithubUser>,
    has_oauth_config: bool,
) -> Html<String> {
    let platform_ram_mb = (metrics.platform_rss_bytes as f64) / (1024.0 * 1024.0);
    let binary_size_mb = (metrics.binary_size_bytes as f64) / (1024.0 * 1024.0);

    let auth_header_html = if let Some(user) = current_user {
        format!(
            r#"<div class="user-pill">
                <img src="{}" alt="{}" class="user-avatar" />
                <span class="user-name">{}</span>
                <button class="btn btn-sm btn-secondary" onclick="openGithubDeployModal()">Deploy GitHub Repo</button>
                <a href="/auth/logout" class="btn btn-sm btn-secondary">Logout</a>
            </div>"#,
            escape_html(&user.avatar_url),
            escape_html(&user.login),
            escape_html(user.name.as_deref().unwrap_or(&user.login))
        )
    } else if has_oauth_config {
        r#"<div class="user-pill">
            <a href="/auth/github/login" class="btn btn-sm btn-info">Login with GitHub</a>
            <button class="btn btn-sm btn-secondary" onclick="openPatModal()">Enter GitHub Token</button>
        </div>"#.to_string()
    } else {
        r#"<div class="user-pill">
            <button class="btn btn-sm btn-info" onclick="openPatModal()">Connect GitHub Token</button>
        </div>"#.to_string()
    };

    let mut apps_rows = String::new();
    if apps.is_empty() {
        apps_rows.push_str(r#"
            <tr>
                <td colspan="7" class="empty-state">
                    No applications deployed yet. Click <strong>+ Deploy App</strong> or <strong>Deploy GitHub Repo</strong> above to get started.
                </td>
            </tr>
        "#);
    } else {
        for app in apps {
            let (status_class, status_label) = if app.is_building {
                ("badge-warn", "Building...".to_string())
            } else {
                match &app.status {
                    AppStatus::Running { pid, .. } => {
                        ("badge-running", format!("Running (PID {})", pid))
                    }
                    AppStatus::Stopped => ("badge-stopped", "Stopped".to_string()),
                    AppStatus::Starting => ("badge-warn", "Starting...".to_string()),
                    AppStatus::Stopping => ("badge-warn", "Stopping...".to_string()),
                    AppStatus::Crashed { exit_code, .. } => {
                        ("badge-crashed", format!("Crashed (exit: {:?})", exit_code))
                    }
                }
            };

            let (health_class, health_label) = match &app.health {
                HealthStatus::Healthy => ("health-healthy", "Healthy"),
                HealthStatus::Degraded(_) => ("health-degraded", "Degraded"),
                HealthStatus::Unhealthy(err) => ("health-unhealthy", err.as_str()),
                HealthStatus::Unknown => ("health-unknown", "Unknown"),
            };

            let app_ram_display = match app.rss_bytes {
                Some(bytes) => format!("{:.1} MB", (bytes as f64) / (1024.0 * 1024.0)),
                None => "—".to_string(),
            };

            let uptime_display = match app.uptime_secs {
                Some(secs) => format_duration(secs),
                None => "—".to_string(),
            };

            let git_info_html = if let Some(ref git) = app.config.git {
                let commit_short = git
                    .last_commit_hash
                    .as_ref()
                    .map(|h| &h[..h.len().min(7)])
                    .unwrap_or("latest");
                format!(
                    r#"<div class="git-badge" title="{}">git: {} @ {}</div>"#,
                    escape_html(&git.repo_url),
                    escape_html(&git.branch),
                    commit_short
                )
            } else {
                "".to_string()
            };

            let domain_badge_html = if let Some(ref domain) = app.config.domain {
                let port_info = app
                    .config
                    .port
                    .map(|p| format!(" (port {})", p))
                    .unwrap_or_default();
                format!(
                    r#"<div><a href="http://{}" target="_blank" class="domain-badge" title="Custom Domain: {}{}">🌐 {}</a></div>"#,
                    escape_html(domain),
                    escape_html(domain),
                    port_info,
                    escape_html(domain)
                )
            } else if let Some(port) = app.config.port {
                format!(
                    r#"<div class="text-muted" style="font-size:0.75rem;">port: {}</div>"#,
                    port
                )
            } else {
                "".to_string()
            };

            let cmd_display = if app.config.args.is_empty() {
                app.config.executable.clone()
            } else {
                format!("{} {}", app.config.executable, app.config.args.join(" "))
            };

            let is_running = matches!(app.status, AppStatus::Running { .. });
            let has_git = app.config.git.is_some();

            let redeploy_btn = if has_git {
                format!(
                    r#"<button class="btn btn-sm btn-info" onclick="redeployApp('{}')">Redeploy</button>"#,
                    app.config.id
                )
            } else {
                "".to_string()
            };

            let actions_html = if is_running {
                format!(
                    r#"<button class="btn btn-sm btn-warn" onclick="appAction('{}', 'stop')">Stop</button>
                       <button class="btn btn-sm btn-info" onclick="appAction('{}', 'restart')">Restart</button>
                       {}
                       <button class="btn btn-sm btn-secondary" onclick="viewLogs('{}', '{}')">Logs</button>"#,
                    app.config.id, app.config.id, redeploy_btn, app.config.id, app.config.name
                )
            } else {
                format!(
                    r#"<button class="btn btn-sm btn-success" onclick="appAction('{}', 'start')">Start</button>
                       {}
                       <button class="btn btn-sm btn-secondary" onclick="viewLogs('{}', '{}')">Logs</button>
                       <button class="btn btn-sm btn-danger" onclick="deleteApp('{}')">Delete</button>"#,
                    app.config.id, redeploy_btn, app.config.id, app.config.name, app.config.id
                )
            };

            apps_rows.push_str(&format!(
                r#"
                <tr>
                    <td>
                        <div class="app-name">{}</div>
                        <div class="app-id">{}</div>
                        {}
                        {}
                    </td>
                    <td><code class="cmd-box" title="{}">{}</code></td>
                    <td><span class="badge {}">{}</span></td>
                    <td><span class="health-badge {}" title="{}">{}</span></td>
                    <td>{}</td>
                    <td>
                        <div>{}</div>
                        <small class="text-muted">Restarts: {}</small>
                    </td>
                    <td class="action-cell">{}</td>
                </tr>
                "#,
                escape_html(&app.config.name),
                escape_html(&app.config.id),
                domain_badge_html,
                git_info_html,
                escape_html(&cmd_display),
                escape_html(&truncate_str(&cmd_display, 30)),
                status_class,
                status_label,
                health_class,
                escape_html(health_label),
                health_label,
                app_ram_display,
                uptime_display,
                app.restart_count,
                actions_html
            ));
        }
    }

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Rust PaaS · Control Platform</title>
    <style>
        :root {{
            --bg: #0d1117;
            --surface: #161b22;
            --surface-hover: #1c2129;
            --border: #30363d;
            --text: #e6edf3;
            --text-muted: #8b949e;
            --primary: #238636;
            --primary-hover: #2ea043;
            --danger: #da3633;
            --danger-hover: #f85149;
            --warn: #d29922;
            --info: #388bfd;
            --info-hover: #58a6ff;
            --code-bg: #0d1117;
        }}
        * {{ box-sizing: border-box; margin: 0; padding: 0; }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
            background-color: var(--bg);
            color: var(--text);
            line-height: 1.5;
            padding: 24px;
        }}
        .container {{ max-width: 1240px; margin: 0 auto; }}
        header {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding-bottom: 20px;
            border-bottom: 1px solid var(--border);
            margin-bottom: 24px;
        }}
        .logo-group {{ display: flex; align-items: center; gap: 12px; }}
        .logo-badge {{
            background: linear-gradient(135deg, #f74c00, #dea584);
            color: #fff;
            font-weight: 700;
            padding: 4px 10px;
            border-radius: 6px;
            font-size: 0.85rem;
            letter-spacing: 0.5px;
        }}
        h1 {{ font-size: 1.5rem; font-weight: 600; }}
        .header-actions {{ display: flex; align-items: center; gap: 10px; }}

        .user-pill {{
            display: flex;
            align-items: center;
            gap: 10px;
            background: #1f242c;
            padding: 4px 10px;
            border-radius: 20px;
            border: 1px solid var(--border);
        }}
        .user-avatar {{ width: 26px; height: 26px; border-radius: 50%; }}
        .user-name {{ font-size: 0.85rem; font-weight: 600; color: #58a6ff; }}

        .metrics-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 16px;
            margin-bottom: 24px;
        }}
        .metric-card {{
            background-color: var(--surface);
            border: 1px solid var(--border);
            border-radius: 8px;
            padding: 16px;
        }}
        .metric-title {{
            font-size: 0.8rem;
            color: var(--text-muted);
            text-transform: uppercase;
            font-weight: 600;
            margin-bottom: 6px;
        }}
        .metric-value {{ font-size: 1.6rem; font-weight: 700; color: #58a6ff; }}
        .metric-sub {{ font-size: 0.75rem; color: var(--text-muted); margin-top: 4px; }}

        .section-header {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 14px;
        }}
        .section-header h2 {{ font-size: 1.2rem; font-weight: 600; }}

        .table-container {{
            background-color: var(--surface);
            border: 1px solid var(--border);
            border-radius: 8px;
            overflow-x: auto;
        }}
        table {{ width: 100%; border-collapse: collapse; text-align: left; font-size: 0.9rem; }}
        th {{
            background-color: #12161c;
            padding: 12px 16px;
            color: var(--text-muted);
            font-weight: 600;
            border-bottom: 1px solid var(--border);
        }}
        td {{
            padding: 12px 16px;
            border-bottom: 1px solid var(--border);
            vertical-align: middle;
        }}
        tr:last-child td {{ border-bottom: none; }}
        tr:hover td {{ background-color: var(--surface-hover); }}

        .app-name {{ font-weight: 600; color: #f0f6fc; }}
        .app-id {{ font-size: 0.75rem; color: var(--text-muted); font-family: monospace; }}
        .domain-badge {{
            font-size: 0.75rem;
            font-weight: 600;
            color: #388bfd;
            background: #388bfd1f;
            border: 1px solid #388bfd55;
            padding: 2px 7px;
            border-radius: 4px;
            display: inline-block;
            margin-top: 3px;
            text-decoration: none;
        }}
        .domain-badge:hover {{ text-decoration: underline; background: #388bfd33; }}
        .git-badge {{
            font-size: 0.7rem;
            color: #79c0ff;
            background: #388bfd1a;
            border: 1px solid #388bfd44;
            padding: 1px 6px;
            border-radius: 4px;
            display: inline-block;
            margin-top: 3px;
        }}
        .cmd-box {{
            background: var(--code-bg);
            padding: 3px 6px;
            border-radius: 4px;
            border: 1px solid var(--border);
            font-size: 0.8rem;
            display: inline-block;
            max-width: 240px;
            overflow: hidden;
            text-overflow: ellipsis;
            white-space: nowrap;
        }}
        .badge {{
            display: inline-block;
            padding: 2px 8px;
            border-radius: 12px;
            font-size: 0.75rem;
            font-weight: 600;
        }}
        .badge-running {{ background: #23863633; color: #3fb950; border: 1px solid #238636; }}
        .badge-stopped {{ background: #30363d55; color: #8b949e; border: 1px solid #30363d; }}
        .badge-warn {{ background: #d2992233; color: #e3b341; border: 1px solid #d29922; }}
        .badge-crashed {{ background: #da363333; color: #f85149; border: 1px solid #da3633; }}

        .health-badge {{
            display: inline-block;
            padding: 2px 8px;
            border-radius: 12px;
            font-size: 0.75rem;
            font-weight: 600;
        }}
        .health-healthy {{ color: #3fb950; }}
        .health-degraded {{ color: #e3b341; }}
        .health-unhealthy {{ color: #f85149; }}
        .health-unknown {{ color: #8b949e; }}

        .action-cell {{ display: flex; gap: 6px; align-items: center; }}
        .btn {{
            cursor: pointer;
            border: 1px solid transparent;
            border-radius: 6px;
            padding: 6px 14px;
            font-size: 0.85rem;
            font-weight: 500;
            color: #fff;
            transition: all 0.15s ease;
            text-decoration: none;
            display: inline-flex;
            align-items: center;
        }}
        .btn-sm {{ padding: 3px 8px; font-size: 0.75rem; }}
        .btn-primary {{ background: var(--primary); border-color: #2ea043; }}
        .btn-primary:hover {{ background: var(--primary-hover); }}
        .btn-secondary {{ background: #21262d; border-color: var(--border); color: #c9d1d9; }}
        .btn-secondary:hover {{ background: #30363d; }}
        .btn-success {{ background: #238636; border-color: #2ea043; }}
        .btn-warn {{ background: #9e6a03; border-color: #bb8009; }}
        .btn-danger {{ background: #b62324; border-color: #da3633; }}
        .btn-info {{ background: #1f6feb; border-color: #388bfd; }}

        .empty-state {{ text-align: center; color: var(--text-muted); padding: 40px !important; }}

        /* Modal styling */
        .modal-overlay {{
            display: none;
            position: fixed;
            top: 0; left: 0; right: 0; bottom: 0;
            background: rgba(0, 0, 0, 0.75);
            z-index: 100;
            align-items: center;
            justify-content: center;
        }}
        .modal {{
            background: var(--surface);
            border: 1px solid var(--border);
            border-radius: 10px;
            width: 100%;
            max-width: 600px;
            max-height: 90vh;
            overflow-y: auto;
            padding: 24px;
            box-shadow: 0 10px 30px rgba(0,0,0,0.5);
        }}
        .modal-header {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 18px;
        }}
        .close-btn {{ background: none; border: none; font-size: 1.5rem; color: var(--text-muted); cursor: pointer; }}
        .form-group {{ margin-bottom: 14px; }}
        .form-group label {{ display: block; font-size: 0.8rem; font-weight: 600; margin-bottom: 4px; color: #c9d1d9; }}
        .form-group input, .form-group select, .form-group textarea {{
            width: 100%;
            background: #0d1117;
            border: 1px solid var(--border);
            border-radius: 6px;
            padding: 8px 12px;
            color: #fff;
            font-size: 0.85rem;
            font-family: inherit;
        }}
        .form-group input:focus, .form-group select:focus, .form-group textarea:focus {{
            border-color: #58a6ff;
            outline: none;
        }}
        .form-row {{ display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }}

        .repo-card {{
            background: #0d1117;
            border: 1px solid var(--border);
            border-radius: 6px;
            padding: 10px 14px;
            margin-bottom: 8px;
            display: flex;
            justify-content: space-between;
            align-items: center;
        }}
        .repo-card:hover {{ border-color: #58a6ff; }}
        .repo-name {{ font-weight: 600; color: #58a6ff; font-size: 0.9rem; }}
        .repo-meta {{ font-size: 0.75rem; color: var(--text-muted); }}

        .log-terminal {{
            background: #05070a;
            color: #a5d6ff;
            font-family: ui-monospace, SFMono-Regular, Consolas, "Liberation Mono", Menlo, monospace;
            font-size: 0.8rem;
            padding: 12px;
            border-radius: 6px;
            border: 1px solid var(--border);
            max-height: 420px;
            overflow-y: auto;
            white-space: pre-wrap;
            word-break: break-all;
        }}
        .log-entry {{ margin-bottom: 4px; }}
        .log-time {{ color: #6e7681; margin-right: 8px; }}
        .log-tag-system {{ color: #d2a8ff; font-weight: bold; }}
        .log-tag-build {{ color: #f0883e; font-weight: bold; }}
        .log-tag-stdout {{ color: #7ee787; }}
        .log-tag-stderr {{ color: #ff7b72; }}
    </style>
</head>
<body>
    <div class="container">
        <header>
            <div class="logo-group">
                <div class="logo-badge">Rust PaaS</div>
                <h1>Control Platform</h1>
            </div>
            <div class="header-actions">
                {}
                <a href="/api/system/caddyfile" target="_blank" class="btn btn-secondary btn-sm" title="View auto-generated Caddy configuration">Caddyfile</a>
                <button class="btn btn-secondary" onclick="location.reload()">Refresh</button>
                <button class="btn btn-primary" onclick="openDeployModal()">+ Deploy App</button>
            </div>
        </header>

        <section class="metrics-grid">
            <div class="metric-card">
                <div class="metric-title">Platform Idle RAM (RSS)</div>
                <div class="metric-value">{:.2} MB</div>
                <div class="metric-sub">Pure Rust zero-dependency runtime</div>
            </div>
            <div class="metric-card">
                <div class="metric-title">Platform Binary Size</div>
                <div class="metric-value">{:.2} MB</div>
                <div class="metric-sub">Single standalone binary</div>
            </div>
            <div class="metric-card">
                <div class="metric-title">Applications Running</div>
                <div class="metric-value">{} / {}</div>
                <div class="metric-sub">Active Linux processes supervised</div>
            </div>
            <div class="metric-card">
                <div class="metric-title">Platform Uptime</div>
                <div class="metric-value">{}</div>
                <div class="metric-sub">Embedded SQLite storage active</div>
            </div>
        </section>

        <section>
            <div class="section-header">
                <h2>Hosted Applications</h2>
            </div>
            <div class="table-container">
                <table>
                    <thead>
                        <tr>
                            <th>Application & Custom Domain</th>
                            <th>Command</th>
                            <th>Status</th>
                            <th>Health</th>
                            <th>RAM</th>
                            <th>Uptime</th>
                            <th>Actions</th>
                        </tr>
                    </thead>
                    <tbody>
                        {}
                    </tbody>
                </table>
            </div>
        </section>
    </div>

    <!-- Direct Deploy Modal -->
    <div class="modal-overlay" id="deployModal">
        <div class="modal">
            <div class="modal-header">
                <h3>Deploy Application</h3>
                <button class="close-btn" onclick="closeDeployModal()">&times;</button>
            </div>
            <form id="deployForm" onsubmit="handleDeploy(event)">
                <div class="form-row">
                    <div class="form-group">
                        <label for="appName">App Name *</label>
                        <input type="text" id="appName" required placeholder="e.g. My Web Service">
                    </div>
                    <div class="form-group">
                        <label for="appId">App ID (optional)</label>
                        <input type="text" id="appId" placeholder="e.g. web-service">
                    </div>
                </div>
                <div class="form-group">
                    <label for="appDomain">Custom Domain (e.g. myapp.domain.com or myapp.local)</label>
                    <input type="text" id="appDomain" placeholder="e.g. api.example.com">
                </div>
                <div class="form-group">
                    <label for="gitRepo">Git Repository URL (Optional)</label>
                    <input type="text" id="gitRepo" placeholder="https://github.com/user/repo.git">
                </div>
                <div class="form-row">
                    <div class="form-group">
                        <label for="gitBranch">Git Branch</label>
                        <input type="text" id="gitBranch" value="main">
                    </div>
                    <div class="form-group">
                        <label for="restartPolicy">Restart Policy</label>
                        <select id="restartPolicy">
                            <option value="never">Never</option>
                            <option value="on_failure">On Failure</option>
                            <option value="always">Always</option>
                        </select>
                    </div>
                </div>
                <div class="form-group">
                    <label for="appExec">Executable / Run Command (Auto-detected if Git URL given)</label>
                    <input type="text" id="appExec" placeholder="e.g. ./target/release/server or node index.js">
                </div>
                <div class="form-group">
                    <label for="appArgs">Arguments (space separated)</label>
                    <input type="text" id="appArgs" placeholder="e.g. -p 8080">
                </div>
                <div class="form-group">
                    <label for="healthUrl">Health Check URL</label>
                    <input type="text" id="healthUrl" placeholder="http://127.0.0.1:8080/health">
                </div>
                <div style="display: flex; justify-content: flex-end; gap: 10px; margin-top: 20px;">
                    <button type="button" class="btn btn-secondary" onclick="closeDeployModal()">Cancel</button>
                    <button type="submit" class="btn btn-primary">Deploy</button>
                </div>
            </form>
        </div>
    </div>

    <!-- GitHub Repos Modal -->
    <div class="modal-overlay" id="githubModal">
        <div class="modal" style="max-width: 650px;">
            <div class="modal-header">
                <h3>Select GitHub Repository to Deploy</h3>
                <button class="close-btn" onclick="closeGithubModal()">&times;</button>
            </div>
            <div class="form-group">
                <input type="text" id="repoSearch" placeholder="Search your repositories..." oninput="filterRepos()">
            </div>
            <div id="repoList" style="max-height: 380px; overflow-y: auto; margin-bottom: 16px;">
                Loading your repositories...
            </div>
            <div style="display: flex; justify-content: flex-end;">
                <button class="btn btn-secondary btn-sm" onclick="closeGithubModal()">Close</button>
            </div>
        </div>
    </div>

    <!-- PAT Modal -->
    <div class="modal-overlay" id="patModal">
        <div class="modal" style="max-width: 480px;">
            <div class="modal-header">
                <h3>Connect GitHub Token</h3>
                <button class="close-btn" onclick="closePatModal()">&times;</button>
            </div>
            <p style="font-size: 0.85rem; color: var(--text-muted); margin-bottom: 12px;">
                Generate a Personal Access Token (classic) on GitHub with <code>repo</code> & <code>read:user</code> scopes to browse your repositories.
            </p>
            <div class="form-group">
                <label for="patInput">Personal Access Token</label>
                <input type="password" id="patInput" placeholder="ghp_...">
            </div>
            <div style="display: flex; justify-content: flex-end; gap: 10px;">
                <button class="btn btn-secondary" onclick="closePatModal()">Cancel</button>
                <button class="btn btn-primary" onclick="submitPat()">Connect</button>
            </div>
        </div>
    </div>

    <!-- Logs Modal -->
    <div class="modal-overlay" id="logsModal">
        <div class="modal" style="max-width: 850px;">
            <div class="modal-header">
                <h3 id="logsTitle">Application Logs</h3>
                <button class="close-btn" onclick="closeLogsModal()">&times;</button>
            </div>
            <div class="log-terminal" id="logContent">Loading logs...</div>
            <div style="display: flex; justify-content: space-between; align-items: center; margin-top: 14px;">
                <span class="text-muted" style="font-size: 0.75rem;">Build logs and application stdout/stderr streaming</span>
                <div style="display: flex; gap: 8px;">
                    <button class="btn btn-secondary btn-sm" onclick="refreshCurrentLogs()">Refresh Logs</button>
                    <button class="btn btn-secondary btn-sm" onclick="closeLogsModal()">Close</button>
                </div>
            </div>
        </div>
    </div>

    <script>
        let currentLogAppId = null;
        let allRepos = [];

        function openDeployModal() {{ document.getElementById('deployModal').style.display = 'flex'; }}
        function closeDeployModal() {{ document.getElementById('deployModal').style.display = 'none'; }}
        function openPatModal() {{ document.getElementById('patModal').style.display = 'flex'; }}
        function closePatModal() {{ document.getElementById('patModal').style.display = 'none'; }}
        function closeGithubModal() {{ document.getElementById('githubModal').style.display = 'none'; }}
        function closeLogsModal() {{ document.getElementById('logsModal').style.display = 'none'; currentLogAppId = null; }}

        async function submitPat() {{
            const token = document.getElementById('patInput').value.trim();
            if (!token) return;
            try {{
                const res = await fetch('/auth/token', {{
                    method: 'POST',
                    headers: {{ 'Content-Type': 'application/json' }},
                    body: JSON.stringify({{ token }})
                }});
                if (!res.ok) {{
                    const err = await res.text();
                    alert('Token validation failed: ' + err);
                    return;
                }}
                closePatModal();
                location.reload();
            }} catch (e) {{
                alert('Error: ' + e);
            }}
        }}

        async function openGithubDeployModal() {{
            document.getElementById('githubModal').style.display = 'flex';
            const listDiv = document.getElementById('repoList');
            listDiv.innerHTML = 'Loading your repositories...';
            try {{
                const res = await fetch('/auth/repos');
                if (!res.ok) {{
                    listDiv.innerHTML = '<p class="text-muted">Failed to load repositories. Please reconnect your GitHub token.</p>';
                    return;
                }}
                allRepos = await res.json();
                renderRepoList(allRepos);
            }} catch (e) {{
                listDiv.innerHTML = 'Error fetching repositories: ' + e;
            }}
        }}

        function renderRepoList(repos) {{
            const listDiv = document.getElementById('repoList');
            if (repos.length === 0) {{
                listDiv.innerHTML = '<p class="text-muted">No repositories found.</p>';
                return;
            }}
            listDiv.innerHTML = repos.map(r => `
                <div class="repo-card">
                    <div>
                        <div class="repo-name">${{escapeHtml(r.full_name)}} ${{r.private ? '<span style="font-size:0.7rem; color:#f0883e;">(Private)</span>' : ''}}</div>
                        <div class="repo-meta">${{r.language || 'Unknown'}} · Branch: ${{r.default_branch}}</div>
                    </div>
                    <button class="btn btn-sm btn-primary" onclick="deploySelectedRepoPrompt('${{r.full_name}}', '${{r.clone_url}}', '${{r.default_branch}}')">Deploy</button>
                </div>
            `).join('');
        }}

        function filterRepos() {{
            const query = document.getElementById('repoSearch').value.toLowerCase();
            const filtered = allRepos.filter(r => r.full_name.toLowerCase().includes(query));
            renderRepoList(filtered);
        }}

        async function deploySelectedRepoPrompt(name, cloneUrl, branch) {{
            const domain = prompt('Custom domain (leave blank if none, e.g. myapp.domain.com):', '');
            closeGithubModal();
            const payload = {{
                name: name,
                repo_url: cloneUrl,
                branch: branch,
                domain: domain ? domain.trim() : undefined,
                restart_policy: 'on_failure'
            }};
            try {{
                const res = await fetch('/api/deploy/git', {{
                    method: 'POST',
                    headers: {{ 'Content-Type': 'application/json' }},
                    body: JSON.stringify(payload)
                }});
                if (!res.ok) {{
                    alert('Deploy failed: ' + await res.text());
                    return;
                }}
                location.reload();
            }} catch (e) {{
                alert('Error deploying: ' + e);
            }}
        }}

        async function handleDeploy(e) {{
            e.preventDefault();
            const gitUrl = document.getElementById('gitRepo').value.trim();
            const domain = document.getElementById('appDomain').value.trim() || undefined;
            const rawArgs = document.getElementById('appArgs').value.trim();
            const args = rawArgs ? rawArgs.split(/\s+/) : [];

            if (gitUrl) {{
                const payload = {{
                    name: document.getElementById('appName').value.trim(),
                    id: document.getElementById('appId').value.trim() || undefined,
                    repo_url: gitUrl,
                    branch: document.getElementById('gitBranch').value.trim() || 'main',
                    domain: domain,
                    health_check_url: document.getElementById('healthUrl').value.trim() || undefined,
                    restart_policy: document.getElementById('restartPolicy').value
                }};
                const res = await fetch('/api/deploy/git', {{
                    method: 'POST',
                    headers: {{ 'Content-Type': 'application/json' }},
                    body: JSON.stringify(payload)
                }});
                if (!res.ok) {{ alert('Deploy failed: ' + await res.text()); return; }}
            }} else {{
                const payload = {{
                    name: document.getElementById('appName').value.trim(),
                    id: document.getElementById('appId').value.trim() || undefined,
                    executable: document.getElementById('appExec').value.trim(),
                    args: args,
                    domain: domain,
                    restart_policy: document.getElementById('restartPolicy').value,
                    health_check_url: document.getElementById('healthUrl').value.trim() || undefined,
                    auto_start: true
                }};
                const res = await fetch('/api/apps', {{
                    method: 'POST',
                    headers: {{ 'Content-Type': 'application/json' }},
                    body: JSON.stringify(payload)
                }});
                if (!res.ok) {{ alert('Deploy failed: ' + await res.text()); return; }}
            }}
            closeDeployModal();
            location.reload();
        }}

        async function redeployApp(id) {{
            if (!confirm(`Redeploy latest commit for '${{id}}'?`)) return;
            try {{
                const res = await fetch(`/api/apps/${{id}}/redeploy`, {{ method: 'POST' }});
                if (!res.ok) {{ alert('Redeploy failed: ' + await res.text()); return; }}
                location.reload();
            }} catch (e) {{
                alert('Error: ' + e);
            }}
        }}

        async function appAction(id, action) {{
            try {{
                const res = await fetch(`/api/apps/${{id}}/${{action}}`, {{ method: 'POST' }});
                if (!res.ok) {{ alert(`Failed to ${{action}}: ` + await res.text()); return; }}
                location.reload();
            }} catch (e) {{ alert('Error: ' + e); }}
        }}

        async function deleteApp(id) {{
            if (!confirm(`Delete application '${{id}}'?`)) return;
            try {{
                const res = await fetch(`/api/apps/${{id}}`, {{ method: 'DELETE' }});
                if (!res.ok) {{ alert('Delete failed: ' + await res.text()); return; }}
                location.reload();
            }} catch (e) {{ alert('Error: ' + e); }}
        }}

        async function viewLogs(id, name) {{
            currentLogAppId = id;
            document.getElementById('logsTitle').innerText = `Logs: ${{name}} (${{id}})`;
            document.getElementById('logsModal').style.display = 'flex';
            await refreshCurrentLogs();
        }}

        async function refreshCurrentLogs() {{
            if (!currentLogAppId) return;
            const logBox = document.getElementById('logContent');
            try {{
                const res = await fetch(`/api/apps/${{currentLogAppId}}/logs`);
                if (!res.ok) {{ logBox.innerText = 'Failed to load logs.'; return; }}
                const logs = await res.json();
                if (logs.length === 0) {{
                    logBox.innerText = 'No logs available for this application yet.';
                    return;
                }}
                logBox.innerHTML = logs.map(l => {{
                    const time = new Date(l.timestamp * 1000).toLocaleTimeString();
                    const tagClass = `log-tag-${{l.stream}}`;
                    return `<div class="log-entry"><span class="log-time">[${{time}}]</span><span class="${{tagClass}}">[${{l.stream}}]</span> ${{escapeHtml(l.message)}}</div>`;
                }}).join('');
                logBox.scrollTop = logBox.scrollHeight;
            }} catch (e) {{
                logBox.innerText = 'Error fetching logs: ' + e;
            }}
        }}

        function escapeHtml(text) {{
            const div = document.createElement('div');
            div.textContent = text;
            return div.innerHTML;
        }}
    </script>
</body>
</html>
"#,
        auth_header_html,
        platform_ram_mb,
        binary_size_mb,
        metrics.running_apps,
        metrics.total_apps,
        format_duration(metrics.platform_uptime_secs),
        apps_rows
    );

    Html(html)
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

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len])
    } else {
        s.to_string()
    }
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
