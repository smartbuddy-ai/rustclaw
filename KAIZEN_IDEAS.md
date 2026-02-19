# Ideas Backlog - Rustclaw

## Priority: HIGH

### 🔴 Bugs sécurité (découverts audit 2026-02-18)
- **[BUG-01] Shell env var injection** : `ShellTool::run()` expose secrets via `env` user params + héritage env parent. Fix : `cmd.env_clear()` + env whitelist + détection `$VAR` dans command string. (S)
- **[BUG-02] Gateway HTTP no-drain on shutdown** : `gateway_handle.abort()` tue axum sans laisser les requêtes se terminer. Fix : CancellationToken + axum graceful shutdown. (S)
- **[BUG-04] ShellTool allowlist contournée par shell operators** : `ls && rm -rf /` passe car `first_word == "ls"`. Fix : rejeter commandes avec `&&`, `||`, `;`, `|`. (S)
- **[BUG-07] Processus zombies sur timeout** : child process non tué quand timeout expire. Fix : SIGTERM → wait 5s → SIGKILL. (S)
- **Atomic session writes** : écriture tmp+rename pour tous les fichiers de session (JSON, SQLite WAL checkpoint atomique). (S)
- **Session file permissions `0o600`** : créer avec perms user-only. (XS)
- **Redaction tokens Telegram dans les logs tracing** : filtrer `bot_token` des messages tracing. (XS)

### 🟡 Bugs stabilité (découverts audit 2026-02-18)
- **[BUG-03] `message_thread_id` envoyé sur DMs Telegram** : `chat_type == "private"` → forcer `None`. (XS)
- **[BUG-05] Cron SQLite écrasé au démarrage** : charger SQLite first, n'insérer config TOML que pour les jobs absents. (XS)
- **[BUG-06] `guardd/credentials.rs` non intégré au runtime** : wirer dans `providers/mod.rs` comme fallback après env var. (S)

### Streaming (toujours priorité 1)
- Streaming Anthropic SSE (reqwest `response.bytes_stream()`). (L)
- Streaming OpenAI (`stream: true` + SSE parse). (L)
- Telegram draft previews (`streamMode: partial`) avec debounce 30-char. (L, dépend streaming provider)
- Slack `chat.startStream` / `appendStream` / `stopStream`. (L)

### Sécurité + Stabilité (OC 2026.2.17)
- URL allowlists pour `WebSearchTool` et `WebFetchTool` — config `tools.web.allowed_domains`. (S)
- Context window overflow guard — tronquer tool-results avant appel modèle. (M)
- Tool loop detection — circuit breaker à 30 appels no-progress identiques. (M)
- Channel health check + auto-restart watchdog — si polling channel silencieux, relancer. (M)

## Priority: MEDIUM

### Cron (OC 2026.2.17)
- Per-job webhook delivery — `delivery_webhook_url` dans `CronJob` struct. (S)
- Cron stagger — randomisation ±N secondes pour jobs top-of-hour. (M)
- `timeoutSeconds: 0` = no-timeout dans `execute_with_retry`. (S)
- Per-job model/provider override dans config. (M)
- Per-job usage telemetry dans `cron_history` (model, tokens, provider). (S)

### Auth + Providers (ZeroClaw post-pull)
- Auth profiles system — port de `zeroclaw/src/auth/` (profiles JSON chiffré, OAuth PKCE, token refresh, file locking). (L)
- OpenAI Codex provider (`chatgpt.com/backend-api/codex/responses` + SSE). (M, dépend auth profiles)
- Runtime model/provider switch `/model` + `/models` dans Telegram et Discord. (M)
- Sonnet 4.6 alias dans config par défaut (`anthropic/claude-sonnet-4-6`). (XS)
- 1M context opt-in — header `anthropic-beta: context-1m-2025-08-07`. (XS)
- DuckDuckGo fallback dans `WebSearchTool`. (S)

### Mémoire (OC 2026.2.17 + ZeroClaw)
- FTS5 SQLite pour mémoire + query expansion. (M)
- PostgreSQL memory backend — port de `zeroclaw/src/memory/postgres.rs`. (M)
- MMR re-ranking + temporal decay pour hybrid search. (M)

### Telegram (OC 2026.2.17)
- Inline buttons avec style (primary/success/danger). (M)
- Reaction notifications (`reactionNotifications: all/allowlist/off`). (M)
- Voice-note transcription + `getFile` retry avec backoff. (M)
- `setMyCommands` registration avec normalisation noms. (S)

### Observabilité
- Enrichir labels Prometheus : `provider`, `model`, `channel`, `direction`. (S)
- Tests E2E agent (port inspiration `zeroclaw/tests/agent_e2e.rs`). (M)
- Reply target field regression tests. (S)

## Priority: LOW
- Discord slash commands natifs avec options (`host/security/ask/node`). (L)
- Discord Components v2 (buttons, selects, modals). (L)
- ProxyConfigTool — gestion proxy HTTP runtime (port zeroclaw). (S)
- Milestone 6: doc complète (README/ARCHITECTURE/examples/CONTRIBUTING).
- Milestone 7: CI + release prep v0.1.0 (changelog, tag, notes).
- Channels additionnels : DingTalk, QQ, IRC, Matrix, Mattermost (zeroclaw parity). (XL)
- PostgreSQL memory backend. (M)

## TRIED
- Audit d'état complet + planification des milestones (BUILD_PLAN.md).
- Sprint B→K : Discord WS, Vector+RAG, tunnels, cron SQLite, Web UI, TUI live, multi-agent, skills, sessions, heartbeat → 142 tests.
- Sprint sécurité/tools/telemetry : gateway auth, channel router, security tests → 194 tests.
- Audit vs OpenClaw 2026.2.17 + ZeroClaw post-pull → AUDIT_VS_OPENCLAW_2026-02-18.md.
