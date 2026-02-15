# Rustclaw Build Plan

## Current State (Assessment Complete)

**✅ Already Implemented:**
1. ✅ Project scaffold (3 commits, compiles with warnings only)
2. ✅ Config loading (TOML config.toml + .env via dotenvy)
3. ✅ Workspace file structure & templates (7 .md templates included)
4. ✅ Workspace initialization (`rustclaw init` — interactive setup flow)
5. ✅ Anthropic chat completion (full implementation)
6. ✅ OpenAI chat completion (full implementation)
7. ✅ Telegram channel (long-polling, send/receive, markdown support)
8. ✅ WhatsApp channel (webhook server, Cloud API)
9. ✅ Slack channel (Events API webhook server)
10. ✅ Chat loop wired (Telegram → build system prompt → LLM → reply)
11. ✅ Cron scheduler (tokio-cron-scheduler, add/list/remove jobs)
12. ✅ Workspace file reading (build_system_prompt reads SOUL.md, IDENTITY.md, AGENTS.md, TOOLS.md, MEMORY.md, daily notes)
13. ✅ Basic CLI (`rustclaw init`, `rustclaw start`, `rustclaw status`, `rustclaw chat`, `rustclaw cron`)
14. ✅ Node presence beacon (writes heartbeat JSON, scans for other nodes)
15. ✅ Secure credential storage (.env with 0600 permissions, never in config.toml)
16. ✅ API key validation during init (Anthropic, OpenAI, Telegram)

**🟡 Partially Implemented:**
- Streaming support: Not yet implemented (Anthropic & OpenAI clients are blocking)
- Conversation history: Chat is currently single-turn only (no session memory)
- Environment variable precedence: Partially working but needs refinement

**❌ Missing / Needs Enhancement:**
1. Streaming chat completions (both Anthropic and OpenAI)
2. Multi-turn conversation history management (session state)
3. Error handling improvements (better retry logic, graceful degradation)
4. Tests (no tests written yet)
5. Better logging (structured tracing setup exists but needs tuning)
6. Heartbeat execution (HEARTBEAT.md exists but not processed by cron)

---

## Implementation Plan (Milestones)

### ✅ Milestone 0: Assessment & Planning
- [x] Read all source files
- [x] Verify compilation
- [x] Document current state
- [x] Write BUILD_PLAN.md

### Milestone 1: Fix Warnings & Add Basic Tests
**Goal:** Clean compile, basic test coverage

**Tasks:**
- [ ] Fix all compiler warnings (unused functions, structs)
- [ ] Write unit tests for config loading
- [ ] Write unit tests for workspace file operations
- [ ] Write integration test for auth module (mock responses)
- [ ] `cargo test` passes

**Acceptance:**
- Zero warnings on `cargo build`
- At least 5 tests passing
- Commit: `test: add basic unit tests for config and workspace`

---

### Milestone 2: Streaming Support (Anthropic & OpenAI)
**Goal:** Enable streaming chat completions

**Tasks:**
- [ ] Implement streaming in `auth/anthropic.rs` (SSE parsing)
- [ ] Implement streaming in `auth/openai.rs` (SSE parsing)
- [ ] Update `auth::complete()` signature to support streaming
- [ ] Add `--stream` flag to `rustclaw chat` command
- [ ] Wire streaming into Telegram channel (send chunks as they arrive)
- [ ] Test streaming end-to-end

**Acceptance:**
- `rustclaw chat --stream "tell me a story"` works
- Telegram replies stream in real-time
- Commit: `feat(auth): add streaming support for Anthropic and OpenAI`

---

### Milestone 3: Conversation History Management
**Goal:** Multi-turn conversations with session state

**Tasks:**
- [ ] Design session store (file-based or in-memory with DashMap)
- [ ] Implement session loading/saving to `workspace/sessions/{chat_id}.json`
- [ ] Update Telegram handler to load/save session per chat
- [ ] Update WhatsApp handler to load/save session
- [ ] Update Slack handler to load/save session
- [ ] Add `--session` flag to `rustclaw chat` for CLI sessions
- [ ] Implement session pruning (max messages, max age)
- [ ] Test multi-turn conversations

**Acceptance:**
- Send 3+ messages to Telegram bot, it remembers context
- Sessions persist across gateway restarts
- Commit: `feat(chat): add multi-turn conversation history with session management`

---

### Milestone 4: Enhanced Cron & Heartbeat Processing
**Goal:** Cron jobs execute with context, heartbeat tasks run

