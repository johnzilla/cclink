# Pitfalls Research

**Domain:** Rust CLI with Ed25519 key management, age encryption, and Pubky/PKARR decentralized publishing
**Researched:** 2026-02-21
**Confidence:** MEDIUM-HIGH (cryptographic pitfalls HIGH from official docs; Pubky-specific pitfalls MEDIUM from limited docs + ecosystem signals)

---

## Critical Pitfalls

### Pitfall 1: Using HKDF Directly on a Low-Entropy PIN

**What goes wrong:**
The `--pin` mode derives an encryption key from a 4-digit PIN using HKDF. HKDF is designed for cryptographically strong input key material — it is a randomness *extractor*, not a password strengthener. A 4-digit PIN has roughly 13 bits of entropy. HKDF applied directly to this yields a key that a password cracker can brute-force in milliseconds against the ciphertext.

**Why it happens:**
Developers see "key derivation function" and assume HKDF handles weak inputs safely. It does not. HKDF-only derivations are efficient by design, which means attackers can iterate all 10,000 PIN values trivially. The project notes acknowledge "low entropy" but the implementation path matters enormously.

**How to avoid:**
Use a memory-hard, deliberately slow KDF (Argon2id) to hash the PIN first, then use HKDF to expand that output into the encryption key. The correct chain is:

```
PIN → Argon2id(salt=random, m=64MB, t=3, p=1) → 32-byte intermediate → HKDF-Expand → encryption key
```

The Argon2 salt must be stored alongside the ciphertext. The PROJECT.md acknowledges low entropy — this acknowledgment must translate into Argon2 in the implementation, not just a comment.

**Warning signs:**
- Code uses `hkdf::Hkdf::new(salt, pin_bytes)` without a preceding Argon2/scrypt/bcrypt call
- Tests for PIN decryption run in under 1ms (should take ~100ms-1s with proper KDF)
- No salt persisted alongside the encrypted payload for PIN-derived keys

**Phase to address:**
Encryption implementation phase (PIN-based handoff feature). Must be correct from day one — changing the KDF after publish invalidates all existing PIN-protected records.

---

### Pitfall 2: Forgetting `StreamWriter::finish()` Produces Silently Truncated Ciphertext

**What goes wrong:**
The `age` crate's streaming encryption API requires an explicit call to `StreamWriter::finish()` after writing all plaintext. Omitting this call produces a truncated encrypted file that fails to decrypt — with no compile-time or immediate runtime error. The write appears to succeed. The error surfaces only on pickup, on a different machine, with a cryptic decryption failure.

**Why it happens:**
The `finish()` step flushes the final STREAM chunk and appends the authentication tag. Rust's `Drop` does not call `finish()` automatically. Developers familiar with RAII assume dropping the writer finalizes it — it does not in this crate.

**How to avoid:**
Always call `finish()` explicitly and propagate the result:

```rust
let mut writer = encryptor.wrap_output(&mut output)?;
writer.write_all(&plaintext)?;
writer.finish()?; // REQUIRED — do not rely on Drop
```

Add a test that decrypts every encrypted payload in-process immediately after encryption to catch this in CI.

**Warning signs:**
- Encrypted output is produced without an explicit `finish()` call in the code path
- No round-trip test (encrypt then decrypt) in the test suite
- Decryption failures only appearing on pickup device but not caught by sender-side tests

**Phase to address:**
Encryption implementation phase. The round-trip test (encrypt → decrypt in same test) catches this unconditionally.

---

### Pitfall 3: Ed25519 Private Key Stored in Plaintext at Rest

**What goes wrong:**
The key file at `~/.cclink/keys` holds the Ed25519 private key — the permanent identity for all published records. Storing this as raw bytes (or base64/hex) with only filesystem permissions (0600) for protection means any process running as the same user, any malware, or any accidental `cat` into a log can exfiltrate it silently. Loss or exposure is permanent: records can be impersonated, past encrypted handoffs may be decryptable.

**Why it happens:**
File permission (0600) feels sufficient for a CLI tool. It is not — it provides no protection against the user's own processes, no protection if the file is accidentally copied, and no protection against memory-scanning attacks.

**How to avoid:**
Two viable approaches:
1. **OS keychain** (`keyring` crate): Stores the key in the OS credential store (libsecret/macOS Keychain/Windows Credential Manager). No plaintext file. Preferred for desktop use.
2. **Age-encrypted key file**: The key file itself is encrypted with age using a passphrase or the OS keychain as the master secret. The plaintext private key never touches disk unencrypted.

At minimum, verify 0600 permissions programmatically on every startup and refuse to operate if the key file has wider permissions.

