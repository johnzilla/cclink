# CCLink

## What This Is

A single Rust CLI binary (`cclink`) that publishes cryptographically signed, encrypted Claude Code session handoff records directly to the PKARR Mainline DHT. Run `cclink` on one machine to publish your session, `cclink pickup` on another to resume it -- no central relay, no accounts, no signup tokens. Your PKARR key is your identity.

## Core Value

Effortless, secure session handoff between devices: `cclink` on one machine, `cclink pickup` on another, you're back in your session.

## Requirements

### Validated

- ✓ Generate and manage PKARR/Ed25519 keypairs (`cclink init`, `cclink whoami`) -- v1.0
- ✓ Discover Claude Code session IDs from `~/.claude/projects/` with cwd scoping -- v1.0
- ✓ Build and sign handoff payload (session ID, hostname, project, timestamps) -- v1.0
- ✓ Encrypt session ID with age (self-encrypt via Ed25519-to-X25519 derivation) -- v1.0
- ✓ Publish encrypted handoff record to DHT -- v1.0 (homeserver), v1.1 (direct DHT)
- ✓ Retrieve and decrypt own handoff (`cclink pickup`) -- v1.0
- ✓ Share-mode encryption to a specific recipient's public key (`--share`) -- v1.0
- ✓ Burn-after-read mode (`--burn`) -- delete record after first retrieval -- v1.0
- ✓ TTL-based expiry (`--ttl`, default 24h) -- v1.0
- ✓ Terminal QR code rendering after publish and on pickup -- v1.0
- ✓ `cclink list` -- show active handoff records with comfy-table -- v1.0
- ✓ `cclink revoke` -- delete/revoke handoff records -- v1.0
- ✓ Auto-execute `claude --resume <id>` after pickup (default behavior) -- v1.0
- ✓ Colored terminal output with status indicators -- v1.0
- ✓ Ed25519 signature verification on all retrieved records -- v1.0
- ✓ Atomic key write (write-to-temp + rename) -- v1.0
- ✓ CI/CD with 4-platform release builds and curl installer -- v1.0
- ✓ Round-trip encryption tests and plaintext leak detection in CI -- v1.0
- ✓ Sign burn + recipient fields in handoff payload (clean break from v1.0 format) -- v1.1
- ✓ Enforce key file permissions (0600) explicitly in cclink code -- v1.1
- ✓ PIN-protected handoffs (`--pin`) with Argon2id+HKDF-derived key -- v1.1
- ✓ Make `--burn` + `--share` mutually exclusive -- v1.1
- ✓ Fix pickup CLI help text -- v1.1
- ✓ Structured error handling (CclinkError variants) -- v1.1
- ✓ Remove dead CclinkError variants -- v1.1
- ✓ Optimize list command -- v1.1
- ✓ Lazy signin / session reuse -- v1.1 (superseded by DHT)
- ✓ Update PRD stale path references -- v1.1
- ✓ Encrypt all sensitive metadata into blob (no cleartext hostname/project on DHT) -- v1.1
- ✓ Direct PKARR Mainline DHT transport (no homeserver dependency) -- v1.1

### Active

<!-- v1.2 Dependency Audit & Code Quality -->

- [ ] Audit and upgrade ed25519-dalek from pre-release (=3.0.0-pre.5)
- [ ] Add cargo clippy and cargo audit to CI pipeline
- [ ] Enforce minimum PIN length at publish time
- [ ] Fix placeholder `user/cclink` repo paths in Cargo.toml and install.sh
- [ ] Remove dead LatestPointer code from DHT migration
- [ ] Fix QR code content when --share + --qr combined

## Current Milestone: v1.2 Dependency Audit & Code Quality

**Goal:** Address code review findings — audit crypto dependencies, harden CI, enforce PIN strength, and clean up tech debt.

**Target features:**
- Investigate and resolve ed25519-dalek pre-release pinning
- Add cargo clippy and cargo audit to CI
- Enforce minimum PIN length/complexity
- Fix known tech debt (placeholder paths, dead code, QR+share bug)

### Out of Scope

- Team/shared namespace handoffs -- v2, not needed for single-user flow
- Web UI at cclink.dev -- optional polish, CLI-first
- Claude Code hook/plugin integration -- future consideration
- Mobile app -- terminal-only
- Session preview/summary -- would require accessing session content
- Override inferred project label via `--project` -- deferred, not in code review scope
- Burn-after-read for shared records -- DHT can only be revoked by key owner

## Context

Shipped v1.1 with 2,633 LOC Rust (src) + 590 LOC tests.
Tech stack: Rust, pkarr 5.0.3 (Mainline DHT), age (X25519), clap, owo-colors, comfy-table, qr2term, argon2.

- Claude Code stores sessions in `~/.claude/projects/` as directories with JSONL progress records
- `claude --resume <sessionID>` resumes a session from any device with filesystem access
- Records published directly to PKARR Mainline DHT as DNS TXT records inside Ed25519-signed packets
- One handoff per identity (DHT stores one SignedPacket per public key)
- Ed25519 keys birationally map to X25519, enabling age encryption with the same keypair
- All sensitive metadata (hostname, project path, session ID) encrypted into blob -- DHT nodes see only ciphertext
- Key storage at `~/.pubky/secret_key` with 0600 permissions
- The pickup device still needs filesystem access to session data (SSH, Tailscale, etc.) -- cclink only transfers the session ID reference

## Constraints

- **Language**: Rust -- single binary distribution, pkarr crate available
- **Identity**: PKARR/Ed25519 -- reuse existing Pubky identity ecosystem
- **Transport**: PKARR Mainline DHT -- no custom relay, no accounts, no homeserver
- **Encryption**: age with X25519 -- lightweight, Ed25519-compatible
- **Key storage**: `~/.pubky/secret_key` with 0600 permissions
- **No session content transit**: Only encrypted session ID and metadata cross the network
- **SignedPacket budget**: 912 bytes max JSON in DHT records (1000-byte DNS payload limit)

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rust single binary | Performance, pkarr crate available, easy distribution | ✓ Good |
| age encryption over NaCl box | Simpler API, well-audited, maps cleanly from Ed25519 | ✓ Good |
| PKARR Mainline DHT transport | No accounts, no tokens, no homeserver -- true decentralization | ✓ Good |
| ~/.pubky/ key storage | Reuse Pubky ecosystem directory | ✓ Good |
| 24h default TTL | More forgiving for cross-timezone handoffs | ✓ Good |
| Exec as default behavior | Pickup always runs claude --resume | ✓ Good |
| Signed burn+recipient (clean break) | Old v1.0 records expire via TTL, no migration needed | ✓ Good |
| --burn + --share mutually exclusive | Recipient can't revoke owner's record | ✓ Good |
| PIN mode with Argon2id+HKDF | Real security feature, not just 4-digit PIN | ✓ Good |
| Replace homeserver with DHT | Eliminates signup tokens, accounts, authentication sessions | ✓ Good |
| Encrypt metadata into blob | No hostname/project leakage on DHT | ✓ Good |
| skip_serializing_if on defaults | Saves ~71 bytes in common case for SignedPacket budget | ✓ Good |

---
*Last updated: 2026-02-23 after v1.2 milestone start*
