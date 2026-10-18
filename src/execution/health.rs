use crate::models::HealthStatus;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

pub async fn check_health(url_str: &str) -> HealthStatus {
    let (host, port, path) = match parse_http_url(url_str) {
        Some(parsed) => parsed,
        None => return HealthStatus::Unhealthy(format!("Invalid health check URL: {}", url_str)),
    };

    let target = format!("{}:{}", host, port);
    let connect_res =
        tokio::time::timeout(Duration::from_secs(3), TcpStream::connect(&target)).await;

    let mut stream = match connect_res {
        Ok(Ok(stream)) => stream,
        Ok(Err(e)) => return HealthStatus::Unhealthy(format!("Connect failed: {}", e)),
        Err(_) => return HealthStatus::Unhealthy("Connect timeout (3s)".to_string()),
    };

    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: serverless-platform-healthcheck/0.1\r\nConnection: close\r\n\r\n",
        path, host
    );

    if let Err(e) = stream.write_all(request.as_bytes()).await {
        return HealthStatus::Unhealthy(format!("Write failed: {}", e));
    }

    let mut buf = [0u8; 512];
    let read_res = tokio::time::timeout(Duration::from_secs(3), stream.read(&mut buf)).await;
    match read_res {
        Ok(Ok(n)) if n > 0 => {
            let resp = String::from_utf8_lossy(&buf[..n]);
            let first_line = resp.lines().next().unwrap_or("");
            if first_line.starts_with("HTTP/1.1 2")
                || first_line.starts_with("HTTP/1.0 2")
                || first_line.starts_with("HTTP/1.1 3")
                || first_line.starts_with("HTTP/1.0 3")
            {
                HealthStatus::Healthy
            } else {
                HealthStatus::Unhealthy(format!("Unexpected response: {}", first_line))
            }
        }
        Ok(Ok(_)) => HealthStatus::Unhealthy("Empty response".to_string()),
        Ok(Err(e)) => HealthStatus::Unhealthy(format!("Read failed: {}", e)),
        Err(_) => HealthStatus::Unhealthy("Read timeout (3s)".to_string()),
    }
}

fn parse_http_url(url: &str) -> Option<(String, u16, String)> {
    let stripped = if let Some(s) = url.strip_prefix("http://") {
        s
    } else {
        url
    };

    let (host_port, path) = match stripped.find('/') {
        Some(idx) => (&stripped[..idx], &stripped[idx..]),
        None => (stripped, "/"),
    };

    let (host, port) = match host_port.find(':') {
        Some(idx) => {
            let host = &host_port[..idx];
            let port = host_port[idx + 1..].parse::<u16>().ok()?;
            (host.to_string(), port)
        }
        None => (host_port.to_string(), 80),
    };

    let host = if host == "localhost" {
        "127.0.0.1".to_string()
    } else {
        host
    };

    Some((host, port, path.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_url() {
        let (host, port, path) = parse_http_url("http://127.0.0.1:8080/health").unwrap();
        assert_eq!(host, "127.0.0.1");
        assert_eq!(port, 8080);
        assert_eq!(path, "/health");

        let (host, port, path) = parse_http_url("http://localhost:3000").unwrap();
        assert_eq!(host, "127.0.0.1");
        assert_eq!(port, 3000);
        assert_eq!(path, "/");
    }
}
