# Rustclaw Audit — vs OpenClaw 2026.2.17 + ZeroClaw (post-pull)
Date: 2026-02-18  
Sources: FULL_AUDIT_2026-02-17.md, GAP_ANALYSIS_ZEROCLAW.md, OpenClaw CHANGELOG 2026.2.17, ZeroClaw src/ (commit post-pull avec nouveaux fichiers)

---

## Résumé exécutif

Rustclaw est en bonne forme structurelle : build ✅, 194 tests ✅, gateway HTTP opérationnel, channels Telegram/WhatsApp/Slack/Discord/Signal present. Les scores audit 2026-02-17 post-sprint étaient Feature 9.0, Security 9.0, Test 9.0, Prod-readiness 8.5.

**Ce qui a changé depuis l'audit précédent :**
- OpenClaw 2026.2.17 introduit ~12 nouveaux gaps sécurité/stabilité non encore portés dans rustclaw
- ZeroClaw a reçu un gros pull avec 5+ nouveaux modules majeurs (auth profiles, OpenAI Codex OAuth, PostgreSQL memory, Prometheus observability, proxy config tool, runtime provider/model switching)
- Ces nouveaux éléments créent ~25 gaps nouveaux à tracker

**Priorités immédiates :**
1. 🔴 SÉCURITÉ : 3 bugs réels trouvés dans le code rustclaw actuel (voir §Bugs)
2. 🔴 AUTH : Le système auth de ZeroClaw (OAuth profiles + token refresh) est maintenant une référence de production — rustclaw est resté à env-var only
3. 🟡 STREAMING : toujours absent, OpenClaw en fait une feature first-class sur tous les canaux
4. 🟡 CRON : 5 nouveaux gaps depuis le changelog 2026.2.17

---

## 🔴 NOUVEAUTÉS OpenClaw 2026.2.17 — Gaps Rustclaw

### 1. Sécurité (CRITIQUE)

| Item | OpenClaw fix | Rustclaw | Effort |
|------|-------------|----------|--------|
| **OC-09 : env var injection dans exec** | Preflight guard détecte `$DM_JSON`, `$TMPDIR`, etc. dans scripts Python/Node avant exécution | ❌ **BUG ACTIF** — `ShellTool` passe les `env` user directement + hérite l'env parent avec tous les secrets | S |
| **`$include` path traversal/symlink** | Confine à config dir, hardening cross-platform | ⚪ N/A (pas de `$include` dans rustclaw) | — |
| **Session files `0o600`** | Crée nouveaux JSONL sessions avec perms user-only | ❌ Rustclaw ne force pas les permissions sur ses fichiers de session SQLite/JSON | XS |
| **Webhook signature (Telegram secret token)** | Vérification du `X-Telegram-Bot-Api-Secret-Token` header | ✅ Implémenté dans le sprint sécurité (LEARNINGS 2026-02-18 dernier) | — |
| **Webhook signature (WhatsApp HMAC)** | Obligatoire en prod | ✅ Optionnel (bypass si pas de `app_secret`) — acceptable | — |
| **Webhook signature (Slack HMAC)** | Obligatoire en prod | ✅ Enforced depuis sprint sécurité | — |
| **Redaction Telegram bot token des logs/stack traces** | OC 2026.2.15 fix | ❌ Pas de redaction dans les logs tracing rustclaw | XS |

### 2. Stabilité / Résistance

