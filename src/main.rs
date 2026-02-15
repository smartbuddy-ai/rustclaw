mod auth;
mod channels;
mod chat;
mod config;
mod cron;
mod nodes;
mod setup;
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
    }

    Ok(())
}

async fn gateway_start(cfg: config::Config) -> anyhow::Result<()> {
    use tokio::signal;

    // Boot workspace files
    workspace::ensure_workspace(&cfg)?;

    // Start channel listeners
    let mut handles = Vec::new();

    if let Some(ref tg) = cfg.channels.telegram {
        if tg.enabled {
            let tg = tg.clone();
            let cfg2 = cfg.clone();
            handles.push(tokio::spawn(async move {
                if let Err(e) = channels::telegram::run(&cfg2, &tg).await {
                    tracing::error!(channel = "telegram", error = %e, "channel exited");
                }
            }));
        }
    }

    if let Some(ref wa) = cfg.channels.whatsapp {
        if wa.enabled {
            let wa = wa.clone();
            let cfg2 = cfg.clone();
            handles.push(tokio::spawn(async move {
                if let Err(e) = channels::whatsapp::run(&cfg2, &wa).await {
                    tracing::error!(channel = "whatsapp", error = %e, "channel exited");
                }
            }));
        }
    }

    if let Some(ref sl) = cfg.channels.slack {
        if sl.enabled {
            let sl = sl.clone();
            let cfg2 = cfg.clone();
            handles.push(tokio::spawn(async move {
                if let Err(e) = channels::slack::run(&cfg2, &sl).await {
                    tracing::error!(channel = "slack", error = %e, "channel exited");
                }
            }));
        }
    }

    // Start cron scheduler
    let scheduler = cron::start_scheduler(&cfg).await?;

    // Start node presence beacon
    let beacon = nodes::start_beacon(&cfg).await;

    tracing::info!("rustclaw gateway running — press Ctrl+C to stop");
    signal::ctrl_c().await?;
    tracing::info!("shutting down");

    drop(scheduler);
    drop(beacon);
    for h in handles {
        h.abort();
    }

    Ok(())
}
