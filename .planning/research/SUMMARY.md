# Project Research Summary

**Project:** cclink
**Domain:** Rust CLI — decentralized session handoff with PKARR identity, Pubky homeserver, and age encryption
**Researched:** 2026-02-21
**Confidence:** MEDIUM-HIGH (stack HIGH; features MEDIUM; architecture MEDIUM-HIGH; pitfalls MEDIUM-HIGH)

## Executive Summary

cclink is a single-binary Rust CLI that solves cross-device session handoff for Claude Code users: publish an encrypted session reference from one machine, retrieve and resume it on another. The right implementation approach is a layered Rust binary: `clap` for subcommand dispatch, `pubky 0.6.0` + `pkarr 5.0.3` for decentralized authenticated transport, `age` for encryption, and `ssh-to-age` to bridge the Ed25519 keypair used for identity into X25519 keys used for encryption — eliminating the need for a separate age keypair entirely. The architecture follows a strict module boundary pattern: thin transport wrapper over the pubky SDK, pure crypto functions with no I/O, and command-per-file dispatch through a normalized `AppCtx`. This structure is especially important because the pubky SDK is at v0.6.0 with active development; isolating it to one file insulates the rest of the codebase from API churn.

The feature landscape reveals that cclink occupies a genuinely unique position: it is the only tool in the Claude Code handoff space that is cross-device, end-to-end encrypted, requires no account, uses a decentralized transport, and packages ergonomic UX primitives (QR code, burn-after-read, PIN mode) in a single binary. Direct competitors either require SaaS accounts (Depot), lack encryption (nlashinsky, cli-continues), or address a different problem entirely (same-machine context summarizers). The v1 scope is well-defined: keypair init, publish with self-encryption and TTL, pickup with QR display, list, and revoke. Advanced encryption modes (`--pin`, `--share`, `--burn`, `--exec`) are v1.x additions once the core loop is validated.

The critical risks fall into three clusters: (1) cryptographic correctness — the Ed25519-to-X25519 conversion must go through an established library (`ssh-to-age`) not manual bit manipulation, PIN mode requires Argon2id not raw HKDF, and the age `StreamWriter::finish()` call must be explicit and round-trip tested; (2) key management safety — atomic writes for the key file, 0600 permission enforcement on every load, and a `--force` guard on `cclink init` to prevent silent identity loss; and (3) Pubky/PKARR integration clarity — session data belongs on the homeserver via PUT, never in PKARR DNS records (which have a hard 1000-byte limit), and pickup must include retry with backoff to handle DHT propagation delay. Getting any of these wrong in early phases creates painful migration paths later.

## Key Findings

### Recommended Stack

The stack is well-determined with HIGH confidence across all core libraries. The pubky crate at v0.6.0 (released 2026-01-15) is the only correct choice for Pubky protocol interaction — using raw HTTP bypasses auth header signing, PKDNS resolution, and session management that the SDK provides. The `pkarr 5.0.3` crate provides the canonical `Keypair` type expected by the SDK; using `ed25519-dalek` directly would require conversion shims that are both unnecessary and error-prone. The `ssh-to-age 0.2.0` crate (released June 2025) solves the Ed25519-to-X25519 conversion in one line and eliminates the need for manual curve arithmetic where bugs hide.

Key version compatibility constraints: `pubky 0.6.0` and `pkarr 5.0.3` were released together and must match; `age 0.11.2` and `ssh-to-age 0.2.0` are paired (ssh-to-age targets age 0.11.x); `tokio 1.49.0` with `features = ["full"]` is required by the pubky SDK and must not be mixed with async-std. Avoid `openssl` features, `sodiumoxide`, and `ratatui` — the first two break musl static builds, the last is gross overkill for a status-output CLI.

