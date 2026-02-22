# Phase 2: Crypto and Transport - Research

**Researched:** 2026-02-21
**Domain:** Age X25519 encryption, Ed25519-to-X25519 key derivation, Pubky homeserver HTTP transport
**Confidence:** HIGH (core crypto/age/reqwest); MEDIUM (homeserver auth token implementation)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**HandoffRecord design:**
- No version field — keep minimal, handle format evolution later if needed
- JSON serialization — human-readable, easy to debug
- Split structure: metadata (hostname, project, timestamp, TTL) in cleartext, session ID age-encrypted as a separate blob field
- Include creator's public key as a field in the record — record is self-describing for authorship verification

**Homeserver path layout:**
- Flat layout: records at `/pub/cclink/<token>`
- Token is timestamp-based (Unix timestamp) — naturally sortable
- Latest pointer at `/pub/cclink/latest`
- latest.json contains token + summary metadata (project, hostname, created_at) so callers can show info without fetching the full record

**Signature & verification model:**
- Sign everything — signature covers the full record (metadata + encrypted blob) to prevent tampering with any field
- Signature is a base64-encoded field in the JSON record itself
- Signed content is canonical JSON (sorted keys, no whitespace) for deterministic verification regardless of serialization differences
- Hard fail on verification failure — treat record as nonexistent, print error, exit. No bypass flag.

**Crate & dependency choices:**
- `age` crate (str4d/age) for X25519 age encryption
- Prefer pkarr/pubky crates for Ed25519 signing and Ed25519-to-X25519 key conversion if available, fall back to ed25519-dalek ecosystem
- Prefer pubky-homeserver client crate for HTTP transport if available, fall back to reqwest
- Blocking/synchronous execution — no tokio async runtime. Use ureq or reqwest::blocking if pubky client isn't available.

### Claude's Discretion
- Exact canonical JSON implementation details
- Error type design for the crypto/transport layer
- Internal module organization
- Test fixture design for round-trip encryption tests

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope

</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| PUB-02 | Handoff record includes hostname, project path, creation timestamp, and TTL | HandoffRecord struct + serde_json serialization; serde is already in lock |
| PUB-03 | Session ID is age-encrypted to the creator's own X25519 key (derived from Ed25519) | age 0.11.2 + ed25519-dalek 3.0.0-pre.6 `to_scalar_bytes()` / `to_montgomery()`; bech32 0.9.x for key injection |
| PUB-05 | A `latest.json` pointer is updated on each publish | Same HTTP PUT path as records; identical transport path |
| UX-02 | Ed25519 signature verification on all retrieved records | pkarr `Keypair::sign()` + `PublicKey::verify()` already in project; canonical JSON via serde_json + BTreeMap |

</phase_requirements>

---

## Summary

Phase 2 builds three independent layers: (1) age X25519 encryption using the creator's own key derived from the Ed25519 keypair, (2) Ed25519 signing of the full HandoffRecord with canonical JSON for deterministic verification, and (3) HTTP transport to the Pubky homeserver using reqwest::blocking with cookie-based session authentication.

The critical dependency finding: the published `pubky` crate (0.6.0) requires `pkarr ^3.10.0` and is **incompatible** with the project's `pkarr 5.0.3`. The pubky-core repo's main branch uses pkarr 5.0.3, but that version is not yet published on crates.io. Therefore, HTTP transport MUST use `reqwest 0.13` blocking mode. The Pubky homeserver requires session cookie authentication for PUT operations, which means implementing a minimal `AuthToken` binary payload using `postcard` to POST to the `/session` endpoint. This is the single highest-complexity item in the phase.

The `age` crate (0.11.2) is the correct choice for encryption. It uses `x25519-dalek 2` / `curve25519-dalek 4` internally, which differs from pkarr's `curve25519-dalek 5.0.0-pre.6`. Cargo handles both as separate types with no conflict — they are never mixed. The key derivation path is: `SigningKey::to_scalar_bytes()` (from `ed25519-dalek 3.0.0-pre.6`, already in lock) gives the X25519 scalar bytes, which are bech32-encoded with the `age-secret-key-` HRP to construct an `age::x25519::Identity` via `from_str`.

