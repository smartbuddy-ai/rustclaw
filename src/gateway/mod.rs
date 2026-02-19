use crate::config::Config;
use crate::memory::SqliteMemory;
use crate::telemetry::Telemetry;
use axum::{
    Router,
    extract::State,
    http::{HeaderMap, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc, time::{Duration, Instant}};
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};

const INDEX_HTML: &str = r#"<!doctype html><html><head><meta charset='utf-8'><title>Rustclaw</title>
<style>body{background:#0d1117;color:#c9d1d9;font-family:system-ui;margin:0}a{color:#58a6ff}.wrap{max-width:1100px;margin:20px auto;padding:20px}.cards{display:grid;grid-template-columns:repeat(4,1fr);gap:12px}.card{background:#161b22;padding:12px;border-radius:10px;border:1px solid #30363d}.chat{margin-top:20px}.row{display:flex;gap:8px}input,button,textarea{background:#0d1117;color:#c9d1d9;border:1px solid #30363d;border-radius:8px;padding:8px}pre{white-space:pre-wrap;background:#161b22;padding:12px;border-radius:8px}</style>
</head><body><div class='wrap'><h1>Rustclaw Dashboard</h1><div id='cards' class='cards'></div><div class='chat'><h2>Chat</h2><div class='row'><input id='msg' style='flex:1' placeholder='Type a message'><button onclick='sendChat()'>Send</button></div><pre id='reply'></pre></div><h2>Config</h2><pre id='cfg'></pre></div>
<script>
async function refresh(){
 const st=await (await fetch('/api/status')).json();
 document.getElementById('cards').innerHTML=`<div class='card'><b>Agent</b><div>${st.status}</div></div><div class='card'><b>Channels</b><div>${st.channels_connected}</div></div><div class='card'><b>Memory</b><div>${st.memory_count}</div></div><div class='card'><b>Uptime</b><div>${st.uptime_secs}s</div></div>`;
 const cfg=await (await fetch('/api/config')).json(); document.getElementById('cfg').textContent=JSON.stringify(cfg,null,2);
}
async function sendChat(){ const m=document.getElementById('msg').value; const r=await fetch('/api/chat',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({message:m})}); const j=await r.json(); document.getElementById('reply').textContent=j.reply; }
refresh(); setInterval(refresh,5000);
</script></body></html>"#;

#[derive(Clone)]
struct AppState {
    cfg: Config,
    limiter: Arc<RwLock<HashMap<String, RateWindow>>>,
    started_at: Instant,
    telemetry: Telemetry,
}

#[derive(Clone, Debug)]
struct RateWindow { started_at: Instant, count: u32 }

#[derive(Debug, Serialize)]
struct StatusResponse {
    status: &'static str,
    version: &'static str,
    workspace: String,
    channels_connected: usize,
    memory_count: usize,
    uptime_secs: u64,
}

#[derive(Debug, Deserialize)]
struct ChatRequest { message: String, session_id: Option<String>, channel: Option<String> }
#[derive(Debug, Serialize)]
struct ChatResponse { reply: String }

pub fn router(cfg: Config) -> Router {
    let state = Arc::new(AppState { cfg: cfg.clone(), limiter: Arc::new(RwLock::new(HashMap::new())), started_at: Instant::now(), telemetry: Telemetry::new() });
    let api = Router::new()
        .route("/status", get(api_status))
        .route("/config", get(api_config))
        .route("/chat", post(api_chat))
        .route("/skills", get(api_skills))
        .route("/health", get(api_health))
        .route("/ready", get(api_ready))
        .route("/metrics", get(api_metrics))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
        .layer(middleware::from_fn_with_state(state.clone(), rate_limit_middleware));

    Router::new().route("/", get(index)).nest("/api", api).layer(build_cors_layer(&cfg)).with_state(state)
}

fn build_cors_layer(cfg: &Config) -> CorsLayer {
    let origins = &cfg.gateway.cors.allowed_origins;
    if origins.iter().any(|o| o == "*") { CorsLayer::new().allow_origin(Any).allow_methods([Method::GET, Method::POST, Method::OPTIONS]).allow_headers(Any) }
    else {
        let mut layer = CorsLayer::new().allow_methods([Method::GET, Method::POST, Method::OPTIONS]).allow_headers(Any);
        for origin in origins { if let Ok(v) = HeaderValue::from_str(origin) { layer = layer.allow_origin(v); } }
        layer
    }
}

async fn auth_middleware(State(state): State<Arc<AppState>>, headers: HeaderMap, request: axum::extract::Request, next: Next) -> Response {
    if state.cfg.gateway.auth.mode.eq_ignore_ascii_case("none") { return next.run(request).await; }
    let expected = state.cfg.gateway.auth.token.clone().or_else(|| std::env::var("RUSTCLAW_GATEWAY_TOKEN").ok()).unwrap_or_default();
    let provided = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer "));
    match provided { Some(token) if timing_safe_eq(token.as_bytes(), expected.as_bytes()) => next.run(request).await, _ => (StatusCode::UNAUTHORIZED, "unauthorized").into_response() }
}

