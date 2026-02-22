# CCLink

## What This Is

A single Rust CLI binary (`cclink`) that publishes cryptographically signed, encrypted Claude Code session handoff links via the Pubky protocol. It lets you grab a session ID from one machine, publish it to your Pubky homeserver, and resume it from any other device — no central relay, no accounts, your PKARR key is your identity.

## Core Value

Effortless, secure session handoff between devices: `cclink` on one machine, `cclink pickup` on another, you're back in your session.

## Requirements

### Validated

<!-- Shipped and confirmed valuable. -->

(None yet — ship to validate)

### Active

<!-- Current scope. Building toward these. -->

- [ ] Generate and manage PKARR/Ed25519 keypairs (`cclink init`, `cclink whoami`)
- [ ] Discover Claude Code session IDs from `~/.claude/sessions/`
- [ ] Build and sign handoff payload (session ID, hostname, project, timestamps)
- [ ] Encrypt session ID with age (self-encrypt via Ed25519→X25519 derivation)
- [ ] Publish encrypted handoff record to Pubky homeserver
- [ ] Retrieve and decrypt own handoff (`cclink pickup`)
- [ ] PIN-protected handoffs (`--pin`) with HKDF-derived key
- [ ] Share-mode encryption to a specific recipient's public key (`--share`)
- [ ] Burn-after-read mode (`--burn`) — delete record after first retrieval
- [ ] TTL-based expiry (`--ttl`, default 8h)
- [ ] Terminal QR code rendering after publish and on pickup
- [ ] `cclink list` — show active handoff records
- [ ] `cclink revoke` — delete specific or all handoff records
- [ ] `cclink pickup --exec` — auto-run `claude --resume <id>`
- [ ] Colored terminal output with status indicators

### Out of Scope

- Team/shared namespace handoffs — v2, not needed for single-user flow
- Web UI at cclink.dev — optional polish, CLI-first
- Claude Code hook/plugin integration — future consideration
- Mobile app — terminal-only for v1
- Notifications/push — out of scope entirely
- Session preview/summary — would require accessing session content
- 3GS integration — future consideration

## Context

- Claude Code stores sessions in `~/.claude/sessions/` as directories named by UUID
- `claude --resume <sessionID>` resumes a session from any device with filesystem access
- Pubky is a decentralized protocol using PKARR (Public Key Addressable Resource Records) for identity
- Ed25519 keys birationally map to X25519, enabling age encryption with the same keypair
- The pickup device still needs filesystem access to the session data (SSH, Tailscale, etc.) — cclink only transfers the session ID reference, not session content
- Handoff records are published to `/pub/cclink/sessions/<token>.json` on the homeserver
- A `latest.json` pointer tracks the most recent handoff

## Constraints

- **Language**: Rust — single binary distribution, pubky crate available
- **Identity**: PKARR/Ed25519 — reuse existing Pubky identity ecosystem
- **Transport**: Pubky protocol (homeserver + DHT) — no custom relay
- **Encryption**: age with x25519 — lightweight, Ed25519-compatible
- **Key storage**: `~/.cclink/keys` with 0600 permissions
- **No session content transit**: Only encrypted session ID and metadata cross the network

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rust single binary | Performance, pubky crate available, easy distribution | — Pending |
| age encryption over NaCl box | Simpler API, well-audited, maps cleanly from Ed25519 | — Pending |
| Pubky homeserver transport | No custom relay needed, censorship-resistant, reuses existing infra | — Pending |
| 4-digit PIN as convenience, not security | Low entropy acknowledged; share mode exists for real access control | — Pending |
| Latest pointer pattern | Simple way to find most recent handoff without listing all records | — Pending |

---
*Last updated: 2026-02-21 after initialization*