| Item | OpenClaw fix | Rustclaw | Effort |
|------|-------------|----------|--------|
| **SIGTERM avant SIGKILL** | `process.kill()` envoie SIGTERM puis SIGKILL après grace period | ❌ **BUG** : `ShellTool` timeout → `tokio::time::timeout` expire → `child.wait()` abandonne sans kill SIGTERM | S |
| **Atomic session-store writes** | tmp-file + rename atomique pour éviter corruption sur crash | ❌ Rustclaw écrit directement (risque de fichier corrompu si crash mid-write) | S |
| **Context window overflow guard** | Tronque proactivement les tool-results oversized avant appel modèle | ❌ Absent — si tool output > context window, le provider rejette avec une erreur opaque | M |
| **Cron spin-loop prevention** | Avance `nextRun` après completion du job + gap minimum entre refires | ❌ Risque de spin si job se termine dans la même seconde — dépend de `tokio-cron-scheduler` mais non durci explicitement | S |
| **`timeoutSeconds: 0` = no-timeout** | `0` = sans limite (pas clampé à 1) | ❌ Rustclaw n'expose pas de `timeoutSeconds` par job cron — timeout hardcodé dans `execute_with_retry` sans option 0 | S |
| **Gateway HTTP drain on shutdown** | Laisse les requêtes in-flight se terminer avant d'arrêter | ❌ **BUG** : `gateway_handle.abort()` dans `gateway_start()` tue le serveur axum immédiatement sans drain | S |
| **`channelHealthCheckMinutes` config** | Redémarre auto les channels qui cessent de répondre | ❌ Absent — si le polling Telegram plante silencieusement, rien ne le relance | M |
| **Loop detection pour tool calls** | Bloque les boucles ping-pong tool → résultat identique | ❌ Absent — un modèle peut looper indéfiniment sur le même tool call | M |
| **Defer transient error snapshots** | Grace window avant résolution `agent.wait` pour éviter early-resolve sur retry/failover | ❌ Absent (gateway moins mature) | S |

### 3. Streaming

| Item | OpenClaw 2026.2.17 | Rustclaw | Effort |
|------|-------------------|----------|--------|
| **Streaming Anthropic SSE** | Actif par défaut | ❌ Absent — completion non-streaming uniquement | L |
| **Streaming OpenAI** | Actif par défaut | ❌ Absent | L |
| **Telegram streaming (draft previews)** | `streamMode: partial/full/off` + preview dans le même message | ❌ Absent — réponse entière envoyée une fois | L |
| **Telegram streaming debounce (30-char threshold)** | Attend 30 chars avant premier preview pour éviter spam notifications | ❌ Absent (dépend du streaming) | XS |
| **Telegram `streamMode: off` disable** | Empêche le splitting en messages multiples | ❌ Absent | S |
| **Slack streaming** | `chat.startStream` / `appendStream` / `stopStream` | ❌ Absent | L |
| **Z.AI `tool_stream`** | `tool_stream: true` par défaut | ❌ Absent (pas de provider Z.AI) | L |

### 4. Telegram

| Item | OpenClaw 2026.2.17 | Rustclaw | Effort |
|------|-------------------|----------|--------|
| **Inline button `style` (primary/success/danger)** | Schema + parsing + runtime | ❌ Absent — pas de boutons inline du tout | M |
| **User reaction notifications** | `reactionNotifications: all/allowlist/off` | ❌ Absent | M |
| **IPv4 fallback (`autoSelectFamily`)** | Auto-activé sur Node.js 22+ pour réseaux IPv6 cassés | ❌ Absent — reqwest utilise le resolver OS ; pas de fallback explicite | S |
| **Voice-note transcription** | CLI fallback handling sur DMs | ❌ Absent | M |
| **`message_thread_id` omis pour DMs** | Fix critique : évite `400 Bad Request: message thread not found` | ❌ **BUG** : rustclaw passe `message_thread_id` sans vérifier si c'est un DM | XS |
| **`setMyCommands` menu registration** | Normalize command names (`-` → `_`) | ❌ Absent | S |
| **`channel_post` inbound** | Support channel-based bot triggers | ❌ Absent | S |
| **Retry `getFile` avec backoff** | 3 tentatives + fallback gracieux pour media oversized | ❌ Absent — si `getFile` échoue, le message est dropped | S |
| **Sticker/poll send** | Actions message complètes | ❌ Absent | M |
| **Reply threading sticky across stream chunks** | `replyToId` préservé sur tous les chunks | ❌ Absent (streaming absent) | XS |

### 5. Cron

| Item | OpenClaw 2026.2.17 | Rustclaw | Effort |
|------|-------------------|----------|--------|
| **Per-job webhook delivery** (`delivery.mode = "webhook"`) | URLs distinctes par job, validation HTTP(S) | ❌ Partiel — seulement `CRON_WEBHOOK_URL` global via env | S |
| **Stagger scheduling** (top-of-hour anti-thundering-herd) | Auto-stagger + `--stagger <duration>` + `--exact` | ❌ Absent | M |
| **Per-job usage telemetry** | Log model/provider/tokens par run dans cron history | ❌ Absent — history stocke juste status/output | S |
| **`timeoutSeconds: 0` = no timeout** | 0 = pas de limite, pas de clamp à 1 | ❌ Absent — timeout non configurable par job | S |
| **Per-job model override** | Chaque job peut spécifier son modèle/provider | ❌ Absent | M |
| **`accountId` resolve depuis agent bindings** | Résout le compte pour sessions isolées | ❌ Absent (pas de multi-agent/multi-account) | M |
| **Schedule-error isolation** | Erreur d'un job n'aborte pas la persistence des autres | ❌ Non vérifié — store SQLite partagé pourrait propager | S |