**Primary recommendation:** Add `age 0.11.2`, `reqwest 0.13` (blocking + cookies), `postcard 1.x`, `bech32 0.9.x`, and `base64 0.22` as dependencies. Implement AuthToken signing manually (50 lines). Build three modules: `crypto` (age encrypt/decrypt + key derivation), `record` (HandoffRecord struct + canonical JSON signing), and `transport` (reqwest blocking client with signin/put/get).

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `age` | 0.11.2 | X25519 self-encryption of session ID blob | The reference age-encryption.org implementation in Rust; simple API |
| `reqwest` | 0.13 (blocking + cookies) | HTTP PUT/GET to Pubky homeserver | Already used by pkarr 5.0.3 (optional dep); blocking mode needs no runtime |
| `postcard` | 1.1.x | Serialize AuthToken for Pubky signin | Homeserver only accepts postcard binary tokens; no other format works |
| `serde_json` | 1.0 | JSON serialization of HandoffRecord | Already in lock (1.0.149); standard; BTreeMap default gives sorted keys |
| `serde` | 1.0 | Derive macros for HandoffRecord | Already in lock |
| `bech32` | 0.9.x | Encode X25519 scalar bytes for age Identity/Recipient | age 0.11.2 depends on bech32 ^0.9; same version avoids extra dep |
| `base64` | 0.22 | Encode Ed25519 signature bytes in JSON record | Standard encoding for binary fields in JSON |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `pkarr` | 5.0.3 (already in Cargo.toml) | Ed25519 sign + verify via `Keypair::sign()` and `PublicKey::verify()` | All signing/verification; already in project |
| `ed25519-dalek` | 3.0.0-pre.6 (transitive via pkarr) | `SigningKey::to_scalar_bytes()` and `VerifyingKey::to_montgomery()` for X25519 derivation | Key conversion only; access through pkarr's `Keypair` inner field |
| `curve25519-dalek` | 5.0.0-pre.6 (transitive via pkarr) | `MontgomeryPoint::to_bytes()` for X25519 public key bytes | Accessed through `VerifyingKey::to_montgomery().to_bytes()` |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `reqwest::blocking` | `ureq 2.x` | ureq is simpler for blocking HTTP but has no cookie jar; session auth needs cookies |
| Manual AuthToken | `pubky` crate via git dep | Git dep is unstable; manual impl is 50 lines and more predictable |
| `bech32 0.9.x` | `bech32 0.11.x` | age 0.11.2 requires `^0.9`; using same version avoids duplicate dep |
| `base64 0.22` | `hex` encoding | age spec uses bech32; handoff signature uses base64; base64 0.22 already in pkarr graph |

**Installation:**
```bash
# Add to Cargo.toml
age = "0.11.2"
reqwest = { version = "0.13", default-features = false, features = ["blocking", "cookies", "rustls-tls"] }
postcard = { version = "1", features = ["alloc"] }
bech32 = "0.9"
base64 = "0.22"
# serde, serde_json, pkarr already present
```

---

## Architecture Patterns

### Recommended Project Structure
```
src/
├── crypto/          # Age encryption/decryption + key derivation
│   └── mod.rs
├── record/          # HandoffRecord struct, canonical JSON, Ed25519 signing
│   └── mod.rs
├── transport/       # reqwest blocking client, Pubky signin + PUT/GET
│   └── mod.rs
├── commands/        # (existing: init, whoami)
├── keys/            # (existing: store, fingerprint)
├── cli.rs           # (existing)
├── error.rs         # (existing — extend with new variants)
└── main.rs          # (existing)
```

### Pattern 1: Ed25519-to-X25519 Key Derivation (via ed25519-dalek 3.x)

**What:** Convert pkarr `Keypair` (Ed25519) to X25519 scalar bytes for age encryption/decryption.

**When to use:** Before constructing `age::x25519::Identity` or `age::x25519::Recipient`.

