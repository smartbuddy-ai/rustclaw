use anyhow::Result;
use tokio::process::Command;

pub async fn start_tailscale_funnel(local_port: u16) -> Result<String> {
    let _ = Command::new("tailscale")
        .args(["funnel", &local_port.to_string()])
        .output()
        .await?;
    let out = Command::new("tailscale").args(["funnel", "status", "--json"]).output().await?;
    let text = String::from_utf8_lossy(&out.stdout);
    parse_tailscale_url(&text).ok_or_else(|| anyhow::anyhow!("tailscale funnel URL not found"))
}

pub fn parse_tailscale_url(s: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(s).ok()?;
    v.get("Self")?
        .get("DNSName")?
        .as_str()
        .map(|dns| format!("https://{}", dns.trim_end_matches('.')))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_url() {
        let s = r#"{"Self":{"DNSName":"host.tailnet.ts.net."}}"#;
        assert_eq!(parse_tailscale_url(s).unwrap(), "https://host.tailnet.ts.net");
    }
}
