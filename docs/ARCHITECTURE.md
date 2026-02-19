# Rustclaw Architecture

## Overview
Rustclaw is a Rust-native AI agent runtime focused on Telegram, WhatsApp, and Slack channels. It provides multi-provider LLM routing, tool execution, memory persistence, and operational controls.

## Module Map

```
src/
├── main.rs          — CLI entrypoint (start, chat, cron, doctor, tui)
├── lib.rs           — Public module exports
├── config.rs        — TOML configuration parsing + validation
├── gateway/         — HTTP API server (axum)
│   └── mod.rs       — Routes: /api/{status,chat,health,ready,metrics,skills,config}
├── channels/        — Messaging platform integrations
│   ├── mod.rs       — Channel router (starts enabled channels)
│   ├── telegram.rs  — Long-polling + webhook support
│   ├── whatsapp.rs  — Cloud API webhook with HMAC verification
│   ├── slack.rs     — Events API with signature verification
│   ├── discord.rs   — Gateway WebSocket + REST polling
│   └── signal.rs    — Signal CLI bridge polling
├── chat/            — LLM conversation management
│   ├── mod.rs       — send() and send_with_session() entrypoints
│   └── session.rs   — Session persistence (SQLite)
├── tools/           — Agent tool implementations
│   ├── mod.rs       — Tool trait + registry
│   ├── shell.rs     — Command execution with allowlist + guardd
│   ├── process.rs   — Background process manager
│   ├── file.rs      — File read/write/list with path guards
│   ├── browser.rs   — CDP + HTTP fallback browser automation
│   ├── web.rs       — Brave search + HTML fetch
│   ├── http.rs      — Generic HTTP requests
│   ├── git.rs       — Git operations
│   ├── memory.rs    — Memory store/recall tool
│   └── image_info.rs — Image metadata extraction
├── guardd/          — Security policy enforcement
│   ├── mod.rs       — Guard daemon (policy + sandbox + audit)
│   ├── policy.rs    — Rule-based policy engine with rate limiting
│   ├── sandbox.rs   — Command/path sandboxing
│   ├── audit.rs     — JSONL audit logging
│   ├── channel_auth.rs — HMAC verification (Slack, WhatsApp, Telegram)
│   └── credentials.rs — AES-256-GCM encrypted credential store
├── telemetry/       — Runtime observability
│   └── mod.rs       — Counters, gauges, histograms, Prometheus export
├── providers/       — LLM provider routing + reliability
│   └── mod.rs       — Router with hint-based routing + fallback chain
├── memory/          — Persistent memory
│   ├── mod.rs       — SQLite KV store
│   └── vector.rs    — Vector embeddings + RAG pipeline
├── sessions/        — Session management
│   └── mod.rs       — SQLite session store with compaction
├── cron/            — Job scheduling
│   └── mod.rs       — SQLite-backed cron with retries + history
├── agent/           — Multi-agent registry
│   └── mod.rs       — Agent definitions + channel bindings
├── skills/          — Skill discovery + prompt injection
│   └── mod.rs       — SKILL.md scanner + trigger matching
├── heartbeat/       — Periodic health checks
│   └── mod.rs       — Active hours + heartbeat loop
├── tunnel/          — Tunnel providers for webhook exposure
│   ├── mod.rs       — Tunnel trait + factory
│   ├── cloudflare.rs, ngrok.rs, tailscale.rs
├── workspace/       — Workspace file management
│   └── mod.rs       — System prompt building, context injection
├── auth/            — LLM authentication
│   ├── mod.rs       — Auth resolution
│   ├── anthropic.rs, openai.rs — Provider-specific auth
│   └── retry.rs     — Retry with exponential backoff
├── nodes/           — Node presence + status
├── setup/           — Interactive initialization wizard
└── tui/             — Terminal UI dashboard
```

## Security Model
1. **Gateway auth**: Bearer token middleware on all API endpoints
2. **Channel auth**: HMAC signature verification (Slack, WhatsApp), secret token (Telegram)
3. **Guardd**: Policy engine authorizes all tool invocations
4. **Shell allowlist**: Only pre-approved commands can execute
5. **Path sandboxing**: File operations confined to workspace directory
6. **Credential encryption**: AES-256-GCM at-rest encryption for secrets
7. **Rate limiting**: Per-IP sliding window on gateway API
8. **Audit logging**: JSONL log of all guard decisions
9. **Input validation**: Max message length, JSON schema checks

## Data Flow
```
Channel (Telegram/WhatsApp/Slack)
  → Channel Handler (auth verification)
  → Chat Engine (session management)
  → Provider Router (LLM selection + fallback)
  → LLM Provider (Anthropic/OpenAI)
  → Tool Execution (guardd authorization)
  → Response (channel reply)
```

## Observability
- `/api/health` — Deep health check (DB, channels, uptime)
- `/api/ready` — Readiness probe for load balancers
- `/api/metrics` — Prometheus-format metrics export
- Structured tracing via `tracing` crate with JSON output support
- Per-request latency tracking for chat endpoint