```rust
// Source: ed25519-dalek 3.0.0-pre.6 signing.rs (local cargo cache)
// pkarr::Keypair wraps ed25519_dalek::SigningKey as .0

// Access the inner SigningKey through pkarr::Keypair
// pkarr Keypair is: pub struct Keypair(pub(crate) SigningKey);
// We must use the pub signing/verifying APIs to get the bytes.

// Secret scalar bytes (for Identity):
// SigningKey::to_scalar_bytes() -> [u8; 32]
// This is SHA-512(sk)[0..32] — the correct X25519 secret scalar

// Public Montgomery point bytes (for Recipient):
// VerifyingKey::to_montgomery() -> MontgomeryPoint
// MontgomeryPoint::to_bytes() -> [u8; 32]

// Through pkarr Keypair:
let secret_key: [u8; 32] = keypair.0.to_scalar_bytes();  // private field access needed
let pubkey_bytes: [u8; 32] = keypair.public_key().verifying_key().to_montgomery().to_bytes();
```

**Note:** `Keypair.0` is `pub(crate)` in pkarr, not accessible from outside. Use `keypair.secret_key()` which returns `SecretKey` = `[u8; 32]` (the raw Ed25519 seed), then reconstruct: `let signing_key = ed25519_dalek::SigningKey::from_bytes(&keypair.secret_key())`, then call `signing_key.to_scalar_bytes()`.

### Pattern 2: Injecting Derived X25519 Bytes into age

**What:** Create `age::x25519::Identity` and `age::x25519::Recipient` from raw bytes derived from Ed25519.

**When to use:** Constructing age encryptor/decryptor for self-encryption.

```rust
// Source: str4d/rage age/src/x25519.rs + bech32 0.9.x docs
// age x25519::Identity parses from Bech32 string with HRP "age-secret-key-" (case-insensitive parse)
// age x25519::Recipient parses from Bech32 string with HRP "age"

use bech32::{ToBase32, Variant};

fn x25519_identity_from_bytes(scalar_bytes: &[u8; 32]) -> age::x25519::Identity {
    let encoded = bech32::encode("age-secret-key-", scalar_bytes.to_base32(), Variant::Bech32)
        .expect("bech32 encode infallible for fixed input");
    // age parses uppercase HRP form "AGE-SECRET-KEY-..."
    encoded.to_uppercase().parse().expect("valid age identity")
}

fn x25519_recipient_from_bytes(pubkey_bytes: &[u8; 32]) -> age::x25519::Recipient {
    let encoded = bech32::encode("age", pubkey_bytes.to_base32(), Variant::Bech32)
        .expect("bech32 encode infallible");
    encoded.parse().expect("valid age recipient")
}
```

### Pattern 3: Age Self-Encryption Round-Trip

**What:** Encrypt session ID bytes to own X25519 key; decrypt with same derived key.

**When to use:** PUB-03 — encrypt session ID before writing to record blob field.

```rust
// Source: docs.rs/age/0.11.2 (verified)
use age::Encryptor;
use std::io::Write;

fn age_encrypt(plaintext: &[u8], recipient: &age::x25519::Recipient) -> Vec<u8> {
    let encryptor = Encryptor::with_recipients(std::iter::once(recipient as &dyn age::Recipient))
        .expect("non-empty recipients");
    let mut ciphertext = vec![];
    let mut writer = encryptor.wrap_output(&mut ciphertext).expect("wrap_output");
    writer.write_all(plaintext).expect("write plaintext");
    writer.finish().expect("finish");
    ciphertext
}

fn age_decrypt(ciphertext: &[u8], identity: &age::x25519::Identity) -> Vec<u8> {
    let decryptor = age::Decryptor::new(ciphertext).expect("valid age ciphertext");
    let mut plaintext = vec![];
    let mut reader = decryptor
        .decrypt(std::iter::once(identity as &dyn age::Identity))
        .expect("decrypt");
    std::io::Read::read_to_end(&mut reader, &mut plaintext).expect("read");
    plaintext
}
```

### Pattern 4: Canonical JSON for Ed25519 Signing

**What:** Produce deterministic JSON (sorted keys, no whitespace) for signing.

**When to use:** Before computing signature over HandoffRecord; before verifying signature.

