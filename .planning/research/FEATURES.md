# Feature Research

**Domain:** CLI session handoff / developer session management tools
**Researched:** 2026-02-21
**Confidence:** MEDIUM — no direct cclink competitors exist; landscape assembled from adjacent tools in secret-sharing, session management, and secure CLI transfer categories

---

## Feature Landscape

### Table Stakes (Users Expect These)

Features users assume exist. Missing these = product feels incomplete.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Publish a session reference | Core value of the tool — no publish, no product | LOW | Session ID + metadata (hostname, project, timestamp); cclink already scoped this correctly |
| Retrieve a session reference | Without pickup, publish is useless | LOW | Must work on any device with same keypair |
| TTL / automatic expiry | Every secret-sharing tool in the ecosystem has this; sessions going stale is obvious | LOW | Default 8h is sensible; max should be configurable. Users leave if records accumulate indefinitely |
| Delete / revoke individual records | Users expect control over what they've published; ability to delete a record before it expires | LOW | `cclink revoke` — standard in all session management tools |
| List active handoff records | Users need to see what's published before they can pick up or revoke | LOW | `cclink list` — table stakes for any "I published something, where is it?" flow |
| Unique identity / keypair | Every tool in this space requires identity; without it, records can't be scoped to a user | MEDIUM | `cclink init` / `cclink whoami` — PKARR keypair on disk at `~/.cclink/keys` |
| Encrypted payload in transit and at rest | Users in developer/security contexts expect encryption; plaintext session IDs over the network would be a trust killer | MEDIUM | age encryption with Ed25519→X25519 derivation; well-understood pattern |
| Human-readable status output | Established CLI UX standard. Tools without colored output or clear error messages feel unfinished | LOW | Colored terminal output, status indicators. Per CLIG.dev standards and Heroku CLI guide |
| Single binary, no runtime deps | Developer CLI tools are expected to install without dependency hell. Rust binary is the right call | LOW | Already decided; distribution is via binary, not npm/pip/brew script |
| Error messages that are actionable | "failed" with no detail causes users to abandon tools; "no handoff found, run cclink publish first" keeps them moving | LOW | Dependency on good error handling in the Rust binary |

### Differentiators (Competitive Advantage)

Features that set the product apart. Not required, but valued.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Decentralized transport (Pubky/PKARR) | No central relay, no accounts, censorship-resistant. Competitors (Depot) are cloud-SaaS with account requirements. cclink's identity IS the keypair — zero signup | HIGH | Core architectural differentiator. Depot requires organization account; cclink requires only `cclink init`. Dependency: Pubky homeserver availability |
| Self-encrypt with same keypair (Ed25519→X25519) | No separate age keypair management — one keypair does everything. Ergonomic elegance that competitors lack | MEDIUM | Ed25519→X25519 biconditional mapping is well-documented; eliminates "which key do I use?" UX problem |
| Terminal QR code after publish | Phone-to-desktop handoff without copying session IDs manually. Magic-wormhole pioneered this UX; it's compelling for cross-device flows | MEDIUM | Renders in terminal via a QR library (qr2term or similar). Enables the phone-as-pickup-device use case without clipboard sharing |
| Burn-after-read mode (`--burn`) | One-time retrieval guarantee. OneTimeSecret built a business on this UX primitive; no equivalent in Claude Code handoff tools | LOW | Delete-on-first-GET from the homeserver. Establishes strong trust that a secret wasn't observed |
| PIN-protected handoffs (`--pin`) | Shared-device or lower-trust handoff without sharing full keypair. Convenience tier below full asymmetric encryption | MEDIUM | HKDF-derived key from PIN; explicitly low-entropy, documented as convenience not security. Users understand the tradeoff |
| Share-mode encryption to recipient's key (`--share`) | Secure handoff to a second user / second identity. Enables the "give someone else my session" use case without giving them your private key | MEDIUM | age recipient encryption to another's public key. Requires knowing the recipient's pubky public key |
| Auto-execute on pickup (`--exec`) | Eliminates the copy-paste step: `cclink pickup --exec` runs `claude --resume <id>` directly. Removes the last step of friction | LOW | Shell exec after decryption. Dependency: `claude` binary must be on PATH on pickup device |
| `latest.json` pointer for "just pick up my last session" | Zero-argument pickup. Users don't want to scroll a list; they want `cclink pickup` with no args to just work | LOW | Maintained as a server-side pointer. Not unique in concept (tmux `last-session` equivalent) but missing in all observed Claude Code handoff tools |
| No cloud account required | Competitors (Depot, Continuous-Claude) require SaaS accounts or cloud infra. cclink's trust model is cryptographic identity | LOW | Feature by virtue of architecture; important to communicate explicitly in docs and error messages |

