# Audit Post-Fixes — 2026-02-18

> Auditeur : subagent rustclaw-audit-post-fixes
> Date : 2026-02-18 16:16 GMT+1
> Projet : `/Users/openclawbot/.openclaw/workspace/projects/rustclaw`

---

## ✅ Build

- **Status : OK** (exit code 0)
- **Warnings : 90** (uniquement `dead_code` — modules non encore branchés : tunnel, sessions, skills, vector)
- **Errors : 0**
- Compilé en mode `--release` sans erreur bloquante.

---

## ✅ Tests

- **Passés : 293 / 293**
- **Échoués : 0**
- **Ignorés : 0**
- Durée totale : ~10s

| Suite | Passés | Échoués |
|-------|--------|---------|
| lib (rustclaw) | 150 | 0 |
| bin (rustclaw main) | 111 | 0 |
| gateway_test | 9 | 0 |
| integration_test | 5 | 0 |
| security_test | 8 | 0 |
| tools_test | 10 | 0 |
| **TOTAL** | **293** | **0** |

---

## 🔧 Clippy (linting)

- **Erreurs bloquantes : 0**
- **Warnings clippy : 131** (quasi-exclusivement `dead_code` — même source que le build)
- Aucune suggestion `clippy::correctness` ou `clippy::suspicious`

---

## 🔴 Bugs Critiques — Status

| Bug | Description | Fix présent | Test | Statut |
|-----|-------------|------------|------|--------|
| BUG-01 | Env injection via héritage | ✅ `cmd.env_clear()` + `SENSITIVE_ENV_PATTERNS` | ✅ `secrets_not_accessible_via_env`, `rejects_sensitive_env_references` | ✅ FIXÉ |
| BUG-02 | Graceful shutdown axum | ✅ `CancellationToken` in main.rs + `with_graceful_shutdown` in gateway | ✅ `gateway::tests::status_endpoint_works` | ✅ FIXÉ |
| BUG-03 | `message_thread_id` sur DMs Telegram | ✅ `chat_type == "private"` → `None` | ✅ `send_message_body_omits_thread_id_for_private_chat` | ✅ FIXÉ |
| BUG-04 | Shell operator bypass (&&, \|\|, ;, \|, backtick) | ✅ `DANGEROUS_OPERATORS` array + détection backtick | ✅ 6 tests `rejects_shell_operator_*` | ✅ FIXÉ |
| BUG-05 | Cron upsert écrase la DB SQLite | ✅ `list_jobs()` AVANT `upsert_job()`, upsert seulement si DB vide | ✅ `config_jobs_do_not_overwrite_existing` | ✅ FIXÉ |
| BUG-06 | CredentialStore vs env::var order | ⚠️ CredentialStore consulté MAIS après env::var (ordre inversé vs spec) | ❌ pas de test d'ordre explicite | ⚠️ PARTIEL |
| BUG-07 | SIGKILL direct sans SIGTERM sur timeout | ✅ SIGTERM → sleep 2s → SIGKILL | ✅ `timeout_kills_process` | ✅ FIXÉ |

### Détail BUG-06

```rust
// auth/mod.rs — commentaire DIT "BUG-06" mais ordre est env::var FIRST :
pub fn resolve_api_key(env_var: &str, cred_name: &str) -> anyhow::Result<String> {
    // Try environment variable first  ← ENV FIRST (contraire au spec)
    if let Ok(val) = std::env::var(env_var) { return Ok(val); }
    // Fallback to credential store    ← CredentialStore SECOND
```

