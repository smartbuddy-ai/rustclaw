# Rustclaw Deployment Guide

## Quick Start
```bash
# Build
cargo build --release

# Initialize workspace
./target/release/rustclaw init

# Run health check
./target/release/rustclaw doctor

# Start gateway
RUST_LOG=info ./target/release/rustclaw start
```

## Configuration
Config file: `~/.rustclaw/config.toml`
Secrets: `~/.rustclaw/.env` (mode 0600)

### Minimal config.toml
```toml
workspace_dir = "~/.rustclaw/workspace"

[auth]
default_provider = "anthropic"
default_model = "claude-sonnet-4-20250514"

[gateway]
host = "0.0.0.0"
port = 8088

[gateway.auth]
mode = "token"  # Set to "token" for production!

[gateway.rate_limit]
enabled = true
requests_per_minute = 60

[channels.telegram]
enabled = true
allow_from = ["YOUR_TELEGRAM_USER_ID"]
```

### .env
```bash
ANTHROPIC_API_KEY=sk-ant-...
TELEGRAM_BOT_TOKEN=123456:ABC...
RUSTCLAW_GATEWAY_TOKEN=your-secret-token
# Optional
OPENAI_API_KEY=sk-...
SLACK_BOT_TOKEN=xoxb-...
SLACK_SIGNING_SECRET=...
WHATSAPP_ACCESS_TOKEN=...
```

## Production Checklist
- [ ] `gateway.auth.mode = "token"` (not "none")
- [ ] `RUSTCLAW_GATEWAY_TOKEN` set in .env
- [ ] `.env` file permissions: `chmod 600 ~/.rustclaw/.env`
- [ ] `gateway.rate_limit.enabled = true`
- [ ] Telegram `allow_from` configured (restrict to known users)
- [ ] Slack `signing_secret` configured
- [ ] WhatsApp `app_secret` configured
- [ ] `RUST_LOG=info` (not debug in production)
- [ ] Run `rustclaw doctor` to verify setup
- [ ] Reverse proxy (nginx/caddy) for TLS termination
- [ ] Monitoring: scrape `/api/metrics` with Prometheus

## Systemd Service
```ini
[Unit]
Description=Rustclaw AI Agent Runtime
After=network.target

[Service]
Type=simple
User=rustclaw
ExecStart=/usr/local/bin/rustclaw start
Restart=always
RestartSec=5
Environment=RUST_LOG=info
EnvironmentFile=/home/rustclaw/.rustclaw/.env

[Install]
WantedBy=multi-user.target
```

## Docker
```dockerfile
FROM rust:1.85 AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/rustclaw /usr/local/bin/
EXPOSE 8088
CMD ["rustclaw", "start"]
```

## Health Monitoring
- `GET /api/health` — Returns `{"status":"healthy","uptime_secs":...}`
- `GET /api/ready` — Returns `{"ready":true}`
- `GET /api/metrics` — Prometheus text format