### Anti-Features (Commonly Requested, Often Problematic)

Features that seem good but create problems.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Session content transfer | "Send my whole conversation" — users conflate session ID with session content | Session content (~/.claude/sessions/) can be gigabytes of JSONL. Transferring it requires file sync infrastructure (rsync, Syncthing, S3), not a session handoff tool. It also creates legal/privacy risk if session content contains secrets | Scope clearly to session ID + metadata only. Document that the pickup device needs filesystem access via SSH, Tailscale, or mount. `cclink` is the pointer, not the payload |
| Team / shared namespace handoffs | "My team should see all sessions" | Shared namespaces require ACL management, user provisioning, and org-level identity. This is a v2 SaaS product, not a CLI tool. Attempting it v1 collapses focus and delays core value | Design record paths to be per-keypair (`/pub/cclink/sessions/`). Multi-user is an opt-in share via `--share`, not a namespace concept |
| Web UI at cclink.dev | "I want a dashboard" | Web UIs require hosting, auth, CORS, and maintenance. The terminal is the right interface for a terminal user. A web UI creates operational overhead disproportionate to benefit | QR code in terminal renders on mobile browsers if needed. Dashboard is v2 or optional polish |
| Push notifications ("session ready") | "Tell me when a session is uploaded" | Notifications require a notification infrastructure (FCM, APNs, WebSocket server). This is a stateless CLI tool — polling on `cclink pickup` is sufficient and simpler | `cclink pickup` with TTL is the polling primitive. Users can script watch loops if needed |
| Session preview / summary | "Show me what was happening in the session before I resume" | Reading session content requires parsing Claude Code's internal JSONL format, which is not stable API. If the format changes, the feature breaks. Also violates the "no session content transit" constraint | Show metadata only: hostname, project path, timestamp, initiating machine. This is sufficient for "is this the right session?" |
| GUI app / desktop client | "Make it an app" | A desktop app is a separate product. Rust TUI is appropriate; an Electron app is not. Violates the single-binary, terminal-first constraint | Ship a polished CLI. Claude Code itself is a CLI; users in this space are terminal-comfortable |
| 4-digit PIN as real security | "I just want to use a PIN, it's secure enough" | PIN is 10,000 combinations. Shoulder-surfed, brute-forced, or guessed trivially. Users will store sensitive session IDs behind a PIN and believe they're protected | Document explicitly: PIN is convenience only. For real access control, use `--share` with keypair encryption. Make this the first line of `--pin --help` |

---

## Feature Dependencies

```
[init / keypair generation]
    └──required by──> [publish]
    └──required by──> [pickup]
    └──required by──> [list]
    └──required by──> [revoke]
    └──required by──> [--share mode]

[publish]
    └──required by──> [pickup]
    └──required by──> [list]
    └──required by──> [revoke]
    └──required by──> [latest.json pointer]

[latest.json pointer]
    └──enables──> [zero-arg pickup]

[pickup]
    └──enhanced by──> [--exec flag]
    └──enhanced by──> [QR code scan on sending side]

[TTL]
    └──implemented alongside──> [publish]
    └──conflicts with──> [burn-after-read] (both control record lifetime; mutually exclusive or last-one-wins)

[--pin]
    └──conflicts with──> [--share] (separate encryption modes; do not combine)
    └──conflicts with──> [self-encrypt default] (pin overrides symmetric key derivation)

[--burn]
    └──implemented in──> [homeserver delete-on-GET]
    └──independent of──> [TTL] (burn happens on first read; TTL happens on time)

[QR code render]
    └──depends on──> [publish completing successfully]
    └──independent of──> [encryption mode]

[--exec]
    └──depends on──> [pickup completing successfully]
    └──depends on──> [claude binary on PATH]
```