```rust
// Source: serde_json docs — BTreeMap serializes in sorted key order by default
// serde_json::Map is BTreeMap when preserve_order feature is disabled (it's NOT enabled in 1.0.149)

use std::collections::BTreeMap;
use serde_json;

// Approach: serialize struct to serde_json::Value, extract into BTreeMap, re-serialize
fn canonical_json(record: &HandoffRecord) -> anyhow::Result<String> {
    // Serialize without the signature field first, then compute signature
    // Use a BTreeMap or derive Serialize on struct with sorted fields
    // Then serde_json::to_string(&btree_map) gives deterministic output
    let value = serde_json::to_value(record)?;
    // serde_json Value::Object uses IndexMap with preserve_order=off → BTreeMap-like
    // Actually: use explicit BTreeMap<String, serde_json::Value> for safety
    serde_json::to_string(&value)  // no_whitespace = default compact
}
```

**Alternative (simpler):** Define `HandoffRecordSignable` as a separate struct without the signature field. Derive `Serialize`. The struct field order in Rust + serde_json's BTreeMap backend = deterministic output when field names are alphabetically ordered. Since this is guaranteed only if `preserve_order` is off (confirmed: lock has serde_json 1.0.149 without preserve_order), this is safe.

### Pattern 5: Pubky Homeserver Transport (reqwest blocking + session auth)

**What:** Sign in to homeserver, PUT record, GET record back.

**When to use:** All homeserver operations.

The homeserver requires:
1. POST binary AuthToken to `https://<homeserver>/session` → receives session cookie
2. PUT body bytes to `https://<homeserver>/pub/cclink/<token>` with session cookie
3. GET from `https://<homeserver>/pub/cclink/<token>` (no auth needed for GET)

The `AuthToken` is a `postcard`-serialized binary struct. See "Don't Hand-Roll" section for details.

```rust
// reqwest blocking client with cookie store
let client = reqwest::blocking::Client::builder()
    .cookie_store(true)
    .build()?;

// Step 1: Sign in
let auth_token_bytes = build_auth_token(&keypair)?;  // see AuthToken section
let signin_url = format!("https://{}/session", homeserver_host);
let response = client.post(&signin_url)
    .body(auth_token_bytes)
    .send()?;
// Session cookie now stored in client's cookie store

// Step 2: PUT record
let put_url = format!("https://{}/pub/cclink/{}", homeserver_host, token);
client.put(&put_url)
    .header("content-type", "application/octet-stream")
    .body(record_bytes)
    .send()?;

// Step 3: GET record (no auth needed)
let get_url = format!("https://{}/pub/cclink/{}", homeserver_host, token);
let body = client.get(&get_url).send()?.bytes()?;
```

**Homeserver host resolution:** The homeserver URL stored in `~/.pubky/cclink_homeserver` is the HTTPS hostname. The PUT/GET paths on that host use the user's own pubkey implicitly through the session cookie (the homeserver routes per-session). The public GET URL for another user's records would be `https://<homeserver>/<user-pubkey>/pub/cclink/<token>` — but for Phase 2, only self-access is needed.

**IMPORTANT FINDING**: After further research, the tenant routes use a `PubkyHost` extractor that reads the `pubky-host` header or uses the URL structure `pubky://<pubkey>/pub/...`. The exact URL routing for multi-tenant hosts needs confirmation during implementation. The safest approach: use `pubky://` URL scheme or the explicit host header. Phase 2 round-trip test can use the homeserver URL directly with a session cookie for PUT.

### Pattern 6: AuthToken Manual Implementation

**What:** Build the postcard-serialized binary signed token for homeserver signin.

The AuthToken binary layout (postcard, no length prefixes for fixed arrays):
```
[0..64]   = Ed25519 signature (64 raw bytes, via serialize_tuple)
[64..74]  = namespace b"PUBKY:AUTH" (10 bytes)
[74]      = version u8 = 0
[75..83]  = timestamp u64 big-endian (8 bytes, postcard u64 = varint, BUT timestamp.to_bytes() shows it's encoded as BE bytes; need to verify varint vs fixed)
[83..115] = pubkey [u8; 32] (32 bytes raw)
[115..]   = capabilities string via postcard (varint length + UTF-8 bytes)
```

The signature covers `bytes[65..]` — note: NOT from byte 64, but from 65. This means the binary layout has 65 bytes before the signable portion. Given: 64 bytes for signature = bytes[0..64], the signable starts at byte 65. Byte 64 itself is either the first byte of namespace OR there is a 1-byte prefix for the outer struct (unlikely in postcard). **This requires empirical verification during implementation** — write a test that encodes an AuthToken with pubky-common and inspect the offset. Since pubky-common 0.5.4 is in the cargo registry, write a test crate to validate the layout.

