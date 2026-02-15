use crate::config::{self, Config};
use anyhow::Result;
use dialoguer::{Confirm, Input, Password};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Secrets collected during setup, keyed by env var name.
type Secrets = BTreeMap<String, String>;

/// Run the full interactive init flow.
pub async fn run_init(cfg: &Config) -> Result<()> {
    println!("\n🦀 Rustclaw Setup\n");

    let rustclaw_dir = rustclaw_home();
    fs::create_dir_all(&rustclaw_dir)?;

    // 1. Workspace files
    println!("📁 Creating workspace files...");
    crate::workspace::ensure_workspace(cfg)?;
    println!("   ✓ Workspace at {}\n", cfg.workspace_dir.display());

    // 2. Collect secrets interactively
    let mut secrets = Secrets::new();
    let mut channels_cfg = config::ChannelsConfig::default();

    // --- LLM keys ---
    println!("🔑 LLM API Keys\n");

    if let Some(key) = prompt_secret(
        "Anthropic API key",
        "ANTHROPIC_API_KEY",
        "sk-ant-",
    )? {
        secrets.insert("ANTHROPIC_API_KEY".into(), key);
    }

    if let Some(key) = prompt_secret(
        "OpenAI API key",
        "OPENAI_API_KEY",
        "sk-",
    )? {
        secrets.insert("OPENAI_API_KEY".into(), key);
    }

    // --- Channels ---
    println!("\n📡 Channel Setup\n");

    // Telegram
    if Confirm::new()
        .with_prompt("Configure Telegram?")
        .default(true)
        .interact()?
    {
        if let Some(token) = prompt_secret("Telegram bot token", "TELEGRAM_BOT_TOKEN", "")? {
            secrets.insert("TELEGRAM_BOT_TOKEN".into(), token);
        }
        let allow_from: String = Input::new()
            .with_prompt("Allowed Telegram user IDs (comma-separated, empty for all)")
            .default(String::new())
            .interact_text()?;
        let allow_vec: Vec<String> = allow_from
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        channels_cfg.telegram = Some(config::TelegramConfig {
            enabled: true,
            bot_token: None, // loaded from .env
            allow_from: allow_vec,
            webhook_url: None,
        });
    }

    // WhatsApp
    if Confirm::new()
        .with_prompt("Configure WhatsApp?")
        .default(false)
        .interact()?
    {
        if let Some(token) =
            prompt_secret("WhatsApp access token", "WHATSAPP_ACCESS_TOKEN", "")?
        {
            secrets.insert("WHATSAPP_ACCESS_TOKEN".into(), token);
        }
        let phone_id: String = Input::new()
            .with_prompt("WhatsApp phone number ID")
            .default(String::new())
            .interact_text()?;
        let verify_token: String = Input::new()
            .with_prompt("Webhook verify token")
            .default("rustclaw".into())
            .interact_text()?;
        let port: u16 = Input::new()
            .with_prompt("Webhook listen port")
            .default(8090)
            .interact()?;

        channels_cfg.whatsapp = Some(config::WhatsAppConfig {
            enabled: true,
            api_url: None,
            access_token: None, // loaded from .env
            verify_token: Some(verify_token),
            phone_number_id: if phone_id.is_empty() {
                None
            } else {
                Some(phone_id)
            },
            webhook_port: port,
        });
    }

    // Slack
    if Confirm::new()
        .with_prompt("Configure Slack?")
        .default(false)
        .interact()?
    {
        if let Some(token) = prompt_secret("Slack bot token (xoxb-...)", "SLACK_BOT_TOKEN", "")? {
            secrets.insert("SLACK_BOT_TOKEN".into(), token);
        }
        if let Some(token) =
            prompt_secret("Slack app token (xapp-..., for socket mode)", "SLACK_APP_TOKEN", "")?
        {
            secrets.insert("SLACK_APP_TOKEN".into(), token);
        }

        channels_cfg.slack = Some(config::SlackConfig {
            enabled: true,
            bot_token: None, // loaded from .env
            app_token: None,
            signing_secret: None,
            socket_mode: true,
        });
    }

    // 3. Write .env file
    let env_path = rustclaw_dir.join(".env");
    write_env_file(&env_path, &secrets)?;
    println!("\n🔒 Secrets saved to {}", env_path.display());

    // 4. Write config.toml (no secrets!)
    let new_cfg = Config {
        workspace_dir: cfg.workspace_dir.clone(),
        auth: config::AuthConfig {
            default_provider: if secrets.contains_key("ANTHROPIC_API_KEY") {
                "anthropic".into()
            } else if secrets.contains_key("OPENAI_API_KEY") {
                "openai".into()
            } else {
                "anthropic".into()
            },
            default_model: if secrets.contains_key("ANTHROPIC_API_KEY") {
                "claude-sonnet-4-20250514".into()
            } else if secrets.contains_key("OPENAI_API_KEY") {
                "gpt-4o".into()
            } else {
                "claude-sonnet-4-20250514".into()
            },
        },
        channels: channels_cfg,
        cron: config::CronConfig::default(),
        node: config::NodeConfig::default(),
    };

    let config_path = rustclaw_dir.join("config.toml");
    let toml_str = toml::to_string_pretty(&new_cfg)?;
    fs::write(&config_path, &toml_str)?;
    println!("📝 Config saved to {}", config_path.display());

    // 5. Ensure .gitignore
    ensure_gitignore(&rustclaw_dir)?;

    // 6. Validate API keys
    println!("\n🧪 Validating API keys...\n");

    // Reload .env so validation can use it
    let _ = dotenvy::from_path(&env_path);

    if secrets.contains_key("ANTHROPIC_API_KEY") {
        print!("   Anthropic... ");
        match validate_anthropic(secrets.get("ANTHROPIC_API_KEY").unwrap()).await {
            Ok(()) => println!("✓ working"),
            Err(e) => println!("✗ {e}"),
        }
    }

    if secrets.contains_key("OPENAI_API_KEY") {
        print!("   OpenAI... ");
        match validate_openai(secrets.get("OPENAI_API_KEY").unwrap()).await {
            Ok(()) => println!("✓ working"),
            Err(e) => println!("✗ {e}"),
        }
    }

    if secrets.contains_key("TELEGRAM_BOT_TOKEN") {
        print!("   Telegram... ");
        match validate_telegram(secrets.get("TELEGRAM_BOT_TOKEN").unwrap()).await {
            Ok(name) => println!("✓ @{name}"),
            Err(e) => println!("✗ {e}"),
        }
    }

    println!("\n✅ Rustclaw initialized! Run `rustclaw start` to launch the gateway.\n");

    Ok(())
}

