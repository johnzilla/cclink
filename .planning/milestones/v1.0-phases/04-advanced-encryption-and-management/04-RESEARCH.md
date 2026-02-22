# Phase 4: Advanced Encryption and Management - Research

**Researched:** 2026-02-22
**Domain:** age encryption to recipient pubkeys, Pubky homeserver DELETE/LIST API, terminal table rendering
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Shared handoffs (--share):**
- Recipient specified by z32-encoded public key: `cclink --share <z32_pubkey>`
- Single recipient only per handoff — no multi-recipient support
- Only the recipient's private key can decrypt the handoff (encrypt to recipient's X25519, not publisher's)
- Wrong-recipient pickup shows clear error message plus cleartext metadata (project, hostname, age) so they know what it was even though they can't decrypt it
- Publisher cannot decrypt their own --share handoffs (they don't have the recipient's secret key)

**Burn-after-read (--burn):**
- Publisher marks the burn flag at publish time: `cclink --burn`
- After first successful pickup, the record is deleted from the homeserver
- Burned records return "not found" on subsequent pickup — indistinguishable from never-existed or revoked
- Yellow warning printed at publish time so publisher knows it's burn-after-read
- Combinable with --share: `cclink --burn --share <z32_pubkey>` — single recipient, single read

**Record listing (cclink list):**
- Columns: token (truncated), project, age, TTL remaining, burn flag, recipient pubkey (if shared)
- Active records only — expired records are excluded, no --all flag
- Colored table output with owo-colors, consistent with existing TTY-aware style
- Empty state: friendly message like "No active handoffs. Publish one with cclink."

**Revocation (cclink revoke):**
- `cclink revoke <token>` shows record details and asks "Revoke this handoff? [y/N]" — skip with --yes/-y
- `cclink revoke --all` shows count: "This will revoke N active handoffs. Continue? [y/N]" — user sees blast radius
- Revoked records return "not found" on pickup — same as burned, same as never-existed
- Success output: green "Revoked." with token/project, consistent with publish success style

### Claude's Discretion

- Exact table formatting and column widths for `cclink list`
- How burn-after-read deletion is triggered (inline during pickup vs. deferred)
- Retry behavior for revoke network calls
- Internal data structures for tracking burn/share state in HandoffRecord

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| ENC-01 | User can encrypt a handoff to a specific recipient's X25519 key via `--share <pubkey>` | age recipient construction from Ed25519 pubkey; pkarr::PublicKey::try_from z32 string; existing ed25519_to_x25519_public() works for any pubkey bytes, not just own keypair |
| ENC-02 | User can mark a handoff as burn-after-read via `--burn` (record deleted after first retrieval) | Pubky homeserver supports HTTP DELETE on `/pub/cclink/{token}`; burn flag stored in HandoffRecord; pickup triggers delete; DELETE requires signin() session cookie |
| MGT-01 | User can list active handoff records via `cclink list` with token, project, age, TTL remaining, and burn status | Homeserver GET `/pub/cclink/` with trailing slash lists records as newline-separated `pubky://` URLs; parse, fetch, filter; comfy-table 7.2.2 for rendering |
| MGT-02 | User can revoke a specific handoff via `cclink revoke <token>` | HTTP DELETE on `/pub/cclink/{token}` after signin; confirmation prompt via dialoguer (already a dep) |
| MGT-03 | User can revoke all handoffs via `cclink revoke --all` | LIST endpoint to get all tokens, then DELETE each; blast radius count shown before confirmation |
</phase_requirements>

---

## Summary

Phase 4 builds on a solid Phase 1–3 foundation. The crypto for `--share` is already 90% done: the project has `ed25519_to_x25519_public()` that converts Ed25519 public key bytes to an X25519 Montgomery point, and `age_recipient()` that wraps those bytes into an age recipient. The missing piece is converting a z32 pubkey string (recipient, not self) to an `age::x25519::Recipient`. This is achieved with `pkarr::PublicKey::try_from(z32_str)` → `.verifying_key().to_montgomery().to_bytes()` → `age_recipient(&bytes)` — exactly the same call chain as the self-encryption path but starting from a parsed PublicKey instead of own Keypair.

