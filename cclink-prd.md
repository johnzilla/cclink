# CCLink — Signed Session Handoff for Claude Code via Pubky

> **Note:** This is the original design document from project inception. The actual implementation evolved significantly -- see [README.md](README.md) for current documentation. Key differences: transport is now direct PKARR Mainline DHT (no homeserver), PINs require 8+ characters, LatestPointer was removed, and all metadata is encrypted.

> One-line pitch: Ephemeral, cryptographically signed session handoff links for Claude Code, powered by Pubky/PKARR decentralized identity.

---

## Problem

Claude Code's `--resume <sessionID>` lets you pick up a session from any device, but session IDs are UUIDs stored in `~/.claude/sessions/`. There's no ergonomic way to transfer a session reference between devices — especially when hours pass between starting and resuming. Current workarounds (copy/paste, tmux+SSH, claude-sync push/pull) all require pre-configuration or manual steps.

## Solution

A single Rust CLI binary (`cclink`) that:

1. Grabs the current Claude Code session ID
2. Signs it with your PKARR/Ed25519 keypair
3. Publishes an encrypted handoff record to your Pubky homeserver
4. Lets you (or a trusted party) retrieve and resume from any device

No central relay. No accounts. Your PKARR key is your identity.

---

## Architecture

```
┌──────────────┐    pubky PUT /cclink/latest     ┌──────────────────┐
│  CLI: cclink │  ──────────────────────────────► │  Pubky Homeserver │
│  (Rust)      │  signed session handoff record  │  (self-hosted or  │
│              │                                  │   pubky.app)      │
│  uses PKARR  │                                  │                   │
│  keypair     │                                  │  pk:<your-pubkey> │
└──────────────┘                                  └────────┬──────────┘
                                                           │
                                            pubky GET /cclink/latest
                                                           │
                                                  ┌────────▼──────────┐
                                                  │  Pickup device    │
                                                  │  cclink pickup    │
                                                  │  <pubkey>         │
                                                  │                   │
                                                  │  → claude --resume│
                                                  │    <session-id>   │
                                                  └───────────────────┘
```

**Key principle:** No session content transits the network. Only the session ID (encrypted) and metadata are published. Actual session data stays in `~/.claude/sessions/` on the origin machine. The pickup device still needs filesystem access (direct, SSH, Tailscale, etc.) to actually resume.

---

## Tech Stack

| Component | Choice | Rationale |
|-----------|--------|-----------|
| Language | Rust | Performance, single binary, pubky crate available |
| Identity | PKARR / Ed25519 | Reuse existing Pubky identity; same as 3GS |
| Transport | Pubky protocol (homeserver + DHT) | No custom relay needed; censorship-resistant |
| Encryption | age (x25519) | Ed25519 keys birationally map to X25519; lightweight |
| QR rendering | qrcode crate (terminal) | Zero-dependency terminal QR output |
| Session discovery | ~/.claude/sessions/ or `claude /status` | Standard Claude Code session storage |

---

## Data Model

### Handoff Record

Published to: `PUT /pub/cclink/sessions/<token>.json`

```json
{
  "version": 1,
  "session_id": "<age-encrypted session UUID>",
  "hostname": "framework-desktop",
  "project": "/home/user/projects/shipsecure",
  "created_at": "2026-02-21T20:00:00Z",
  "ttl": 28800,
  "burn_after_read": false,
  "pin_protected": false,
  "creator_pubkey": "<ed25519-public-key>"
}
```

### Latest Pointer

Published to: `PUT /pub/cclink/sessions/latest.json`

```json
{
  "token": "<token>",
  "created_at": "2026-02-21T20:00:00Z"
}
```

### Encryption Scheme

- **Default:** Session ID encrypted to creator's own X25519 public key (derived from Ed25519 PKARR key). Only the creator can decrypt.
- **PIN mode:** Session ID encrypted with a key derived from `HKDF(PIN + creator_pubkey)`. Anyone with the PIN and knowledge of the creator's pubkey can decrypt.
- **Share mode:** Session ID encrypted to a specific recipient's X25519 public key.

---

## CLI Interface

### Publish a handoff

```bash
cclink                              # publish current session to homeserver
cclink <session-id>                 # publish explicit session ID
cclink --ttl 4h                     # custom expiry (default: 8h)
cclink --pin                        # prompt for 4-digit PIN for shared decryption
cclink --pin 4821                   # inline PIN
cclink --burn                       # mark as single-use (deleted after first retrieval)
cclink --share <pubkey>             # encrypt to a specific recipient's key
cclink --project shipsecure         # label override (default: inferred from cwd)
```

### Retrieve a handoff

```bash
cclink pickup                       # resolve your own latest handoff
cclink pickup <pubkey>              # resolve someone else's latest
cclink pickup <pubkey> --pin 4821   # decrypt PIN-protected handoff
cclink pickup --qr                  # display QR code with pk: URI
cclink pickup --exec                # auto-execute: runs `claude --resume <id>` directly
```

### Management

```bash
cclink list                         # show your active handoff records
cclink revoke <token>               # delete a specific handoff
cclink revoke --all                 # delete all handoffs
cclink whoami                       # show your PKARR public key and homeserver
cclink init                         # generate keypair + configure homeserver
```

---

## Phases

### Phase 1: Core CLI + Keypair (Saturday morning)

**Goal:** Rust binary that generates/manages PKARR keypair, reads Claude Code session IDs, and signs handoff payloads.

