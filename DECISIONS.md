# DECISIONS — Rustclaw

## 2026-02-17 — Construire un runtime Rust “subset” d’OpenClaw
- **Décision**: Implémenter un sous-ensemble ciblé plutôt qu’un clone complet.
- **Alternatives considérées**:
  - Parité complète immédiate avec OpenClaw.
  - Wrapper léger autour de Node existant.
- **Rationale**: Permet d’obtenir vite une base performante et maintenable, avec dette maîtrisée.

## 2026-02-17 — Conserver les channels clés (Telegram, WhatsApp, Slack)
- **Décision**: Prioriser les canaux à plus forte utilité opérationnelle.
- **Alternatives considérées**:
  - Supporter tous les channels dès v0.
  - Un seul channel pour simplifier extrême.
- **Rationale**: Bon compromis couverture/complexité; valide l’architecture multi-channel sans explosion de scope.

## 2026-02-17 — Conserver Anthropic + OpenAI en premier
- **Décision**: Implémenter d’abord 2 providers majeurs avec retry/fallback.
- **Alternatives considérées**:
  - Multi-provider large dès départ.
  - Provider unique.
- **Rationale**: Réduit le risque d’implémentation tout en couvrant les usages principaux.

## 2026-02-17 — Stockage sessions en fichiers
- **Décision**: Persistance des sessions en JSON par chat dans le workspace.
- **Alternatives considérées**:
  - Base de données.
  - In-memory uniquement.
- **Rationale**: Simplicité, portabilité, debug facile, coût opérationnel minimal.

## 2026-02-17 — Séparation stricte config/secrets
- **Décision**: `config.toml` pour réglages, `.env` (0600) pour secrets.
- **Alternatives considérées**:
  - Tout dans un seul fichier.
  - Secret manager externe imposé.
- **Rationale**: Bonne sécurité pratique, setup rapide, faible friction utilisateur.

## 2026-02-17 — Inclure un scheduler cron natif
- **Décision**: Intégrer cron dans le runtime plutôt que dépendre d’un orchestrateur externe.
- **Alternatives considérées**:
  - Cron externe (systemd/OS).
  - Pas de scheduling natif.
- **Rationale**: Rend les workflows proactifs (heartbeat, jobs) cohérents et portables.

## 2026-02-17 — Hors scope initial assumé
- **Décision**: Exclure voice/image/web UI/DB au départ.
- **Alternatives considérées**:
  - Ajouter ces briques dès v0.
- **Rationale**: Préserver la fiabilité du noyau conversationnel avant extension fonctionnelle.

## Signaux / préférences (FR)
- Favoriser des décisions révisables à faible coût (fichier-based, modules isolés).
- Ne pas élargir le scope sans métrique claire de valeur.