**Capabilities string**: Root capability = `/pub/cclink/:rw` is sufficient for cclink's writes, but `/pub/:rw` or the root capability `/:rw` will also work. The simplest: use `/:rw` which the pubky-common `Capability::root()` produces.

### Anti-Patterns to Avoid
- **Storing the reqwest Client across phases**: build fresh per operation (blocking clients are cheap)
- **Using preserve_order feature in serde_json**: breaks canonical JSON sort order guarantee
- **Signing with age's ephemeral key**: age always generates a fresh ephemeral key per encryption — only our X25519 key is the recipient, not the sender
- **Trusting GET response without verification**: always verify Ed25519 signature before returning record data
- **Using age's Identity string format for long-term storage**: the bech32 string is just for injection; never persist it

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| X25519 ECDH encryption | Custom DH + AEAD | `age 0.11.2` | age handles ephemeral key generation, nonce, HKDF, ChaCha20-Poly1305 correctly |
| Ed25519→X25519 conversion | Manual SHA-512 expand + clamp | `ed25519-dalek::SigningKey::to_scalar_bytes()` | Already exists in locked transitive dep; clamping is subtle |
| Bech32 encoding | Custom base32 | `bech32 0.9.x` | age depends on this exact version; avoids duplicate dep |
| Binary token serialization | Manual bit packing | `postcard 1.x` | homeserver verifier uses postcard; any other format fails verification |
| Cookie session management | Manual Set-Cookie parsing | `reqwest::blocking` with `cookie_store(true)` | Handles cookie domain, path, and expiry automatically |
| Canonical JSON | Custom key-sort | serde_json + BTreeMap fields | serde_json default map is BTreeMap-ordered without preserve_order |

**Key insight:** The age crate's X25519 implementation is well-audited and handles all the subtle cryptographic details (HKDF salt construction, nonce management). Do not replicate any of it.

---

## Common Pitfalls

### Pitfall 1: pubky Crate Version Incompatibility
**What goes wrong:** Adding `pubky = "0.6.0"` fails to compile because it requires `pkarr ^3.10.0` while the project uses `pkarr 5.0.3`.
**Why it happens:** The published pubky 0.6.0 crate on crates.io was built against the older pkarr API. The pubky-core workspace main branch uses pkarr 5.0.3 but this was not re-published to crates.io.
**How to avoid:** Do NOT add the `pubky` crate from crates.io. Use `reqwest::blocking` directly.
**Warning signs:** `error[E0308]: mismatched types` on pkarr types when mixing pubky + pkarr 5.x.

### Pitfall 2: age Dependency Conflict (curve25519-dalek versions)
**What goes wrong:** age 0.11.2 uses `curve25519-dalek 4` and `x25519-dalek 2`, while pkarr 5.0.3 uses `curve25519-dalek 5.0.0-pre.6`. These coexist as separate semver-incompatible crates.
**Why it happens:** Semver major version differences mean two distinct copies of curve25519-dalek are compiled.
**How to avoid:** Never try to pass curve25519-dalek types between pkarr and age code. Convert via raw `[u8; 32]` bytes only.
**Warning signs:** If you see `expected MontgomeryPoint (curve25519_dalek_4::...), found MontgomeryPoint (curve25519_dalek_5::...)`.

### Pitfall 3: AuthToken Byte Offset for Signing
**What goes wrong:** Signing `bytes[64..]` instead of `bytes[65..]` produces tokens the homeserver rejects.
**Why it happens:** The signable region starts at offset 65, not 64 (one byte into the namespace). Reason is likely a 1-byte postcard struct header or the [u8; 10] array uses a length prefix in older postcard versions.
**How to avoid:** Write an empirical test using pubky-common 0.5.4 (available in cargo registry) to serialize an AuthToken and inspect the byte layout before implementing manually.
**Warning signs:** Homeserver returns 401 or 403 on signin attempt.