### 6. Mémoire

| Item | OpenClaw 2026.2.17 | Rustclaw | Effort |
|------|-------------------|----------|--------|
| **FTS fallback + query expansion** | Full-text search SQLite FTS5 + expansion si peu de résultats | ❌ Absent — `LIKE '%query%'` seulement, pas de FTS | M |
| **Unicode-aware FTS (CJK)** | `buildFtsQuery` Unicode-aware | ❌ Absent | S |
| **MMR re-ranking** | Maximal Marginal Relevance pour diversity hybrid search | ❌ Absent | M |
| **Temporal decay scoring** | Scoring hybride avec half-life configurable | ❌ Absent | M |
| **Vector memory production** | Embeddings + cosine similarity | ✅ Implémenté (`src/memory/vector.rs`) mais non wired dans le chat path par défaut | S |

### 7. Providers

| Item | OpenClaw 2026.2.17 | Rustclaw | Effort |
|------|-------------------|----------|--------|
| **1M context opt-in** | `params.context1m: true` → header `anthropic-beta: context-1m-2025-08-07` | ❌ Absent | XS |
| **Per-model `thinkingDefault` overrides** | Override thinking per modèle dans config | ❌ Absent | S |
| **`llms.txt` discovery** | Auto-découverte des capacités LLM d'un serveur | ❌ Absent | M |
| **Sonnet 4.6 alias** | `anthropic/claude-sonnet-4-6` avec fallback compat | ❌ Absent — model string hardcodé dans config | XS |
| **Model auth cooldown recovery** | Probe primary quand cooldown expire | ❌ Absent — si Anthropic revient, rustclaw ne bascule pas | M |
| **Failover sur abort stop-reason** | Classify `stop reason: abort` comme timeout-class → trigger fallback | ❌ Absent | S |

### 8. Gateway / HTTP

| Item | OpenClaw 2026.2.17 | Rustclaw | Effort |
|------|-------------------|----------|--------|
| **`channelHealthCheckMinutes`** | Validation dans config strict, auto-restart hardened | ❌ Absent | M |
| **Channel auto-restart hardening** | Preserve restart caps, propagate enabled/configured flags | ❌ Absent — si un channel crache, son task Tokio meurt silencieusement | M |
| **`chat.history` byte limit cap** | Tronque les historiques oversized, évite freeze UI | ❌ Absent | S |
| **`config.patch` partial array merge** | Gère les updates partiels d'arrays sans écraser | ❌ Absent (pas de `config.patch` endpoint) | M |
| **IPv6 Host header normalization** | Préserve `[::1]` dans Host sans double-bracket | ❌ Absent — axum gère par défaut mais non vérifié | XS |

### 9. Outils / Tools

| Item | OpenClaw 2026.2.17 | Rustclaw | Effort |
|------|-------------------|----------|--------|
| **URL allowlists pour `web_search`/`web_fetch`** | Liste blanche configurable de domaines autorisés | ❌ Absent — `WebSearchTool` et `WebFetchTool` requêtent n'importe quelle URL | S |
| **`read` auto-page avec context budget** | Scale le per-call output budget sur `contextWindow` | ❌ Absent — `FileTool` retourne tout sans pagination adaptative | M |
| **Tool loop detection** | Circuit breaker à 30 repeats no-progress | ❌ Absent | M |
| **`message` tool scoped au channel** | Telegram = `buttons`, Discord = `components` | ❌ Absent (pas de message tool générique) | M |
| **Browser `extraArgs`** | Custom Chrome launch args | ❌ Absent dans `BrowserTool` | S |
| **Non-zero exit codes comme succès** | Exit code non-0 = completed (pas error) | ❌ **BUG** : `ShellTool` retourne `exit_code` dans JSON mais ne distingue pas "erreur process" de "erreur outil" | XS |

### 10. Discord

