# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-22)

**Core value:** Effortless, secure session handoff between devices: `cclink` on one machine, `cclink pickup` on another, you're back in your session.
**Current focus:** v1.1 Security Hardening & Code Review Fixes

## Current Position

Phase: Not started (defining requirements)
Plan: —
Status: Defining requirements
Last activity: 2026-02-22 — Milestone v1.1 started

## Accumulated Context

### Decisions

Full decision log archived with v1.0 milestone. See: .planning/milestones/v1.0-ROADMAP.md

Key architectural decisions carried forward:
- Rust single binary with pubky 0.6.0 + pkarr 5.0.3
- age encryption via Ed25519-to-X25519 derivation
- Pubky homeserver transport (no custom relay)
- Key storage at ~/.pubky/secret_key (0600 permissions)
- burn/recipient as unsigned metadata (backwards-compatible records) — **CHANGING IN v1.1**

v1.1 decisions:
- Clean break on signed metadata (no v1/v2 version negotiation)
- --burn + --share mutually exclusive (instead of silent skip or warning)

### Pending Todos

None.

### Blockers/Concerns

- HomeserverClient URL routing for multi-tenant PUT needs empirical verification against live pubky.app
- QR code content wrong when --share + --qr combined (minor bug, tracked in tech debt)
- Cargo.toml/install.sh use placeholder 'user/cclink' — must update before first release

## Session Continuity

Last session: 2026-02-22
Stopped at: Starting v1.1 milestone
Resume file: None