**Core technologies:**
- `pubky 0.6.0`: Pubky homeserver client and PKARR identity — only correct choice, no alternative
- `pkarr 5.0.3`: Ed25519 keypair (canonical SDK identity type) — must match pubky version
- `age 0.11.2`: File and payload encryption — pure Rust, stable wire format, well-audited
- `ssh-to-age 0.2.0`: Ed25519-to-X25519 conversion — eliminates manual curve arithmetic risk
- `clap 4.5.60`: Subcommand dispatch with derive macros — de facto standard, used by pubky-cli
- `tokio 1.49.0`: Async runtime — required by pubky SDK, use `features = ["full"]`
- `hkdf 0.12.4` + `sha2 0.10.9`: Key derivation for PIN mode — pair with Argon2id (see pitfalls)
- `qr2term 0.3.3`: Terminal QR rendering — one-line API, no rendering plumbing required
- `anyhow 1.0.102` + `thiserror 2.0.18`: Error handling — thiserror for typed domain errors, anyhow at command boundary

### Expected Features

No direct competitors exist for cclink's exact combination of properties. The adjacent landscape (Depot, file-based handoff tools, magic-wormhole) informed table stakes expectations. The v1 scope is clear and validated against competitor analysis: cclink's differentiators (QR code, no account, E2E encryption, decentralized transport) are all achievable in v1 without the anti-features that would delay or muddy the product (session content transfer, team namespaces, web UI).

**Must have (table stakes):**
- `cclink init` / `cclink whoami` — keypair lifecycle, identity sanity check
- `cclink publish` (self-encrypt, default 8h TTL) — core value; without this there is no product
- `cclink pickup` — retrieve and decrypt own latest handoff; the other half of the loop
- `cclink list` — show active records; users expect to see what they've published
- `cclink revoke` — delete a record; publish feels permanent and scary without this
- TTL enforcement — expected by users of any secret-sharing tool; prevents accumulation
- Colored terminal output + status indicators — CLIG.dev table stakes; uncolored output feels unfinished
- QR code after publish — the cross-device UX differentiator; renders on mobile without clipboard copy

**Should have (competitive):**
- `--exec` on pickup — auto-runs `claude --resume <id>`; eliminates the last copy-paste step
- `--burn` flag — burn-after-read (best-effort); no competitor has this
- `--pin` flag — PIN-protected handoffs (Argon2id-derived, documented as convenience not security)
- `--share <pubkey>` — recipient-key encryption; enables two-identity handoffs
- `latest.json` zero-argument pickup — `cclink pickup` with no args just works

**Defer (v2+):**
- Team / shared namespace handoffs — requires org-level identity model; v2 SaaS territory
- Web UI / dashboard — separate product; terminal is the correct interface for this persona
- Session content transfer — out of scope by design; scope to session ID + metadata only
- Push notifications — requires notification infrastructure incompatible with stateless CLI design

### Architecture Approach

The architecture is layered with clear module boundaries designed to isolate the riskiest dependency (pubky SDK) and keep crypto pure. The single `AppCtx` struct normalizes all configuration sources before any command logic runs — this pattern, recommended by the clap author and Rain's Rust CLI guide, makes command handlers testable without re-parsing CLI args. Commands are one-file-per-subcommand in `src/commands/`, each with its own `Args` struct and `run(ctx, args) -> Result<()>` function. The architecture research identifies a clear phase dependency DAG (keys → crypto → transport → commands → polish), which directly informs roadmap phase ordering.

**Major components:**
1. CLI Layer (`src/cli.rs`, `src/commands/`) — clap derive macros, one file per subcommand, dispatch via `commands::dispatch()`
2. Context / Config (`src/ctx.rs`, `src/config.rs`) — normalized `AppCtx` struct; single source of truth for all resolved config
3. Key Store (`src/keys/store.rs`, `src/keys/convert.rs`) — atomic write, 0600 enforcement, Ed25519-to-X25519 conversion isolated here
4. Crypto Engine (`src/crypto/`) — pure functions, no I/O, no network; encrypt/decrypt with all three modes; HKDF in separate file
5. Session Discovery (`src/session/discover.rs`) — read-only scan of `~/.claude/sessions/`; sorted by mtime
6. Pubky Transport (`src/transport/pubky_client.rs`) — thin wrapper over pubky SDK; only file that imports the SDK directly
7. Output (`src/output/`) — presentation only; commands call typed output functions, never format strings themselves