The homeserver API is fully confirmed: Pubky supports HTTP DELETE on `/pub/cclink/{token}` (returning 204 No Content) after an authenticated session. The LIST endpoint is `GET /pub/cclink/` (trailing slash signals directory listing) and returns `text/plain` newline-separated `pubky://` URLs. Both operations require a signed-in session cookie. For `cclink list`, the implementation is: sign in, GET the directory listing, parse the tokens, fetch each full HandoffRecord, filter expired ones, render the table.

For table rendering, `comfy-table` 7.2.2 is the right choice: it accepts pre-colored strings as cell content (ANSI-passthrough), integrates cleanly with existing `owo-colors` TTY-aware patterns, and produces aligned output. The `HandoffRecord` needs two new optional fields (`burn: bool` and `recipient: Option<String>`) added in alphabetical order to maintain canonical JSON determinism and preserve backwards compatibility with Phase 3 records (serde defaults).

**Primary recommendation:** Extend `HandoffRecord` with `burn` and `recipient` fields (serde defaults), add a `delete_record()` method to `HomeserverClient`, add a `list_records()` method using the directory listing endpoint, add the `--share`/`--burn` flags to `Cli`, and add `List`/`Revoke` subcommands.

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| age | 0.11 (already dep) | Encrypt to recipient X25519 key | Phase 2 established; single-recipient encryption to arbitrary pubkey |
| pkarr | 5.0.3 (already dep) | Parse z32 pubkey → PublicKey → VerifyingKey → Montgomery point | Already used for keypair ops; TryFrom<&str> confirmed |
| reqwest::blocking | 0.13 (already dep) | HTTP DELETE and GET for list/revoke | Session cookie persists; `.delete(url).send()` is the DELETE method |
| comfy-table | 7.2.2 | Terminal table for `cclink list` | Accepts pre-colored strings, minimal API, no custom renderer needed |
| dialoguer | 0.12 (already dep) | Confirmation prompts for revoke | Already used in pickup; Confirm::new() pattern established |
| owo-colors | 4 (already dep) | Color cells in list output | TTY-aware pattern already established |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| serde default | N/A (already dep) | `#[serde(default)]` for new HandoffRecord fields | Required for backwards compatibility with Phase 3 records that lack burn/recipient fields |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| comfy-table | tabled 0.20 | tabled has derive macro magic but heavier; comfy-table is simpler for manual row building |
| comfy-table | raw println! with pad_str | Plain padding works but misaligns on varying-width content; comfy-table handles this automatically |

**Installation:**
```bash
# comfy-table is the only new dep needed
cargo add comfy-table@7.2.2
```

---

## Architecture Patterns

### Recommended Project Structure

```
src/
├── cli.rs              # Add --share, --burn to Cli; add List, Revoke subcommands
├── commands/
│   ├── publish.rs      # Extend: handle --share recipient, --burn flag
│   ├── pickup.rs       # Extend: detect --share record, trigger burn-delete after decrypt
│   ├── list.rs         # NEW: cclink list command
│   └── revoke.rs       # NEW: cclink revoke command
├── record/mod.rs       # Extend HandoffRecord: add burn + recipient fields with serde defaults
├── transport/mod.rs    # Add: delete_record(), list_record_tokens()
└── crypto/mod.rs       # Add: recipient_from_z32() helper
```

### Pattern 1: Recipient Key Derivation from z32 Pubkey

**What:** Convert a z32 pubkey string (recipient, no private key access) into an age X25519 Recipient.
**When to use:** During `--share` publish flow, after parsing the CLI argument.

```rust
// Source: pkarr docs.rs TryFrom<&str>; existing crypto::age_recipient() in project
pub fn recipient_from_z32(z32: &str) -> anyhow::Result<age::x25519::Recipient> {
    let pubkey = pkarr::PublicKey::try_from(z32)
        .map_err(|e| anyhow::anyhow!("invalid recipient pubkey: {}", e))?;
    // .verifying_key() returns ed25519_dalek::VerifyingKey (curve25519-dalek 5)
    // .to_montgomery().to_bytes() gives the X25519 Montgomery point [u8; 32]
    let x25519_bytes: [u8; 32] = pubkey.verifying_key().to_montgomery().to_bytes();
    Ok(crate::crypto::age_recipient(&x25519_bytes))
}
```

