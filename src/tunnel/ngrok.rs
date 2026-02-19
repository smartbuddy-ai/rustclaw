use anyhow::Result;
use tokio::process::Command;

pub async fn start_ngrok(local_port: u16) -> Result<String> {
    let _ = Command::new("sh")
        .arg("-lc")
        .arg(format!("ngrok http {local_port} >/tmp/rustclaw-ngrok.log 2>&1 &"))
        .status()
        .await?;
    let out = Command::new("sh")
        .arg("-lc")
        .arg("curl -s http://127.0.0.1:4040/api/tunnels")
        .output()
        .await?;
    let text = String::from_utf8_lossy(&out.stdout);
    parse_ngrok_url(&text).ok_or_else(|| anyhow::anyhow!("ngrok URL not found"))
}

pub fn parse_ngrok_url(s: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(s).ok()?;
    v.get("tunnels")?
        .as_array()?
        .iter()
        .find_map(|t| t.get("public_url").and_then(|u| u.as_str()))
        .map(|x| x.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_url() {
        let s = r#"{"tunnels":[{"public_url":"https://x.ngrok-free.app"}]}"#;
        assert_eq!(parse_ngrok_url(s).unwrap(), "https://x.ngrok-free.app");
    }
}
