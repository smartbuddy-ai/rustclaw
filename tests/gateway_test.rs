use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

fn test_cfg() -> rustclaw::config::Config {
    let workspace_dir = std::env::temp_dir().join(format!("rustclaw-gw-itest-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&workspace_dir).unwrap();
    rustclaw::config::Config {
        workspace_dir,
        auth: rustclaw::config::AuthConfig::default(),
        channels: rustclaw::config::ChannelsConfig::default(),
        cron: rustclaw::config::CronConfig::default(),
        node: rustclaw::config::NodeConfig::default(),
        memory: rustclaw::config::MemoryConfig::default(),
        tunnel: rustclaw::config::TunnelConfig::default(),
        gateway: rustclaw::config::GatewayConfig::default(),
        tools: rustclaw::config::ToolsConfig::default(),
    }
}

#[tokio::test]
async fn health_endpoint_returns_healthy() {
    let app = rustclaw::gateway::router(test_cfg());
    let resp = app.oneshot(Request::builder().uri("/api/health").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "healthy");
}

#[tokio::test]
async fn ready_endpoint_returns_ready() {
    let app = rustclaw::gateway::router(test_cfg());
    let resp = app.oneshot(Request::builder().uri("/api/ready").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn metrics_endpoint_returns_prometheus_format() {
    let app = rustclaw::gateway::router(test_cfg());
    let resp = app.oneshot(Request::builder().uri("/api/metrics").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("rustclaw_uptime_seconds"));
}

#[tokio::test]
async fn chat_rejects_empty_message() {
    let app = rustclaw::gateway::router(test_cfg());
    let resp = app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/chat")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"message":""}"#))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn chat_rejects_oversized_message() {
    let app = rustclaw::gateway::router(test_cfg());
    let big_msg = "x".repeat(33_000);
    let body = serde_json::json!({"message": big_msg}).to_string();
    let resp = app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/chat")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn auth_rejects_without_token() {
    let mut cfg = test_cfg();
    cfg.gateway.auth.mode = "token".into();
    cfg.gateway.auth.token = Some("secret123".into());
    let app = rustclaw::gateway::router(cfg);
    let resp = app.oneshot(Request::builder().uri("/api/status").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn auth_accepts_valid_token() {
    let mut cfg = test_cfg();
    cfg.gateway.auth.mode = "token".into();
    cfg.gateway.auth.token = Some("secret123".into());
    let app = rustclaw::gateway::router(cfg);
    let resp = app.oneshot(
        Request::builder()
            .uri("/api/status")
            .header("authorization", "Bearer secret123")
            .body(Body::empty())
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn rate_limit_blocks_after_limit() {
    let mut cfg = test_cfg();
    cfg.gateway.rate_limit.enabled = true;
    cfg.gateway.rate_limit.requests_per_minute = 2;
    let app = rustclaw::gateway::router(cfg);

    // Tower oneshot consumes the router, so we need to clone via into_service
    // Actually for rate limit test we need multiple requests — use a shared service
    use tower::Service;
    let mut svc = app.into_service();

    for i in 0..3 {
        let req = Request::builder().uri("/api/status").body(Body::empty()).unwrap();
        let resp = svc.call(req).await.unwrap();
        if i < 2 {
            assert_eq!(resp.status(), StatusCode::OK, "request {i} should succeed");
        } else {
            assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS, "request {i} should be rate limited");
        }
    }
}

#[tokio::test]
async fn skills_endpoint_returns_list() {
    let app = rustclaw::gateway::router(test_cfg());
    let resp = app.oneshot(Request::builder().uri("/api/skills").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}
