# 🦀 Rustclaw

Experimental subset of [OpenClaw](https://github.com/smartbuddy-ai/openclaw) — a lightweight AI agent runtime built in Rust.

## Features

- **Workspace files** — SOUL.md, USER.md, IDENTITY.md, AGENTS.md, TOOLS.md, MEMORY.md, HEARTBEAT.md
- **Channels** — Telegram (polling), WhatsApp (Cloud API webhook), Slack (Events API)
- **LLM Auth** — Anthropic (Claude) and OpenAI API support
- **Chat** — Core conversation with system prompt built from workspace files
- **Cron** — Scheduled jobs with cron expressions, auto-executing via LLM
- **Nodes** — Presence beacons, connected instance discovery

## Quick Start

```bash
# Build
cargo build --release

# Initialize workspace
rustclaw init

# Configure
cat > ~/.rustclaw/config.toml << 'EOF'
workspace_dir = "~/.rustclaw/workspace"

[auth]
default_provider = "anthropic"
default_model = "claude-sonnet-4-20250514"
# anthropic_api_key = "sk-ant-..." # or set ANTHROPIC_API_KEY env var

[channels.telegram]
enabled = true
# bot_token = "123:ABC" # or set TELEGRAM_BOT_TOKEN env var
allow_from = ["YOUR_USER_ID"]
EOF

# Start the gateway
rustclaw start

# One-shot chat
rustclaw chat "Hello, what can you do?"

# Cron jobs
rustclaw cron add "0 9 * * MON" "Check my calendar for the week"
rustclaw cron list
rustclaw cron remove <id>
```

## Configuration

Config lives at `~/.rustclaw/config.toml`. Secrets can reference env vars:

```toml
[auth]
anthropic_api_key = "env:ANTHROPIC_API_KEY"
openai_api_key = "env:OPENAI_API_KEY"
```

## Architecture

```
src/
├── main.rs          # CLI entry point & gateway orchestration
├── config.rs        # TOML configuration
├── auth/            # LLM API clients (Anthropic, OpenAI)
├── channels/        # Channel integrations
│   ├── telegram.rs  # Telegram Bot API (long-polling)
│   ├── whatsapp.rs  # WhatsApp Cloud API (webhook)
│   └── slack.rs     # Slack Events API
├── chat/            # Core conversation management
├── cron/            # Scheduled job execution
├── nodes/           # Presence beacons & instance discovery
└── workspace/       # .md file management & system prompt builder
    └── templates/   # Default workspace file templates
```

## License

MIT
