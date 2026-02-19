use crate::config::{Config, SignalConfig};
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Clone)]
struct SignalEnvelope {
    source: String,
    timestamp: u64,
    message: Option<String>,
}

#[derive(Debug, Serialize)]
struct SendSignalBody<'a> {
    number: &'a str,
    recipients: Vec<&'a str>,
    message: &'a str,
}

async fn poll_messages(client: &reqwest::Client, api_url: &str) -> Result<Vec<SignalEnvelope>> {
    let resp = client
        .get(format!("{api_url}/v1/receive"))
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("signal receive failed: {}", resp.status());
    }

    Ok(resp.json::<Vec<SignalEnvelope>>().await.unwrap_or_default())
}

async fn send_message(
    client: &reqwest::Client,
    api_url: &str,
    from: &str,
    to: &str,
    message: &str,
) -> Result<()> {
    let body = SendSignalBody {
        number: from,
        recipients: vec![to],
        message,
    };

    let resp = client
        .post(format!("{api_url}/v2/send"))
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("signal send failed: {}", resp.status());
    }

    Ok(())
}

pub async fn run(cfg: &Config, sg: &SignalConfig) -> Result<()> {
    let api_url = sg
        .api_url
        .clone()
        .or_else(|| std::env::var("SIGNAL_API_URL").ok())
        .unwrap_or_else(|| "http://127.0.0.1:8080".to_string());

    let number = sg
        .number
        .clone()
        .or_else(|| std::env::var("SIGNAL_NUMBER").ok())
        .ok_or_else(|| anyhow::anyhow!("SIGNAL number not set"))?;

    let client = reqwest::Client::new();
    let mut last_ts: u64 = 0;

    tracing::info!(channel = "signal", api_url = %api_url, "starting polling loop");
    loop {
        let messages = match poll_messages(&client, &api_url).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "signal poll error");
                tokio::time::sleep(std::time::Duration::from_secs(sg.poll_interval_secs)).await;
                continue;
            }
        };

        for env in messages {
            if env.timestamp <= last_ts {
                continue;
            }
            last_ts = env.timestamp;

            let text = match env.message.clone() {
                Some(t) if !t.trim().is_empty() => t,
                _ => continue,
            };

            let session_id = format!("signal:{}", env.source);
            let reply = match crate::chat::send_with_session(cfg, &session_id, &text).await {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "signal LLM error");
                    "⚠️ Error processing your message.".into()
                }
            };

            if let Err(e) = send_message(&client, &api_url, &number, &env.source, &reply).await {
                tracing::error!(error = %e, "signal send failed");
            }
        }

        tokio::time::sleep(std::time::Duration::from_secs(sg.poll_interval_secs)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_send_body_serializes() {
        let body = SendSignalBody {
            number: "+33123456789",
            recipients: vec!["+33999999999"],
            message: "hi",
        };
        let value = serde_json::to_value(&body).unwrap();
        assert_eq!(value["message"], "hi");
    }
}