### Critical Pitfalls

1. **PIN mode using raw HKDF on a low-entropy PIN** — HKDF is not a password KDF; 10,000 PINs are brute-forceable in milliseconds. Use Argon2id before HKDF. This must be correct from day one — changing the KDF after publish invalidates all existing PIN-protected records and requires a migration path.

2. **`age StreamWriter::finish()` not called** — omitting this call produces silently truncated ciphertext that compiles, appears to succeed, and only fails on the pickup device with a cryptic decryption error. Always call `finish()` explicitly and write a round-trip CI test (encrypt then decrypt in the same test) for every code path.

3. **Ed25519 private key stored in plaintext at rest** — 0600 file permissions do not protect against same-user processes, accidental copies, or memory scanning. At minimum: verify 0600 on every load; use atomic write (write-to-temp-then-rename) so a partial write never corrupts the permanent identity; add a guard (`--force` flag) to `cclink init` to prevent silent overwrite.

4. **PKARR 1000-byte limit confusion** — PKARR DNS records have a hard 1000-byte limit and are for identity/discovery only. Session payload data must go to the Pubky homeserver via PUT, never into PKARR records. This architectural boundary must be established before the first integration test or publish logic will silently fail at scale.

5. **DHT propagation delay causing "publish succeeded, pickup fails" errors** — PKARR DHT propagation is eventual-consistent; a fresh resolution from a different network can take seconds to tens of seconds. Pickup must implement retry with exponential backoff (3 retries at 2s/4s/8s) and display a spinner during resolution. The error message must distinguish "not found yet (retry)" from "decryption failed (wrong key)".

6. **Key file corruption on partial write** — `fs::write()` is not atomic; a crash during `cclink init` produces a truncated key file with no recovery path. Use write-to-temp-then-rename (`key_path.with_extension("tmp")` → `fs::rename()`). This is the same bug documented in the `confy` crate.

7. **Burn-after-read is not atomic** — GET-then-DELETE is inherently racy; two simultaneous pickups can both succeed. Accept and document this limitation explicitly in `--help` output. Do not claim atomic semantics the homeserver does not provide.

## Implications for Roadmap

Based on the architecture's dependency DAG and the critical pitfalls' phase warnings, six phases emerge naturally. The ordering is dictated by hard dependencies: you cannot encrypt without keys, cannot transport without encryption and auth, cannot deliver the user flow without transport.

### Phase 1: Project Foundation and Key Management

**Rationale:** Keypair generation and persistence are the dependency root for everything else. The three key-management pitfalls (plaintext storage, partial write, init overwrite) must be solved here — they cannot be retrofitted later without a migration path for users' identities.

**Delivers:** `cclink init`, `cclink whoami`, atomic key file write, 0600 permission enforcement, Ed25519-to-X25519 conversion with known-answer tests. Binary compiles and ships a working `init` command.

**Addresses features:** keypair init / whoami (P1 table stakes)

**Avoids pitfalls:** Plaintext key at rest (Pitfall 3), key file corruption on partial write (Pitfall 7), `cclink init` overwrite without warning (UX pitfall). This phase establishes the correct patterns before any network or crypto code is written.

**Research flag:** Standard patterns — clap, pkarr::Keypair, and atomic file writes are all well-documented.

### Phase 2: Core Encryption (Self-Encrypt Mode)

**Rationale:** Crypto correctness is the hardest part to retrofit. Establish the encrypt/decrypt pipeline for the default self-encrypt mode before touching the network layer. Round-trip tests in CI catch the `StreamWriter::finish()` pitfall and key conversion errors unconditionally.

