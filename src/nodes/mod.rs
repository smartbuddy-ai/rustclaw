use crate::config::Config;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::fs;

fn gethostname() -> OsString {
    #[cfg(unix)]
    {
        let mut buf = vec![0u8; 256];
        unsafe {
            libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len());
        }
        let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        OsString::from(String::from_utf8_lossy(&buf[..len]).to_string())
    }
    #[cfg(not(unix))]
    {
        OsString::from("unknown")
    }
}

/// Node presence beacon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeBeacon {
    pub node_id: String,
    pub name: String,
    pub started_at: chrono::DateTime<Utc>,
    pub last_heartbeat: chrono::DateTime<Utc>,
    pub hostname: String,
    pub pid: u32,
    pub channels: Vec<String>,
}

/// Get current node status.
pub async fn status(cfg: &Config) -> serde_json::Value {
    let state_dir = &cfg.node.state_dir;
    let beacon_file = state_dir.join("beacon.json");

    let beacon = if beacon_file.exists() {
        fs::read_to_string(&beacon_file)
            .ok()
            .and_then(|s| serde_json::from_str::<NodeBeacon>(&s).ok())
    } else {
        None
    };

    // Scan for other node beacons
    let nodes_dir = state_dir.join("nodes");
    let mut connected_nodes = Vec::new();
    if nodes_dir.exists() {
        if let Ok(entries) = fs::read_dir(&nodes_dir) {
            for entry in entries.flatten() {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    if let Ok(node) = serde_json::from_str::<NodeBeacon>(&content) {
                        // Consider alive if heartbeat < 60s ago
                        let age = Utc::now() - node.last_heartbeat;
                        if age.num_seconds() < 60 {
                            connected_nodes.push(node);
                        }
                    }
                }
            }
        }
    }

    serde_json::json!({
        "self": beacon,
        "connected_nodes": connected_nodes,
        "gateway_running": beacon.is_some(),
    })
}

/// Start the presence beacon — writes a heartbeat file periodically.
pub async fn start_beacon(cfg: &Config) -> tokio::task::JoinHandle<()> {
    let state_dir = cfg.node.state_dir.clone();
    let name = cfg.node.name.clone().unwrap_or_else(|| {
        gethostname().to_string_lossy().to_string()
    });
    let interval = cfg.node.beacon_interval_secs;
    let node_id = uuid::Uuid::new_v4().to_string();

    // Determine active channels
    let mut channels = Vec::new();
    if cfg.channels.telegram.as_ref().is_some_and(|t| t.enabled) {
        channels.push("telegram".into());
    }
    if cfg.channels.whatsapp.as_ref().is_some_and(|w| w.enabled) {
        channels.push("whatsapp".into());
    }
    if cfg.channels.slack.as_ref().is_some_and(|s| s.enabled) {
        channels.push("slack".into());
    }

    let started_at = Utc::now();

    tokio::spawn(async move {
        loop {
            let beacon = NodeBeacon {
                node_id: node_id.clone(),
                name: name.clone(),
                started_at,
                last_heartbeat: Utc::now(),
                hostname: gethostname().to_string_lossy().to_string(),
                pid: std::process::id(),
                channels: channels.clone(),
            };

            if let Err(e) = write_beacon(&state_dir, &beacon) {
                tracing::warn!(error = %e, "failed to write beacon");
            }

            tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
        }
    })
}

fn write_beacon(state_dir: &std::path::Path, beacon: &NodeBeacon) -> anyhow::Result<()> {
    fs::create_dir_all(state_dir)?;
    let path = state_dir.join("beacon.json");
    let json = serde_json::to_string_pretty(beacon)?;
    fs::write(path, json)?;
    Ok(())
}
