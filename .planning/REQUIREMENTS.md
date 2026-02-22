# Requirements: CCLink

**Defined:** 2026-02-21
**Core Value:** Effortless, secure session handoff between devices: `cclink` on one machine, `cclink pickup` on another, you're back in your session.

## v1 Requirements

Requirements for initial release. Each maps to roadmap phases.

### Key Management

- [x] **KEY-01**: User can generate an Ed25519/PKARR keypair and store it securely in `~/.cclink/keys` with 0600 permissions
- [ ] **KEY-02**: User can view their PKARR public key and homeserver info via `cclink whoami`
- [x] **KEY-03**: User can import an existing PKARR keypair via `cclink init --import`
- [x] **KEY-04**: Private key file is written atomically (write-to-temp + rename) to prevent corruption

### Session Discovery

- [ ] **SESS-01**: CLI can discover the most recent session ID from `~/.claude/sessions/`
- [ ] **SESS-02**: User can provide an explicit session ID as a CLI argument

### Publishing

- [ ] **PUB-01**: User can publish an encrypted handoff record to their Pubky homeserver via `cclink` or `cclink <session-id>`
- [ ] **PUB-02**: Handoff record includes hostname, project path, creation timestamp, and TTL
- [ ] **PUB-03**: Session ID is age-encrypted to the creator's own X25519 key (derived from Ed25519)
- [ ] **PUB-04**: User can set a custom TTL via `--ttl` (default 8 hours)
- [ ] **PUB-05**: A `latest.json` pointer is updated on each publish
- [ ] **PUB-06**: Terminal QR code is rendered after successful publish

### Retrieval

- [ ] **RET-01**: User can retrieve and decrypt their own latest handoff via `cclink pickup`
- [ ] **RET-02**: User can retrieve another user's latest handoff via `cclink pickup <pubkey>`
- [ ] **RET-03**: Expired records (past TTL) are refused on retrieval
- [ ] **RET-04**: User can auto-execute `claude --resume <id>` via `cclink pickup --exec`
- [ ] **RET-05**: User can display a scannable QR code via `cclink pickup --qr`
- [ ] **RET-06**: Retrieval retries with backoff to handle DHT propagation delay

### Advanced Encryption

- [ ] **ENC-01**: User can encrypt a handoff to a specific recipient's X25519 key via `--share <pubkey>`
- [ ] **ENC-02**: User can mark a handoff as burn-after-read via `--burn` (record deleted after first retrieval)

### Management

- [ ] **MGT-01**: User can list active handoff records via `cclink list` with token, project, age, TTL remaining, and burn status
- [ ] **MGT-02**: User can revoke a specific handoff via `cclink revoke <token>`
- [ ] **MGT-03**: User can revoke all handoffs via `cclink revoke --all`

### Terminal UX

- [ ] **UX-01**: Colored terminal output with clear success/error states
- [ ] **UX-02**: Ed25519 signature verification on all retrieved records

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Advanced Encryption

- **ENC-03**: User can protect a handoff with a 4-digit PIN via `--pin` (Argon2id+HKDF derived key)

### Publishing

- **PUB-07**: User can override the inferred project label via `--project`

### Team Features

- **TEAM-01**: Shared team Pubky namespace for multi-engineer handoffs
- **TEAM-02**: Audit log of handoff events

### Integration

- **INT-01**: Claude Code hook/plugin for automatic session publishing
- **INT-02**: Web pickup page at cclink.dev via Pubky HTTP gateway

## Out of Scope

| Feature | Reason |
|---------|--------|
| Session content transfer | Only session ID reference transits the network; content stays on origin machine |
| Mobile app | Terminal-only tool; QR codes bridge to mobile for reference only |
| Push notifications | Overcomplicated for CLI-first tool |
| Session preview/summary | Would require accessing session content, violating security model |
| 3GS integration | Future consideration, not core to handoff flow |
| Web UI | CLI-first; optional static page is v2 |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| KEY-01 | Phase 1 | Complete |
| KEY-02 | Phase 1 | Pending |
| KEY-03 | Phase 1 | Complete |
| KEY-04 | Phase 1 | Complete |
| SESS-01 | Phase 3 | Pending |
| SESS-02 | Phase 3 | Pending |
| PUB-01 | Phase 3 | Pending |
| PUB-02 | Phase 2 | Pending |
| PUB-03 | Phase 2 | Pending |
| PUB-04 | Phase 3 | Pending |
| PUB-05 | Phase 2 | Pending |
| PUB-06 | Phase 3 | Pending |
| RET-01 | Phase 3 | Pending |
| RET-02 | Phase 3 | Pending |
| RET-03 | Phase 3 | Pending |
| RET-04 | Phase 3 | Pending |
| RET-05 | Phase 3 | Pending |
| RET-06 | Phase 3 | Pending |
| ENC-01 | Phase 4 | Pending |
| ENC-02 | Phase 4 | Pending |
| MGT-01 | Phase 4 | Pending |
| MGT-02 | Phase 4 | Pending |
| MGT-03 | Phase 4 | Pending |
| UX-01 | Phase 3 | Pending |
| UX-02 | Phase 2 | Pending |

**Coverage:**
- v1 requirements: 25 total
- Mapped to phases: 25
- Unmapped: 0 ✓

---
*Requirements defined: 2026-02-21*
*Last updated: 2026-02-21 after roadmap creation*