This uses the exact same conversion path as `ed25519_to_x25519_public()` in `crypto/mod.rs`, but starting from a parsed `PublicKey` instead of a `Keypair`. No new crates needed.

### Pattern 2: HandoffRecord Extension with serde defaults

**What:** Add `burn` and `recipient` fields that default to `false`/`None` so existing Phase 3 records deserialize correctly.
**When to use:** When extending the record struct.

```rust
// Source: serde documentation — #[serde(default)] uses Default::default()
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HandoffRecord {
    pub blob: String,
    #[serde(default)]          // false if absent in old records
    pub burn: bool,
    pub created_at: u64,
    pub hostname: String,
    pub project: String,
    pub pubkey: String,
    // Optional: z32 pubkey of the intended recipient (None = self-encrypted)
    #[serde(default)]          // None if absent in old records
    pub recipient: Option<String>,
    pub signature: String,
    pub ttl: u64,
}
```

CRITICAL: `burn` and `recipient` must be added to `HandoffRecordSignable` as well, and they must be placed in alphabetical order in both structs. `burn` sorts before `created_at`; `recipient` sorts between `project` and `signature`. The canonical JSON signing covers ALL fields in HandoffRecordSignable — including burn and recipient. Old records will fail verification if they lack these fields but a new signer included them, but that is expected: old records are signed without these fields, and should be verified without them. The planner must decide whether to include these fields in the signable struct or keep the signable struct as-is and treat them as out-of-band metadata. Recommendation: include them in signable — they are policy-bearing fields, not just metadata.

### Pattern 3: HTTP DELETE for Record Deletion

**What:** Sign in to the homeserver (acquire session cookie) then HTTP DELETE the record path.
**When to use:** For both burn-after-read (triggered inline in pickup) and revoke.

```rust
// Source: confirmed from pubky-homeserver routes/tenants/write.rs; reqwest blocking docs
pub fn delete_record(&self, token: &str) -> anyhow::Result<()> {
    let url = format!("https://{}/pub/cclink/{}", self.homeserver, token);
    let response = self
        .client
        .delete(&url)
        .send()
        .map_err(|e| anyhow::anyhow!("DELETE request failed: {}", e))?;

    // 204 No Content on success; 404 if already gone (treat as success for idempotency)
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(()); // Already deleted — idempotent
    }
    if !response.status().is_success() {
        let status = response.status();
        anyhow::bail!("DELETE failed (status {}): {}", status, token);
    }
    Ok(())
}
```

Authentication: the existing `cookie_store(true)` on `HomeserverClient` means `delete_record()` must be called after `signin()`. The session cookie from signin is automatically sent with the DELETE request.

### Pattern 4: Listing Records via Directory GET

**What:** GET the `/pub/cclink/` directory endpoint (trailing slash = directory listing) to discover all published tokens.
**When to use:** For `cclink list` and `cclink revoke --all`.

```rust
// Source: confirmed from pubky-homeserver routes/tenants/read.rs
// Response: text/plain, newline-separated "pubky://{homeserver}/{pubkey_z32}/pub/cclink/{token}" URLs
pub fn list_record_tokens(&self, keypair: &pkarr::Keypair) -> anyhow::Result<Vec<String>> {
    let pubkey_z32 = keypair.public_key().to_z32();
    let url = format!("https://{}/pub/cclink/", self.homeserver);

    // Sign in first — list requires session auth (own records only)
    self.signin(keypair)?;

    let response = self
        .client
        .get(&url)
        .send()
        .map_err(|e| anyhow::anyhow!("LIST request failed: {}", e))?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(vec![]); // No records published yet
    }
    if !response.status().is_success() {
        anyhow::bail!("LIST failed (status {})", response.status());
    }

    let body = response.text()
        .map_err(|e| anyhow::anyhow!("failed to read LIST response: {}", e))?;

    // Parse "pubky://{homeserver}/{pubkey_z32}/pub/cclink/{token}" lines
    // Token is the last path component
    let tokens: Vec<String> = body
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            // URL ends with /pub/cclink/{token}
            line.split("/pub/cclink/").nth(1)
                .map(|t| t.trim_end_matches('/').to_string())
        })
        .collect();

    Ok(tokens)
}
```

