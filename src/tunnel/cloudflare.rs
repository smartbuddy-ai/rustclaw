use anyhow::Result;
use tokio::process::Command;

pub async fn start_cloudflared(local_host: &str, local_port: u16) -> Result<String> {
    let target = format!("http://{local_host}:{local_port}");
    let out = Command::new("sh")
        .arg("-lc")
        .arg(format!("cloudflared tunnel --url {target} 2>&1 | head -n 30"))
        .output()
        .await?;
    let log = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    parse_cloudflared_url(&log).ok_or_else(|| anyhow::anyhow!("cloudflared URL not found"))
}

pub fn parse_cloudflared_url(log: &str) -> Option<String> {
    log.split_whitespace()
        .find(|s| s.contains("trycloudflare.com"))
        .map(|s| s.trim_matches(|c| c == '"' || c == '\'').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_url() {
        let log = "INF | https://abc.trycloudflare.com | connected";
        assert_eq!(parse_cloudflared_url(log).unwrap(), "https://abc.trycloudflare.com");
    }
}
