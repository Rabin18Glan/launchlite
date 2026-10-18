# Lightweight Rust PaaS

A minimal, ultra-low-overhead application hosting platform written in pure Rust.

## Architecture & Goals
- **Minimal RAM footprint**: ~4.8 MB to 6.4 MB idle RSS memory.
- **Small binary size**: Standalone release binary of ~4.1 MB.
- **Modular architecture**: Clean separation across models, db, auth, github, builder, execution, api, and web layers.
- **Custom Domains & Built-in Reverse Proxy**: Assign custom domains (like Vercel) with zero external proxy dependencies. Incoming HTTP requests are dynamically routed to the respective internal application port based on the `Host` header.
- **Caddyfile Auto-Generation**: Instant Caddy configuration export (`GET /api/system/caddyfile`) for automated Let's Encrypt TLS certificates in production.
- **Direct GitHub & Git Deployment**: Deploy directly from any GitHub repository with automatic project type detection (Rust, Node.js, Python, Go, Static HTML).
- **Dual Authentication**: GitHub OAuth 2.0 flow or instant Personal Access Token (PAT) connection.
- **Safe process supervision**: Graceful process termination with `SIGTERM` followed by a configurable timeout and `SIGKILL`, exit status capturing, and crash recovery policies (`Never`, `Always`, `OnFailure`).
- **Real-time log capture**: In-memory ring buffer (up to 500 lines per app) capturing build logs, process `stdout`, and `stderr`.

---

## Modular Codebase Structure

```
src/
├── config.rs              # Environment configuration & OAuth credentials
├── models/                # Domain models
│   ├── app.rs             # AppConfig (domain, port, git), AppStatus, HealthStatus
│   ├── auth.rs            # UserSession, GithubUser, GithubRepo
│   ├── build.rs           # BuildJob, BuildStatus, ProjectType
│   └── mod.rs             # SystemMetrics and re-exports
├── db/                    # SQLite database persistence
│   ├── apps.rs            # Application CRUD (with domain & port columns)
│   ├── builds.rs          # Build history and status
│   ├── sessions.rs        # User sessions & OAuth tokens
│   └── mod.rs             # Thread-safe Database wrapper
├── auth/                  # Authentication & OAuth
│   ├── oauth.rs           # OAuth authorization URL, code exchange, PAT auth
│   └── mod.rs
├── github/                # GitHub API integration
│   ├── client.rs          # User profile and repository listing client
│   └── mod.rs
├── builder/               # Git deployment & build pipeline
│   ├── detector.rs        # Auto-detects Rust, Node, Python, Go, Static projects
│   ├── git.rs             # Git clone, fetch, branch checkout, commit hash
│   ├── runner.rs          # Build execution with live log streaming
│   └── mod.rs
├── execution/             # Process runtime & supervisor
│   ├── health.rs          # Zero-dependency async TCP/HTTP health checker
│   ├── manager.rs         # ExecutionManager (port allocation, domains, Caddyfile)
│   ├── process.rs         # Safe process execution (SIGTERM/SIGKILL, log buffer)
│   └── mod.rs
├── api/                   # Axum REST API endpoints
│   ├── apps.rs            # App CRUD and control
│   ├── auth.rs            # GitHub OAuth & repository endpoints
│   ├── builds.rs          # Git deployment and redeployment triggers
│   ├── proxy.rs           # Built-in Host header reverse proxy forwarder
│   ├── system.rs          # Health, metrics, and Caddyfile export
│   └── mod.rs             # Router assembly & app state
├── web/                   # Server-rendered HTML frontend
│   ├── dashboard.rs       # Modern responsive UI with domain badges & GitHub modal
│   └── mod.rs
├── metrics.rs             # Linux /proc reader (RSS memory, binary size)
└── main.rs                # Platform entrypoint & graceful shutdown trap
```

---

## Custom Domain Configuration (Vercel-style)

Each application can have a custom domain (e.g., `api.example.com`, `myapp.domain.com`, `myapp.local`):

### 1. In the Web Dashboard
1. Click **+ Deploy App** or **Deploy GitHub Repo**.
2. Enter your custom domain in the **Custom Domain** field (e.g. `api.example.com`).
3. Deploy! The platform will:
   - Automatically allocate an internal port (e.g. `10000`) and inject `PORT=10000` into the app.
   - Show a clickable `🌐 api.example.com` badge in the dashboard.
   - Route any incoming HTTP request with `Host: api.example.com` directly to the app.

### 2. Via the API
```bash
curl -X POST http://127.0.0.1:3000/api/deploy/git \
  -H "Content-Type: application/json" \
  -d '{
    "name": "my-web-api",
    "repo_url": "https://github.com/my-user/my-web-api.git",
    "domain": "api.example.com",
    "branch": "main",
    "restart_policy": "on_failure"
  }'
```

### 3. Production SSL with Caddy (Optional)
If running behind Caddy for automatic Let's Encrypt SSL certificates, view or export the auto-generated Caddyfile:
```bash
curl http://127.0.0.1:3000/api/system/caddyfile
```
Output:
```caddyfile
api.example.com {
    reverse_proxy 127.0.0.1:10000
}
```
Or simply point Caddy / Cloudflare to the platform's port, and the built-in Rust reverse proxy handles the `Host` header routing automatically!