**Warning signs:**
- Key file readable with `cat` without any passphrase prompt
- No permission check on key file load
- Key stored as a bare hex or base64 string in a JSON config file alongside non-sensitive config

**Phase to address:**
Key management initialization phase (`cclink init`). Getting this wrong at init means users need a migration path later, which is painful.

---

### Pitfall 4: PKARR Record Size Exceeding the 1000-Byte DNS Packet Limit

**What goes wrong:**
PKARR records are encoded as signed DNS packets with a hard 1000-byte limit. The handoff payload (`/pub/cclink/sessions/<token>.json`) is published to the Pubky homeserver's key-value store (not directly in PKARR records), but if the project conflates PKARR identity records with data records, or attempts to embed the handoff payload in the PKARR DNS packet, it will silently fail or be rejected above this limit.

**Why it happens:**
The 1000-byte limit is a core PKARR constraint that is easy to miss when reading Pubky SDK documentation. Developers assume the DHT can store arbitrary payloads. PKARR is designed for discovery (TXT records, homeserver pointers), not data storage.

**How to avoid:**
Maintain a clear architectural boundary: PKARR records contain only identity/discovery data (homeserver URL pointer). All session payload data goes to the Pubky homeserver key-value store via PUT at `/pub/cclink/sessions/<token>.json`. Never put session content in PKARR.

Monitor payload sizes in tests with assertions: `assert!(json_bytes.len() < 8192, "payload too large for homeserver record")`.

**Warning signs:**
- Code that builds a handoff record and attempts to publish it via `pkarr::Client::publish()` instead of `pubky::Client::put()`
- No size check before PUT to homeserver
- Encrypted payload growing as more metadata is added (hostname, project path, timestamps, nonces)

**Phase to address:**
Publish/pickup architecture phase. Architectural confusion between PKARR and Pubky homeserver must be resolved before first integration test.

---

### Pitfall 5: Burn-After-Read Race Condition Between Read and Delete

**What goes wrong:**
The `--burn` mode deletes a record after the first retrieval. If implemented as read-then-delete, two simultaneous `cclink pickup` calls on different machines (or a network retry) can both read the record successfully before either deletes it. The record is "burned" twice — both machines get the handoff, defeating the single-use intent.

**Why it happens:**
HTTP key-value stores (Pubky homeserver) do not provide compare-and-swap or conditional-delete primitives. Developers implement burn as `GET` → verify → `DELETE`, which is inherently racy.

**How to avoid:**
Accept the limitation explicitly. Options in order of practicality:
1. **Document it**: Burn-after-read is best-effort, not guaranteed. Two rapid retrievals can both succeed. This is acceptable for the single-user use case where concurrent pickup is unlikely.
2. **Short TTL as substitute**: Use a very short TTL (e.g., 1 hour) so the record expires quickly regardless of burn.
3. **Nonce-based tokens**: Include a one-time token in the record; the picking-up machine must present the token on delete to prove it was the first reader (requires server support that Pubky homeserver may not provide).

Do not promise atomic burn semantics in documentation or the `--help` text without verifying the homeserver supports conditional operations.

**Warning signs:**
- `--help` text says "deleted after first read" without a "best-effort" caveat
- Implementation is a sequential GET then DELETE with no error handling if DELETE fails
- No test covering what happens when DELETE returns 404 (already deleted by another caller)

**Phase to address:**
Burn-after-read feature phase. Design with explicit acknowledgment of the limitation in `--help` output.

---

### Pitfall 6: DHT Propagation Delay Causing Apparent "Publish Succeeded, Pickup Fails" Errors

**What goes wrong:**
PKARR DHT propagation is eventual-consistent. After `cclink` publishes a record, it may not be immediately resolvable from a different machine on a different network segment, especially on a cache miss. Users see "publish succeeded" then immediately try to `cclink pickup` from another device and get a "not found" error, leading them to believe the tool is broken.

**Why it happens:**
The PKARR docs note "traversing the DHT might take a few seconds" on cache miss. The Mainline DHT has 10M+ nodes; full propagation can take longer. The Pubky homeserver record itself is available immediately after PUT, but if the pickup device resolves the homeserver URL via PKARR (rather than a cached or configured URL), it must first traverse the DHT.

**How to avoid:**
1. Display propagation guidance after publish: "Record published. Allow a few seconds for DHT propagation before pickup on a new device."
2. On pickup failure, implement a retry loop with exponential backoff (e.g., 3 retries, 2s/4s/8s delay) before surfacing an error.
3. On `cclink pickup`, prefer homeserver direct URL (if known/cached) over DHT resolution.
4. The `latest.json` pointer is homeserver-side (fast) — the bottleneck is only if the pickup device does not know the homeserver URL and must resolve it from PKARR.