**Deliverables:**
- [ ] `cargo init cclink`
- [ ] Keypair generation and storage in ~/.pubky/secret_key (or reuse existing PKARR key with --import)
- [ ] Session ID discovery: parse `~/.claude/sessions/` directory, pick most recent or accept explicit ID
- [ ] Payload construction: build handoff JSON record
- [ ] Ed25519 signing of payload
- [ ] `cclink whoami` and `cclink init` commands working
- [ ] Unit tests for signing/verification round-trip

**Dependencies:** `ed25519-dalek`, `serde`, `serde_json`, `chrono`, `clap`

### Phase 2: Pubky Publish + Retrieve (Saturday afternoon)

**Goal:** Publish encrypted handoff records to Pubky homeserver and retrieve them.

**Deliverables:**
- [ ] Integrate `pubky` crate for homeserver communication
- [ ] `age` encryption of session ID (self-encrypt by default)
- [ ] `PUT /pub/cclink/sessions/<token>.json` — publish handoff
- [ ] `PUT /pub/cclink/sessions/latest.json` — update latest pointer
- [ ] `GET` path resolution — retrieve and decrypt handoff
- [ ] `--pin` mode: HKDF-derived key encryption/decryption
- [ ] `--burn` mode: DELETE record after successful retrieval
- [ ] `--ttl` mode: include expiry in record; refuse to return expired records
- [ ] `cclink pickup` and `cclink pickup <pubkey>` working end-to-end

**Dependencies:** `pubky`, `age`, `hkdf`, `sha2`

### Phase 3: QR + Terminal UX (Saturday evening)

**Goal:** Polish the user experience with QR codes, pretty output, and convenience features.

**Deliverables:**
- [ ] Terminal QR code rendering on `cclink` (shows QR after publish)
- [ ] QR encodes: `pk:<pubkey>/cclink/<token>` or a short web URL
- [ ] `cclink pickup --qr` renders scannable QR for mobile
- [ ] `cclink pickup --exec` auto-runs `claude --resume <id>`
- [ ] Colored terminal output: success/error states, TTL countdown
- [ ] `cclink list` with human-readable table (token, project, age, TTL remaining, burn status)
- [ ] `cclink revoke` working

**Dependencies:** `qrcode`, `colored` or `termcolor`

### Phase 4: Polish + Ship (Sunday)

**Goal:** Open source, document, and optionally add a minimal web pickup page.

**Deliverables:**
- [ ] README.md with install instructions, usage examples, security model explanation
- [ ] LICENSE (MIT or Apache-2.0)
- [ ] GitHub repo at github.com/<your-handle>/cclink
- [ ] Optional: static HTML page at cclink.dev that can resolve `pk:<pubkey>/cclink/<token>` via Pubky HTTP gateway and display the resume command (progressive enhancement — CLI-first, web-optional)
- [ ] Optional: `cargo install` support / release binaries via `cargo-dist` or GitHub Actions
- [ ] Optional: 60-second demo video for X

---

## Security Model

### Threat Model

| Threat | Mitigation |
|--------|------------|
| Homeserver operator reads session IDs | Session IDs are age-encrypted; homeserver sees only ciphertext |
| Attacker intercepts QR code | PIN mode adds second factor; burn mode limits window |
| Attacker forges handoff record | Ed25519 signature verification on all records |
| Replay attack (reuse old handoff) | TTL expiry + optional burn-after-read |
| Key compromise | Standard Ed25519 key hygiene; keys stored in ~/.pubky/secret_key with 0600 perms |
| Brute-force PIN | 4-digit PIN is convenience, not security; use share mode for real access control |

### Trust Boundaries

1. **The CLI is trusted** — it runs on your machine, has access to your keys and session data.
2. **The homeserver is semi-trusted** — it stores encrypted blobs but cannot read session IDs. It could deny service but not forge or decrypt.
3. **The DHT is untrusted** — PKARR signatures ensure authenticity; age encryption ensures confidentiality.
4. **The pickup device is trusted** — it needs your private key (self mode) or PIN (PIN mode) to decrypt.

---

## Future Considerations (v2, not in scope)

- **Team handoffs:** Shared team Pubky namespace where multiple engineers can publish/claim sessions
- **Audit log:** Append-only log of handoff events for compliance
- **Claude Code hook/plugin:** Native integration so `cclink` runs automatically on session start
- **Web app:** Full web UI at cclink.dev for non-CLI users
- **Notifications:** Push notification to your phone when a new handoff is published
- **Session preview:** Show session summary/context without revealing full session data
- **3GS integration:** Use 3GS as a source registry for session context/documentation links

---

## Success Criteria

Phase 1 complete when:
- [ ] `cclink init` generates and stores a PKARR keypair
- [ ] `cclink whoami` prints public key
- [ ] Session ID is correctly read from `~/.claude/sessions/`
- [ ] Payload is signed and verifiable

Phase 2 complete when:
- [ ] `cclink` publishes an encrypted handoff to a Pubky homeserver
- [ ] `cclink pickup` retrieves and decrypts your own handoff
- [ ] `cclink pickup <pubkey> --pin <pin>` works for PIN-protected handoffs
- [ ] Expired and burned records are correctly handled

Phase 3 complete when:
- [ ] QR code renders in terminal after publish
- [ ] `cclink list` shows active handoffs
- [ ] `cclink revoke` deletes records
- [ ] `cclink pickup --exec` launches Claude Code with the resumed session

Phase 4 complete when:
- [ ] Repo is public on GitHub with README and LICENSE
- [ ] `cargo install cclink` works (or release binary available)
- [ ] At least one successful end-to-end handoff between two devices demonstrated
