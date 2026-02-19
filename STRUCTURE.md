# 🦀 Rustclaw — Project Structure

> AI agent runtime in Rust — secure replacement for OpenClaw
> Status: ~40% complete | 85 tests pass | Compiles clean

```
rustclaw/                              (3.3 GB — target/ is 3.3 GB, nettoyable)
├── Cargo.toml                         ← Dependencies + project metadata (Rust 2024 edition)
├── Cargo.lock                         ← Locked dependency versions
├── README.md                          ← Full documentation (setup, usage, architecture)
├── BUILD_PLAN.md                      ← 7 milestones roadmap
├── AUDIT_REPORT.md                    ← Full audit (2026-02-17)
│
├── 📈 Kaizen Files
│   ├── LEARNINGS.md                   ← Lessons learned
│   ├── DECISIONS.md                   ← Architecture decisions + rationale
│   ├── STOP_DOING.md                  ← Anti-patterns
│   ├── KAIZEN_STATE.json              ← Current improvement state
│   ├── KAIZEN_LOG.md                  ← Experiment/iteration log
│   └── KAIZEN_IDEAS.md                ← Prioritized ideas backlog
│
├── src/
│   ├── main.rs                        ← CLI entry point (clap)
│   │                                    Commands: init, start, status, chat, cron, tui
│   ├── lib.rs                         ← Public library interface (re-exports all modules)
│   ├── config.rs                      ← TOML config parsing (~/.rustclaw/config.toml)
│   ├── config_test.rs                 ← Config unit tests
│   │
│   ├── auth/                          ← 🤖 LLM Provider Clients
│   │   ├── mod.rs                     ← Provider trait, ChatMessage, CompletionResponse, routing
│   │   ├── anthropic.rs               ← Claude Messages API (blocking, no streaming yet)
│   │   ├── openai.rs                  ← OpenAI Chat Completions API
│   │   └── retry.rs                   ← Exponential backoff with configurable attempts
│   │
│   ├── channels/                      ← 📡 Multi-channel Messaging
│   │   ├── mod.rs                     ← Channel trait, InboundMessage
│   │   ├── telegram.rs               ← Telegram Bot API (long-polling + allowlist)
│   │   ├── whatsapp.rs               ← WhatsApp Cloud API (axum webhook)
│   │   └── slack.rs                   ← Slack Events API (⚠️ no signature verification)
│   │
│   ├── chat/                          ← 💬 Conversation Engine
│   │   ├── mod.rs                     ← Chat loop: msg → system prompt → LLM → reply
│   │   └── session.rs                 ← JSON-backed multi-turn sessions + pruning
│   │
│   ├── guardd/                        ← 🔐 SECURITY KERNEL (NEW — 2026-02-17)
│   │   ├── mod.rs                     ← GuardDaemon orchestrator
│   │   │                                Action enum: SendMessage/RunCommand/AccessFile/ApiCall/WebhookInbound
│   │   │                                Verdict enum: Allow/Deny/AskHuman
│   │   ├── policy.rs                  ← Rule engine + rate limiting + workspace reads
│   │   ├── credentials.rs            ← AES-256-GCM encrypted store + zeroize on drop
│   │   ├── sandbox.rs                ← Command/path allowlist + deny patterns (rm -rf, sudo)
│   │   ├── audit.rs                  ← Append-only JSONL audit trail (~/.rustclaw/audit.jsonl)
│   │   └── channel_auth.rs           ← HMAC-SHA256 webhook verification + replay protection
│   │
│   ├── cron/
│   │   └── mod.rs                     ← Job scheduler (tokio-cron-scheduler) + heartbeat
│   │
│   ├── nodes/
│   │   └── mod.rs                     ← Presence beacon + local node discovery
│   │
│   ├── setup/
│   │   └── mod.rs                     ← Interactive `rustclaw init` wizard (dialoguer)
│   │
│   ├── tui/                           ← 🖥 Terminal UI (ratatui) — prototype
│   │   ├── mod.rs                     ← Module exports
│   │   ├── app.rs                     ← Main TUI loop + keyboard handling
│   │   ├── panel.rs                   ← Menu items + status indicators
│   │   ├── theme.rs                   ← Color palette (professional dark theme)
│   │   └── widgets.rs                 ← Custom ratatui widgets
│   │
│   └── workspace/                     ← 📁 Workspace File Management
│       ├── mod.rs                     ← Read/write .md files, build_system_prompt()
│       ├── workspace_test.rs          ← Unit tests
│       └── templates/                 ← Default workspace files
│           ├── AGENTS.md / SOUL.md / USER.md
│           ├── IDENTITY.md / TOOLS.md
│           ├── MEMORY.md / HEARTBEAT.md
│           └── (created by `rustclaw init`)
│
├── tests/
│   └── integration_test.rs            ← 5 E2E tests (config, session, workspace, cron, prompt)
│
└── target/                            ← Build cache (3.3 GB — `cargo clean` to reclaim)
```

## Key Dependencies
`tokio, serde, reqwest, axum, clap, toml, aes-gcm, zeroize, hmac, sha2, ratatui, crossterm, dialoguer, chrono, uuid, dotenvy, anyhow, tracing`

## Data Flow
```
Telegram msg → allowlist check → load session → build system prompt (SOUL+AGENTS+MEMORY+...)
→ guardd.authorize(ApiCall) → LLM API → save response to session → reply to Telegram
```
