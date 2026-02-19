# Rustclaw Refactoring Plan — Feb 2026

## Goal: 7.6 → 9.0+ across all metrics

## Phase 1: Critical Gaps (Feature Parity)

### 1A. Process/Shell Tool (MISSING)
- New `src/tools/shell.rs` — execute shell commands with guardd authorization
- Timeout, working directory, env vars support
- Output capture (stdout + stderr + exit code)
- Guard integration: allowlist patterns, workspace-scoped paths

### 1B. Browser Tool (WEAK → FULL)
- CDP (Chrome DevTools Protocol) integration via websocket
- Actions: navigate, snapshot (DOM text), screenshot (CDP), click, type, evaluate JS
- Fallback: headless chromium launch if no existing CDP endpoint
- Keep existing HTTP fetch as lightweight fallback

### 1C. Telegram Webhook Secret Verification (MISSING)
- Verify `X-Telegram-Bot-Api-Secret-Token` header on webhook mode
- Add to channel_auth module

### 1D. Webhook Auth Enforcement in Channel Handlers
- Slack: enforce signature verification in webhook handler (not just available as util)
- WhatsApp: same
- Telegram: webhook secret check

## Phase 2: Observability & Telemetry

### 2A. Metrics Module (`src/telemetry/mod.rs`)
- In-process metrics: request count, latency histogram, error rate, token usage
- `/api/metrics` endpoint (Prometheus text format)
- Structured tracing integration (already have tracing dep)
- Per-channel, per-provider metrics

### 2B. Health Check Endpoint
- `/api/health` — deep health (DB, providers reachable, channels connected)
- `/api/ready` — readiness probe

## Phase 3: Security Hardening

### 3A. Input Validation
- Max message length enforcement on all channels
- JSON schema validation on API endpoints
- Path traversal protection audit on all file operations

### 3B. Secret Rotation Support
- Config reload without restart (SIGHUP handler)
- Token refresh for providers

### 3C. Guardd Integration Audit
- Ensure ALL tool executions go through guardd
- Shell tool: mandatory allowlist mode by default
- Audit log every tool invocation with user/session context

## Phase 4: Reliability & Ops

### 4A. Graceful Shutdown
- Signal handler (SIGTERM/SIGINT)
- Drain in-flight requests
- Close DB connections cleanly
- Flush audit logs

### 4B. Error Recovery
- Provider circuit breaker (after N failures, skip for cooldown)
- Channel reconnection with exponential backoff
- Cron job failure alerting

### 4C. Configuration Validation
- Validate config on startup (not just parse)
- Warn on missing optional but recommended settings
- `rustclaw doctor` command

## Phase 5: Test Maturity

### 5A. Integration Tests
- Gateway API full flow (auth → chat → response)
- Channel webhook simulation (Telegram, Slack, WhatsApp)
- Tool execution through guardd
- Cron lifecycle with real scheduler

### 5B. Error Path Tests
- Invalid auth tokens
- Provider failures and fallback verification
- Rate limit exhaustion
- Malformed webhook payloads

### 5C. Property/Fuzz Tests
- Config parsing with arbitrary TOML
- Channel auth with edge-case signatures

## Phase 6: Documentation

### 6A. Module-level doc comments on every public item
### 6B. Architecture doc (`docs/ARCHITECTURE.md`)
### 6C. Deployment guide (`docs/DEPLOYMENT.md`)
### 6D. Security model doc (`docs/SECURITY.md`)

## Implementation Order
1. Phase 1 (gaps) → 2 (telemetry) → 3 (security) → 4 (reliability) → 5 (tests) → 6 (docs)
2. Each phase = one Sonnet sub-agent spawn
3. After each phase: `cargo test` must pass, review output