| Item | OpenClaw 2026.2.17 | Rustclaw | Effort |
|------|-------------------|----------|--------|
| **Native `/exec` slash command options** | `host/security/ask/node` avec autocomplete | ❌ Absent — Discord rustclaw = REST polling sans slash commands | L |
| **Reusable interactive components** | `components.reusable=true` | ❌ Absent | L |
| **Per-button `allowedUsers`** | Allowlist par bouton | ❌ Absent | M |
| **Components v2** (buttons, selects, modals) | Depuis 2026.2.15 | ❌ Absent | L |
| **HTTP proxy config** | Proxy configurable pour REST resolution | ❌ Absent | S |
| **`allowFrom` avec préfixes** | `user:` / `discord:` normalization | ❌ Absent | S |

---

## 🆕 Nouveautés ZeroClaw post-audit — Gaps supplémentaires

Ces features sont dans le ZeroClaw récupéré depuis GitHub et absentes de `GAP_ANALYSIS_ZEROCLAW.md`.

### A. Système Auth Profiles (MAJEUR — `src/auth/`)

ZeroClaw implémente un système d'auth complet inspiré d'OpenClaw :

| Composant | ZeroClaw | Rustclaw |
|-----------|----------|----------|
| Named auth profiles (token + OAuth) | ✅ `AuthProfilesStore` avec JSON chiffré | ❌ Env vars seulement |
| OpenAI Codex OAuth (PKCE + device code) | ✅ `openai_oauth.rs` — PKCE flow, loopback callback, device code polling | ❌ Absent |
| Token refresh automatique avec backoff | ✅ `get_valid_openai_access_token` + refresh lock + backoff | ❌ Absent |
| File locking pour concurrent access | ✅ `auth-profiles.lock` avec timeout | ❌ Absent |
| Atomic writes (tmp → rename) | ✅ `write_persisted_locked` | ❌ Absent |
| Encryption at-rest | ✅ `SecretStore` avec migration `enc2:` | ✅ `guardd/credentials.rs` AES-GCM — mais non intégré au runtime |
| Multi-provider (anthropic, openai-codex, custom) | ✅ `normalize_provider()` | ❌ 2 providers hardcodés |
| Profile selection (override → active → default) | ✅ `select_profile_id()` | ❌ Absent |

**Effort pour porter l'essentiel : L**

### B. OpenAI Codex Provider (`src/providers/openai_codex.rs`)

ZeroClaw appelle `https://chatgpt.com/backend-api/codex/responses` avec :
- SSE streaming parsé pour extraire le texte
- Gestion `chatgpt-account-id` header (extrait du JWT)
- Clamp du reasoning effort par model (`gpt-5.1-codex-mini`, `gpt-5.2`, `gpt-5.3`)
- `ZEROCLAW_CODEX_REASONING_EFFORT` env var

**Rustclaw** : provider absent. Effort : **M** (dépend du système auth OAuth).

### C. Runtime Provider/Model Switching dans les Channels (`src/channels/mod.rs` +803 lignes)

ZeroClaw expose dans Telegram et Discord :
- `/models` — liste les providers disponibles
- `/models <provider>` — switch de provider en live
- `/model` — affiche le modèle actuel
- `/model <model>` — switch de modèle en live

Avec `RouteSelectionMap` (DashMap per-channel/sender) + `ProviderCacheMap` pour réutiliser les instances.

**Rustclaw** : aucun runtime switch. Modèle hardcodé au démarrage. Effort : **M**

### D. ZeroClaw Multi-Provider Ecosystem (`src/providers/mod.rs` +281 lignes)

ZeroClaw ajoute :
- **MiniMax OAuth** (global/CN) avec token refresh et multiple aliases (`minimax`, `minimax-cn`, `minimax-oauth`, etc.)
- **GLM/Zhipu** (global/CN via z.ai)  
- **Moonshot/Kimi** (global/CN)
- **Qwen/DashScope** (CN/Intl/US)
- **Z.AI Coding** endpoint  
- **Qianfan/Baidu** provider

Rustclaw a seulement Anthropic + OpenAI. Ces providers sont surtout pertinents pour les marchés CN/Asia.
**Effort pour les providers essentiels (OpenRouter, Ollama, Gemini) : M**

### E. PostgreSQL Memory Backend (`src/memory/postgres.rs`)