**Warning signs:**
- Pickup with `--timeout` defaulting to 0 or 100ms
- No retry logic in the pickup HTTP client
- Integration tests that test publish + pickup in the same process (will always pass) but no tests across real network paths

**Phase to address:**
Publish/pickup flow. The retry logic and user-facing propagation messaging belong in the first end-to-end integration test.

---

### Pitfall 7: Key File Corruption on Partial Write (Power Loss / Crash During `cclink init`)

**What goes wrong:**
Writing the private key to `~/.cclink/keys` with a plain `fs::write()` call is not atomic. A power failure or process kill during the write produces a truncated or zeroed key file. On next startup, the corrupted key file causes a parse error, and the user's identity is gone. There is no recovery path without a backup.

**Why it happens:**
`fs::write()` is not atomic on any common filesystem. The `confy` crate has a documented issue for this same pattern. Developers assume filesystem writes are atomic for small files.

**How to avoid:**
Use atomic write via write-to-temp-then-rename:

```rust
// Write to a temp file in the same directory (same filesystem = atomic rename)
let tmp_path = key_path.with_extension("tmp");
fs::write(&tmp_path, key_bytes)?;
fs::rename(&tmp_path, &key_path)?;
```

Or use the `atomic-write-file` crate directly. Set permissions to 0600 on the temp file *before* rename.

**Warning signs:**
- `fs::write(key_path, key_bytes)` with no temp-file intermediary
- No check on startup that the key file parses to a valid length (32 bytes for Ed25519 seed)
- No mention of backup/export in `cclink init` output

**Phase to address:**
Key management initialization phase. Fix before the first release — migration after the fact requires users to regenerate their identity (losing published records).

---

## Technical Debt Patterns

Shortcuts that seem reasonable but create long-term problems.

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Skip Argon2 on PIN, use HKDF directly | Simpler code, faster | All existing PIN-protected records become trivially brute-forceable | Never — security regression |
| Store private key as plaintext JSON field | Easier serialization/debugging | Key exposed in any debug output, logs, crash reports | Never |
| `fs::write()` for key file (non-atomic) | One line of code | Corrupted key on power failure = permanent identity loss | Never for key material |
| Hardcode homeserver URL instead of resolving from PKARR | No DHT latency | Users can't change their homeserver; portability broken | Only in test fixtures |
| Use `unwrap()` on decryption errors in pickup | Faster to write | Cryptic panic message when decryption fails; no actionable user message | Never in CLI user path |
| Skip `StreamWriter::finish()` or wrap in `Drop` override | Seems idiomatic | Silent truncation that only surfaces on remote device | Never |
| `latest.json` written non-atomically (two-step: write data + update pointer) | Simple implementation | Pointer points to non-existent or partial record if crash between steps | Never — write data record first, update pointer second |

---

## Integration Gotchas

Common mistakes when connecting to external services.

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Pubky homeserver PUT | Assuming PUT is idempotent with same path; not handling 409/conflict | Treat paths as unique tokens; generate a new UUID token per handoff |
| PKARR DHT publish | Calling DHT publish for each handoff record (session data) | DHT publish is for identity records only; session data goes to homeserver via PUT |
| age encryption with `x25519` | Constructing `x25519::Recipient` directly from raw Ed25519 public key bytes without conversion | Use the Ed25519→X25519 birational conversion (multiply y-coordinate by Montgomery map); the `ssh-to-age` crate or equivalent handles this |
| Pubky homeserver authentication | Assuming a static Bearer token; not handling session expiry | Use Pubky SDK session management; re-authenticate on 401 responses |
| DHT resolution timeout | Using default HTTP client timeout (no timeout) for DHT relay queries | Set explicit timeouts (e.g., 10s) and retry; DHT misses can hang indefinitely |
| `cclink pickup --exec` subprocess spawn | Using `std::process::Command::spawn()` and not waiting | Use `exec()` (Unix) to replace the current process, or `Command::spawn()` + `wait()` with signal propagation |

---

## Performance Traps