### Dependency Notes

- **init required before everything:** No keypair = no Pubky identity = no homeserver writes. Must be enforced with a clear error: "Run `cclink init` first."
- **TTL conflicts with --burn:** Both control record lifetime. Design decision: `--burn` takes priority (burn on read, TTL is a fallback). Or disallow combining them — simpler UX.
- **--pin conflicts with --share:** These are separate encryption modes. Attempting both should produce a clear error: "Use --pin for convenience or --share for cryptographic access control, not both."
- **QR code depends on publish success:** Do not render QR if publish failed. The QR encodes the pickup URL/token — it only exists post-publish.
- **--exec depends on claude in PATH:** Fail gracefully with "claude not found on PATH — run: claude --resume <id>" as fallback.

---

## MVP Definition

### Launch With (v1)

Minimum viable product — validates the core "effortless session handoff between devices" promise.

- [ ] `cclink init` — Generate and store Ed25519 keypair at `~/.cclink/keys` — *without identity, nothing else works*
- [ ] `cclink whoami` — Show public key and homeserver URL — *sanity check for "am I set up correctly?"*
- [ ] `cclink publish` (default: self-encrypt, default TTL 8h) — Discover latest session, build payload, encrypt, publish to homeserver — *the entire reason the tool exists*
- [ ] `cclink pickup` — Retrieve and decrypt own latest handoff, print session ID — *the other half of the core loop*
- [ ] `cclink list` — Show active handoff records with status — *required to answer "what did I publish?"*
- [ ] `cclink revoke` — Delete a specific record — *users need control; without this, publish feels permanent/scary*
- [ ] TTL enforcement (default 8h, configurable) — *prevents record accumulation; expected in any secret-sharing tool*
- [ ] Colored terminal output + status indicators — *CLI UX table stakes; uncolored output feels unfinished*
- [ ] QR code after publish — *the cross-device UX differentiator that makes the tool feel magical*

### Add After Validation (v1.x)

Add once core publish/pickup loop is working and users have adopted it.

- [ ] `cclink pickup --exec` — Auto-run `claude --resume <id>` — *add when users report the copy-paste step is friction*
- [ ] `--burn` flag — burn-after-read mode — *add when security-conscious users ask for it; implement is low complexity*
- [ ] `--pin` flag — PIN-protected handoffs — *add when shared-device or convenience use cases emerge; document security limitations prominently*
- [ ] `--share` flag — recipient-key encryption — *add when users want to hand off to a second identity or device with a different keypair*

### Future Consideration (v2+)

Defer until product-market fit is established.

- [ ] Team / shared namespace handoffs — requires org-level identity model; v2 SaaS territory
- [ ] Web UI / dashboard — separate product; terminal-first is the right call for v1
- [ ] Claude Code hook / plugin integration — powerful but requires Anthropic cooperation or hook API stability
- [ ] Mobile app — not the target persona for v1; phone access via SSH + QR pickup covers the use case

---

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| keypair init / whoami | HIGH | LOW | P1 |
| publish (self-encrypt, TTL) | HIGH | MEDIUM | P1 |
| pickup (decrypt, print ID) | HIGH | MEDIUM | P1 |
| list active records | HIGH | LOW | P1 |
| revoke record | HIGH | LOW | P1 |
| TTL expiry | HIGH | LOW | P1 |
| colored output / status | MEDIUM | LOW | P1 |
| QR code after publish | HIGH | LOW | P1 |
| --exec on pickup | MEDIUM | LOW | P2 |
| --burn flag | MEDIUM | LOW | P2 |
| --pin flag | MEDIUM | MEDIUM | P2 |
| --share flag | MEDIUM | MEDIUM | P2 |
| latest.json zero-arg pickup | MEDIUM | LOW | P2 |
| team/shared namespaces | LOW | HIGH | P3 |
| web UI | LOW | HIGH | P3 |
| session content transfer | LOW | HIGH | P3 (anti-feature) |

**Priority key:**
- P1: Must have for launch
- P2: Should have, add when possible
- P3: Nice to have, future consideration

