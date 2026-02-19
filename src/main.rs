mod agent;
mod auth;
mod channels;
mod chat;
mod config;
mod cron;
mod gateway;
mod guardd;
mod heartbeat;
mod memory;
mod nodes;
mod providers;
mod sessions;
mod setup;
mod skills;
mod streaming;
mod telemetry;
mod tui;
mod tunnel;
mod workspace;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Parser)]
#[command(name = "rustclaw", version, about = "Lightweight AI agent runtime")]
struct Cli {
    /// Config file path (default: ~/.rustclaw/config.toml)
    #[arg(short, long)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the gateway daemon
    Start,
    /// Start the gateway daemon (alias for start)
    Run,
    /// Show status
    Status,
    /// Send a chat message
    Chat {
        /// Message text
        message: String,
        /// Channel to use (e.g. telegram, slack, whatsapp)
        #[arg(short = 'C', long)]
        channel: Option<String>,
    },
    /// Manage cron jobs
    Cron {
        #[command(subcommand)]
        action: CronCommands,
    },
    /// Initialize workspace files
    Init,
    /// Launch the interactive TUI dashboard
    Tui,
    /// Validate configuration and check system health
    Doctor,
}

#[derive(Subcommand)]
enum CronCommands {
    /// List scheduled jobs
    List,
    /// Add a new cron job
    Add {
        /// Cron expression (e.g. "0 9 * * MON")
        schedule: String,
        /// Task description / prompt
        task: String,
    },
    /// Remove a cron job by ID
    Remove { id: String },
    /// Test a cron job by manually executing it
    Test { id: String },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .init();

    // Load .env from ~/.rustclaw/.env (secrets live here, not in config.toml)
    let env_path = directories::BaseDirs::new()
        .map(|d| d.home_dir().join(".rustclaw/.env"))
        .unwrap_or_else(|| std::path::PathBuf::from(".rustclaw/.env"));
    let _ = dotenvy::from_path(&env_path);

    let cli = Cli::parse();
    let cfg = config::load_config(cli.config.as_deref())?;

    match cli.command {
        Commands::Start | Commands::Run => {
            tracing::info!("starting rustclaw gateway");
            gateway_start(cfg).await?;
        }
        Commands::Status => {
            let status = nodes::status(&cfg).await;
            println!("{}", serde_json::to_string_pretty(&status)?);
        }
        Commands::Chat { message, channel } => {
            let response = chat::send(&cfg, &message, channel.as_deref()).await?;
            println!("{response}");
        }
        Commands::Cron { action } => match action {
            CronCommands::List => {
                let jobs = cron::list(&cfg)?;
                for job in &jobs {
                    println!("{}\t{}\t{}", job.id, job.schedule, job.task);
                }
                if jobs.is_empty() {
                    println!("No cron jobs configured.");
                }
            }
            CronCommands::Add { schedule, task } => {
                let id = cron::add(&cfg, &schedule, &task)?;
                println!("Added cron job: {id}");
            }
            CronCommands::Remove { id } => {
                cron::remove(&cfg, &id)?;
                println!("Removed cron job: {id}");
            }
            CronCommands::Test { id } => {
                println!("Executing cron job: {id}");
                let response = cron::execute_job(&cfg, &id).await?;
                println!("\n--- Response ---\n{response}");
            }
        },
        Commands::Init => {
            setup::run_init(&cfg).await?;
        }
        Commands::Tui => {
            tui::run_tui()?;
        }
        Commands::Doctor => {
            doctor_check(&cfg).await?;
        }
    }

    Ok(())
}

