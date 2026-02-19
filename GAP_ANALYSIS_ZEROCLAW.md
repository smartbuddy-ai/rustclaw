# Rustclaw Gap Analysis — What to Port from ZeroClaw

Date: 2026-02-17
Sources reviewed:
- ZeroClaw audit: `projects/zeroclaw/AUDIT_VS_OPENCLAW.md`
- Rustclaw audit: `projects/rustclaw/AUDIT_REPORT.md`
- ZeroClaw source slices (security/memory/providers/config/cron/channels/tools/gateway/tunnel/rag)
- Rustclaw source (`src/**/*.rs`)

---

## Priority 1: CRITICAL (port immediately)

### 1) Provider reliability + router stack
- **What it is**: multi-provider retries/backoff/failover + hint-based routing (`hint:fast`, `hint:reasoning`) + fallback order.
- **ZeroClaw source + LOC**:
  - `src/providers/reliable.rs` (927)
  - `src/providers/router.rs` (385)
- **Rustclaw current state**:
  - Had only single-provider dispatch in `src/auth/mod.rs` + basic retry helper in `src/auth/retry.rs` (139).
  - No route table, no provider failover chain.
- **Effort**: **M**
- **Implementation plan**:
  1. Introduce provider trait abstraction for completion providers.
  2. Add router (`hint:*`) + route table in config.
  3. Add reliable wrapper with retry/backoff + fallback provider list.
  4. Wire `chat::send` path through reliable router.
  5. Add tests for route resolution + fallback behavior.

### 2) Memory system upgrade (SQLite brain)
- **What it is**: persistent memory store with indexed retrieval and upsert semantics.
- **ZeroClaw source + LOC**:
  - `src/memory/sqlite.rs` (1586)
  - `src/memory/mod.rs` (249), `backend.rs` (146), `vector.rs` (402), `lucid.rs` (675)
- **Rustclaw current state**:
  - Session memory exists only as JSON chat history (`src/chat/session.rs`, 171).
  - No long-term structured memory DB.
- **Effort**: **L**
- **Implementation plan**:
  1. Add SQLite-backed memory module (`memory/brain.db`).
  2. Schema: `memories` table + indexes + WAL pragma.
  3. Add API: `upsert`, `get`, `search`.
  4. Add unit tests with temp dir.
  5. Integrate later with chat prompt hydration.

### 3) Config schema hardening
- **What it is**: typed schema coverage for reliability/routing/memory/tunnel.
- **ZeroClaw source + LOC**:
  - `src/config/schema.rs` (3762)
- **Rustclaw current state**:
  - Single-file compact config (`src/config.rs`, 271) lacked schema for advanced runtime knobs.
- **Effort**: **M**
- **Implementation plan**:
  1. Add nested structs for provider reliability + routes.
  2. Add memory backend/path config.
  3. Add tunnel provider config.
  4. Ensure defaults + serialization tests.

### 4) Cron reliability semantics
- **What it is**: retry-aware scheduler behavior for job failures.
- **ZeroClaw source + LOC**:
  - `src/cron/scheduler.rs` (672), `types.rs` (140), `store.rs` (668)
- **Rustclaw current state**:
  - Cron exists (`src/cron/mod.rs`, 223) but retry semantics were minimal and implicit.
- **Effort**: **S/M**
- **Implementation plan**:
  1. Add per-job retry field.
  2. Use retries in execution loop.
  3. Add regression tests for lifecycle.

---

## Priority 2: HIGH (port this week)

### 5) Security policy parity pass
- **What it is**: mature risk policy, command/path gating, action budget, pairing hardening.
- **ZeroClaw source + LOC**:
  - `src/security/policy.rs` (1328), `pairing.rs` (475), `secrets.rs` (851), `audit.rs` (335), sandbox files.
- **Rustclaw current state**:
  - Guard modules exist but not deeply integrated across runtime.
- **Effort**: **L/XL**
- **Implementation plan**: policy-first integration at action boundaries (chat tools, cron shell, channel webhooks), then pairing + audit rollups.

### 6) Gateway/webhook security hardening
- **What it is**: idempotency/rate-limits/secret auth patterns for inbound webhooks.
- **ZeroClaw source + LOC**: `src/gateway/mod.rs` (1396)
- **Rustclaw current state**: basic channel handlers, known signature-verification gaps from audit.
- **Effort**: **L**
- **Implementation plan**: verify signatures, enforce replay windows, add idempotency keys.

### 7) Tool surface growth
- **What it is**: standardized tool registry + traits.
- **ZeroClaw source + LOC**: `src/tools/mod.rs` (488), `src/tools/traits.rs` (121)
- **Rustclaw current state**: no equivalent unified tool framework.
- **Effort**: **L**
- **Implementation plan**: introduce typed tool trait + registry + policy gate.

---

## Priority 3: MEDIUM (port this month)

### 8) Tunnel provider matrix
- **What it is**: cloudflare/ngrok/tailscale/custom pluggable tunnels.
- **ZeroClaw source + LOC**:
  - `src/tunnel/mod.rs` (375)
  - `cloudflare.rs` (141), `ngrok.rs` (151), `tailscale.rs` (133), `custom.rs` (220), `none.rs` (64)
- **Rustclaw current state**:
  - No tunnel module before this mission.
- **Effort**: **M**
- **Implementation plan**: start with none+custom abstraction, then add concrete providers.

### 9) RAG subsystem
- **What it is**: retrieval pipeline for docs/datasheets and memory-grounded responses.
- **ZeroClaw source + LOC**: `src/rag/mod.rs` (395)
- **Rustclaw current state**: no dedicated RAG module.
- **Effort**: **M/L**
- **Implementation plan**: start with local markdown retrieval + citation injection.

### 10) Channel parity expansion
- **What it is**: richer telegram actions/media/chunking and more channels.
- **ZeroClaw source + LOC**: `src/channels/mod.rs` (2227), `src/channels/telegram.rs` (1908)
- **Rustclaw current state**: Telegram/WhatsApp/Slack basic paths.
- **Effort**: **L**
- **Implementation plan**: implement message action API first (send/edit/delete/react/media).

---

## Priority 4: LOW (reference only)

### 11) Lucid/advanced memory hygiene layers
- **What it is**: advanced memory processing/cleanup.
- **ZeroClaw source + LOC**: `memory/lucid.rs` (675), `hygiene.rs` (540), `snapshot.rs` (470), `response_cache.rs` (351).
- **Rustclaw current state**: not present.
- **Effort**: **L**
- **Implementation plan**: only after baseline reliability/security parity is stable.

### 12) Full provider ecosystem aliasing
- **What it is**: broad provider aliases/openai-compatible matrix.
- **ZeroClaw source + LOC**: `src/providers/mod.rs` (1523)
- **Rustclaw current state**: Anthropic/OpenAI only.
- **Effort**: **M/L**
- **Implementation plan**: add generic OpenAI-compatible adapter + alias map.

---

## Delta Snapshot After This Mission (implemented now)
- ✅ Added provider router + reliability module in Rustclaw (`src/providers/mod.rs`)
- ✅ Upgraded config schema for reliability/routes/memory/tunnel (`src/config.rs`)
- ✅ Added SQLite memory module (`src/memory/mod.rs`)
- ✅ Added tunnel abstraction (none/custom) (`src/tunnel/mod.rs`)
- ✅ Added tests for new modules and updated existing test fixtures

Remaining largest unported blocks from ZeroClaw:
1. Full memory stack (vector/embeddings/cache/hygiene)
2. Full scheduler/store model
3. Deep security integration and gateway hardening
4. Full tunnel provider implementations