### Pitfall 4: Homeserver URL Structure
**What goes wrong:** Sending PUT to `https://<homeserver>/pub/cclink/<token>` returns 404 because the tenant routing requires a different URL or a host header.
**Why it happens:** The Pubky homeserver is multi-tenant; it routes by user identity, not just path. The exact URL pattern depends on whether the homeserver uses subdomain routing or explicit pubky-host headers.
**How to avoid:** Test against `pubky.app` homeserver or a local testnet. Check if the URL is `pubky://<pubkey>/pub/cclink/<token>` (rewritten to `https://_pubky.<pubkey>/<path>`) or direct `https://<homeserver>/<pubkey>/pub/cclink/<token>`.
**Warning signs:** 404 on PUT despite successful signin.

### Pitfall 5: Canonical JSON Ordering Not Guaranteed
**What goes wrong:** Verification fails because the signing step and verification step produce different JSON bytes.
**Why it happens:** If the record struct is serialized through `HashMap` or `serde_json::Map` with `preserve_order` feature, field order is insertion-order not alphabetical.
**How to avoid:** Serialize `HandoffRecordSignable` (a struct without the `signature` field) using serde_json default map (BTreeMap). Never use `preserve_order` feature. Sort field names alphabetically in the struct definition to be explicit.
**Warning signs:** Intermittent verification failure across different machines or Rust compiler versions.

### Pitfall 6: age Encryption Includes Ephemeral Public Key
**What goes wrong:** Assuming age ciphertext is just encrypted bytes; trying to strip or inspect internal headers.
**Why it happens:** age format includes the age header (recipient stanza with ephemeral public key) prepended to the payload. The full blob must be stored and passed to the decryptor intact.
**How to avoid:** Store the complete age output (including header) as the `blob` field in HandoffRecord. Never try to split or truncate it.
**Warning signs:** `InvalidHeader` error during decryption.

---

## Code Examples

Verified patterns from official sources:

### Ed25519-to-X25519 Scalar Bytes

```rust
// Source: ed25519-dalek 3.0.0-pre.6/src/signing.rs (local cache)
// Access ed25519_dalek::SigningKey from pkarr::Keypair
use ed25519_dalek::SigningKey;

pub fn ed25519_to_x25519_secret(keypair: &pkarr::Keypair) -> [u8; 32] {
    // keypair.secret_key() returns [u8; 32] (the raw Ed25519 seed bytes)
    let signing_key = SigningKey::from_bytes(&keypair.secret_key());
    signing_key.to_scalar_bytes()
    // to_scalar_bytes() = SHA-512(seed)[0..32] — compatible with x25519-dalek StaticSecret
}

pub fn ed25519_to_x25519_public(keypair: &pkarr::Keypair) -> [u8; 32] {
    // VerifyingKey::to_montgomery() -> MontgomeryPoint (curve25519-dalek 5.x type)
    keypair.public_key().verifying_key().to_montgomery().to_bytes()
}
```

### Age Identity/Recipient from Derived Bytes

```rust
// Source: str4d/rage age/src/x25519.rs (via WebFetch) + bech32 0.9.x docs
use bech32::{ToBase32, Variant};

pub fn age_identity(x25519_secret: &[u8; 32]) -> age::x25519::Identity {
    let encoded = bech32::encode("age-secret-key-", x25519_secret.to_base32(), Variant::Bech32)
        .expect("infallible");
    // age from_str is case-insensitive for the HRP; uppercase is canonical
    encoded.to_ascii_uppercase().parse().expect("valid age identity string")
}

pub fn age_recipient(x25519_pubkey: &[u8; 32]) -> age::x25519::Recipient {
    let encoded = bech32::encode("age", x25519_pubkey.to_base32(), Variant::Bech32)
        .expect("infallible");
    encoded.parse().expect("valid age recipient string")
}
```

### Ed25519 Signature over Canonical JSON

```rust
// Source: pkarr 5.0.3/src/keys.rs (local cache)
use base64::{engine::general_purpose::STANDARD, Engine};

pub fn sign_record_json(canonical_json: &str, keypair: &pkarr::Keypair) -> String {
    let sig = keypair.sign(canonical_json.as_bytes());
    STANDARD.encode(sig.to_bytes())
}

pub fn verify_record_json(
    canonical_json: &str,
    sig_b64: &str,
    pubkey: &pkarr::PublicKey,
) -> anyhow::Result<()> {
    use ed25519_dalek::Signature;
    let sig_bytes = STANDARD.decode(sig_b64)?;
    let sig = Signature::from_bytes(sig_bytes.as_slice().try_into()?);
    pubkey.verify(canonical_json.as_bytes(), &sig)
        .map_err(|e| anyhow::anyhow!("Signature verification failed: {}", e))
}
```