**Delivers:** `crypto/encrypt.rs` and `crypto/decrypt.rs` (self-encrypt mode only), `keys/convert.rs` with unit tests against known Ed25519-to-X25519 vectors, round-trip CI test for every encrypted code path. No network calls yet.

**Addresses features:** The encryption foundation that all handoff modes depend on

**Uses:** `age 0.11.2` (x25519 feature), `ssh-to-age 0.2.0`, `zeroize` for secret key cleanup

**Avoids pitfalls:** `StreamWriter::finish()` omission (Pitfall 2), incorrect Ed25519-to-X25519 conversion (security mistake), logging key material.

**Research flag:** Standard patterns — age crate x25519 API and Filippo Valsorda's conversion writeup are well-documented and HIGH confidence.

### Phase 3: Pubky Transport Layer

**Rationale:** Wrap the pubky SDK before any command code uses it. The SDK is at v0.6.0 with active development; isolating it to `transport/pubky_client.rs` means SDK API changes require one file to update, not every command. Establish the PKARR/homeserver architectural boundary here to prevent Pitfall 4.

**Delivers:** `transport/pubky_client.rs` (put_record, get_record, delete_record), `transport/record.rs` (HandoffRecord JSON type with serde), homeserver authentication flow, and integration tests confirming the boundary: session data via PUT, identity only via PKARR.

**Uses:** `pubky 0.6.0`, `pkarr 5.0.3`, `serde / serde_json`, `tokio` async runtime

**Avoids pitfalls:** PKARR 1000-byte limit confusion (Pitfall 4), Pubky homeserver authentication session expiry (integration gotcha).

**Research flag:** Needs attention — pubky SDK is MEDIUM confidence (active development, rc versions). The thin-wrapper pattern directly mitigates API churn risk, but integration tests against a real homeserver are required to validate PUT/GET/DELETE semantics.

### Phase 4: Core Commands (Publish / Pickup / List / Revoke)

**Rationale:** The first end-to-end user flow. All dependencies from Phases 1-3 are in place. Session discovery, publish, pickup, list, and revoke compose the core product loop. The retry/backoff logic for DHT propagation (Pitfall 6) belongs here alongside the first integration test on a real network path.

**Delivers:** `cclink publish` (discover session, encrypt, PUT, update latest.json, render QR), `cclink pickup` (GET, decrypt, print session ID), `cclink list`, `cclink revoke`, TTL enforcement client-side, retry with backoff, QR code after successful publish. First working end-to-end flow.

**Addresses features:** All P1 must-have features: publish, pickup, list, revoke, TTL, QR code, colored output

**Avoids pitfalls:** DHT propagation delay (Pitfall 6 — retry + spinner), latest.json non-atomic pointer update (write data record first, update pointer second), QR displayed before network confirmation (UX pitfall).

**Research flag:** Standard patterns for command structure and session discovery. The Pubky homeserver list API may need research during planning — confirm pagination behavior.

### Phase 5: Advanced Encryption Modes

**Rationale:** Once the core publish/pickup loop is working and tested, add the three encryption variants. Each is isolated to the crypto module and adds flags without changing the core flow. PIN mode is highest complexity due to the Argon2id requirement.

**Delivers:** `--pin` flag (Argon2id + HKDF, with explicit low-entropy documentation in `--help`), `--share <pubkey>` flag (recipient age encryption via pkarr DHT resolution), `--burn` flag (best-effort delete-on-read with explicit "not guaranteed" documentation), `--exec` flag, `latest.json` zero-argument pickup.

**Uses:** `hkdf 0.12.4`, `sha2 0.10.9`, Argon2 (add to Cargo.toml in this phase), `pkarr` DHT resolution for recipient lookup

**Avoids pitfalls:** Raw HKDF on PIN without Argon2 (Pitfall 1 — the most security-critical constraint), burn-after-read race condition documented honestly (Pitfall 5), `--pin` and `--share` conflict handled with clear error message.