/// Prompt for an optional secret. Returns None if user skips.
fn prompt_secret(label: &str, env_var: &str, _prefix_hint: &str) -> Result<Option<String>> {
    // Check if already set in environment
    if let Ok(existing) = std::env::var(env_var) {
        if !existing.is_empty() {
            println!("   {label}: found in ${env_var}");
            if Confirm::new()
                .with_prompt(format!("   Use existing ${env_var}?"))
                .default(true)
                .interact()?
            {
                return Ok(Some(existing));
            }
        }
    }

    if !Confirm::new()
        .with_prompt(format!("   Set up {label}?"))
        .default(true)
        .interact()?
    {
        return Ok(None);
    }

    let value = Password::new()
        .with_prompt(format!("   {label}"))
        .interact()?;

    if value.trim().is_empty() {
        return Ok(None);
    }

    Ok(Some(value.trim().to_string()))
}

/// Write secrets to .env file, preserving any existing entries not being overwritten.
fn write_env_file(path: &Path, secrets: &Secrets) -> Result<()> {
    let mut existing = BTreeMap::new();

    // Read existing .env if present
    if path.exists() {
        let content = fs::read_to_string(path)?;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, val)) = line.split_once('=') {
                existing.insert(key.trim().to_string(), val.trim().to_string());
            }
        }
    }

    // Merge new secrets (overwrite existing)
    for (k, v) in secrets {
        existing.insert(k.clone(), v.clone());
    }

    // Write out
    let mut content = String::from("# Rustclaw secrets — DO NOT COMMIT\n\n");
    for (k, v) in &existing {
        content.push_str(&format!("{k}={v}\n"));
    }

    fs::write(path, content)?;

    // Set restrictive permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }

    Ok(())
}

/// Ensure .gitignore excludes .env in the rustclaw home dir.
fn ensure_gitignore(dir: &Path) -> Result<()> {
    let gitignore_path = dir.join(".gitignore");
    let required_entries = [".env", "state/", "*.log"];

    let mut content = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path)?
    } else {
        String::new()
    };

    let mut changed = false;
    for entry in required_entries {
        if !content.lines().any(|l| l.trim() == entry) {
            if !content.is_empty() && !content.ends_with('\n') {
                content.push('\n');
            }
            content.push_str(entry);
            content.push('\n');
            changed = true;
        }
    }

    if changed {
        fs::write(&gitignore_path, &content)?;
        println!("📄 .gitignore updated at {}", gitignore_path.display());
    }

    Ok(())
}

fn rustclaw_home() -> PathBuf {
    directories::BaseDirs::new()
        .map(|d| d.home_dir().join(".rustclaw"))
        .unwrap_or_else(|| PathBuf::from(".rustclaw"))
}

/// Validate Anthropic API key with a minimal request.
async fn validate_anthropic(api_key: &str) -> Result<()> {
    let client = reqwest::Client::new();
    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&serde_json::json!({
            "model": "claude-sonnet-4-20250514",
            "max_tokens": 1,
            "messages": [{"role": "user", "content": "hi"}]
        }))
        .send()
        .await?;

    if resp.status().is_success() || resp.status().as_u16() == 200 {
        Ok(())
    } else if resp.status().as_u16() == 401 {
        anyhow::bail!("invalid API key")
    } else {
        // 400/429 etc still means the key authenticated
        Ok(())
    }
}

/// Validate OpenAI API key.
async fn validate_openai(api_key: &str) -> Result<()> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.openai.com/v1/models")
        .bearer_auth(api_key)
        .send()
        .await?;

    if resp.status().as_u16() == 401 {
        anyhow::bail!("invalid API key")
    } else {
        Ok(())
    }
}

/// Validate Telegram bot token, returns bot username.
async fn validate_telegram(token: &str) -> Result<String> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("https://api.telegram.org/bot{token}/getMe"))
        .send()
        .await?;

    let data: serde_json::Value = resp.json().await?;
    if data["ok"].as_bool() == Some(true) {
        Ok(data["result"]["username"]
            .as_str()
            .unwrap_or("unknown")
            .to_string())
    } else {
        anyhow::bail!(
            "{}",
            data["description"].as_str().unwrap_or("invalid token")
        )
    }
}
