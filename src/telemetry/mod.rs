use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

/// Runtime telemetry collector for metrics and health checks.
#[derive(Clone)]
pub struct Telemetry {
    inner: Arc<TelemetryInner>,
}

struct TelemetryInner {
    started_at: Instant,
    counters: RwLock<HashMap<String, AtomicU64>>,
    histograms: RwLock<HashMap<String, Vec<f64>>>,
    gauges: RwLock<HashMap<String, f64>>,
}

impl Telemetry {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(TelemetryInner {
                started_at: Instant::now(),
                counters: RwLock::new(HashMap::new()),
                histograms: RwLock::new(HashMap::new()),
                gauges: RwLock::new(HashMap::new()),
            }),
        }
    }

    /// Increment a counter by 1.
    pub async fn inc(&self, name: &str) {
        self.inc_by(name, 1).await;
    }

    /// Increment a counter by N.
    pub async fn inc_by(&self, name: &str, n: u64) {
        let counters = self.inner.counters.read().await;
        if let Some(c) = counters.get(name) {
            c.fetch_add(n, Ordering::Relaxed);
            return;
        }
        drop(counters);
        let mut counters = self.inner.counters.write().await;
        counters.entry(name.to_string())
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(n, Ordering::Relaxed);
    }

    /// Record a duration/value in a histogram.
    pub async fn observe(&self, name: &str, value: f64) {
        let mut histograms = self.inner.histograms.write().await;
        histograms.entry(name.to_string())
            .or_default()
            .push(value);
    }

    /// Set a gauge value.
    pub async fn set_gauge(&self, name: &str, value: f64) {
        let mut gauges = self.inner.gauges.write().await;
        gauges.insert(name.to_string(), value);
    }

    /// Return uptime in seconds.
    pub fn uptime_secs(&self) -> u64 {
        self.inner.started_at.elapsed().as_secs()
    }

    /// Export metrics in Prometheus text format.
    pub async fn prometheus_export(&self) -> String {
        let mut out = String::new();

        // Uptime
        out.push_str(&format!("# TYPE rustclaw_uptime_seconds gauge\nrustclaw_uptime_seconds {}\n", self.uptime_secs()));

        // Counters
        let counters = self.inner.counters.read().await;
        for (name, val) in counters.iter() {
            let safe_name = sanitize_metric_name(name);
            out.push_str(&format!("# TYPE {safe_name} counter\n{safe_name} {}\n", val.load(Ordering::Relaxed)));
        }

        // Gauges
        let gauges = self.inner.gauges.read().await;
        for (name, val) in gauges.iter() {
            let safe_name = sanitize_metric_name(name);
            out.push_str(&format!("# TYPE {safe_name} gauge\n{safe_name} {val}\n"));
        }

        // Histograms (summary stats)
        let histograms = self.inner.histograms.read().await;
        for (name, values) in histograms.iter() {
            if values.is_empty() { continue; }
            let safe_name = sanitize_metric_name(name);
            let count = values.len();
            let sum: f64 = values.iter().sum();
            let avg = sum / count as f64;
            let mut sorted = values.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let p50 = sorted[count / 2];
            let p99 = sorted[(count as f64 * 0.99) as usize];
            out.push_str(&format!("# TYPE {safe_name} summary\n"));
            out.push_str(&format!("{safe_name}_count {count}\n"));
            out.push_str(&format!("{safe_name}_sum {sum}\n"));
            out.push_str(&format!("{safe_name}_avg {avg}\n"));
            out.push_str(&format!("{safe_name}{{quantile=\"0.5\"}} {p50}\n"));
            out.push_str(&format!("{safe_name}{{quantile=\"0.99\"}} {p99}\n"));
        }

        out
    }

    /// JSON health check output.
    pub async fn health_json(&self) -> serde_json::Value {
        let counters = self.inner.counters.read().await;
        let mut metrics = serde_json::Map::new();
        for (name, val) in counters.iter() {
            metrics.insert(name.clone(), serde_json::json!(val.load(Ordering::Relaxed)));
        }
        serde_json::json!({
            "status": "healthy",
            "uptime_secs": self.uptime_secs(),
            "metrics": metrics,
        })
    }
}