Patterns that work at small scale but fail as usage grows.

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Encrypting large payloads in memory (age streaming not used) | OOM on large session metadata | Use `age` streaming API with `wrap_output()` | Payload > available RAM (unlikely for session IDs, but relevant if metadata grows) |
| QR code rendering very large URLs | QR code too dense to scan reliably in terminal; version 40 (177×177) barely scannable | Keep the published URL short: use a `pubky://` URI scheme with just the key and token, not the full payload | Payload URL > ~2953 bytes encoded |
| Listing all records (`cclink list`) by fetching each record | N HTTP requests per list call | Paginate with the Pubky homeserver list API; cache the `latest.json` pointer | > 20 active handoff records |
| Synchronous DHT resolution blocking the CLI main thread | CLI appears frozen for 3-10 seconds | Use `tokio` async runtime; show a spinner during resolution | First pickup on a cold cache on a new network |

---

## Security Mistakes

Domain-specific security issues beyond general Rust/CLI security.

| Mistake | Risk | Prevention |
|---------|------|------------|
| Using Ed25519 private key bytes directly as X25519 private key without SHA-512 derivation | Incorrect decryption; potential subtle key reuse vulnerability | Follow RFC 8032: derive X25519 scalar via `SHA-512(ed25519_seed)[0..32]` with clamping; use a library that handles this (e.g., `ed25519-dalek` to `x25519-dalek` conversion path) |
| Logging or printing key material during error handling | Private key exposed in terminal history, log files, crash reports | Use `secrecy` crate (`SecretBox<[u8; 32]>`) for all key material; implement `Debug` to redact; never include key bytes in `anyhow` error context strings |
| PIN with no brute-force rate limiting (local offline attack) | 10,000 guesses take milliseconds without a slow KDF | Argon2id with memory-hard parameters before HKDF (see Pitfall 1) |
| Publishing homeserver URL that leaks device hostname | Hostname enumeration; cross-device correlation | Use a stable alias or omit hostname from published metadata; or make hostname opt-in |
| Using non-CSPRNG for token generation | Predictable session tokens enable record enumeration | Always use `rand::rngs::OsRng` for generating record tokens; never `rand::thread_rng()` without explicit OS-seeding check |
| TTL expiry relying on homeserver-side enforcement only | Records persist beyond intended lifetime if homeserver is compromised or self-hosted | Include TTL in the signed payload; pickup verifies `expires_at > now()` client-side regardless of server-side deletion |

---

## UX Pitfalls

Common user experience mistakes in this domain.

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Surfacing raw cryptographic error messages on decryption failure | "DecryptError: NoMatchingKeys" tells the user nothing actionable | Map all crypto errors to human messages: "Could not decrypt: wrong key, corrupted record, or wrong PIN" |
| `cclink init` overwrites existing key without warning | User permanently loses their Pubky identity | Check for existing key; require `--force` flag or prompt for confirmation; show current public key before overwrite |
| QR code displayed before network confirmation | User scans QR, moves to pickup device, record doesn't exist yet | Publish first, confirm 200 OK, then display QR |
| No feedback during DHT resolution (silent hang) | User thinks CLI is broken after 5 seconds of nothing | Show a spinner or progress message: "Resolving homeserver via DHT..." |
| `cclink pickup` failing with "not found" on first try post-publish | User assumes publish failed; retries publish creating duplicate records | Retry with backoff on pickup (see Pitfall 6); distinguish "not found" from "decryption failed" in error messages |
| Executing `claude --resume` before verifying session ID format | claude launched with invalid argument; confusing error from claude itself | Validate UUID format of session ID before `--exec`; show the session ID being resumed before executing |

---

## "Looks Done But Isn't" Checklist

Things that appear complete but are missing critical pieces.

- [ ] **Key init:** `cclink init` creates a key file — but does it verify the file is readable and parseable immediately after write? Does it display the public key so the user can record it?
- [ ] **Encryption:** age encryption produces output — but is `StreamWriter::finish()` called? Is there a round-trip decrypt test in CI?
- [ ] **PIN security:** PIN-based encryption compiles and runs — but is Argon2 (not just HKDF) in the derivation chain?
- [ ] **Publish:** Homeserver PUT returns 200 — but is the `latest.json` pointer updated atomically after the data record is confirmed?
- [ ] **Pickup:** Decryption succeeds in a unit test — but does it succeed after a real publish to a homeserver and retrieve via a different Pubky client instance?
- [ ] **Burn:** Record is deleted after retrieval — but is the DELETE failure (404, already gone) handled gracefully rather than surfaced as an error?
- [ ] **TTL expiry:** TTL is written into the record — but does pickup verify `expires_at` client-side and reject expired records even if the server hasn't deleted them yet?
- [ ] **Key file permissions:** Key file is created — but are permissions set to 0600 *before* writing key material, and checked on every load?
- [ ] **QR code:** QR code renders in terminal — but does it remain scannable with the actual encoded URL (not a short test string)?
- [ ] **`--exec` mode:** `claude --resume <id>` is launched — but does cclink wait for it correctly, propagate signals, and not leave zombie processes?