ZeroClaw ajoute un backend PostgreSQL `Memory` trait-compatible avec :
- Schema auto-init avec indexes
- ILIKE keyword search avec scoring (`key` match = 2.0, `content` match = 1.0)
- `session_id` filtering
- Connection timeout cap (300s max)
- `parking_lot::Mutex` pour thread safety
- Identifier validation anti-injection SQL
- Health check via `SELECT 1`

**Rustclaw** : SQLite uniquement. Effort pour PostgreSQL : **M**

### F. Prometheus Observability (`src/observability/prometheus.rs`)

ZeroClaw a un `PrometheusObserver` complet :
- Counters : `agent_starts`, `tool_calls`, `channel_messages`, `heartbeat_ticks`, `errors`
- Histograms : `agent_duration`, `tool_duration`, `request_latency`
- Gauges : `tokens_used`, `active_sessions`, `queue_depth`
- Endpoint `/metrics` pour scraping

**Rustclaw** : `src/telemetry/mod.rs` implémenté (counters/gauges/histograms/Prometheus export).  
**Écart** : labels zeroclaw plus riches (provider, model, tool, channel, direction). **Gap : S**

### G. ProxyConfigTool (`src/tools/proxy_config.rs`)

ZeroClaw expose un outil qui permet à l'agent de lire/écrire la config proxy HTTP à runtime, avec :
- Scopes (provider, channel, tool)
- Security policy gate (read-only autonomy check + rate limit)
- Persistence dans config file

**Rustclaw** : absent. Effort : **S** (utile surtout en entreprise derrière proxy)

### H. WebSearchTool avec DuckDuckGo (`src/tools/web_search_tool.rs`)

ZeroClaw a un `WebSearchTool` multi-provider (DuckDuckGo + Brave) :
- DuckDuckGo free (HTML scraping + DDG redirect decode)
- Brave avec API key
- `max_results` clamped 1-10, timeout configurable
- `parameters_schema()` pour injection dans prompt LLM

**Rustclaw** `WebSearchTool` : Brave uniquement, pas de fallback DuckDuckGo, pas de `parameters_schema()`.  
**Gap : S**

### I. Tests E2E et Regression (`tests/agent_e2e.rs`, `tests/reply_target_field_regression.rs`)

ZeroClaw a des tests E2E agent et des tests de régression sur `reply_target` field.

**Rustclaw** : 5 tests d'intégration basiques. Pas de tests E2E agent ni de régression sur reply threading. **Gap : M**

---

## 🐛 Bugfixes Prioritaires

Ces bugs sont dans le code rustclaw actuel — pas des features manquantes.

### BUG-01 🔴 Shell env var credential theft (OC-09 équivalent)
**Fichier** : `src/tools/shell.rs`, méthode `run()`  
**Description** : La struct `ShellReq` accepte un `env: Option<HashMap<String, String>>` qui est passé directement à `cmd.env(k, v)`. Problème double :
1. La commande hérite l'environment parent complet (`ANTHROPIC_API_KEY`, `TELEGRAM_BOT_TOKEN`, etc.)
2. Un attaquant (ou un modèle trompé) peut injecter `{"command": "echo $ANTHROPIC_API_KEY"}` sans même passer d'env override

**Fix requis** :
```rust
// Avant d'exécuter :
// 1. Détecter les références shell $VAR dans la commande
if req.command.contains("$ANTHROPIC") || req.command.contains("$TELEGRAM") ... {
    return Err(anyhow!("Potential credential injection detected"));
}
// 2. Utiliser cmd.env_clear() + env whitelist explicite
cmd.env_clear();
cmd.env("PATH", std::env::var("PATH").unwrap_or_default());
cmd.env("HOME", std::env::var("HOME").unwrap_or_default());
```
**Effort** : S | **Criticité** : 🔴 CRITIQUE

### BUG-02 🔴 Gateway HTTP server abort sans drain
**Fichier** : `src/main.rs`, fonction `gateway_start()`  
**Description** : `gateway_handle.abort()` à la ligne de shutdown appelle `JoinHandle::abort()` qui annule le task Tokio immédiatement. Les requêtes HTTP en cours (ex : appel provider en flight) perdent leur contexte et génèrent des réponses corrompues ou des erreurs 500 côté client.

**Fix requis** :
```rust
// Envoyer un signal shutdown propre via CancellationToken ou oneshot channel
// Laisser axum drainer avec tower's graceful shutdown:
server.with_graceful_shutdown(async { shutdown_rx.await.ok(); }).await?;
```
**Effort** : S | **Criticité** : 🔴 CRITIQUE (prod)

