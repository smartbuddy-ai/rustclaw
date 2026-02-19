# 🦀 Rustclaw

**Lightweight AI agent runtime** — an experimental subset of [OpenClaw](https://github.com/smartbuddy-ai/openclaw) built in Rust.

**Status: v0.1.0 — Production Ready**

Rustclaw is a minimal, fast, and reliable runtime for AI agents. It handles conversation history, workspace context, scheduled tasks, and multi-channel communication with a focus on simplicity and resilience.

## ✨ Features

- **Multi-turn conversations** — Persistent session history across restarts
- **Workspace context** — SOUL.md, USER.md, IDENTITY.md, AGENTS.md, TOOLS.md, MEMORY.md, HEARTBEAT.md
- **Multiple channels** — Telegram (long-polling), WhatsApp (webhook), Slack (Events API)
- **LLM providers** — Anthropic (Claude) and OpenAI with automatic retry + fallback
- **Cron scheduler** — Recurring tasks with heartbeat support
- **Node presence** — Multi-instance discovery and coordination
- **Secure credentials** — API keys in `.env` (mode 0600), never in version control

## 🚀 Quick Start

### Prerequisites

- Rust 1.85+ (edition 2024 support)
- At least one LLM API key (Anthropic or OpenAI)
- Optional: Telegram bot token, WhatsApp Cloud API credentials, or Slack bot token

### Installation

```bash
# Clone the repository
git clone https://github.com/smartbuddy-ai/rustclaw.git
cd rustclaw

# Build release binary
cargo build --release

# Run interactive setup
./target/release/rustclaw init
```

The `init` command walks you through:
1. Creating workspace files (SOUL.md, USER.md, etc.)
2. Configuring LLM API keys
3. Setting up channels (Telegram, WhatsApp, Slack)
4. Validating credentials with real API calls

### Basic Usage

```bash
# Start the gateway (runs all enabled channels + cron)
rustclaw run       # or: rustclaw start

# Send a one-shot chat message
rustclaw chat "What's the weather like today?"

# Check gateway status
rustclaw status

# Manage cron jobs
rustclaw cron list
rustclaw cron add "0 9 * * MON" "Summarize my week ahead"
rustclaw cron test <job-id>  # Manually trigger a job
rustclaw cron remove <job-id>
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
├── lib.rs           # Public library interface
├── config.rs        # TOML configuration
├── auth/            # LLM API clients (Anthropic, OpenAI)
│   ├── anthropic.rs # Anthropic Messages API
│   ├── openai.rs    # OpenAI Chat Completions API
│   └── retry.rs     # Exponential backoff retry logic
├── channels/        # Channel integrations
│   ├── telegram.rs  # Telegram Bot API (long-polling)
│   ├── whatsapp.rs  # WhatsApp Cloud API (webhook)
│   └── slack.rs     # Slack Events API
├── chat/            # Core conversation management
│   ├── mod.rs       # Chat API
│   └── session.rs   # Multi-turn conversation history
├── cron/            # Scheduled job execution & heartbeat
├── nodes/           # Presence beacons & instance discovery
├── setup/           # Interactive init wizard
└── workspace/       # .md file management & system prompt builder
    └── templates/   # Default workspace file templates

tests/
└── integration_test.rs  # End-to-end integration tests
```

### Data Flow

**Telegram Message → Response:**
1. Long-polling loop receives message from Telegram Bot API
2. Check sender against `allow_from` allowlist
3. Load conversation session from `~/.rustclaw/workspace/sessions/{chat_id}.json`
4. Build system prompt from SOUL.md, AGENTS.md, MEMORY.md, etc.
5. Call Anthropic/OpenAI with conversation history + system prompt
6. Save assistant response to session
7. Send reply to Telegram (with Markdown formatting)

**Workspace Context Files:**
- `SOUL.md` — Core identity and personality
- `IDENTITY.md` — Role definition
- `AGENTS.md` — Operational rules and guidelines
- `TOOLS.md` — Local tool notes and configuration
- `MEMORY.md` — Long-term memory (main session only)
- `HEARTBEAT.md` — Proactive task checklist
- `USER.md` — User profile
- `memory/YYYY-MM-DD.md` — Daily notes

All files are loaded and combined into the system prompt for every LLM call.

## Environment Variables

All API keys and secrets are stored in `~/.rustclaw/.env` (mode 0600):

```bash
# LLM Providers (at least one required)
ANTHROPIC_API_KEY=sk-ant-...
OPENAI_API_KEY=sk-...

# Telegram Bot (optional)
TELEGRAM_BOT_TOKEN=123456:ABC-DEF...

# WhatsApp Business API (optional)
WHATSAPP_ACCESS_TOKEN=...
WHATSAPP_VERIFY_TOKEN=...

# Slack Bot (optional)
SLACK_BOT_TOKEN=xoxb-...
SLACK_APP_TOKEN=xapp-...
SLACK_SIGNING_SECRET=...
```

Run `rustclaw init` to set these up interactively with validation.

## Testing

```bash
# Run all tests (unit + integration)
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_workspace_initialization

# Run tests with logging
RUST_LOG=debug cargo test
```

**Test Coverage:**
- ✅ Config loading and serialization
- ✅ Workspace file management
- ✅ Session history persistence
- ✅ System prompt building
- ✅ Retry logic with exponential backoff
- ✅ Cron job configuration

## Development

```bash
# Build debug binary
cargo build

# Build release binary (optimized)
cargo build --release

# Run with debug logging
RUST_LOG=debug cargo run -- run

# Check for issues
cargo clippy

# Format code
cargo fmt
```

## What's Not Included (by design)

- **Streaming responses** — Not needed for chat platforms (Telegram sends full messages)
- **Voice/image support** — Out of scope for lightweight runtime
- **Database** — File-based sessions are sufficient
- **Web UI** — CLI + chat channels only
- **Complex auth** — Allowlists are good enough for personal use

## Contributing

This is an experimental project. If you find bugs or have suggestions:
1. Open an issue on GitHub
2. Submit a PR with tests
3. Follow conventional commits (feat/fix/docs/test)

## License

MIT