---

## Recovery Strategies

When pitfalls occur despite prevention, how to recover.

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Private key lost (corruption, accidental deletion) | HIGH | No recovery of published records encrypted to old key; `cclink init --force` creates new identity; old homeserver records orphaned |
| Private key exposed (logged, copied) | HIGH | Immediately run `cclink init --force` to generate new keypair; revoke all published records under old key; notify any recipients of share-mode handoffs |
| PIN-protected records with weak KDF discovered | MEDIUM | Re-publish all active records with corrected KDF; update binary and notify users to re-encrypt; old records remain vulnerable until TTL expiry |
| `latest.json` pointer corrupted (points to missing record) | LOW | Delete the pointer with `cclink revoke`; re-publish the most recent session |
| Homeserver unreachable at pickup | LOW | Retry with backoff; if persistent, publish to alternate homeserver (requires identity migration) |
| `StreamWriter::finish()` omitted — truncated record published | LOW | Delete the bad record; fix the bug; re-publish with corrected binary |

---

## Pitfall-to-Phase Mapping

How roadmap phases should address these pitfalls.

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| HKDF on PIN (no Argon2) | PIN-based encryption feature | Test: measure time to exhaustively try all 10,000 PINs against ciphertext; must take > 10 minutes |
| `StreamWriter::finish()` omission | Encryption implementation | CI: round-trip test (encrypt then decrypt in same test) for every code path |
| Plaintext key at rest | Key management init (`cclink init`) | Test: verify key file contains no recognizable plaintext key material; verify 0600 permissions |
| PKARR 1000-byte limit confusion | Architecture design before first integration | Integration test: assert payload size stays under 8KB for homeserver records; confirm PKARR records contain only identity data |
| Burn-after-read race condition | Burn feature implementation | Document limitation explicitly; test DELETE-after-read with a mock that returns 404 |
| DHT propagation delay | Publish/pickup UX flow | End-to-end test on real network with retry logic; UX: spinner + retry + human error message |
| Key file corruption on partial write | Key management init (`cclink init`) | Test: simulate interrupted write; verify startup detects and rejects corrupted key file |
| Ed25519→X25519 conversion errors | Encryption implementation | Test: known-answer test for key conversion against reference implementation vectors |
| Logging key material | Throughout all phases | CI: run with `RUST_LOG=trace` and grep output for key material patterns |
| `latest.json` non-atomic pointer update | Publish implementation | Test: simulate crash between data write and pointer update; verify pickup handles missing record gracefully |

---

## Sources

- Trail of Bits: [Best practices for key derivation](https://blog.trailofbits.com/2025/01/28/best-practices-for-key-derivation/) — HKDF vs password KDFs (HIGH confidence)
- Filippo Valsorda: [Using Ed25519 keys for encryption](https://words.filippo.io/using-ed25519-keys-for-encryption/) — Ed25519→X25519 conversion security (HIGH confidence)
- age crate docs: [docs.rs/age](https://docs.rs/age/latest/age/) — `StreamWriter::finish()` requirement, API beta status (HIGH confidence)
- PKARR docs: [pubky.github.io/pkarr](https://pubky.github.io/pkarr/) — 1000-byte limit, DHT TTL/republish, caching behavior (HIGH confidence)
- Pubky docs: [docs.pubky.org](https://docs.pubky.org/) — work-in-progress status, homeserver architecture (MEDIUM confidence)
- Rust Forum: [Having a decryption issue with the age crate](https://users.rust-lang.org/t/having-a-decryption-issue-with-the-age-crate/106664) — real-world age crate decryption failures (MEDIUM confidence)
- confy issue #47: [Doesn't atomically save to file](https://github.com/rust-cli/confy/issues/47) — non-atomic write bug (HIGH confidence, known issue)
- secrecy crate: [docs.rs/secrecy](https://docs.rs/secrecy/latest/secrecy/) — memory protection and logging prevention for key material (HIGH confidence)
- IPFS DHT latency paper: [USENIX NSDI 2024](https://www.usenix.org/system/files/nsdi24-wei.pdf) — DHT propagation delay empirical data (MEDIUM confidence, different DHT but same fundamentals)
- eprint.iacr.org: [On using the same key pair for Ed25519 and X25519 KEM](https://eprint.iacr.org/2021/509.pdf) — key reuse security analysis (MEDIUM confidence)

---
*Pitfalls research for: Rust CLI with Ed25519 key management, age encryption, Pubky/PKARR decentralized publishing*
*Researched: 2026-02-21*