The `latest` key will appear in this listing (`/pub/cclink/latest`). Filter it out: it is not a numeric timestamp token.

### Pattern 5: comfy-table for List Display

**What:** Render a colored, aligned table for `cclink list`.
**When to use:** The list command output.

```rust
// Source: comfy-table 7.2.2 docs
use comfy_table::{Table, Cell, Color, Attribute};

let mut table = Table::new();
table.set_header(vec!["Token", "Project", "Age", "TTL Left", "Burn", "Recipient"]);

for record in active_records {
    table.add_row(vec![
        Cell::new(&token_prefix),         // e.g. "1706..."
        Cell::new(&record.project),
        Cell::new(&human_age),
        Cell::new(&ttl_remaining),
        Cell::new(if record.burn { "yes" } else { "" })
            .fg(if record.burn { Color::Yellow } else { Color::Reset }),
        Cell::new(record.recipient.as_deref().unwrap_or("")),
    ]);
}
println!("{table}");
```

Alternative: skip comfy-table entirely and use manual `format!` with fixed-width columns. This avoids a new dep. Given the small number of columns and that terminal widths vary, comfy-table is preferred but not required.

### Pattern 6: Burn-After-Read Inline Deletion

**What:** After successful decryption in pickup, if `record.burn == true`, DELETE the record.
**When to use:** In `run_pickup()` after the session ID is successfully decrypted.

Decision: inline during pickup (not deferred). Rationale: deferred deletion requires state management (retry on crash, etc.) which adds complexity without benefit. If the DELETE fails, log a warning but do not fail the pickup — the TTL will expire the record anyway.

```rust
// In run_pickup(), after successful decryption:
if record.burn {
    // Sign in for delete (may already be signed in from self-pickup; signin is idempotent)
    if let Err(e) = client.delete_record(&token) {
        // Warn but do not fail — TTL will expire the record
        eprintln!("{}", format!("Warning: burn deletion failed: {}", e)
            .if_supports_color(Stdout, |t| t.yellow()));
    }
}
```

For `--share` records picked up by the recipient: the recipient must sign in with their OWN keypair to DELETE the record. The record lives under the publisher's pubkey path. Problem: only the publisher's session cookie can DELETE their own records. The recipient cannot DELETE from the publisher's namespace.

Implication: burn-after-read for `--share` records is NOT enforceable by the recipient. Options:
1. Only the publisher can burn — but the publisher doesn't know when pickup happened
2. Accept that `--burn --share` cannot guarantee deletion by recipient — document as best-effort TTL
3. Design: burn flag signals "publisher wants this deleted" but deletion only happens if the record is picked up by the publisher's own key (self-pickup scenario)

**Recommended resolution:** For `--share --burn` combination, the record expires via TTL. The "burn" flag on shared records means the publisher intends it to be one-time, but the homeserver's `DELETE` can only be exercised by the record owner (publisher). Document this as a known limitation. The recipient sees a "burn" indicator in `cclink list` but the actual deletion must come from the publisher's next `cclink revoke <token>` or TTL expiry.

Wait — re-reading the CONTEXT.md: "After first successful pickup, the record is deleted from the homeserver." This implies the homeserver itself enforces burn, OR the pickup caller deletes it. The pickup caller is the person doing `cclink pickup` — for self-pickup, they own the session and can DELETE. For cross-user `--share` pickup, they cannot DELETE the publisher's record. This is a design gap that needs resolution during planning. See Open Questions.

### Anti-Patterns to Avoid

