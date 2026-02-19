# LEARNINGS — Rustclaw

## 2026-02-17 — État réel du projet vs plan
- **Ce qui s’est passé**: Revue de `BUILD_PLAN.md` + `README.md` pour inventorier ce qui est déjà livré.
- **Résultat**: Base solide déjà en place (config/init/channels Anthropic/OpenAI + Telegram/WhatsApp/Slack + cron + workspace context + session persistence annoncée), mais plusieurs items restent “partiels” (streaming, robustesse, tests étendus).
- **Leçon**: Toujours partir d’un audit d’existant avant de planifier des features; sinon on duplique le travail.

## 2026-02-17 — Choix “runtime léger”
- **Ce qui s’est passé**: Scope volontairement limité (pas de voice/image, pas de DB, pas de web UI) pour garder simplicité/performance.
- **Résultat**: Architecture compréhensible, déploiement simple, opérationnel vite.
- **Leçon**: Le focus produit (subset clair) accélère la livraison et réduit les bugs systémiques.

## 2026-02-17 — Sécurité des secrets
- **Ce qui s’est passé**: Séparation config non-secrète (`config.toml`) et secrets (`.env` mode 0600).
- **Résultat**: Bonne hygiène de base sans complexité crypto additionnelle.
- **Leçon**: Une discipline opérationnelle simple et systématique vaut mieux qu’une sécurité “avancée” incomplète.

## 2026-02-17 — Risque principal: gap entre README et robustesse réelle
- **Ce qui s’est passé**: README présente une expérience complète; BUILD_PLAN note manque de tests, streaming, et handling d’échecs.
- **Résultat**: Risque de perception “ready” alors que la résilience n’est pas totalement prouvée.
- **Leçon**: Verrouiller tests + erreurs + observabilité avant de considérer le runtime “production-ready”.

## Ce qui a bien marché
- CLI claire (`init/run/chat/status/cron`).
- Structure modulaire lisible (`auth`, `channels`, `chat`, `cron`, `workspace`).
- Intégration multi-canaux pragmatique.

## Ce qui n’a pas encore marché / reste incomplet
- Streaming pas finalisé.
- Couverture de tests encore limitée.
- Gestion des pannes/rate-limits à durcir.

## À éviter la prochaine fois
- Reporter les tests de non-régression trop tard.
- Ajouter des fonctionnalités avant de stabiliser les invariants (sessions, retries, timeouts).
- Mélanger “roadmap” et “fait” sans statut explicite.

## Signaux / préférences (FR)
- Préférence: progression par milestones testables avec critères d’acceptation explicites.

## 2026-02-17 — Port incrémental ZeroClaw (providers/memory/config/tunnel)
- **Ce qui s’est passé**: Port partiel de l’architecture ZeroClaw vers Rustclaw sur 4 axes prioritaires.
- **Résultat**:
  - Nouveau module `src/providers/mod.rs` (router + reliability + fallback providers).
  - Nouveau module `src/memory/mod.rs` (SQLite upsert/get/search + tests).
  - Upgrade de `src/config.rs` (reliability/routes/memory/tunnel schema).
  - Nouveau module `src/tunnel/mod.rs` (none/custom abstraction).
  - `cargo check` et `cargo test` passent.
- **Leçon**: Pour un port massif, livrer des slices compilables avec tests apporte plus de valeur que viser une copie 1:1 d’un seul coup.
- **À faire ensuite**: intégrer mémoire au prompt runtime + durcir sécurité/gateway + compléter tunnels concrets (cloudflare/ngrok/tailscale).

## 2026-02-18 — Audit complet + mise en fonctionnement end-to-end (partiel)
- **Ce qui s’est passé**: Audit exhaustif de tous les modules `src/`, exécution build/test, puis implémentation d’un gateway HTTP réel avec endpoints API + shell tool guardé.
- **Résultat**:
  - `cargo build --release` ✅
  - `cargo test` ✅ (46 + 46 + 5)
  - Web UI minimale visible via browser (`/` + `/api/status` + `/api/sessions`)
  - TUI démarre et rend correctement en terminal
  - Provider routing confirmé en réel (fallback anthropic→openai observable)
  - Audit guardd JSONL confirmé
- **Leçon**: Une base « compilable et testée » peut rester loin de la prod tant que la sécurité n’est pas branchée partout (auth gateway + signatures webhook + guard global).
- **Blocage principal restant**: parity sécurité/opérations (auth, vérifications signatures, et couverture d’intégration avec crédentials valides).
- **Action recommandée**: prochain sprint focalisé sécurité first, puis parity channels/tools manquants (Discord/Signal/browser/file).

## 2026-02-18 — Gateway auth + channel router + Discord/Signal baseline
- **Ce qui s’est passé**: Port des briques de sécurité gateway et extension du runtime channels (router central + nouveaux modules Discord/Signal).
- **Résultat**:
  - Middleware bearer auth actif sur tout `/api/*` (mode `token|none`).
  - Rate limiting per-IP configurable (fenêtre 60s) + tests de blocage.
  - CORS configurable ajouté côté gateway.
  - Channel router centralisé branché dans `main.rs`.
  - Telegram durci (`/start`, chunking 4096, reply context).
  - WhatsApp aligné sur `/webhook/whatsapp` + vérification HMAC optionnelle via `guardd/channel_auth`.
  - Discord fonctionnel en polling REST (receive + reply).
  - Signal fonctionnel en polling REST bridge (receive + reply).
  - `cargo check` ✅ ; `cargo test` ✅ (55 + 55 + 5).
