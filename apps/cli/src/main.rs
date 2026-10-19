use common::Config;
use std::env;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

const DEFAULT_PORT: u16 = 3000;
const PID_FILE: &str = "./data/launchlite.pid";
const PORT_FILE: &str = "./data/launchlite.port";
const LOG_FILE: &str = "./data/launchlite.log";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    // Check for subcommands or flags
    if args.len() > 1 {
        match args[1].as_str() {
            "--help" | "-h" | "help" => {
                print_help();
                return Ok(());
            }
            "--version" | "-v" | "version" => {
                println!("LaunchLite v0.1.0 (modular PaaS)");
                return Ok(());
            }
            "start" => {
                let port = parse_port(&args, 2).unwrap_or(DEFAULT_PORT);
                return start_background(port);
            }
            "stop" => {
                return stop_background();
            }
            "status" => {
                return check_status().await;
            }
            "service" => {
                if args.len() > 2 && args[2] == "install" {
                    let port = parse_port(&args, 3).unwrap_or(DEFAULT_PORT);
                    return install_systemd_service(port);
                } else {
                    println!("Usage: launchlite service install [--port <PORT>]");
                    return Ok(());
                }
            }
            "run" => {
                let port = parse_port(&args, 2).unwrap_or(DEFAULT_PORT);
                return run_foreground(port).await;
            }
            _ => {
                // If flag like --daemon or -d is passed
                if args.contains(&"--daemon".to_string()) || args.contains(&"-d".to_string()) {
                    let port = parse_port(&args, 1).unwrap_or(DEFAULT_PORT);
                    return start_background(port);
                }
                // If --port or -p is passed
                if let Some(port) = parse_port(&args, 1) {
                    return run_foreground(port).await;
                }
            }
        }
    }

    // Default: run foreground with env or default port
    let default_port = env::var("LAUNCHLITE_PORT")
        .or_else(|_| env::var("PLATFORM_PORT"))
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_PORT);

    run_foreground(default_port).await
}

fn parse_port(args: &[String], start_idx: usize) -> Option<u16> {
    for i in start_idx..args.len() {
        if (args[i] == "--port" || args[i] == "-p") && i + 1 < args.len() {
            if let Ok(p) = args[i + 1].parse::<u16>() {
                return Some(p);
            }
        } else if let Some(stripped) = args[i].strip_prefix("--port=") {
            if let Ok(p) = stripped.parse::<u16>() {
                return Some(p);
            }
        } else if let Ok(p) = args[i].parse::<u16>() {
            return Some(p);
        }
    }
    None
}

async fn run_foreground(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "api=info,launchlite=info,cli=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let mut config = Config::from_env();
    config.port = port;

    api::run_server(config).await
}

fn start_background(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let _ = fs::create_dir_all("./data");

    // Check if already running
    if Path::new(PID_FILE).exists() {
        if let Ok(pid_str) = fs::read_to_string(PID_FILE) {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                if is_pid_alive(pid) {
                    println!(
                        "LaunchLite is already running in background (PID: {}).",
                        pid
                    );
                    println!("To stop it: launchlite stop");
                    return Ok(());
                }
            }
        }
    }

    let exe = env::current_exe()?;
    let log_file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(LOG_FILE)?;

    let mut cmd = Command::new(&exe);
    cmd.arg("run")
        .arg("--port")
        .arg(port.to_string())
        .stdout(Stdio::from(log_file.try_clone()?))
        .stderr(Stdio::from(log_file))
        .stdin(Stdio::null());

    #[cfg(unix)]
    cmd.process_group(0);

    let child = cmd.spawn()?;
    let pid = child.id();
    fs::write(PID_FILE, pid.to_string())?;
    fs::write(PORT_FILE, port.to_string())?;

    // Give process 200ms to boot
    std::thread::sleep(std::time::Duration::from_millis(200));

    println!(
        "\x1b[32m✓ LaunchLite started in background (PID: {})\x1b[0m",
        pid
    );
    println!("  Dashboard & API : http://127.0.0.1:{}", port);
    println!("  Log output      : {}", LOG_FILE);
    println!("  PID file        : {}", PID_FILE);
    println!("  Stop service    : launchlite stop");
    Ok(())
}