- **Fetching all records to find one:** Use the direct `/pub/cclink/{token}` path, not the listing, when you know the token.
- **Treating "not found" as error in DELETE:** A DELETE returning 404 is already-deleted; treat as success for idempotency.
- **Signing the record WITHOUT burn/recipient fields:** If burn and recipient are policy-bearing fields, they must be in `HandoffRecordSignable` or tampering is possible.
- **Putting burn/recipient fields out of alphabetical order:** Breaks canonical JSON determinism.
- **Forgetting to sign in before DELETE:** The client's cookie_store only has a cookie if signin() was called on this client instance; each new HomeserverClient must sign in.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| X25519 key derivation from Ed25519 pubkey | Custom Montgomery conversion | Existing `ed25519_to_x25519_public()` + `age_recipient()` from crypto module | Already battle-tested in Phase 2 |
| Parsing z32 pubkey string | Manual z32 decode + byte conversion | `pkarr::PublicKey::try_from(z32)` | TryFrom<&str> handles all valid formats; returns clean error |
| Terminal table rendering | Manual `format!("{:<20}", col)` padding | `comfy-table` 7.2.2 | Handles variable-width content, ANSI passthrough, minimal API surface |
| Confirmation prompts | Custom readline | `dialoguer::Confirm` (already a dep) | Already used in pickup; same `--yes/-y` skip pattern |
| Partial URL parsing for list tokens | Custom regex | `str::split("/pub/cclink/")` | Simple string split on the known segment is sufficient |

**Key insight:** All cryptographic primitives for Phase 4 already exist in the codebase. No new crypto libraries needed — only the `recipient_from_z32()` convenience wrapper must be written.

---

## Common Pitfalls

### Pitfall 1: Signing vs. Signable Struct Mismatch

**What goes wrong:** New `burn` and `recipient` fields are added to `HandoffRecord` but not to `HandoffRecordSignable`, so the fields are not covered by the Ed25519 signature. An attacker can modify these fields without invalidating the signature.

**Why it happens:** Developer adds fields for JSON storage but forgets the parallel signable struct.

**How to avoid:** Always add new fields to BOTH `HandoffRecord` AND `HandoffRecordSignable` (unless there is a deliberate reason not to sign them). Update the `From<&HandoffRecord> for HandoffRecordSignable` conversion.

**Warning signs:** `HandoffRecord` has fields that `HandoffRecordSignable` does not — a code review check.

### Pitfall 2: serde Alphabetical Order Break

**What goes wrong:** `burn` field is added but placed after `blob` in struct declaration order (out of alphabetical sequence), causing non-deterministic canonical JSON or ordering mismatch with existing records.

**Why it happens:** Forgetting that field declaration order = JSON key order in this project.

**How to avoid:** Strictly maintain alphabetical order: `blob`, `burn`, `created_at`, `hostname`, `project`, `pubkey`, `recipient`, `signature`, `ttl`. Verify with the existing alphabetical order test.

**Warning signs:** The canonical JSON alphabetical ordering tests fail.

### Pitfall 3: Directory Listing Includes "latest" Key

**What goes wrong:** `list_record_tokens()` returns "latest" as a token, and the code tries to fetch `GET /pub/cclink/latest` as a HandoffRecord (it is a LatestPointer, not a HandoffRecord — deserialization fails).

**Why it happens:** The homeserver lists all entries under `/pub/cclink/` including the `latest` pointer file.

**How to avoid:** Filter out any token that is not a numeric string (all real tokens are Unix timestamps as strings). Use `token.parse::<u64>().is_ok()` as the filter condition.

**Warning signs:** `cclink list` crashes with a deserialization error.

### Pitfall 4: DELETE Requires Fresh Sign-In on Each HomeserverClient

**What goes wrong:** `delete_record()` is called without a prior `signin()` and returns 401 Unauthorized.

**Why it happens:** `cookie_store(true)` preserves cookies within a single client instance, but does not persist across process restarts or new HomeserverClient instances.

**How to avoid:** Always call `self.signin(keypair)?` before `self.delete_record(token)`. Pattern: `client.signin(keypair)?; client.delete_record(token)?;`.

