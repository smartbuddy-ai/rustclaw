# Rustclaw Audit Report — 2026-02-17

## Executive Summary
Rustclaw is a functional early-stage runtime with solid foundations (config, workspace bootstrap, session persistence, channel adapters, cron, and basic node beaconing) and good baseline reliability (all tests pass), but it is still far from OpenClaw feature parity. Estimated overall completeness vs OpenClaw: **~35%**. The biggest gaps are the **tooling/runtime surface** (browser/canvas/nodes remote actions, message abstraction parity, subagent orchestration), **security kernel/guardd absence**, and **production hardening** (channel auth verification, filesystem safety, clippy hygiene, and runtime robustness).

## Compilation & Tests
- **cargo check**: ✅ success
  - Compiler warnings: **5** (unused imports/dead code in TUI modules)
- **cargo test**: ✅ success
  - Unit + integration results:
    - `src/lib.rs` tests: 19 passed
    - `src/main.rs` tests: 19 passed
    - `tests/integration_test.rs`: 5 passed
    - Doc tests: 0
  - Total observed: **43 passed, 0 failed**
- **cargo clippy --all-targets --all-features**: ⚠️ success with warnings
  - `rustclaw` lib: **7 warnings**
  - bin/test targets: duplicate + additional style warnings (collapsible if chains, needless borrow, unused imports/dead code)
- **cargo clippy --all-targets --all-features -- -D warnings**: ❌ fails (7 lint errors elevated to hard errors)

## Feature Completeness (vs OpenClaw)
| Feature | Status | Notes |
|---------|--------|-------|
| CLI entrypoint/runtime start/stop surface | Partial | `start/run/status/chat/cron/init/tui` implemented; no full gateway service manager parity with OpenClaw command ecosystem. |
| Workspace bootstrap & prompt assembly | Implemented | Creates/reads standard files and builds composite system prompt from workspace files. |
| Multi-turn chat sessions | Implemented | JSON-backed sessions with pruning; per-channel session IDs. |
| LLM providers | Partial | Anthropic + OpenAI implemented with retry wrapper; no provider multiplexing/routing policies beyond default provider. |
| Retry/backoff | Implemented | Exponential backoff logic in `auth/retry.rs` with tests. |
| Telegram channel | Partial | Polling + reply + allowlist supported; no webhook mode, no advanced Telegram actions/reactions/media/thread features parity. |
| WhatsApp channel | Partial | Webhook receive + text send supported; no guardrails/compliance depth, no richer media/actions. |
| Slack channel | Partial | Events API endpoint and postMessage flow implemented; no signature verification, no socket-mode implementation despite config field. |
| Cron scheduler | Partial | Add/remove/list/execute + heartbeat trigger; config persistence works, but output channels are minimally implemented (Telegram only). |
| Heartbeat model | Partial | HEARTBEAT.md prompt execution exists; no robust scheduler policy framework/quiet hours/notification strategy parity. |
| Node presence | Partial | Beacon file + local discovery of nodes directory; no paired-node RPC/invoke/camera/screen/location capabilities. |
| Browser automation | Missing | No browser control subsystem equivalent. |
| Canvas subsystem | Missing | No canvas present/eval/snapshot layer. |
| Message plugin abstraction parity | Missing/Partial | Per-channel modules exist, but not a unified rich channel action API equivalent to OpenClaw plugin surface. |
| Subagent orchestration | Missing | No sessions spawn/worker orchestration logic found. |
| Security kernel (`guardd`/policy engine) | Missing | No guardd daemon/module or centralized policy enforcement runtime. |
| TUI dashboard | Partial (prototype) | Ratatui UI exists and is visually structured but currently mostly static/mock state, not wired to live runtime telemetry/actions. |

## Code Quality
- **General structure**: clean modular layout (`auth/`, `channels/`, `chat/`, `cron/`, `nodes/`, `workspace/`, `setup/`, `tui/`).
- **Error handling**:
  - Positive: broad `anyhow::Result` usage with propagated errors, contextual errors in config loading.
  - Weak spots: some external HTTP calls do not validate response body/status deeply (e.g., WhatsApp send path returns Ok if HTTP request succeeds even if API returns error payload).
- **Idiomatic Rust**:
  - Mostly idiomatic async usage and serde models.
  - Clippy indicates several style/code-smell items (collapsible `if` chains, needless borrow, dead code/unused imports).