**Tasks:**
- [ ] Add heartbeat cron job processor (reads HEARTBEAT.md, executes tasks)
- [ ] Implement default heartbeat job (every 30 min) if enabled in config
- [ ] Wire heartbeat output to configured channel (Telegram, Slack, etc.)
- [ ] Add `rustclaw cron test <id>` to manually trigger a job
- [ ] Add logging for cron job execution (success/failure, duration)
- [ ] Test heartbeat + manual cron trigger

**Acceptance:**
- Heartbeat runs automatically and sends proactive messages
- `rustclaw cron test` works
- Commit: `feat(cron): add heartbeat processing and manual job triggers`

---

### Milestone 5: Error Handling & Resilience
**Goal:** Graceful failures, retry logic, better diagnostics

**Tasks:**
- [ ] Add retry logic to LLM API calls (exponential backoff)
- [ ] Add timeout handling for all HTTP requests
- [ ] Improve error messages (include context, suggestions)
- [ ] Add circuit breaker pattern for channel failures
- [ ] Graceful shutdown on SIGTERM (not just SIGINT)
- [ ] Test failure scenarios (invalid API keys, network errors, rate limits)

**Acceptance:**
- Gateway stays up when LLM API is down
- Helpful error messages guide troubleshooting
- Commit: `fix: add retry logic and graceful error handling`

---

### Milestone 6: Documentation & Integration Testing
**Goal:** Complete README, end-to-end test, deployment guide

**Tasks:**
- [ ] Update README.md with full setup instructions
- [ ] Document all environment variables in README
- [ ] Write ARCHITECTURE.md (data flow diagrams)
- [ ] Create example config files (examples/config.toml)
- [ ] Write integration test script (test.sh — full flow simulation)
- [ ] Document known limitations and future work
- [ ] Add CONTRIBUTING.md

**Acceptance:**
- README is comprehensive and up-to-date
- Integration test passes end-to-end
- Commit: `docs: complete README and integration test`

---

### Milestone 7: Polish & Release Prep
**Goal:** Production-ready v0.1.0

**Tasks:**
- [ ] Version bump to 0.1.0 in Cargo.toml
- [ ] Add GitHub Actions CI (build + test on push)
- [ ] Generate CHANGELOG.md from git log
- [ ] Tag release: `v0.1.0`
- [ ] Push to GitHub with release notes
- [ ] Optional: Publish to crates.io (if open source)

**Acceptance:**
- Clean git history with conventional commits
- CI passes on GitHub
- Release tagged and published
- Commit: `chore: release v0.1.0`

---

## Dependencies & Assumptions

**Assumptions:**
- GitHub credentials are configured (`git push` works)
- Rust toolchain is up-to-date (edition 2024 supported)
- No API keys needed for basic functionality tests (can mock)

**Blockers:**
- If API keys are missing, integration tests will be documented as manual-only
- External service dependencies (Telegram, Anthropic) may introduce flakiness

---

## Success Criteria

**At completion:**
1. All milestones implemented and tested
2. `cargo build` — zero warnings
3. `cargo test` — all tests pass
4. Full integration test: config → init → Telegram msg → Anthropic → reply (works)
5. README documents all setup steps and env vars
6. Git history shows ~15-20 clean, conventional commits
7. All commits pushed to GitHub

**Known Limitations (documented, not blocking):**
- No voice/image support (out of scope for lightweight runtime)
- No database (file-based sessions only)
- No auth beyond allowlists (good enough for personal use)
- No web UI (CLI + chat channels only)

---

## Timeline Estimate

- Milestone 1: ~1 hour (tests + cleanup)
- Milestone 2: ~2 hours (streaming is tricky)
- Milestone 3: ~2 hours (session management design)
- Milestone 4: ~1 hour (cron + heartbeat)
- Milestone 5: ~1.5 hours (error handling)
- Milestone 6: ~1 hour (docs)
- Milestone 7: ~0.5 hour (release)

**Total: ~9 hours of focused work**

---

## Post-Build Report Template

```
# Rustclaw Build Report

## What I Built
- [List of implemented features]

## What Works
- [End-to-end test results]
- [Known working configurations]

## What's Left
- [Remaining TODOs or future work]

## Git Log
[Output of `git log --oneline`]

## Test Output
[Output of `cargo test`]

## Integration Test
[Manual test results: Telegram → LLM → reply]
```