fn sanitize_metric_name(name: &str) -> String {
    name.chars().map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' }).collect()
}

/// Track request latency via a guard.
pub struct LatencyTracker {
    telemetry: Telemetry,
    metric: String,
    start: Instant,
}

impl LatencyTracker {
    pub fn start(telemetry: &Telemetry, metric: &str) -> Self {
        Self {
            telemetry: telemetry.clone(),
            metric: metric.to_string(),
            start: Instant::now(),
        }
    }

    pub async fn finish(self) {
        let elapsed = self.start.elapsed().as_secs_f64();
        self.telemetry.observe(&self.metric, elapsed).await;
    }
}

/// Bonus: Redact known secret env var values from a string.
/// This can be used as a safety net to prevent token leakage in logs.
pub fn redact_secrets(input: &str) -> String {
    let mut output = input.to_string();
    let secret_env_vars = [
        "ANTHROPIC_API_KEY",
        "OPENAI_API_KEY",
        "TELEGRAM_BOT_TOKEN",
        "SLACK_BOT_TOKEN",
        "SLACK_SIGNING_SECRET",
        "WHATSAPP_ACCESS_TOKEN",
        "DISCORD_BOT_TOKEN",
        "RUSTCLAW_GATEWAY_TOKEN",
        "BRAVE_API_KEY",
    ];
    for var in &secret_env_vars {
        if let Ok(val) = std::env::var(var) {
            if !val.is_empty() && val.len() >= 8 {
                output = output.replace(&val, &format!("[REDACTED:{var}]"));
            }
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn counter_increments() {
        let t = Telemetry::new();
        t.inc("requests_total").await;
        t.inc("requests_total").await;
        let export = t.prometheus_export().await;
        assert!(export.contains("requests_total 2"));
    }

    #[tokio::test]
    async fn gauge_sets() {
        let t = Telemetry::new();
        t.set_gauge("active_sessions", 5.0).await;
        let export = t.prometheus_export().await;
        assert!(export.contains("active_sessions 5"));
    }

    #[tokio::test]
    async fn histogram_stats() {
        let t = Telemetry::new();
        for v in [0.1, 0.2, 0.3, 0.4, 0.5] {
            t.observe("latency_secs", v).await;
        }
        let export = t.prometheus_export().await;
        assert!(export.contains("latency_secs_count 5"));
        assert!(export.contains("quantile=\"0.5\""));
    }

    #[tokio::test]
    async fn health_json_works() {
        let t = Telemetry::new();
        t.inc("test_counter").await;
        let health = t.health_json().await;
        assert_eq!(health["status"], "healthy");
        assert!(health["uptime_secs"].as_u64().is_some());
    }

    #[tokio::test]
    async fn latency_tracker() {
        let t = Telemetry::new();
        let tracker = LatencyTracker::start(&t, "test_latency");
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        tracker.finish().await;
        let export = t.prometheus_export().await;
        assert!(export.contains("test_latency_count 1"));
    }

    // Bonus: Test secret redaction in log output
    #[test]
    fn redact_secrets_works() {
        // SAFETY: test runs sequentially — env mutation is acceptable here
        unsafe { std::env::set_var("ANTHROPIC_API_KEY", "sk-ant-secret-12345678"); }
        let input = "API call failed with key sk-ant-secret-12345678 to endpoint";
        let redacted = redact_secrets(input);
        assert!(!redacted.contains("sk-ant-secret-12345678"), "secret should be redacted");
        assert!(redacted.contains("[REDACTED:ANTHROPIC_API_KEY]"), "should show redaction marker");
        unsafe { std::env::remove_var("ANTHROPIC_API_KEY"); }
    }

    #[test]
    fn redact_secrets_no_env_no_change() {
        // When no env var is set, the string should be unchanged
        let input = "some normal log message";
        let result = redact_secrets(input);
        assert_eq!(result, input);
    }
}