async fn rate_limit_middleware(State(state): State<Arc<AppState>>, headers: HeaderMap, request: axum::extract::Request, next: Next) -> Response {
    let rl = &state.cfg.gateway.rate_limit;
    if !rl.enabled { return next.run(request).await; }
    let ip = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()).and_then(|v| v.split(',').next()).map(|v| v.trim().to_string()).unwrap_or_else(|| "local".to_string());
    let mut map = state.limiter.write().await;
    let now = Instant::now();
    let window = map.entry(ip).or_insert(RateWindow { started_at: now, count: 0 });
    if now.duration_since(window.started_at) >= Duration::from_secs(60) { window.started_at = now; window.count = 0; }
    if window.count >= rl.requests_per_minute { return (StatusCode::TOO_MANY_REQUESTS, "rate limit exceeded").into_response(); }
    window.count += 1;
    next.run(request).await
}

fn timing_safe_eq(a: &[u8], b: &[u8]) -> bool { if a.len()!=b.len(){return false;} let mut d=0u8; for (x,y) in a.iter().zip(b.iter()){ d |= x ^ y;} d==0 }

pub async fn run(cfg: Config) -> anyhow::Result<()> {
    let addr = format!("{}:{}", cfg.gateway.host, cfg.gateway.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, router(cfg)).await?;
    Ok(())
}

/// Run the gateway with graceful shutdown support via a CancellationToken.
pub async fn run_with_shutdown(
    cfg: Config,
    cancel: tokio_util::sync::CancellationToken,
) -> anyhow::Result<()> {
    let addr = format!("{}:{}", cfg.gateway.host, cfg.gateway.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(addr = %addr, "gateway listening");
    axum::serve(listener, router(cfg))
        .with_graceful_shutdown(async move { cancel.cancelled().await })
        .await?;
    Ok(())
}

async fn index() -> Html<&'static str> { Html(INDEX_HTML) }

async fn api_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let memory_count = count_memories(&state.cfg).unwrap_or(0);
    let channels_connected = [state.cfg.channels.telegram.is_some(), state.cfg.channels.whatsapp.is_some(), state.cfg.channels.slack.is_some(), state.cfg.channels.discord.is_some(), state.cfg.channels.signal.is_some()].into_iter().filter(|x| *x).count();
    axum::Json(StatusResponse { status: "ok", version: env!("CARGO_PKG_VERSION"), workspace: state.cfg.workspace_dir.display().to_string(), channels_connected, memory_count, uptime_secs: state.started_at.elapsed().as_secs() })
}

fn count_memories(cfg: &Config) -> anyhow::Result<usize> {
    let mem = SqliteMemory::from_config(cfg)?;
    Ok(mem.search("", 10_000)?.len())
}

async fn api_config(State(state): State<Arc<AppState>>) -> impl IntoResponse { axum::Json(serde_json::to_value(&state.cfg).unwrap_or(serde_json::json!({}))) }

async fn api_chat(State(state): State<Arc<AppState>>, axum::Json(req): axum::Json<ChatRequest>) -> Result<axum::Json<ChatResponse>, (StatusCode, String)> {
    if req.message.trim().is_empty() { return Err((StatusCode::BAD_REQUEST, "message cannot be empty".into())); }

    // Enforce max message length
    if req.message.len() > 32_000 {
        return Err((StatusCode::BAD_REQUEST, "message too long (max 32000 chars)".into()));
    }

    state.telemetry.inc("chat_requests_total").await;
    let tracker = crate::telemetry::LatencyTracker::start(&state.telemetry, "chat_latency_secs");

    let reply = if let Some(session_id) = req.session_id.as_deref() { crate::chat::send_with_session(&state.cfg, session_id, &req.message).await.map_err(internal_err)? } else { crate::chat::send(&state.cfg, &req.message, req.channel.as_deref()).await.map_err(internal_err)? };

    tracker.finish().await;
    Ok(axum::Json(ChatResponse { reply }))
}

async fn api_skills(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let out = crate::skills::SkillRegistry::scan(&state.cfg.workspace_dir).map(|r| r.list()).unwrap_or_default();
    axum::Json(out)
}

async fn api_health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let health = state.telemetry.health_json().await;
    axum::Json(health)
}

async fn api_ready() -> impl IntoResponse {
    axum::Json(serde_json::json!({"ready": true}))
}

async fn api_metrics(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let metrics = state.telemetry.prometheus_export().await;
    (StatusCode::OK, [("content-type", "text/plain; charset=utf-8")], metrics)
}

fn internal_err<E: std::fmt::Display>(e: E) -> (StatusCode, String) { (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()) }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AuthConfig, ChannelsConfig, Config, CronConfig, GatewayConfig, MemoryConfig, NodeConfig, TunnelConfig};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    fn test_cfg() -> Config {
        let workspace_dir = std::env::temp_dir().join(format!("rustclaw-gw-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&workspace_dir).unwrap();
        Config { workspace_dir, auth: AuthConfig::default(), channels: ChannelsConfig::default(), cron: CronConfig::default(), node: NodeConfig::default(), memory: MemoryConfig::default(), tunnel: TunnelConfig::default(), gateway: GatewayConfig::default(), tools: crate::config::ToolsConfig::default() }
    }

    #[tokio::test]
    async fn status_endpoint_works() {
        let app = router(test_cfg());
        let resp = app.oneshot(Request::builder().uri("/api/status").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