La spec demande : **CredentialStore avant env::var**. Le code fait l'inverse.
Impact sécurité : mineur (env::var peut être injecté si l'environnement est compromis, mais CredentialStore reste consulté en fallback).

---

## 🟡 Nouvelles Features — Status

| Feature | Code présent | Fichier | Test | Statut |
|---------|-------------|---------|------|--------|
| URL allowlists | ✅ `allowed_domains: Vec<String>` | `src/tools/web.rs:11,16` | ✅ 6 tests `domain_*` | ✅ |
| DuckDuckGo fallback | ✅ `ddg_search()` + fallback si pas de BRAVE_API_KEY | `src/tools/web.rs:74` | ✅ `web_search_tool_falls_back_to_ddg` | ✅ |
| Cron webhook | ✅ `delivery_webhook_url: Option<String>` dans `CronJob` | `src/config.rs:228` | ✅ `job_with_webhook_url` | ✅ |
| Cron stagger | ✅ `stagger_seconds: Option<u64>` + `compute_stagger_delay()` | `src/config.rs:231`, `src/cron/mod.rs:89` | ✅ 4 tests `stagger_*` | ✅ |
| Cron timeout=0 | ✅ `Some(0)` = no limit (`match` sur `timeout_seconds`) | `src/cron/mod.rs:111` | ✅ `resolve_timeout_zero_means_no_timeout` | ✅ |
| Context overflow guard | ✅ `guard_context_overflow()` tronque les tool results | `src/agent/context_guard.rs` | ✅ 4 tests context_guard | ✅ |
| Tool loop detection | ✅ `LoopDetector` + circuit breaker `BREAK_THRESHOLD` | `src/agent/loop_detector.rs` | ✅ 6 tests loop_detector | ✅ |
| FTS5 mémoire | ✅ `USING fts5` + LIKE fallback si FTS5 indispo | `src/memory/fts.rs:25`, `src/memory/mod.rs:111` | ✅ 4 tests fts5_* | ✅ |
| Channel watchdog | ✅ `HealthRegistry` + `start_watchdog()` | `src/channels/watchdog.rs` | ✅ 5 tests watchdog | ✅ |
| Sonnet 4.6 alias | ✅ `"claude-sonnet-4-6" => "claude-sonnet-4-20250514"` | `src/providers/mod.rs:191` | ✅ `sonnet_46_alias_resolves` | ✅ |
| 1M context header | ✅ `header("anthropic-beta", "context-1m-2025-08-07")` optionnel | `src/auth/anthropic.rs:83` | ✅ `context_1m_header_needed_for_claude4` | ✅ |

---

## 🔐 Fichiers Clés — Sécurité

| Check | Présent | Fichier | Détail |
|-------|---------|---------|--------|
| Permissions 0o600 | ✅ | `src/setup/mod.rs:287`, `src/guardd/credentials.rs:158` | Config files + credential store |
| Atomic writes (tmp→rename) | ✅ | `src/guardd/credentials.rs:149-162` | Write to `.json.tmp` puis `fs::rename()` |
| Redaction secrets en logs | ✅ | `src/telemetry/mod.rs:157-166` | `redact_secrets()` couvre ANTHROPIC_API_KEY, TELEGRAM_BOT_TOKEN, SLACK_BOT_TOKEN, DISCORD_BOT_TOKEN |

---

## ⚠️ Problèmes trouvés

### 🔴 Critique
_(aucun)_

### 🟡 Modéré
1. **BUG-06 ordre inversé** : `resolve_api_key()` vérifie `env::var` AVANT `CredentialStore`. Le spec demandait l'inverse (CredentialStore en priorité). Impact : si l'environnement est compromis par injection, l'attaquant peut supplanter le credential store.

### 🟠 Mineur
2. **Dead code massif (90 warnings)** : Les modules `tunnel/`, `sessions/`, `skills/`, `memory/vector/` contiennent de nombreux items publics non utilisés. Pas de problème fonctionnel, mais indique que ces modules sont scaffoldés mais pas intégrés.
3. **FTS5 fallback architecturalement splitté** : `fts.rs` log "falling back to LIKE" mais la vraie logique LIKE est dans `memory/mod.rs`. Cohérent mais source de confusion future.
4. **`redact_secrets()` déclarée mais inutilisée** (warning clippy) : La fonction existe et est testée mais pas appelée dans les hot paths de logging (`tracing` macros). Les secrets pourraient donc apparaître dans des logs si `tracing` est utilisé directement.

---

## 📊 Score global

- **Bugs fixés : 6.5 / 7** (BUG-06 partiellement correct)
- **Features implémentées : 11 / 11** (100% ✅)
- **Tests : 293 / 293 passés** (100% ✅)
- **0 erreur de build, 0 erreur clippy**

### Prêt pour prod : **PRESQUE** (OUI sous conditions)

**À faire avant production :**
1. Corriger l'ordre BUG-06 : CredentialStore en priorité, env::var en fallback
2. Brancher `redact_secrets()` dans le subscriber tracing (middleware de log)
3. Nettoyer le dead code ou marquer les items avec `#[allow(dead_code)]` intentionnellement

**OK pour prod si :**
- Environnement de déploiement est sécurisé (BUG-06 mitigé)
- Logs ne contiennent pas de secrets (vérifier config tracing)

---

*Rapport généré par subagent rustclaw-audit-post-fixes — 2026-02-18*
