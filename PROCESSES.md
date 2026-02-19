# 🦀 Rustclaw — Processes & Operations

## 🔄 Development Process
```
Gabriel demande feature → Opus conçoit l'architecture
    → Codex implémente (via `codex exec --full-auto`)
    → cargo check + cargo test → Opus review → merge
```

## ⏰ Cron Jobs
Aucun cron spécifique. Audité par le **Nightly Infrastructure Audit** (midnight CET) qui vérifie les 7 fichiers kaizen.

## 🤖 Agents
| Agent | Rôle |
|---|---|
| **Opus 4.6** | Architecte — conçoit les modules, review le code |
| **Codex 5.3** | Développeur — code en Rust via `codex exec --full-auto` en PTY |

## 🎯 Skills
| Skill | Usage |
|---|---|
| **Coding-agent** | Lance Codex en PTY background pour coder dans le repo |
| **Kaizen** | Amélioration continue (BUILD_PLAN.md = roadmap) |

## 📁 Key Files
| File | Rôle | Mis à jour par |
|---|---|---|
| `BUILD_PLAN.md` | 7 milestones roadmap | Opus |
| `AUDIT_REPORT.md` | Audit complet (2026-02-17) | Codex (sub-agent) |
| `Cargo.toml` | Dépendances Rust | Codex |
| `src/guardd/` | Security kernel (6 fichiers) | Codex |

## 🔗 Dépendances
| Dep | Quoi |
|---|---|
| Rust 1.85+ | Toolchain (edition 2024) |
| `codex` CLI | `/opt/homebrew/bin/codex` |
| `cargo` | Build + test |

## 📊 Métriques
| Métrique | Valeur |
|---|---|
| Tests | 85 pass, 0 fail |
| Warnings | 36 (clippy) |
| Modules | 9 (auth, channels, chat, guardd, cron, nodes, setup, tui, workspace) |
| Complétion vs OpenClaw | ~40% |