---

## Competitor Feature Analysis

| Feature | Depot claude sessions | nlashinsky/claude-code-handoff | Sonovore/claude-code-handoff | cli-continues | cclink |
|---------|----------------------|-------------------------------|------------------------------|---------------|--------|
| Cross-device session transfer | YES (cloud-synced) | YES (file-based, manual) | NO (same-machine focus) | YES (session injection) | YES (decentralized) |
| Encryption | YES (Depot API, account-gated) | NO (plaintext JSON) | NO (plaintext markdown) | NO (plaintext extraction) | YES (age, E2E) |
| No account required | NO (requires Depot org) | YES | YES | YES | YES |
| Team / org sharing | YES | NO | NO | YES (cross-tool) | NO (v2) |
| Burn-after-read | NO | NO | NO | NO | YES (--burn) |
| TTL / auto-expiry | YES (org config) | NO | NO | NO | YES |
| QR code for pickup | NO | NO | NO | NO | YES |
| Decentralized transport | NO (Depot API) | NO (git/file) | NO (file) | NO (local) | YES (Pubky/DHT) |
| Session content transfer | YES (full context) | YES (markdown summary) | YES (markdown summary) | YES (injection) | NO (by design) |
| Auto-execute on pickup | NO | NO | NO | YES | YES (--exec) |
| PIN protection | NO | NO | NO | NO | YES (--pin) |
| Recipient-key sharing | NO | NO | NO | NO | YES (--share) |

### Analysis Notes

**Depot** is the closest functional competitor for cross-device session transfer. It wins on team/org features but loses on: requires SaaS account, no encryption transparency, no burn-after-read, no QR code, no decentralized transport. Depot is cloud-infrastructure; cclink is cryptographic identity.

**File-based handoff tools** (nlashinsky, Sonovore) solve a different problem — context preservation across context-limit resets on the *same machine*. They are not session ID transfer tools. They're complements, not competitors.

**cli-continues** solves cross-tool session injection (Claude to Gemini, etc.). It does not solve cross-device transfer. It's also plaintext — no encryption. Not a competitor.

**cclink's unique position:** The only tool in this space that is (a) cross-device, (b) end-to-end encrypted, (c) requires no account, (d) uses a decentralized transport, and (e) includes UX primitives like QR code, burn-after-read, and PIN protection from a single binary.

---

## Sources

- Depot Claude Code Sessions announcement: https://depot.dev/blog/now-available-claude-code-sessions-in-depot (MEDIUM confidence — official blog post)
- Claude Code session export/import feature request: https://github.com/anthropics/claude-code/issues/18645 (HIGH confidence — official repo)
- Claude Code session handoff continuity request: https://github.com/anthropics/claude-code/issues/11455 (HIGH confidence — official repo)
- nlashinsky/claude-code-handoff: https://github.com/nlashinsky/claude-code-handoff (HIGH confidence — direct source inspection)
- Sonovore/claude-code-handoff: https://github.com/Sonovore/claude-code-handoff (HIGH confidence — direct source inspection)
- cli-continues (cross-tool handoff): https://github.com/yigitkonur/cli-continues (HIGH confidence — direct source inspection)
- One-time secret sharing features guide: https://cipherprojects.com/blog/posts/complete-guide-one-time-secret-sharing-tools-2025/ (MEDIUM confidence — industry survey)
- Magic-wormhole QR and secure transfer patterns: https://magic-wormhole.readthedocs.io/en/latest/welcome.html (HIGH confidence — official docs)
- CLIG.dev CLI UX standards: https://clig.dev/ (HIGH confidence — community standard)
- tmux+SSH+Claude Code handoff workflow: https://elliotbonneville.com/phone-to-mac-persistent-terminal/ (MEDIUM confidence — practitioner blog)
- Pubky Core protocol: https://docs.pubky.org/ (MEDIUM confidence — official docs, nascent protocol)
- QRClip CLI secure terminal-to-phone transfer: https://github.com/qrclip/qrclip-cli (MEDIUM confidence — open source reference)

---

*Feature research for: CLI session handoff / developer session management (cclink)*
*Researched: 2026-02-21*