### reqwest Blocking Client with Cookie Store

```rust
// Source: docs.rs/reqwest/0.13/reqwest/blocking (verified)
use reqwest::blocking::Client;

pub fn build_homeserver_client() -> anyhow::Result<Client> {
    Client::builder()
        .cookie_store(true)
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(Into::into)
}

pub fn put_record(
    client: &Client,
    homeserver: &str,
    path: &str,
    body: Vec<u8>,
) -> anyhow::Result<()> {
    let url = format!("https://{}/{}", homeserver, path);
    let response = client.put(&url)
        .header("content-type", "application/octet-stream")
        .body(body)
        .send()?;
    if !response.status().is_success() {
        anyhow::bail!("PUT failed: {}", response.status());
    }
    Ok(())
}

pub fn get_record(client: &Client, homeserver: &str, path: &str) -> anyhow::Result<Vec<u8>> {
    let url = format!("https://{}/{}", homeserver, path);
    let response = client.get(&url).send()?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        anyhow::bail!("Record not found");
    }
    if !response.status().is_success() {
        anyhow::bail!("GET failed: {}", response.status());
    }
    Ok(response.bytes()?.to_vec())
}
```

### HandoffRecord Struct Sketch

```rust
// serde_json with no preserve_order -> BTreeMap -> sorted keys -> canonical JSON
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HandoffRecord {
    // Cleartext metadata (alphabetically ordered fields for canonical JSON)
    pub blob: String,         // base64-encoded age ciphertext of session_id
    pub created_at: u64,      // Unix timestamp seconds
    pub hostname: String,     // creator's machine hostname
    pub project: String,      // project path (cwd or git root)
    pub pubkey: String,       // creator's Ed25519 pubkey (z32 format)
    pub signature: String,    // base64 Ed25519 sig over canonical JSON of record minus signature
    pub ttl: u64,             // TTL in seconds
}

// Signable version (no signature field) for computing the signature
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HandoffRecordSignable {
    pub blob: String,
    pub created_at: u64,
    pub hostname: String,
    pub project: String,
    pub pubkey: String,
    pub ttl: u64,
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual X25519 DH + NaCl | `age` crate (spec-compliant age-encryption) | age 0.5+ (2021) | Interoperability with rage CLI, spec-compliant |
| ed25519-dalek 1.x (no X25519 conversion) | ed25519-dalek 3.x `to_scalar_bytes()` | ed25519-dalek 2.0 (2023) | Built-in Ed25519→X25519 conversion method |
| pubky 0.4.x (pkarr 2.x) | Must use reqwest directly (pubky 0.6 is incompatible with pkarr 5.x) | Jan 2026 | Phase 2 must implement AuthToken manually |
| `reqwest 0.11` blocking | `reqwest 0.13` blocking (same API) | 2025 | No API changes for blocking use; 0.13 is the version pkarr 5.0.3 depends on |

**Deprecated/outdated:**
- `crypto_box` crate for X25519: use age instead, which handles all the KEM details
- `ssh-key` crate for Ed25519→X25519: unnecessary since ed25519-dalek 3.x has `to_scalar_bytes()` built in

---

## Open Questions

1. **AuthToken binary layout offset (bytes[65..])**
   - What we know: The homeserver verifies `AuthToken::signable(v, bytes) = bytes[65..]` per pubky-common source
   - What's unclear: Why offset 65 instead of 64 (64-byte signature would end at index 63, namespace starts at 64)
   - Recommendation: Write an empirical test using pubky-common 0.5.4 (in cargo registry at `/home/john/.cargo/registry/src/.../pubky-common-0.5.4/`) to serialize a real AuthToken and print the byte offsets. Do this as the FIRST task in Phase 2.

2. **Pubky homeserver URL routing for multi-tenant PUT**
   - What we know: GET requests don't need auth; PUT requires session cookie; homeserver is multi-tenant
   - What's unclear: Is the PUT URL `https://<homeserver>/pub/cclink/<token>` (session scopes to user automatically) or `https://<homeserver>/<pubkey>/pub/cclink/<token>` (explicit user path)?
   - Recommendation: Test against pubky.app or a local homeserver using the pubky-cli as reference. From the homeserver route code, `/{*path}` with PubkyHost header suggests the URL is `https://<homeserver>/pub/cclink/<token>` with a `pubky-host: <z32-pubkey>` header, OR the pubky:// URL scheme which the homeserver rewrites.

