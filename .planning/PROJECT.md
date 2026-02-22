# CCLink

## What This Is

A single Rust CLI binary (`cclink`) that publishes cryptographically signed, encrypted Claude Code session handoff links via the Pubky protocol. Run `cclink` on one machine to publish your session, `cclink pickup` on another to resume it — no central relay, no accounts, your PKARR key is your identity.

## Core Value

Effortless, secure session handoff between devices: `cclink` on one machine, `cclink pickup` on another, you're back in your session.

## Requirements

### Validated

- ✓ Generate and manage PKARR/Ed25519 keypairs (`cclink init`, `cclink whoami`) — v1.0
- ✓ Discover Claude Code session IDs from `~/.claude/projects/` with cwd scoping — v1.0
- ✓ Build and sign handoff payload (session ID, hostname, project, timestamps) — v1.0
- ✓ Encrypt session ID with age (self-encrypt via Ed25519-to-X25519 derivation) — v1.0
- ✓ Publish encrypted handoff record to Pubky homeserver — v1.0
- ✓ Retrieve and decrypt own handoff (`cclink pickup`) — v1.0
- ✓ Share-mode encryption to a specific recipient's public key (`--share`) — v1.0
- ✓ Burn-after-read mode (`--burn`) — delete record after first retrieval — v1.0
- ✓ TTL-based expiry (`--ttl`, default 24h) — v1.0
- ✓ Terminal QR code rendering after publish and on pickup — v1.0
- ✓ `cclink list` — show active handoff records with comfy-table — v1.0
- ✓ `cclink revoke` — delete specific or all handoff records — v1.0
- ✓ Auto-execute `claude --resume <id>` after pickup (default behavior) — v1.0
- ✓ Colored terminal output with status indicators — v1.0
- ✓ Ed25519 signature verification on all retrieved records — v1.0
- ✓ Atomic key write (write-to-temp + rename) — v1.0
- ✓ CI/CD with 4-platform release builds and curl installer — v1.0
- ✓ Round-trip encryption tests and plaintext leak detection in CI — v1.0

### Active

- [ ] Sign burn + recipient fields in handoff payload (clean break from v1.0 format)
- [ ] Enforce key file permissions (0600) explicitly in cclink code
- [ ] PIN-protected handoffs (`--pin`) with Argon2id+HKDF-derived key
- [ ] Make `--burn` + `--share` mutually exclusive
- [ ] Fix pickup CLI help text (self-publish suggests wrong command)
- [ ] Lazy signin / session reuse in HomeserverClient
- [ ] Deduplicate `human_duration` into shared utility
- [ ] Structured error handling (replace string matching with CclinkError variants)
- [ ] Remove dead CclinkError variants
- [ ] Optimize list command (reduce N+1 HTTP requests)
- [ ] Update PRD stale path references (~/.cclink → ~/.pubky)

### Out of Scope

- Team/shared namespace handoffs — v2, not needed for single-user flow
- Web UI at cclink.dev — optional polish, CLI-first
- Claude Code hook/plugin integration — future consideration
- Mobile app — terminal-only
- Notifications/push — out of scope entirely
- Session preview/summary — would require accessing session content
- 3GS integration — future consideration
- Override inferred project label via `--project` — deferred, not in code review scope
- Burn-after-read for shared records — homeserver can't support delegated delete; --burn + --share now mutually exclusive

## Current Milestone: v1.1 Security Hardening & Code Review Fixes

**Goal:** Address all findings from external code review — fix security gaps, resolve functional discrepancies, and improve code quality.

**Target features:**
- Signed metadata (burn + recipient in payload)
- Key file permission enforcement
- PIN-protected handoffs
- Lazy authentication
- Structured error handling
- List command optimization

## Context

Shipped v1.0 with 2,851 LOC Rust.
Tech stack: Rust, pkarr 5.0.3, age (x25519), reqwest (rustls), clap, owo-colors, comfy-table, qr2term.

- Claude Code stores sessions in `~/.claude/projects/` as directories with JSONL progress records
- `claude --resume <sessionID>` resumes a session from any device with filesystem access
- Pubky is a decentralized protocol using PKARR (Public Key Addressable Resource Records) for identity
- Ed25519 keys birationally map to X25519, enabling age encryption with the same keypair
- The pickup device still needs filesystem access to the session data (SSH, Tailscale, etc.) — cclink only transfers the session ID reference, not session content
- Handoff records are published to `/pub/cclink/<token>` on the homeserver
- A `latest` pointer tracks the most recent handoff
- Key storage at `~/.pubky/secret_key` with 0600 permissions (reuses Pubky ecosystem path)

## Constraints

- **Language**: Rust — single binary distribution, pubky crate available
- **Identity**: PKARR/Ed25519 — reuse existing Pubky identity ecosystem
- **Transport**: Pubky protocol (homeserver + DHT) — no custom relay
- **Encryption**: age with x25519 — lightweight, Ed25519-compatible
- **Key storage**: `~/.pubky/secret_key` with 0600 permissions
- **No session content transit**: Only encrypted session ID and metadata cross the network

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rust single binary | Performance, pubky crate available, easy distribution | ✓ Good — 2,851 LOC, compiles clean |
| age encryption over NaCl box | Simpler API, well-audited, maps cleanly from Ed25519 | ✓ Good — round-trip verified, no plaintext leaks |
| Pubky homeserver transport | No custom relay needed, censorship-resistant, reuses existing infra | ✓ Good — PUT/GET/DELETE all working |
| Latest pointer pattern | Simple way to find most recent handoff without listing all records | ✓ Good — clean single-lookup pickup path |
| ~/.pubky/ key storage | Reuse Pubky ecosystem directory instead of ~/.cclink/ | ✓ Good — consistent with pubky tooling |
| 24h default TTL (not 8h) | More forgiving for cross-timezone handoffs | ✓ Good — context decision in Phase 3 |
| Exec as default behavior | Pickup always runs claude --resume after confirm (not opt-in --exec) | ✓ Good — fewer flags, natural flow |
| burn/recipient as unsigned metadata | Preserve Phase 3 signature compatibility | ✓ Good — backwards-compatible record format |
| httpmock for sync integration tests | Works with reqwest::blocking without tokio runtime conflicts | ✓ Good — 7 integration tests, all #[test] |
| PIN mode deferred to v2 | Low entropy (4 digits); --share provides real access control | — Deferred |
| Sign burn+recipient (clean break) | Old v1.0 records expire via TTL, no migration needed | — Pending |
| --burn + --share mutually exclusive | Recipient can't DELETE owner's record; silent skip is worse | — Pending |

---
*Last updated: 2026-02-22 after v1.1 milestone start*
