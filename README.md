# LaunchLite — Lightweight Pure-Rust PaaS

A minimal, ultra-low-overhead application hosting platform written in pure Rust, organized as a modular Cargo workspace monorepo.

## One-Line Install & Run

### 1. Install LaunchLite
```bash
curl -fsSL https://raw.githubusercontent.com/Rabin18Glan/launchlite/main/install.sh | bash
```

### 2. Run as Background Service (One-Liner)
```bash
# Start in background on default port 3000
launchlite start

# Or start on a custom port
launchlite start --port 8080
```

### 3. Manage the Background Service
```bash
# Check live status, memory (RSS), and running apps
launchlite status

# Stop the background service
launchlite stop

# (Optional) Install as permanent systemd service (auto-starts on boot)
sudo launchlite service install --port 3000
```

Open `http://localhost:3000` (or your configured port) in your browser.

---

## Architecture & Goals
- **Minimal RAM footprint**: ~4.8 MB to 6.4 MB idle RSS memory.
- **Small binary size**: Standalone release binary of ~4.7 MB.
- **Modern Clean UI**: Server-rendered frontend using [`axum-template`](https://docs.rs/axum-template/latest/axum_template/) with [MiniJinja](https://github.com/mitsuhiko/minijinja) and Tailwind CSS via CDN. Zero Node.js/npm dependencies, instant fast iteration, and clean dark aesthetic.
- **VPS Migration (Export & Import)**: 1-click migration between VPS servers. Export all project configs, custom domains, git repos, and environment variables to a portable backup JSON file, and restore on any new server instantly.
- **Custom Domains & Built-in Reverse Proxy**: Assign custom domains (e.g. `api.example.com`, `myapp.domain.com`) with zero external proxy dependencies. Incoming HTTP requests are dynamically routed to the respective internal application port based on the `Host` header.
- **Caddyfile Auto-Generation**: Instant Caddy configuration export (`GET /api/system/caddyfile`) for automated Let's Encrypt TLS certificates in production.
- **Direct GitHub & Git Deployment**: Deploy directly from any GitHub repository with automatic project type detection (Rust, Node.js, Python, Go, Static HTML).
- **Dual Authentication**: GitHub OAuth 2.0 flow or instant Personal Access Token (PAT) connection.
- **Safe process supervision**: Graceful process termination with `SIGTERM` followed by a configurable timeout and `SIGKILL`, exit status capturing, and crash recovery policies (`Never`, `Always`, `OnFailure`).
- **Real-time log capture**: In-memory ring buffer (up to 500 lines per app) capturing build logs, process `stdout`, and `stderr`.

---

## CLI Command Reference

| Command | Description |
| :--- | :--- |
| `launchlite` | Runs the server in foreground on port `3000` (or `LAUNCHLITE_PORT`) |
| `launchlite -p <PORT>` | Runs the server in foreground on specified port |
| `launchlite start [-p PORT]` | Spawns LaunchLite as a detached background service |
| `launchlite stop` | Stops the running background service |
| `launchlite status` | Displays live background PID, HTTP API status, and RAM RSS |
| `launchlite service install [-p PORT]` | Generates and enables a Linux `systemd` service unit |
| `launchlite --help` | Displays help message and command examples |

---

## Development & Building

```bash
# Build all crates in workspace
cargo build

# Run unit tests across all crates
cargo test --workspace

# Auto-recompile during development with cargo-watch
cargo watch -c -x "run -p cli"

# Compile optimized release binary
cargo build --release -p cli
```

---

## Migrating Between VPS Servers (Export & Import)

LaunchLite includes built-in migration to easily move between VPS providers:

### In the Web Dashboard
1. **On your old VPS**: Click **Export** in the top navigation bar to download `launchlite-backup-<timestamp>.json`.
2. **On your new VPS**: Click **Import**, select your backup JSON file (or paste its content), and click **Import & Restore**. All applications, ports, domains, and git repos are immediately registered.

### Via the REST API
```bash
# 1. Export backup from old server
curl -s http://old-vps:3000/api/system/export > backup.json

# 2. Import backup into new server
curl -X POST http://new-vps:3000/api/system/import \
  -H "Content-Type: application/json" \
  --data-binary @backup.json
```

---

## Custom Domain Configuration

Each application can have a custom domain (e.g., `api.example.com`, `myapp.domain.com`, `myapp.local`):

### 1. In the Web Dashboard
1. Click **+ New Project**.
2. Select **Git Repository** or **Executable / Command**.
3. Enter your custom domain in the **Custom Domain** field (e.g. `api.example.com`).
4. Deploy! The platform will:
   - Automatically allocate an internal port (e.g. `10000`) and inject `PORT=10000` into the app.
   - Show a clickable `🌐 api.example.com` pill in the project card.
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

---

## License

This project is licensed under the [MIT License](LICENSE).