- **Testing**:
  - Good baseline unit tests for config/session/workspace/retry.
  - Integration tests are present but some are placeholders (cron lifecycle test does not fully assert add/remove behavior).
- **Unsafe usage**:
  - One `unsafe` block in `nodes/mod.rs` (`libc::gethostname`) without explicit return-code checking or safety commentary.
  - No broader unsafe patterns detected.

## Security Assessment
- **Credential handling**:
  - Secrets are separated into `~/.rustclaw/.env` and excluded from config.
  - Unix permission hardening (`0600`) applied in setup.
- **Missing security controls**:
  - **No `guardd`** or equivalent security kernel/policy daemon.
  - No centralized permission gating for tool/channel actions.
  - No sandboxing framework for command/runtime operations.
- **Channel security gaps**:
  - Slack Events endpoint lacks signing secret verification (critical for webhook authenticity).
  - WhatsApp webhook handles verification challenge but no additional request signature validation was observed.
  - Telegram polling model has allowlist support, but no deeper anti-abuse controls/rate limiting.
- **Filesystem safety**:
  - Session filename uses raw `session_id` (`{session_id}.json`) with no sanitization; channel-controlled IDs should be normalized to prevent path manipulation edge cases.

## TUI Assessment
- **Current maturity**: prototype/demo level.
- **What exists**:
  - Split-pane ratatui UI (left navigation panel + right chat pane).
  - Keyboard navigation, section expand/collapse, focus switching.
  - Theming and custom widgets are reasonably clean.
- **What is missing for production utility**:
  - No live binding to runtime state (channels, jobs, nodes are hardcoded examples).
  - No actionable controls (start/stop channel, run cron now, inspect logs, etc.).
  - Chat pane is placeholder (`Processing...`), not connected to real LLM exchange loop.
  - Multiple unused imports/methods/constants indicate unfinished iteration.

## Dependencies
### Declared dependencies reviewed
`tokio, serde, serde_json, serde_yaml, reqwest, axum, tower, tower-http, tracing, tracing-subscriber, chrono, uuid, dashmap, clap, toml, thiserror, anyhow, async-trait, futures, tokio-cron-scheduler, cron, directories, libc, dotenvy, dialoguer, ratatui, crossterm`

### Likely unnecessary (currently unused in `src/`)
- `serde_yaml`
- `tower`
- `tower-http`
- `dashmap`
- `thiserror`
- `async-trait`
- `futures`

### Potentially misleading/incomplete usage
- Slack config includes `socket_mode` and app token fields, but no socket-mode runtime implementation.
- `axum` websocket feature enabled, but no websocket handlers observed.

### Outdated check
- `cargo outdated` could not be run because `cargo-outdated` is not installed in this environment, so version staleness could not be verified directly.

## Critical Issues
1. **No webhook authenticity verification for Slack events** (signing secret not enforced).
2. **No security kernel/guardd equivalent** for policy enforcement and sensitive action control.
3. **Session file path not sanitized** before filesystem write.
4. **Clippy strict mode fails** (`-D warnings`) due to multiple style/lint issues.
5. **Feature gap vs OpenClaw is large** in tools/runtime surfaces (browser/canvas/node RPC/subagents/message actions).

## Recommendations
1. **Security first (highest priority)**
   - Add webhook signature verification for Slack (and WhatsApp if applicable).
   - Introduce a policy layer (guardd-lite) for action authorization and sensitive operations.
   - Sanitize/encode session IDs before mapping to filesystem paths.
2. **Reliability hardening**
   - Make `cargo clippy -- -D warnings` pass in CI.
   - Improve outbound API error parsing/handling (especially channel send paths).
   - Add integration tests for real cron add/remove/list persistence flow.
3. **Feature parity roadmap**
   - Implement unified tool/action abstraction closer to OpenClaw (message actions, node invoke, browser/canvas).
   - Add subagent orchestration primitives.
4. **TUI evolution**
   - Bind panels to live runtime state.
   - Add operational actions (trigger job, inspect sessions/nodes/channels).
   - Wire chat pane to actual async request pipeline.
5. **Dependency cleanup**
   - Remove unused crates or implement planned features that justify them.
   - Add periodic dependency audit (`cargo audit`, `cargo deny`, `cargo outdated`).