3. **postcard u64 encoding (varint vs fixed bytes)**
   - What we know: postcard spec says u64 is varint; but Timestamp::to_bytes() returns BE [u8; 8]
   - What's unclear: Whether Timestamp serializes via serde as a struct wrapping u64 (→ varint) or as raw bytes
   - Recommendation: Empirical test (same as question 1) resolves this.

4. **reqwest 0.13 blocking compilation with rustls**
   - What we know: pkarr 5.0.3 already uses reqwest 0.13 as optional dep with rustls feature
   - What's unclear: Whether enabling reqwest 0.13 blocking in the project creates duplicate tokio runtime or compile errors on non-async binary
   - Recommendation: reqwest::blocking creates its own single-threaded tokio runtime internally; this is fine in a non-async binary. Confirmed pattern in reqwest docs.

---

## Sources

### Primary (HIGH confidence)
- `/home/john/.cargo/registry/src/.../pkarr-5.0.3/src/keys.rs` — `Keypair::sign()`, `PublicKey::verify()`, `secret_key()` API
- `/home/john/.cargo/registry/src/.../ed25519-dalek-3.0.0-pre.6/src/signing.rs` — `to_scalar_bytes()` method (local source)
- `/home/john/.cargo/registry/src/.../ed25519-dalek-3.0.0-pre.6/src/verifying.rs` — `to_montgomery()` method (local source)
- `/home/john/.cargo/registry/src/.../pubky-common-0.5.4/src/auth.rs` — AuthToken structure and signing logic (local source)
- `/home/john/.cargo/registry/src/.../pubky-0.5.4/src/api/http.rs` — HTTP PUT/GET methods on pubky Client (local source)
- `https://docs.rs/age/0.11.2/age/` — age crate API (WebFetch verified)
- `str4d/rage age/src/x25519.rs` — Identity/Recipient from_str + bech32 variant (WebFetch verified)
- `https://docs.rs/reqwest/latest/reqwest/blocking/index.html` — reqwest::blocking API (WebFetch verified)
- `pubky-core/pubky-homeserver/src/client_server/routes/tenants/write.rs` — homeserver PUT routing (WebFetch verified)
- `pubky-core/pubky-homeserver/src/client_server/layers/authz.rs` — cookie-only auth (WebFetch verified)

### Secondary (MEDIUM confidence)
- `https://crates.io/api/v1/crates/pubky/0.6.0/dependencies` — confirmed pubky 0.6.0 requires pkarr ^3.10.0 (incompatible)
- `https://crates.io/api/v1/crates/age/0.11.2/dependencies` — confirmed age uses x25519-dalek 2 + bech32 ^0.9
- `pubky-core workspace Cargo.toml` — confirmed workspace uses pkarr 5.0.3 (WebFetch on GitHub raw URL)
- `https://crates.io/api/v1/crates/reqwest` — confirmed reqwest 0.13.2 is latest stable (2026-02-06)

### Tertiary (LOW confidence)
- Homeserver PUT URL structure: `https://<homeserver>/pub/cclink/<token>` with pubky-host header — inferred from route analysis but not directly tested; NEEDS empirical verification in Phase 2 task 1

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all versions verified against local cargo registry and crates.io API
- Architecture: HIGH for crypto patterns (code verified in local cache); MEDIUM for homeserver transport (URL structure needs empirical test)
- Pitfalls: HIGH for dependency conflicts (directly verified); MEDIUM for AuthToken byte layout (analytically derived, needs empirical confirmation)

**Research date:** 2026-02-21
**Valid until:** 2026-03-21 (30 days; pubky-core is actively developed but pkarr 5.x compat is a known gap)