fn stop_background() -> Result<(), Box<dyn std::error::Error>> {
    if !Path::new(PID_FILE).exists() {
        println!("LaunchLite background service is not running (no PID file found).");
        return Ok(());
    }

    let pid_str = fs::read_to_string(PID_FILE)?;
    let pid = pid_str.trim().parse::<i32>()?;

    if is_pid_alive(pid) {
        unsafe {
            libc::kill(pid, libc::SIGTERM);
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
        let _ = fs::remove_file(PID_FILE);
        let _ = fs::remove_file(PORT_FILE);
        println!(
            "\x1b[32m✓ LaunchLite (PID: {}) stopped successfully.\x1b[0m",
            pid
        );
    } else {
        let _ = fs::remove_file(PID_FILE);
        let _ = fs::remove_file(PORT_FILE);
        println!(
            "LaunchLite process (PID: {}) was already terminated. Cleaned up PID file.",
            pid
        );
    }
    Ok(())
}

async fn check_status() -> Result<(), Box<dyn std::error::Error>> {
    let pid_info = if Path::new(PID_FILE).exists() {
        fs::read_to_string(PID_FILE).ok()
    } else {
        None
    };

    println!("\x1b[1mLaunchLite Status\x1b[0m");
    println!("─────────────────────────────────────────");

    let is_alive = if let Some(ref pid_str) = pid_info {
        if let Ok(pid) = pid_str.trim().parse::<i32>() {
            if is_pid_alive(pid) {
                println!(
                    "  Status     : \x1b[32m● Running\x1b[0m (Background PID: {})",
                    pid
                );
                true
            } else {
                println!("  Status     : \x1b[31m● Stale (Dead PID: {})\x1b[0m", pid);
                false
            }
        } else {
            false
        }
    } else {
        println!("  Status     : \x1b[33m○ No active background PID\x1b[0m");
        false
    };

    // Detect port from port file or env or default
    let port = fs::read_to_string(PORT_FILE)
        .ok()
        .and_then(|s| s.trim().parse::<u16>().ok())
        .or_else(|| {
            env::var("LAUNCHLITE_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
        })
        .unwrap_or(DEFAULT_PORT);

    let url = format!("http://127.0.0.1:{}/api/system", port);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()?;

    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(metrics) = resp.json::<serde_json::Value>().await {
                println!("  API URL    : http://127.0.0.1:{}", port);
                println!(
                    "  RAM (RSS)  : {:.1} MB",
                    metrics["platform_rss_bytes"].as_f64().unwrap_or(0.0) / (1024.0 * 1024.0)
                );
                println!(
                    "  Apps Total : {}",
                    metrics["total_apps"].as_i64().unwrap_or(0)
                );
                println!(
                    "  Apps Live  : {}",
                    metrics["running_apps"].as_i64().unwrap_or(0)
                );
            }
        }
        _ => {
            if is_alive {
                println!(
                    "  HTTP API   : Starting up or listening on different port ({})",
                    port
                );
            } else {
                println!("  HTTP API   : Offline");
            }
        }
    }
    println!("─────────────────────────────────────────");
    Ok(())
}

fn install_systemd_service(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let exe = env::current_exe()?;
    let exe_path = exe.canonicalize()?.to_string_lossy().to_string();
    let cwd = env::current_dir()?
        .canonicalize()?
        .to_string_lossy()
        .to_string();

    let service_content = format!(
        r#"[Unit]
Description=LaunchLite Lightweight PaaS
After=network.target

[Service]
Type=simple
WorkingDirectory={cwd}
ExecStart={exe_path} run --port {port}
Restart=always
RestartSec=3
LimitNOFILE=65535
Environment=LAUNCHLITE_PORT={port}

[Install]
WantedBy=multi-user.target
"#
    );

    let systemd_dir = "/etc/systemd/system";
    let service_file = format!("{}/launchlite.service", systemd_dir);

    if unsafe { libc::geteuid() } == 0 {
        fs::write(&service_file, service_content)?;
        let _ = Command::new("systemctl").arg("daemon-reload").status();
        let _ = Command::new("systemctl")
            .arg("enable")
            .arg("--now")
            .arg("launchlite")
            .status();
        println!(
            "\x1b[32m✓ systemd service installed and started at {}\x1b[0m",
            service_file
        );
        println!("  Status: systemctl status launchlite");
        println!("  Logs  : journalctl -u launchlite -f");
    } else {
        println!("Root privileges required to install to /etc/systemd/system.");
        println!(
            "Run with sudo or copy this file manually to {}:\n",
            service_file
        );
        println!("{}", service_content);
    }
    Ok(())
}

fn is_pid_alive(pid: i32) -> bool {
    unsafe { libc::kill(pid, 0) == 0 }
}

fn print_help() {
    println!(
        r#"LaunchLite — Minimal & Fast Pure-Rust PaaS

USAGE:
    launchlite [COMMAND] [OPTIONS]

COMMANDS:
    start                     Start LaunchLite as a background service
    stop                      Stop the running background service
    status                    Show current status, live apps, and RAM
    run                       Run in foreground (interactive logs)
    service install           Install and enable a systemd service unit

OPTIONS:
    -p, --port <PORT>         Port to listen on (default: 3000)
    -d, --daemon              Run in background (same as 'start')
    -h, --help                Show this help message
    -v, --version             Show version

EXAMPLES:
    # Run in foreground on default port 3000
    launchlite

    # Start in background on custom port 8080
    launchlite start --port 8080

    # Check live status
    launchlite status

    # Stop background service
    launchlite stop
"#
    );
}
