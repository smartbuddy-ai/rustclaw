use crate::config::Config;
use anyhow::Result;
use std::fs;

/// Standard workspace files that make the agent work.
const WORKSPACE_FILES: &[(&str, &str)] = &[
    ("AGENTS.md", include_str!("templates/AGENTS.md")),
    ("SOUL.md", include_str!("templates/SOUL.md")),
    ("USER.md", include_str!("templates/USER.md")),
    ("IDENTITY.md", include_str!("templates/IDENTITY.md")),
    ("TOOLS.md", include_str!("templates/TOOLS.md")),
    ("MEMORY.md", include_str!("templates/MEMORY.md")),
    ("HEARTBEAT.md", include_str!("templates/HEARTBEAT.md")),
];

/// Ensure workspace directory and standard files exist.
pub fn ensure_workspace(cfg: &Config) -> Result<()> {
    let dir = &cfg.workspace_dir;
    fs::create_dir_all(dir)?;
    fs::create_dir_all(dir.join("memory"))?;

    for (name, template) in WORKSPACE_FILES {
        let path = dir.join(name);
        if !path.exists() {
            fs::write(&path, template)?;
            tracing::debug!(file = name, "created workspace file");
        }
    }

    Ok(())
}

/// Initialize workspace (same as ensure, but logs to user).
pub fn init(cfg: &Config) -> Result<()> {
    ensure_workspace(cfg)?;
    tracing::info!(dir = %cfg.workspace_dir.display(), "workspace initialized");
    Ok(())
}

/// Read a workspace file by name.
pub fn read_file(cfg: &Config, name: &str) -> Result<Option<String>> {
    let path = cfg.workspace_dir.join(name);
    if path.exists() {
        Ok(Some(fs::read_to_string(&path)?))
    } else {
        Ok(None)
    }
}

/// Write a workspace file.
pub fn write_file(cfg: &Config, name: &str, content: &str) -> Result<()> {
    let path = cfg.workspace_dir.join(name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, content)?;
    Ok(())
}

/// Build the system prompt from workspace files (SOUL.md + IDENTITY.md + context).
pub fn build_system_prompt(cfg: &Config) -> Result<String> {
    let mut parts = Vec::new();

    if let Some(soul) = read_file(cfg, "SOUL.md")? {
        parts.push(soul);
    }
    if let Some(identity) = read_file(cfg, "IDENTITY.md")? {
        parts.push(identity);
    }
    if let Some(agents) = read_file(cfg, "AGENTS.md")? {
        parts.push(format!("## Workspace Rules\n{agents}"));
    }
    if let Some(tools) = read_file(cfg, "TOOLS.md")? {
        parts.push(format!("## Tools\n{tools}"));
    }
    if let Some(memory) = read_file(cfg, "MEMORY.md")? {
        if !memory.trim().is_empty() {
            parts.push(format!("## Long-term Memory\n{memory}"));
        }
    }

    // Load today's daily memory if it exists
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    if let Some(daily) = read_file(cfg, &format!("memory/{today}.md"))? {
        parts.push(format!("## Today's Notes ({today})\n{daily}"));
    }

    Ok(parts.join("\n\n---\n\n"))
}