async fn gateway_start(cfg: config::Config) -> anyhow::Result<()> {
    use tokio::signal;
    use tokio_util::sync::CancellationToken;

    // Boot workspace files
    workspace::ensure_workspace(&cfg)?;

    // Start tunnel if configured (Tailscale/Cloudflare/ngrok)
    let tunnel = tunnel::create_tunnel(&cfg.tunnel)?;
    let tunnel_url = tunnel.start("127.0.0.1", cfg.gateway.port).await;
    match &tunnel_url {
        Ok(url) => tracing::info!(tunnel = tunnel.name(), url = %url, "tunnel started"),
        Err(e) if cfg.tunnel.provider == "none" || cfg.tunnel.provider.is_empty() => {
            tracing::debug!("no tunnel configured: {e}");
        }
        Err(e) => tracing::warn!(error = %e, "tunnel start failed"),
    }

    // Initialize SQLite session persistence
    let session_db_path = cfg.workspace_dir.join("sessions.db");
    match sessions::SessionStore::open(&session_db_path) {
        Ok(_) => tracing::info!("session persistence initialized at {}", session_db_path.display()),
        Err(e) => tracing::warn!(error = %e, "session persistence unavailable"),
    }

    // Scan and register skills
    let skill_registry = skills::SkillRegistry::scan(&cfg.workspace_dir).unwrap_or_default();
    let skill_count = skill_registry.list().len();
    if skill_count > 0 {
        tracing::info!(skills = skill_count, "skills loaded");
    }

    // Start enabled channels via channel router
    let handles = channels::start_enabled_channels(&cfg);

    // Start cron scheduler
    let scheduler = cron::start_scheduler(&cfg).await?;

    // BUG-02: Use CancellationToken for graceful shutdown instead of abort()
    let cancel = CancellationToken::new();
    let gateway_cfg = cfg.clone();
    let gateway_cancel = cancel.clone();
    let gateway_handle = tokio::spawn(async move {
        if let Err(e) = gateway::run_with_shutdown(gateway_cfg, gateway_cancel).await {
            tracing::error!(error = %e, "gateway server exited");
        }
    });

    // Start node presence beacon
    let beacon = nodes::start_beacon(&cfg).await;

    tracing::info!("rustclaw gateway running — press Ctrl+C to stop");
    signal::ctrl_c().await?;
    tracing::info!("graceful shutdown initiated");

    // Graceful shutdown: drain in-flight work
    tracing::info!("stopping cron scheduler");
    drop(scheduler);
    tracing::info!("stopping node beacon");
    drop(beacon);

    // Signal gateway to stop accepting new connections and drain in-flight requests
    tracing::info!("stopping gateway (graceful drain)");
    cancel.cancel();

    // Wait up to 30s for gateway to finish draining
    let drain_timeout = std::time::Duration::from_secs(30);
    if tokio::time::timeout(drain_timeout, gateway_handle).await.is_err() {
        tracing::warn!("gateway drain timed out after 30s, forcing shutdown");
    }

    tracing::info!("stopping channel handlers");
    for h in handles {
        h.abort();
    }
    // Flush audit logs
    tracing::info!("shutdown complete");

    Ok(())
}

async fn doctor_check(cfg: &config::Config) -> anyhow::Result<()> {
    println!("🩺 Rustclaw Doctor");
    println!("==================\n");

    // Config check
    print!("Config file ............ ");
    println!("✅ loaded");

    // Workspace
    print!("Workspace .............. ");
    if cfg.workspace_dir.exists() {
        println!("✅ {}", cfg.workspace_dir.display());
    } else {
        println!("⚠️  missing (run `rustclaw init`)");
    }

    // Memory DB
    print!("Memory DB .............. ");
    match memory::SqliteMemory::from_config(cfg) {
        Ok(mem) => {
            let count = mem.search("", 10_000).map(|r| r.len()).unwrap_or(0);
            println!("✅ {} entries", count);
        }
        Err(e) => println!("❌ {e}"),
    }

    // Channels
    println!("\nChannels:");
    if let Some(tg) = &cfg.channels.telegram {
        print!("  Telegram ............. ");
        if tg.bot_token.is_some() || std::env::var("TELEGRAM_BOT_TOKEN").is_ok() {
            println!("✅ configured");
        } else {
            println!("⚠️  no bot token");
        }
    }
    if let Some(wa) = &cfg.channels.whatsapp {
        print!("  WhatsApp ............. ");
        if wa.access_token.is_some() || std::env::var("WHATSAPP_ACCESS_TOKEN").is_ok() {
            println!("✅ configured");
        } else {
            println!("⚠️  no access token");
        }
    }
    if let Some(sl) = &cfg.channels.slack {
        print!("  Slack ................ ");
        if sl.bot_token.is_some() || std::env::var("SLACK_BOT_TOKEN").is_ok() {
            println!("✅ configured");
        } else {
            println!("⚠️  no bot token");
        }
        if sl.signing_secret.is_none() && std::env::var("SLACK_SIGNING_SECRET").is_err() {
            println!("  ⚠️  No signing secret — webhook auth disabled!");
        }
    }

    // Gateway
    println!("\nGateway:");
    print!("  Auth mode ............ ");
    println!("{}", if cfg.gateway.auth.mode == "none" { "⚠️  none (set to 'token' for production)" } else { "✅ token" });
    print!("  Rate limiting ........ ");
    println!("{}", if cfg.gateway.rate_limit.enabled { "✅ enabled" } else { "⚠️  disabled" });

    // Provider
    println!("\nProviders:");
    print!("  Default .............. ");
    println!("{} / {}", cfg.auth.default_provider, cfg.auth.default_model);

    println!("\n✅ Doctor check complete.");
    Ok(())
}