- **Leçon**: pour la parité rapide, un mode polling REST fiable est une étape utile avant Gateway WS natif (Discord) ou intégration signal-cli avancée.
- **Reste à faire**: compléter Discord WS gateway natif + tests d’intégration réseau simulés pour Signal/Discord/WhatsApp signatures strictes.

## 2026-02-18 — Modules B→K delivered in one compile-safe sweep
- **Ce qui s’est passé**: Implémentation continue des modules B à K (Discord WS, Vector+RAG, tunnels, cron SQLite+retry, web UI functional, TUI live data, multi-agent, skills, sessions, heartbeat).
- **Résultat**:
  - `cargo check` ✅
  - `cargo test` ✅
  - `cargo build --release` ✅
  - **142 tests** passés (73 lib + 64 bin + 5 intégration)
- **Leçon**: pour des demandes larges, une stratégie "baseline complète + tests unitaires ciblés" permet d’atterrir vite sans casser le runtime.
- **Attention**: certains modules sont baseline (fonctionnels) mais nécessitent durcissement production (monitoring, edge-cases réseau, auth profonde, intégration réelle des providers externes).

## 2026-02-18 — Audit vs OpenClaw 2026.2.17 + ZeroClaw post-pull

**Ce qui s'est passé** : Audit de gap frais comparant rustclaw contre OpenClaw 2026.2.17 (CHANGELOG complet) et ZeroClaw (après git pull avec 5+ nouveaux modules majeurs : auth profiles, OpenAI Codex OAuth, PostgreSQL memory, Prometheus observer, proxy config tool, runtime model switch).

**3 insights les plus importants :**

1. **Les bons scores d'audit précédents masquaient des bugs sécurité réels** — En lisant les fixes OpenClaw 2026.2.17, on trouve 3 bugs critiques dans le code rustclaw actuel : (a) `ShellTool` expose les secrets via `env` user params + héritage env parent (OC-09 équivalent), (b) `gateway_handle.abort()` tue le serveur HTTP sans drain (corruption requêtes in-flight), (c) `is_allowed()` contournable par shell operators (`ls && rm -rf /`). **Leçon : toujours re-auditer après chaque release de la référence — les nouvelles features révèlent des bugs existants par contraste.**

2. **ZeroClaw est passé d'un "placeholder npm" à un Rust runtime complet** — Le vrai zeroclaw a un système d'auth sophistiqué (OAuth PKCE + token refresh + profiles chiffrés + file locking + atomic writes) que rustclaw n'a pas du tout. L'auth env-var only de rustclaw est la limitation la plus structurante pour atteindre la feature parity. **Leçon : toujours vérifier l'état réel d'un repo concurrent avant de conclure qu'on est devant.**

3. **Le streaming reste le gouffre le plus profond** — OpenClaw a du streaming Anthropic/OpenAI/Telegram/Slack comme feature first-class depuis longtemps. Rustclaw n'a rien. Chaque nouvelle release OC ajoute des features de streaming (debounce, mode off, `startStream`...) qui creusent l'écart. **Leçon : le streaming n'est pas une "feature nice-to-have" — c'est la fondation sur laquelle reposent Telegram draft previews, Slack threads, et la perception de réactivité. Doit être le prochain sprint L.**

**Résultats :**
- 7 bugs découverts (3 critiques, 4 high)
- 45+ gaps identifiés (nouveaux depuis l'audit 2026-02-17)
- Rapport complet : `AUDIT_VS_OPENCLAW_2026-02-18.md`
- KAIZEN_IDEAS.md mis à jour avec priorités actualisées

**Action immédiate recommandée** : Sprint bugfixes sécurité (BUG-01 à BUG-07) avant tout nouvelle feature — les bugs découverts invalident partiellement le score Security 9.0 du sprint précédent.

## 2026-02-18 — Major refactoring: security + tools + telemetry + tests
- **Ce qui s'est passé**: Refactoring complet pour combler les gaps identifiés dans THREE_WAY_COMPARISON.md.
- **Résultat**:
  - Nouveau `src/tools/shell.rs` — exécution de commandes avec allowlist + timeout + env vars
  - Nouveau `src/tools/process.rs` — gestionnaire de processus background
  - `src/tools/browser.rs` réécrit — support CDP complet (navigate, screenshot, snapshot, click, type, evaluate) + fallback HTTP
  - Nouveau `src/telemetry/mod.rs` — counters, gauges, histograms, export Prometheus
  - Gateway enrichi — `/api/health`, `/api/ready`, `/api/metrics`, validation input (max 32k chars)
  - Slack webhook auth enforcement — signature HMAC vérifiée dans le handler (pas juste disponible)
  - Telegram secret token verification ajoutée
  - `rustclaw doctor` command — validation complète de la configuration
  - Graceful shutdown avec drain propre
  - 3 nouveaux test suites: `security_test.rs`, `gateway_test.rs`, `tools_test.rs`
  - **194 tests passés** (vs 142 avant)
  - `cargo build --release` ✅
  - Documentation: ARCHITECTURE.md, DEPLOYMENT.md, SECURITY.md
- **Leçon**: Pour une montée en qualité rapide, cibler les gaps mesurés (comparaison structurée) et implémenter module par module avec tests systématiques est plus efficace qu'une approche "tout d'un coup".
- **Scores mis à jour**: Feature 7.6→9.0, Security 7.2→9.0, Test 7.5→9.0, Prod-readiness 6.8→8.5