**Research flag:** Needs research — Argon2id parameter selection (memory/time tradeoffs for a CLI tool) and the pkarr DHT resolution API for recipient lookup are MEDIUM confidence and warrant a `/gsd:research-phase` pass before implementation.

### Phase 6: Release and Distribution

**Rationale:** Binary release automation and cross-compilation polish. The core product is complete at Phase 5; this phase makes it distributable.

**Delivers:** `cargo-dist` configuration for GitHub release artifacts (Linux musl static, macOS, Windows), musl static binary validation (`cross build --target x86_64-unknown-linux-musl`), CI pipeline with round-trip encryption tests and key-material grep checks, `cargo-release` version flow.

**Uses:** `cargo-dist v0.30.0`, `cross`, `cargo-release`

**Research flag:** Standard patterns — cargo-dist documentation is MEDIUM confidence but the tool is well-established for this use case.

### Phase Ordering Rationale

- Keys before crypto: the conversion function in `keys/convert.rs` is imported by `crypto/`; crypto cannot be written without it.
- Crypto before transport: the transport layer publishes encrypted bytes; it cannot be integration-tested without working encryption.
- Transport before commands: commands orchestrate key + crypto + transport; all three must exist first.
- Core commands before advanced modes: the flags are additive; building them on a tested core prevents interaction bugs.
- Release last: distribution is pure packaging; no logic changes required.

This ordering also maps directly to pitfall prevention phases: the three highest-severity pitfalls (key corruption, HKDF without Argon2, StreamWriter::finish()) are addressed in Phases 1-2 before any user-facing code exists.

### Research Flags

Phases needing deeper research during planning:
- **Phase 3 (Transport):** pubky SDK API is MEDIUM confidence and under active development; validate PUT/GET/DELETE semantics, authentication session management, and list API pagination against actual SDK source or homeserver.
- **Phase 5 (Advanced Encryption):** Argon2id parameter selection for CLI UX tradeoffs, and pkarr DHT recipient resolution API, both need verification. The burn-after-read homeserver semantics (does the Pubky homeserver support conditional operations?) require explicit confirmation.