**Warning signs:** DELETE returns 401 or similar auth failure.

### Pitfall 5: Burn-After-Read on Shared Records — Deletion Auth Gap

**What goes wrong:** `cclink --burn --share <pubkey>` creates a record. The recipient does `cclink pickup <publisher_pubkey>`, decrypts successfully, but calling `client.delete_record(token)` fails because the recipient's session cookie cannot DELETE from the publisher's namespace.

**Why it happens:** Pubky homeserver DELETE requires the requestor's session to match the record's owner pubkey. The recipient is not the owner.

**How to avoid:** At implementation time, restrict burn-after-read deletion to cases where the pickup is self-pickup (own keypair). For shared records, either: (a) skip the DELETE and rely on TTL, or (b) implement a mechanism where the publisher's client is notified (out of scope). The planning step must decide.

**Warning signs:** DELETE on `/{pubkey_z32}/pub/cclink/{token}` returns 403 Forbidden when called by a different user's session.

### Pitfall 6: `--share` Pickup Decryption Fails Silently

**What goes wrong:** A recipient attempts `cclink pickup` (self-pickup path) on a `--share` record. The age decrypt call returns an error because the record was encrypted to the recipient's key, not the publisher's key. Without proper error handling, the user gets a cryptic "age decrypt error" instead of the intended "this was shared with you" message.

**Why it happens:** The pickup command doesn't check `record.recipient` before attempting decryption.

**How to avoid:** In `run_pickup()`, before decryption, check: if `record.recipient.is_some()` and `record.recipient != Some(own_pubkey_z32)`, show "This record was shared with a specific recipient. Use `cclink pickup <publisher_pubkey>` from the recipient's machine." Then attempt decryption normally and let the age error provide the definitive answer.

---

## Code Examples

### Recipient From z32 (New crypto helper)

```rust
// Source: pkarr docs.rs TryFrom; existing crypto/mod.rs age_recipient()
// Add to src/crypto/mod.rs
pub fn recipient_from_z32(z32: &str) -> anyhow::Result<age::x25519::Recipient> {
    let pubkey = pkarr::PublicKey::try_from(z32)
        .map_err(|e| anyhow::anyhow!("invalid recipient pubkey '{}': {}", z32, e))?;
    let x25519_bytes: [u8; 32] = pubkey.verifying_key().to_montgomery().to_bytes();
    Ok(age_recipient(&x25519_bytes))
}
```

### Publishing with --share

```rust
// Source: existing publish.rs patterns
// In run_publish(), replace the encryption step:
let recipient = if let Some(ref share_pubkey) = cli.share {
    crate::crypto::recipient_from_z32(share_pubkey)?
} else {
    let x25519_pubkey = crate::crypto::ed25519_to_x25519_public(&keypair);
    crate::crypto::age_recipient(&x25519_pubkey)
};
let ciphertext = crate::crypto::age_encrypt(session.session_id.as_bytes(), &recipient)?;
```

### HandoffRecord Extended Fields

```rust
// Source: existing record/mod.rs; serde docs for #[serde(default)]
// Alphabetical ordering: blob, burn, created_at, hostname, project, pubkey, recipient, signature, ttl
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HandoffRecord {
    pub blob: String,
    #[serde(default)]
    pub burn: bool,
    pub created_at: u64,
    pub hostname: String,
    pub project: String,
    pub pubkey: String,
    #[serde(default)]
    pub recipient: Option<String>,
    pub signature: String,
    pub ttl: u64,
}

// HandoffRecordSignable must also include burn and recipient in alphabetical order
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HandoffRecordSignable {
    pub blob: String,
    pub burn: bool,
    pub created_at: u64,
    pub hostname: String,
    pub project: String,
    pub pubkey: String,
    pub recipient: Option<String>,
    pub signature: String,  // WAIT — signature is NOT in HandoffRecordSignable
    pub ttl: u64,
}
```

Correction: `HandoffRecordSignable` does NOT include `signature` — that is its entire point. Alphabetical for signable: `blob`, `burn`, `created_at`, `hostname`, `project`, `pubkey`, `recipient`, `ttl`.

