pub mod cloudflare;
pub mod ngrok;
pub mod tailscale;

use crate::config::TunnelConfig;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::{Arc, Mutex};

#[async_trait]
pub trait Tunnel: Send + Sync {
    fn name(&self) -> &str;
    async fn start(&self, local_host: &str, local_port: u16) -> Result<String>;
    async fn stop(&self) -> Result<()>;
    fn status(&self) -> String;
    fn public_url(&self) -> Option<String>;
}

#[derive(Default)]
struct State {
    running: bool,
    url: Option<String>,
}

pub struct NoneTunnel { state: Arc<Mutex<State>> }
pub struct CloudflareTunnel { state: Arc<Mutex<State>> }
pub struct NgrokTunnel { state: Arc<Mutex<State>> }
pub struct TailscaleTunnel { state: Arc<Mutex<State>> }

impl NoneTunnel { pub fn new() -> Self { Self { state: Arc::new(Mutex::new(State::default())) } } }
impl CloudflareTunnel { pub fn new() -> Self { Self { state: Arc::new(Mutex::new(State::default())) } } }
impl NgrokTunnel { pub fn new() -> Self { Self { state: Arc::new(Mutex::new(State::default())) } } }
impl TailscaleTunnel { pub fn new() -> Self { Self { state: Arc::new(Mutex::new(State::default())) } } }

macro_rules! status_impl {
    ($self:ident) => {{
        let s = $self.state.lock().unwrap();
        if s.running { "running".to_string() } else { "stopped".to_string() }
    }};
}

#[async_trait]
impl Tunnel for NoneTunnel {
    fn name(&self) -> &str { "none" }
    async fn start(&self, local_host: &str, local_port: u16) -> Result<String> {
        let url = format!("http://{local_host}:{local_port}");
        let mut s = self.state.lock().unwrap(); s.running = true; s.url = Some(url.clone());
        Ok(url)
    }
    async fn stop(&self) -> Result<()> { let mut s=self.state.lock().unwrap(); s.running=false; Ok(()) }
    fn status(&self) -> String { status_impl!(self) }
    fn public_url(&self) -> Option<String> { self.state.lock().unwrap().url.clone() }
}

#[async_trait]
impl Tunnel for CloudflareTunnel {
    fn name(&self) -> &str { "cloudflare" }
    async fn start(&self, local_host: &str, local_port: u16) -> Result<String> {
        let url = cloudflare::start_cloudflared(local_host, local_port).await?;
        let mut s = self.state.lock().unwrap(); s.running = true; s.url = Some(url.clone());
        Ok(url)
    }
    async fn stop(&self) -> Result<()> { let mut s=self.state.lock().unwrap(); s.running=false; Ok(()) }
    fn status(&self) -> String { status_impl!(self) }
    fn public_url(&self) -> Option<String> { self.state.lock().unwrap().url.clone() }
}

#[async_trait]
impl Tunnel for NgrokTunnel {
    fn name(&self) -> &str { "ngrok" }
    async fn start(&self, _local_host: &str, local_port: u16) -> Result<String> {
        let url = ngrok::start_ngrok(local_port).await?;
        let mut s = self.state.lock().unwrap(); s.running = true; s.url = Some(url.clone());
        Ok(url)
    }
    async fn stop(&self) -> Result<()> { let mut s=self.state.lock().unwrap(); s.running=false; Ok(()) }
    fn status(&self) -> String { status_impl!(self) }
    fn public_url(&self) -> Option<String> { self.state.lock().unwrap().url.clone() }
}

#[async_trait]
impl Tunnel for TailscaleTunnel {
    fn name(&self) -> &str { "tailscale" }
    async fn start(&self, _local_host: &str, local_port: u16) -> Result<String> {
        let url = tailscale::start_tailscale_funnel(local_port).await?;
        let mut s = self.state.lock().unwrap(); s.running = true; s.url = Some(url.clone());
        Ok(url)
    }
    async fn stop(&self) -> Result<()> { let mut s=self.state.lock().unwrap(); s.running=false; Ok(()) }
    fn status(&self) -> String { status_impl!(self) }
    fn public_url(&self) -> Option<String> { self.state.lock().unwrap().url.clone() }
}

pub fn create_tunnel(cfg: &TunnelConfig) -> Result<Box<dyn Tunnel>> {
    match cfg.provider.as_str() {
        "none" | "" => Ok(Box::new(NoneTunnel::new())),
        "cloudflare" => Ok(Box::new(CloudflareTunnel::new())),
        "ngrok" => Ok(Box::new(NgrokTunnel::new())),
        "tailscale" => Ok(Box::new(TailscaleTunnel::new())),
        other => anyhow::bail!("unsupported tunnel provider: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn none_tunnel_start() {
        let t = NoneTunnel::new();
        let u = t.start("127.0.0.1", 8080).await.unwrap();
        assert_eq!(u, "http://127.0.0.1:8080");
        assert_eq!(t.status(), "running");
    }
    #[test]
    fn factory_supports_new_providers() {
        for p in ["none", "cloudflare", "ngrok", "tailscale"] {
            let cfg = TunnelConfig { provider: p.into(), custom_start_command: None, public_url: None };
            assert!(create_tunnel(&cfg).is_ok());
        }
    }
}