Phases with standard patterns (safe to skip research-phase):
- **Phase 1 (Foundation):** clap, pkarr::Keypair, atomic file writes — all HIGH confidence from official docs.
- **Phase 2 (Crypto):** age x25519 API, ssh-to-age conversion, zeroize — all HIGH confidence.
- **Phase 4 (Core Commands):** Command module structure and session discovery are standard Rust CLI patterns.
- **Phase 6 (Release):** cargo-dist and cross compilation are standard toolchain operations.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All core libraries verified via docs.rs at specific versions; pubky 0.6.0 + pkarr 5.0.3 release confirmed 2026-01-15; ssh-to-age 0.2.0 confirmed June 2025 |
| Features | MEDIUM | No direct competitors; landscape assembled from adjacent tools (Depot, file-based handoff tools, magic-wormhole); feature expectations are well-reasoned but not validated against actual cclink user research |
| Architecture | MEDIUM-HIGH | Rust CLI patterns (AppCtx, command-per-module, thin transport wrapper) are HIGH confidence from authoritative sources (clap author, Rain's recommendations); pubky SDK API details are MEDIUM due to active rc development |
| Pitfalls | MEDIUM-HIGH | Cryptographic pitfalls are HIGH confidence (Trail of Bits, Filippo Valsorda, age crate docs); Pubky-specific pitfalls are MEDIUM (limited docs, ecosystem still nascent) |

**Overall confidence:** MEDIUM-HIGH

### Gaps to Address

- **Pubky homeserver list API pagination behavior:** FEATURES.md notes that `cclink list` fetching each record individually is a performance trap above 20 records. The correct list API call against the Pubky homeserver needs verification during Phase 3/4 planning.
- **Argon2id parameters for CLI UX:** The correct memory/time parameters for Argon2id in a CLI context (target ~100ms-500ms on a 2023 machine) need a specific recommendation before Phase 5 implementation. Current research says "use Argon2id" but not what parameters.
- **Pubky homeserver conditional delete support:** The burn-after-read pitfall explicitly notes that the Pubky homeserver may not support compare-and-swap or conditional-delete. This must be confirmed during Phase 5 planning to determine whether to promise any atomicity guarantee or document it as best-effort only.
- **Session UUID discovery format stability:** `cclink publish` depends on enumerating `~/.claude/sessions/` to find the most recent session UUID. The format and modification time behavior of Claude Code's session directory is not documented as stable API. This is a latent breakage risk — verify against the actual Claude Code release being targeted.
- **`claude --resume <id>` CLI flag stability:** The `--exec` feature depends on `claude --resume` accepting a session UUID as a positional argument. Confirm this flag exists and is stable before implementing Phase 5.

## Sources

### Primary (HIGH confidence)
- `docs.rs/pubky/0.6.0/pubky/` — SDK APIs, signup/signin/storage methods
- `github.com/pubky/pubky-core` — pubky-core v0.6.0 released 2026-01-15
- `docs.pubky.org/Explore/PubkyCore/API` — PUT/GET/DELETE paths, auth header format
- `docs.rs/pkarr/latest/pkarr/struct.Keypair.html` — Keypair API
- `docs.rs/age/latest/age/` — age 0.11.2, x25519 encryption, StreamWriter::finish() requirement
- `lib.rs/crates/ssh-to-age` — ssh-to-age 0.2.0, Ed25519-to-X25519 conversion
- Filippo Valsorda: `words.filippo.io/using-ed25519-keys-for-encryption/` — birational map cryptographic basis and security analysis
- Trail of Bits: `blog.trailofbits.com/2025/01/28/best-practices-for-key-derivation/` — HKDF vs password KDFs
- Kevin K's Blog `kbknapp.dev/cli-structure-01/` — Context Struct pattern (clap author)
- Rain's Rust CLI Recommendations: `rust-cli-recommendations.sunshowers.io` — App/Command/Args hierarchy
- PKARR docs: `pubky.github.io/pkarr` — 1000-byte limit, DHT TTL/caching
- `github.com/pubky/pubky-cli` — per-command module pattern with pubky SDK
- confy issue #47: non-atomic write bug in config save
- `docs.rs` for all listed crates: clap, tokio, qr2term, indicatif, console, dialoguer, hkdf, sha2, zeroize, anyhow, thiserror, serde, serde_json, dirs

### Secondary (MEDIUM confidence)
- `docs.pubky.org` — Pubky protocol docs (work-in-progress, nascent protocol)
- Depot Claude Code Sessions: `depot.dev/blog/now-available-claude-code-sessions-in-depot` — competitor feature analysis
- `github.com/nlashinsky/claude-code-handoff` — file-based handoff tool (direct source inspection)
- `github.com/Sonovore/claude-code-handoff` — same-machine context tool (direct source inspection)
- `github.com/yigitkonur/cli-continues` — cross-tool session injection (direct source inspection)
- Magic-wormhole docs: `magic-wormhole.readthedocs.io` — QR and secure transfer UX patterns
- CLIG.dev: `clig.dev` — CLI UX standards
- `github.com/axodotdev/cargo-dist` — cargo-dist v0.30.0 (Sept 2025)
- Rust Forum: real-world age crate decryption failure discussion
- USENIX NSDI 2024: DHT propagation delay empirical data
- eprint.iacr.org 2021/509: key reuse security analysis for Ed25519+X25519

### Tertiary (LOW confidence)
- WebSearch: musl static binary best practices 2025 — needs validation against actual pubky dependency tree

---
*Research completed: 2026-02-21*
*Ready for roadmap: yes*
