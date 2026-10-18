use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderName, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use reqwest::Method;
use std::str::FromStr;

use crate::api::AppState;

pub async fn proxy_handler(State(state): State<AppState>, req: Request) -> Response {
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    // Check if Host matches a configured running app domain
    if let Some((_app_id, target_port)) = state.manager.get_app_by_domain(&host).await {
        let path_and_query = req
            .uri()
            .path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or("/");

        let target_url = format!("http://127.0.0.1:{}{}", target_port, path_and_query);

        match forward_request(req, &target_url, &host).await {
            Ok(resp) => return resp,
            Err(e) => {
                return (
                    StatusCode::BAD_GATEWAY,
                    format!(
                        "Bad Gateway (Failed to proxy to 127.0.0.1:{}): {}",
                        target_port, e
                    ),
                )
                    .into_response();
            }
        }
    }

    (
        StatusCode::NOT_FOUND,
        format!(
            "404 Not Found: No active application found for domain '{}'. Ensure the custom domain is configured in the dashboard and the application is running.",
            host
        ),
    )
        .into_response()
}

pub async fn forward_request(
    req: Request,
    target_url: &str,
    original_host: &str,
) -> Result<Response, String> {
    let method = Method::from_str(req.method().as_str()).map_err(|e| e.to_string())?;
    let headers = req.headers().clone();

    let body_bytes = axum::body::to_bytes(req.into_body(), usize::MAX)
        .await
        .map_err(|e| format!("Failed to read request body: {}", e))?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_default();

    let mut builder = client.request(method, target_url);

    // Forward headers
    for (name, value) in headers.iter() {
        if name != header::HOST && name != header::CONTENT_LENGTH {
            builder = builder.header(name.as_str(), value.as_bytes());
        }
    }

    // Standard proxy forwarding headers
    builder = builder.header("X-Forwarded-Host", original_host);
    builder = builder.header("X-Forwarded-For", "127.0.0.1");
    builder = builder.header("X-Forwarded-Proto", "http");

    if !body_bytes.is_empty() {
        builder = builder.body(body_bytes);
    }

    let upstream_resp = builder
        .send()
        .await
        .map_err(|e| format!("Upstream connect error: {}", e))?;

    let status = StatusCode::from_u16(upstream_resp.status().as_u16())
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    let mut response_builder = Response::builder().status(status);

    for (name, val) in upstream_resp.headers().iter() {
        if name != header::TRANSFER_ENCODING && name != header::CONTENT_LENGTH {
            if let Ok(h_name) = HeaderName::from_str(name.as_str()) {
                if let Ok(h_val) = HeaderValue::from_bytes(val.as_bytes()) {
                    response_builder = response_builder.header(h_name, h_val);
                }
            }
        }
    }

    let body_bytes = upstream_resp.bytes().await.map_err(|e| e.to_string())?;
    let response = response_builder
        .body(Body::from(body_bytes))
        .map_err(|e| e.to_string())?;

    Ok(response)
}