### CLI Extensions

```rust
// Source: existing cli.rs clap patterns
pub struct Cli {
    // ... existing fields ...

    /// Encrypt for a specific recipient (z32-encoded pubkey)
    #[arg(long, value_name = "PUBKEY")]
    pub share: Option<String>,

    /// Mark as burn-after-read (deleted after first successful pickup)
    #[arg(long)]
    pub burn: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    Init(InitArgs),
    Whoami,
    Pickup(PickupArgs),
    /// List all active handoff records
    List,
    /// Revoke a handoff record
    Revoke(RevokeArgs),
}

#[derive(Parser)]
pub struct RevokeArgs {
    /// Token to revoke (omit to use --all)
    #[arg(value_name = "TOKEN")]
    pub token: Option<String>,

    /// Revoke all active handoffs
    #[arg(long)]
    pub all: bool,

    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    pub yes: bool,
}
```

### Revoke with Confirmation

```rust
// Source: existing pickup.rs dialoguer pattern; CONTEXT.md decision
pub fn run_revoke(args: RevokeArgs) -> anyhow::Result<()> {
    let keypair = crate::keys::store::load_keypair()?;
    let homeserver = crate::keys::store::read_homeserver()?;
    let client = crate::transport::HomeserverClient::new(&homeserver)?;
    client.signin(&keypair)?;

    if args.all {
        let tokens = client.list_record_tokens(&keypair)?;
        let count = tokens.len();
        if count == 0 {
            println!("No active handoffs.");
            return Ok(());
        }
        let skip = args.yes || !std::io::stdin().is_terminal();
        if !skip {
            let confirmed = dialoguer::Confirm::new()
                .with_prompt(format!("This will revoke {} active handoffs. Continue?", count))
                .default(false)
                .interact()?;
            if !confirmed { println!("Aborted."); return Ok(()); }
        }
        for token in &tokens {
            client.delete_record(token)?;
        }
        println!("{}", format!("Revoked {} handoffs.", count)
            .if_supports_color(Stdout, |t| t.green()));
    } else if let Some(token) = args.token {
        // Fetch record for details, show to user, confirm, delete
        let record = client.get_record(&token, &keypair.public_key())?;
        // show record.project, token prefix
        let skip = args.yes || !std::io::stdin().is_terminal();
        if !skip {
            let confirmed = dialoguer::Confirm::new()
                .with_prompt(format!("Revoke this handoff? ({} in {})", &token[..8.min(token.len())], record.project))
                .default(false)
                .interact()?;
            if !confirmed { println!("Aborted."); return Ok(()); }
        }
        client.delete_record(&token)?;
        println!("{}", "Revoked.".if_supports_color(Stdout, |t| t.green()));
    } else {
        anyhow::bail!("Provide a token or --all");
    }
    Ok(())
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| age encrypt to self only | age encrypt to arbitrary X25519 recipient | Phase 4 | Enables `--share` without new crypto deps |
| Records never deleted | HTTP DELETE + burn flag | Phase 4 | Homeserver supports DELETE; session cookie auth applies |
| No management commands | `list` + `revoke` subcommands | Phase 4 | LIST endpoint returns `pubky://` URLs; parse token from URL tail |

**Confirmed current behavior:**
- Pubky homeserver DELETE returns 204 No Content on success (confirmed from source)
- Pubky homeserver LIST (directory GET) returns `text/plain` newline-separated `pubky://` URLs (confirmed from source)
- pkarr `PublicKey::try_from(&str)` accepts z32 strings (confirmed from docs.rs)
- `VerifyingKey::to_montgomery()` is available from the ed25519-dalek dep already in Cargo.toml (confirmed from existing crypto/mod.rs usage)

---

## Open Questions

