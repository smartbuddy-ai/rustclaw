use anyhow::{Context, Result};
use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::Sha256;

/// Verify Slack request signature with replay protection.
pub fn verify_slack_signature(
    signing_secret: &str,
    timestamp: i64,
    body: &str,
    signature: &str,
) -> Result<bool> {
    if is_stale(timestamp)? {
        return Ok(false);
    }
    let base = format!("v0:{timestamp}:{body}");
    let expected = format!("v0={}", hmac_hex(signing_secret.as_bytes(), base.as_bytes())?);
    Ok(timing_safe_eq(signature.as_bytes(), expected.as_bytes()))
}

/// Verify WhatsApp request signature with replay protection.
pub fn verify_whatsapp_signature(
    app_secret: &str,
    timestamp: i64,
    body: &str,
    signature: &str,
) -> Result<bool> {
    if is_stale(timestamp)? {
        return Ok(false);
    }
    let expected = format!(
        "sha256={}",
        hmac_hex(app_secret.as_bytes(), body.as_bytes())?
    );
    Ok(timing_safe_eq(signature.as_bytes(), expected.as_bytes()))
}

fn is_stale(timestamp: i64) -> Result<bool> {
    let now = Utc::now().timestamp();
    let age = (now - timestamp).abs();
    Ok(age > 300)
}

fn hmac_hex(key: &[u8], message: &[u8]) -> Result<String> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).context("invalid hmac key")?;
    mac.update(message);
    let bytes = mac.finalize().into_bytes();
    Ok(hex_encode(&bytes))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// Verify Telegram webhook secret token header.
/// Telegram sends `X-Telegram-Bot-Api-Secret-Token` header matching the
/// secret_token set via setWebhook.
pub fn verify_telegram_secret(expected_secret: &str, provided_secret: &str) -> bool {
    timing_safe_eq(expected_secret.as_bytes(), provided_secret.as_bytes())
}

fn timing_safe_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slack_signature_roundtrip() {
        let timestamp = Utc::now().timestamp();
        let body = "payload";
        let secret = "secret";
        let base = format!("v0:{timestamp}:{body}");
        let signature = format!(
            "v0={}",
            hmac_hex(secret.as_bytes(), base.as_bytes()).expect("hmac")
        );
        let ok = verify_slack_signature(secret, timestamp, body, &signature).expect("verify");
        assert!(ok);
    }

    #[test]
    fn slack_rejects_stale() {
        let timestamp = Utc::now().timestamp() - 600;
        let ok = verify_slack_signature("secret", timestamp, "body", "v0=abc")
            .expect("verify");
        assert!(!ok);
    }

    #[test]
    fn whatsapp_signature_roundtrip() {
        let timestamp = Utc::now().timestamp();
        let body = "hello";
        let secret = "app-secret";
        let signature = format!(
            "sha256={}",
            hmac_hex(secret.as_bytes(), body.as_bytes()).expect("hmac")
        );
        let ok = verify_whatsapp_signature(secret, timestamp, body, &signature).expect("verify");
        assert!(ok);
    }

    #[test]
    fn telegram_secret_valid() {
        assert!(verify_telegram_secret("my-secret-token", "my-secret-token"));
    }

    #[test]
    fn telegram_secret_invalid() {
        assert!(!verify_telegram_secret("my-secret-token", "wrong-token"));
    }

    #[test]
    fn whatsapp_rejects_invalid() {
        let timestamp = Utc::now().timestamp();
        let ok = verify_whatsapp_signature("secret", timestamp, "body", "sha256=bad")
            .expect("verify");
        assert!(!ok);
    }
}