### BUG-03 🟡 `message_thread_id` envoyé sur les DMs Telegram
**Fichier** : `src/channels/telegram.rs`  
**Description** : `SendMessageBody` inclut `message_thread_id` quand il est présent dans le message original. Sur les DMs, ça génère `400 Bad Request: message thread not found`. OpenClaw a dû fixer ce même bug (fix dans 2026.2.15).

**Fix requis** : Vérifier `chat.chat_type == "private"` et forcer `message_thread_id = None` pour les DMs.  
**Effort** : XS | **Criticité** : 🟡 HIGH (Telegram DMs cassés si thread_id dans le contexte)

### BUG-04 🟡 ShellTool allowlist contournée par shell operators
**Fichier** : `src/tools/shell.rs`, méthode `is_allowed()`  
**Description** : La vérification prend seulement le premier mot : `"ls && rm -rf /"` passe car `first_word = "ls"`. L'allowlist est contournée par `&&`, `||`, `;`, `|`.

**Fix requis** : Parser la commande shell complète ou exiger un mode no-shell (`Command::new("ls")` sans `sh -c`). Alternativement, rejeter toute commande contenant des opérateurs shell dangereux.  
**Effort** : S | **Criticité** : 🟡 HIGH

### BUG-05 🟡 Cron jobs config rechargés sur chaque démarrage (écrase SQLite)
**Fichier** : `src/cron/mod.rs`, `start_scheduler()`  
**Description** : 
```rust
for j in &cfg.cron.jobs { store.upsert_job(j)?; }
```
Chaque redémarrage du daemon remet à jour tous les jobs depuis la config TOML vers SQLite. Si un job a été modifié en live via l'API (ex: `/api/cron`), les modifications sont perdues au prochain redémarrage.

**Fix requis** : Charger depuis SQLite en premier ; n'insérer depuis config que les jobs qui n'existent pas encore dans le store.  
**Effort** : XS | **Criticité** : 🟡 MEDIUM

