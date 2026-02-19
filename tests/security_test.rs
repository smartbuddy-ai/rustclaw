use rustclaw::guardd::channel_auth::{verify_slack_signature, verify_whatsapp_signature, verify_telegram_secret};

#[test]
fn slack_signature_valid_roundtrip() {
    let secret = "test_secret_123";
    let timestamp = chrono::Utc::now().timestamp();
    let body = r#"{"type":"event_callback","event":{"text":"hello"}}"#;
    let base = format!("v0:{timestamp}:{body}");
    let expected = format!("v0={}", hmac_hex(secret, &base));
    assert!(verify_slack_signature(secret, timestamp, body, &expected).unwrap());
}

#[test]
fn slack_signature_rejects_tampered_body() {
    let secret = "test_secret";
    let timestamp = chrono::Utc::now().timestamp();
    let body = "original";
    let base = format!("v0:{timestamp}:{body}");
    let sig = format!("v0={}", hmac_hex(secret, &base));
    assert!(!verify_slack_signature(secret, timestamp, "tampered", &sig).unwrap());
}

#[test]
fn slack_signature_rejects_replay() {
    let old_timestamp = chrono::Utc::now().timestamp() - 600;
    assert!(!verify_slack_signature("secret", old_timestamp, "body", "v0=abc").unwrap());
}

#[test]
fn whatsapp_signature_valid() {
    let secret = "app_secret";
    let timestamp = chrono::Utc::now().timestamp();
    let body = r#"{"entry":[{"changes":[]}]}"#;
    let sig = format!("sha256={}", hmac_hex(secret, body));
    assert!(verify_whatsapp_signature(secret, timestamp, body, &sig).unwrap());
}

#[test]
fn whatsapp_signature_rejects_wrong_secret() {
    let timestamp = chrono::Utc::now().timestamp();
    let body = "test";
    let sig = format!("sha256={}", hmac_hex("wrong", body));
    assert!(!verify_whatsapp_signature("correct", timestamp, body, &sig).unwrap());
}

#[test]
fn telegram_secret_matches() {
    assert!(verify_telegram_secret("my-webhook-secret", "my-webhook-secret"));
}

#[test]
fn telegram_secret_rejects_mismatch() {
    assert!(!verify_telegram_secret("correct-secret", "wrong-secret"));
}

#[test]
fn telegram_secret_rejects_empty() {
    assert!(!verify_telegram_secret("secret", ""));
}

fn hmac_hex(key: &str, message: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let mut mac = Hmac::<Sha256>::new_from_slice(key.as_bytes()).unwrap();
    mac.update(message.as_bytes());
    let bytes = mac.finalize().into_bytes();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