1. **Burn-after-read for `--share` records: who deletes?**
   - What we know: HTTP DELETE requires the record owner's session. The recipient cannot delete the publisher's record.
   - What's unclear: Should `--burn --share` silently downgrade to TTL-only expiry? Or should we reject the combination?
   - Recommendation: At plan time, decide to either (a) allow `--burn --share` with documented best-effort behavior (burns on TTL if recipient doesn't delete), or (b) for the phase, implement burn-after-read only for self-encrypted records; shared records with burn flag show the flag in `cclink list` but deletion is publisher-initiated via `cclink revoke`.

2. **Backwards compatibility: Phase 3 records lack burn/recipient fields**
   - What we know: serde `#[serde(default)]` on `HandoffRecord` fields handles deserialization of old records.
   - What's unclear: Old records signed without burn/recipient will fail verification if the new `HandoffRecordSignable` includes those fields (canonical JSON will differ).
   - Recommendation: Either (a) do NOT add burn/recipient to `HandoffRecordSignable` and treat them as non-signed metadata — simpler, but means they can be tampered, OR (b) accept that Phase 3 records are incompatible with Phase 4 verification and document it. Option (a) is pragmatic for a single-user tool; option (b) is more secure. The planner should make this call explicitly.

3. **LIST endpoint URL: own records only or cross-user?**
   - What we know: The directory listing via session cookie lists own records. Cross-user listing would use `/{pubkey_z32}/pub/cclink/` path (same as cross-user GET).
   - What's unclear: `cclink list` shows only own records — confirmed by CONTEXT.md. No cross-user listing needed. But: does the homeserver require a trailing slash on the directory path? From source read: directory detection is based on `entry_path.path().is_directory()`. A trailing slash on the URL signals directory intent.
   - Recommendation: Use trailing slash: `GET /pub/cclink/` (with slash). Verify empirically.

4. **comfy-table vs. plain format for list output**
   - What we know: comfy-table 7.2.2 works; it is a new dep.
   - What's unclear: Whether the added dep weight is worth it vs. `format!("{:<40}", project)` manual alignment.
   - Recommendation: Use comfy-table. It handles variable-width content correctly and the installation is one `cargo add` line.

---

## Sources

### Primary (HIGH confidence)

- `pubky-homeserver/src/client_server/routes/tenants/write.rs` (fetched from raw.githubusercontent.com) — confirmed DELETE returns 204, requires PubkyHost auth
- `pubky-homeserver/src/client_server/routes/tenants/read.rs` (fetched from raw.githubusercontent.com) — confirmed LIST returns text/plain pubky:// URLs, query params: cursor, limit, shallow, reverse
- `docs.rs/pkarr/latest/pkarr/struct.PublicKey.html` (fetched) — confirmed TryFrom<&str>, to_z32(), verifying_key() methods
- `docs.rs/reqwest/latest/reqwest/blocking/struct.Client.html` (fetched) — confirmed `.delete(url)` method returns RequestBuilder
- `src/crypto/mod.rs` (read from codebase) — ed25519_to_x25519_public(), age_recipient(), age_encrypt() already exist
- `src/record/mod.rs` (read from codebase) — HandoffRecord struct, alphabetical field ordering, HandoffRecordSignable pattern
- `src/transport/mod.rs` (read from codebase) — HomeserverClient, signin(), put_record(), cookie_store pattern
- `Cargo.toml` (read from codebase) — all existing deps; comfy-table is the only new dep needed

### Secondary (MEDIUM confidence)

- `docs.rs/comfy-table/latest/comfy_table/` (fetched) — version 7.2.2, Cell API, accepts pre-colored strings
- pubky-core README (WebSearch/WebFetch) — confirmed PUT/GET/DELETE HTTP API with pagination description

### Tertiary (LOW confidence)

- WebSearch results re: pubky homeserver LIST URL format — not directly confirmed, inferred from source read + known GET pattern

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all existing deps; only comfy-table is new and confirmed from docs.rs
- Architecture: HIGH — homeserver DELETE/LIST API confirmed from source; crypto path confirmed from existing code
- Pitfalls: HIGH for serde/signing issues (from existing code patterns); MEDIUM for burn-delete auth gap (confirmed from protocol understanding, no integration test)

**Research date:** 2026-02-22
**Valid until:** 2026-03-22 (30 days — stable libraries, homeserver API unlikely to change)
