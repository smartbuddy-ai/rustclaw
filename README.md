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

# Interactive setup — creates workspace, config, and .env
rustclaw init

# Start the gateway
rustclaw start

# One-shot chat
rustclaw chat "Hello, what can you do?"

# Cron jobs
rustclaw cron add "0 9 * * MON" "Check my calendar for the week"
rustclaw cron list
rustclaw cron remove <id>
```

## Setup Flow (`rustclaw init`)

The init command walks you through:

1. **Workspace files** — creates SOUL.md, USER.md, IDENTITY.md, AGENTS.md, TOOLS.md, MEMORY.md, HEARTBEAT.md
2. **LLM API keys** — prompts for Anthropic and/or OpenAI keys
3. **Channel config** — Telegram bot token, WhatsApp webhook, Slack bot token
4. **Secure storage** — credentials saved to `~/.rustclaw/.env` (mode 0600), never in config.toml
5. **Validation** — tests each API key with a real API call

## Configuration

- **Config:** `~/.rustclaw/config.toml` — non-secret settings (model, channels, cron)
- **Secrets:** `~/.rustclaw/.env` — API keys loaded at runtime via dotenvy
- **Workspace:** `~/.rustclaw/workspace/` — agent .md files and memory

Secrets are **never** stored in config.toml. They live in `.env` with 0600 permissions.

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