### BUG-06 🟡 `auth/mod.rs::resolve_auth` non utilisé
**Fichier** : `src/auth/mod.rs`  
**Description** : La fonction `resolve_auth` (pointée dans l'audit précédent) reste non wired au runtime. Le credential store `guardd/credentials.rs` (AES-GCM) n'est jamais consulté pour résoudre les API keys — tout passe par `std::env::var()`. Le credential store existe mais est une dead code.

**Fix requis** : Intégrer `credentials::CredentialStore` dans `providers/mod.rs::AnthropicProvider::complete()` et `OpenAiProvider::complete()` comme fallback après `std::env::var`.  
**Effort** : S | **Criticité** : 🟡 MEDIUM

### BUG-07 🟡 Process termination sans SIGTERM
**Fichier** : `src/tools/shell.rs`  
**Description** : En cas de timeout, `tokio::time::timeout` expire et le `Command` spawné continue à tourner en background (Tokio ne kill pas le child process automatiquement à l'abandon du future). Résultat : processus zombies.

**Fix requis** :
```rust
match tokio::time::timeout(timeout, cmd.output()).await {
    Err(_) => {
        // Kill explicitement
        let _ = child.kill().await;  // SIGKILL
        // Idéalement : SIGTERM wait 5s puis SIGKILL
        return Err(anyhow!("timeout"));
    }
    Ok(out) => out?
}
```
**Effort** : S | **Criticité** : 🟡 MEDIUM

---

## 📊 Matrice de Parité Mise à Jour

| Feature | OpenClaw 2026.2.17 | ZeroClaw (post-pull) | Rustclaw | Delta |
|---------|---------------------|----------------------|----------|-------|
| Provider routing + fallback | ✅ Rich | ✅ Rich + OAuth | ✅ Basic (2 providers) | -Multi-provider |
| Auth profiles + OAuth | ✅ Full | ✅ Full | ❌ Env only | -Majeur |
| OpenAI Codex provider | ✅ | ✅ Implémenté | ❌ | -Provider |
| MiniMax/GLM/Kimi/Qwen | ✅ | ✅ Implémenté | ❌ | -Providers CN |
| Streaming (Anthropic/OpenAI) | ✅ | ✅ | ❌ | -Streaming |
| Telegram streaming | ✅ draft previews | ✅ | ❌ | -Streaming |
| Telegram inline buttons | ✅ style | Partiel | ❌ | -UI |
| Slack streaming | ✅ startStream | ❌ | ❌ | -Streaming |
| Discord Components v2 | ✅ | ✅ | ❌ | -Discord |
| Gateway auth (bearer token) | ✅ | ✅ | ✅ | = |
| Rate limiting | ✅ | ✅ | ✅ | = |
| CORS | ✅ | ✅ | ✅ | = |
| Channel webhook signatures | ✅ Enforced | ✅ | ✅ | = |
| SQLite memory | ✅ | ✅ | ✅ | = |
| PostgreSQL memory | ✅ QMD | ✅ | ❌ | -Backend |
| Vector memory | ✅ | ✅ | ✅ Basic | ≈ |
| FTS memory search | ✅ FTS5 + expand | ✅ | ❌ LIKE only | -Search |
| Prometheus metrics | ✅ | ✅ Rich labels | ✅ Basic | ≈ |
| Cron retries + history | ✅ | ✅ | ✅ | = |
| Cron per-job webhook | ✅ | ✅ | ❌ Global only | -Cron |
| Cron stagger | ✅ | ✅ | ❌ | -Cron |
| Cron per-job model override | ✅ | ✅ | ❌ | -Cron |
| Shell tool guardd | ✅ | ✅ | ✅ | = |
| Shell env var injection fix | ✅ OC-09 | ✅ | ❌ BUG | -Security |
| SIGTERM before SIGKILL | ✅ | ✅ | ❌ BUG | -Stability |
| Atomic session writes | ✅ | ✅ | ❌ | -Stability |
| Context window overflow guard | ✅ | ✅ | ❌ | -Stability |
| URL allowlists web tools | ✅ | ✅ | ❌ | -Security |
| DuckDuckGo fallback search | ✅ | ✅ | ❌ | -Tools |
| Runtime model switch (/model) | ✅ | ✅ Telegram+Discord | ❌ | -UX |
| Channel health check + restart | ✅ | ✅ | ❌ | -Ops |
| Tool loop detection | ✅ Circuit breaker | ✅ | ❌ | -Safety |
| 1M context opt-in | ✅ | Partiel | ❌ | -Providers |
| TUI live data | ✅ | ✅ | ✅ | = |
| Web UI dashboard | ✅ | ✅ | ✅ Basic | ≈ |
| Discord/Signal channels | ✅ | ✅ | ✅ REST polling | ≈ |
| DingTalk/QQ/IRC/Matrix | ✅ | ✅ | ❌ | -Channels |
| Multi-agent | ✅ | ✅ | ✅ Basic | ≈ |
| Skills system | ✅ | ✅ | ✅ | = |
| Heartbeat | ✅ | ✅ | ✅ | = |
| Tunnel providers | ✅ Ngrok/CF/TS | ✅ | ✅ | = |
| iOS / Share extension | ✅ | ❌ | ❌ | N/A |

---

## 🎯 Plan d'Action — Priorités

### Sprint immédiat (cette semaine) — Sécurité + Bugs bloquants

1. **[BUG-01]** Fixer env var injection dans ShellTool — `env_clear()` + allowlist env + détection `$VAR` dans command string (S)
2. **[BUG-02]** Graceful drain du gateway HTTP sur shutdown — CancellationToken + axum graceful (S)
3. **[BUG-03]** Fixer `message_thread_id` pour DMs Telegram (XS)
4. **[BUG-04]** Rejeter les shell operators dans allowlist check (S)
5. **[BUG-07]** SIGTERM → SIGKILL avec grace period dans ShellTool (S)
6. **Atomic session writes** — écriture tmp+rename pour les fichiers de session (S)
7. **Session file permissions `0o600`** — créer les fichiers avec les bonnes perms (XS)
8. **Redaction des tokens dans les logs** — filter dans le tracing subscriber (XS)

### Ce mois — Stabilité + Features prioritaires

1. **[BUG-05]** Cron : charger SQLite first, ne pas écraser les jobs live (XS)
2. **[BUG-06]** Intégrer `guardd/credentials.rs` dans le chemin auth runtime (S)
3. **URL allowlists** pour `WebSearchTool` et `WebFetchTool` — config `tools.web.allowed_domains` (S)
4. **DuckDuckGo fallback** dans WebSearchTool (S)
5. **`timeoutSeconds: 0 = no-timeout`** dans cron execute_with_retry (S)
6. **Cron per-job webhook** — ajouter `delivery_webhook_url` dans `CronJob` struct (S)
7. **Cron stagger** — randomisation dans la fenêtre ±N secondes pour les jobs top-of-hour (M)
8. **Context window overflow guard** — tronquer les tool-results avant appel provider (M)
9. **Tool loop detection** — détecter les appels identiques répétés, circuit breaker à N (M)
10. **Sonnet 4.6 alias** dans la config par défaut (XS)
11. **1M context opt-in** — header `anthropic-beta: context-1m-2025-08-07` optionnel (XS)
12. **FTS SQLite** pour la mémoire — activer FTS5 + fallback sur `LIKE` (M)
13. **Channel health check + restart** — watchdog task qui vérifie si le polling channel répond (M)

### Long terme (ce trimestre)

1. **Streaming Anthropic SSE + OpenAI** — prérequis pour Telegram draft previews (L)
2. **Telegram streaming** avec debounce et `streamMode` (L, dépend du streaming provider)
3. **Système Auth Profiles** — port de `zeroclaw/src/auth/` : profiles JSON chiffré, OAuth PKCE, token refresh (L)
4. **OpenAI Codex provider** — `chatgpt.com/backend-api/codex/responses` avec auth OAuth (M, dépend auth profiles)
5. **Runtime model/provider switch** `/model` + `/models` dans Telegram et Discord (M)
6. **Telegram inline buttons** avec style (primary/success/danger) (M)
7. **Telegram reaction notifications** (M)
8. **PostgreSQL memory backend** (M)
9. **Slack streaming** (L)
10. **Discord slash commands** (L)
11. **Observabilité Prometheus** : enrichir les labels (provider, model, channel, direction) (S)

---

## ✅ Ce qui va bien (ne pas casser)

- **Build + tests stables** : 194 tests, `cargo build --release` passe, zéro régression depuis le sprint B→K
- **Gateway auth** : middleware bearer token + rate limiting per-IP bien implémentés
- **Webhook HMAC** : Slack et WhatsApp vérifiés, Telegram secret token vérifié
- **Provider routing + fallback** : `ReliableRouter` avec retry exponentiel et failover Anthropic→OpenAI fonctionne
- **Channel router centralisé** : `start_enabled_channels` démarre proprement tous les canaux configurés
- **Guardd audit trail** : JSONL logs de toutes les décisions d'autorisation
- **SQLite memory** : upsert/get/search fonctionnels, intégrés au chat path
- **Vector memory baseline** : `src/memory/vector.rs` avec cosine similarity opérationnel
- **Tunnel providers** : none/custom/cloudflare/ngrok/tailscale abstraits avec tests
- **Multi-agent baseline** : `AgentRegistry` + sub-agent spawn
- **Skills system** : scan workspace, trigger matching, injection prompt
- **Session management** : SQLite persistance, compaction, per-channel-peer isolation
- **Heartbeat** : timer + active-hours gate + HEARTBEAT_OK no-op
- **TUI** : ratatui dashboard fonctionnel avec polling API status live
- **`rustclaw doctor`** : validation config complète (channels, gateway, memory, providers)
- **DEPLOYMENT.md + ARCHITECTURE.md + SECURITY.md** : documentation à jour

---

## Scores post-audit (estimés)

| Dimension | 2026-02-17 pré-sprint | 2026-02-17 post-sprint | 2026-02-18 (cet audit) |
|-----------|----------------------|----------------------|----------------------|
| Feature parity vs OpenClaw | 7.6 | 9.0 | **7.5** ↘ (nouvelles features OC 2026.2.17 non portées) |
| Feature parity vs ZeroClaw | 6.0 | 7.5 | **6.5** ↘ (nouveau pull ZeroClaw) |
| Security | 7.2 | 9.0 | **7.0** ↘ (3 bugs sécurité découverts) |
| Stability | 7.0 | 8.5 | **7.5** ↘ (5 bugs stabilité dont 2 critiques) |
| Test coverage | 7.5 | 9.0 | **8.5** (194 tests — mais E2E manquants) |
| Production readiness | 6.8 | 8.5 | **7.0** ↘ (bugs critiques non fixés) |

> Note : les baisses de scores ne signifient pas une régression du code — elles reflètent que la barre de référence (OpenClaw + ZeroClaw) a monté depuis l'audit précédent.
