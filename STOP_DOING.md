# STOP_DOING — Rustclaw

- Stop à accumuler des features sans tests de non-régression.
- Stop à la doc qui promet plus que l’implémentation effective.
- Stop au “partiellement implémenté” sans ticket de finalisation clair.
- Stop au traitement mono-tour si l’objectif est conversation persistante.
- Stop aux retries/timeouts implicites non testés en scénarios d’échec.
- Stop à considérer le runtime “ready” avant durcissement résilience (erreurs/rate-limits).
- Stop à reporter streaming et tests à la fin sans milestone d’acceptation explicite.

## Liens vers causes racines
- LEARNINGS.md: 2026-02-17 (gap README vs robustesse, streaming/tests incomplets)
- BUILD_PLAN.md: Missing/Partial items (streaming, session state, resilience, tests)
