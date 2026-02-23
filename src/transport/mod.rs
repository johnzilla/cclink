//! Transport module: Pubky homeserver HTTP client with AuthToken authentication.
//!
//! Implements the binary AuthToken format required by the Pubky homeserver for session
//! authentication. Uses reqwest::blocking with cookie_store(true) for session persistence
//! across PUT operations.
//!
//! AuthToken binary layout (postcard-serialized):
//!   - bytes[0]:      varint length of signature (0x40 = 64, one byte in postcard varint)
//!   - bytes[1..65]:  Ed25519 signature (64 raw bytes)
//!   - bytes[65..75]: namespace b"PUBKY:AUTH" (10 bytes, raw fixed array)
//!   - bytes[75]:     version u8 = 0
//!   - bytes[76..84]: timestamp [u8; 8] big-endian microseconds (postcard fixed array)
//!   - bytes[84..116]: pubkey [u8; 32] (raw fixed array)
//!   - bytes[116..]:  capabilities string (postcard varint length + UTF-8 bytes)
//!
//! Signable region: bytes[65..] (confirmed from pubky-common-0.5.4/src/auth.rs)

use std::cell::Cell;
use std::time::{Duration, SystemTime};

use crate::record::{HandoffRecord, LatestPointer};

// ── AuthToken ──────────────────────────────────────────────────────────────

/// Namespace constant for Pubky auth tokens.
const PUBKY_AUTH: &[u8; 10] = b"PUBKY:AUTH";

/// Build a postcard-serialized AuthToken binary ready to POST to `/session`.
///
/// Binary layout follows pubky-common 0.5.4 exactly. The layout is built
/// manually because serde 1.0.228 does not implement Serialize for [u8; 64]:
///
/// ```text
/// bytes[0]:      varint(64) = 0x40  (postcard length prefix for Signature)
/// bytes[1..65]:  Ed25519 signature  (64 raw bytes)
/// bytes[65..75]: b"PUBKY:AUTH"      (10 raw bytes — fixed [u8; 10] has no prefix)
/// bytes[75]:     0u8                (version)
/// bytes[76..84]: timestamp BE [u8;8] (microseconds since UNIX_EPOCH)
/// bytes[84..116]: pubkey [u8;32]    (32 raw bytes — no prefix for fixed arrays)
/// bytes[116..]:  capabilities       (postcard String: varint(len) + UTF-8)
/// ```
///
/// The signature covers `bytes[65..]` — confirmed from pubky-common 0.5.4
/// auth.rs: `token.signature = keypair.sign(&serialized[65..])`.
pub fn build_auth_token(keypair: &pkarr::Keypair) -> anyhow::Result<Vec<u8>> {
    // Current time in microseconds (Pubky homeserver uses microsecond timestamps)
    let timestamp_us = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| anyhow::anyhow!("system clock error: {}", e))?
        .as_micros() as u64;
    let timestamp_bytes = timestamp_us.to_be_bytes();

    let pubkey_bytes: [u8; 32] = keypair.public_key().to_bytes();

    // Build the signable region first (bytes[65..] in the final token):
    // namespace(10) + version(1) + timestamp(8) + pubkey(32) + capabilities(varint+str)
    let capabilities = "/:rw";
    let cap_bytes = capabilities.as_bytes();

    // postcard encodes String as varint(len) + UTF-8. For "/:rw" (4 bytes), varint(4) = 0x04.
    // Encode varint manually for the capabilities length.
    let mut cap_varint: Vec<u8> = Vec::new();
    let mut remaining = cap_bytes.len() as u64;
    loop {
        let byte = (remaining & 0x7F) as u8;
        remaining >>= 7;
        if remaining == 0 {
            cap_varint.push(byte);
            break;
        } else {
            cap_varint.push(byte | 0x80);
        }
    }

    // Construct signable bytes (everything after the signature region)
    let mut signable: Vec<u8> = Vec::with_capacity(10 + 1 + 8 + 32 + cap_varint.len() + cap_bytes.len());
    signable.extend_from_slice(PUBKY_AUTH);        // namespace: 10 bytes
    signable.push(0u8);                             // version: 1 byte
    signable.extend_from_slice(&timestamp_bytes);   // timestamp: 8 bytes
    signable.extend_from_slice(&pubkey_bytes);      // pubkey: 32 bytes
    signable.extend_from_slice(&cap_varint);        // capabilities varint length
    signable.extend_from_slice(cap_bytes);          // capabilities UTF-8 bytes

    // Sign the signable region
    let signature = keypair.sign(&signable);
    let sig_bytes = signature.to_bytes();

    // Build the full token:
    // postcard serializes ed25519_dalek::Signature as a byte slice: varint(64) + [64 bytes]
    // varint(64) = 0x40 (single byte since 64 < 128)
    let mut token: Vec<u8> = Vec::with_capacity(1 + 64 + signable.len());
    token.push(0x40u8);                    // varint(64) — postcard length prefix for Signature
    token.extend_from_slice(&sig_bytes);   // 64-byte signature
    token.extend_from_slice(&signable);    // signable region

    Ok(token)
}

// ── HomeserverClient ───────────────────────────────────────────────────────

/// HTTP client for the Pubky homeserver.
///
/// Wraps `reqwest::blocking::Client` with `cookie_store(true)` so that the
/// session cookie acquired via `signin()` is automatically sent on subsequent
/// PUT requests.
///
/// The Pubky homeserver uses virtual hosting: every request must include a
/// `Host` header containing the z32-encoded public key of the tenant. Without
/// it, the server cannot identify which user's namespace to operate on and
/// returns 404 for all requests.
///
/// The `signed_in` flag ensures signin HTTP POST is called at most once per
/// client lifetime, regardless of how many authenticated operations follow.
pub struct HomeserverClient {
    client: reqwest::blocking::Client,
    /// Homeserver hostname, e.g. "pubky.app" (no scheme, no trailing slash).
    homeserver: String,
    /// z32-encoded public key of the keypair that owns this client instance.
    /// Used as the `Host` header value on all self-operations.
    pubkey_z32: String,
    /// Tracks whether a session has been established. Uses Cell<bool> for
    /// interior mutability so &self methods can update state without &mut self.
    signed_in: Cell<bool>,
}

impl HomeserverClient {
    /// Create a new HomeserverClient.
    ///
    /// `homeserver` should be a plain hostname like "pubky.app". Any "https://"
    /// prefix is stripped automatically.
    ///
    /// `pubkey_z32` is the z32-encoded public key of the keypair that owns this
    /// client. It is included as the `Host` header on all self-operations so the
    /// Pubky homeserver can identify the correct tenant namespace.
    pub fn new(homeserver: &str, pubkey_z32: &str) -> anyhow::Result<Self> {
        let homeserver = homeserver
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_end_matches('/')
            .to_string();

        let client = reqwest::blocking::Client::builder()
            .cookie_store(true)
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| anyhow::anyhow!("failed to build HTTP client: {}", e))?;

        Ok(Self {
            client,
            homeserver,
            pubkey_z32: pubkey_z32.to_string(),
            signed_in: Cell::new(false),
        })
    }

    /// Return the pubkey z32 to use as the `Host` header value.
    ///
    /// If `pubkey_z32` is `Some(pk)`, use that (cross-user operation targeting
    /// a specific tenant). Otherwise use `self.pubkey_z32` (self-operation on
    /// the client's own namespace).
    fn host_header(&self, pubkey_z32: Option<&str>) -> String {
        match pubkey_z32 {
            Some(pk) => pk.to_string(),
            None => self.pubkey_z32.clone(),
        }
    }

    /// Sign in to the homeserver with a Pubky AuthToken.
    ///
    /// POSTs the binary AuthToken to `/session`. The homeserver sets a session
    /// cookie that is automatically stored in the client's cookie jar.
    ///
    /// For first-time users (accounts that have never signed up), the homeserver
    /// returns 404 on POST `/session`. In that case, this method automatically
    /// falls back to POST `/signup` with the same token. If `/signup` returns 409
    /// (user already exists — a race condition), it retries `/session` once.
    ///
    /// Sets the `signed_in` flag on success so that `ensure_signed_in()` will
    /// skip subsequent calls within the same client lifetime.
    ///
    /// Both `/session` and `/signup` requests include the `Host` header so the
    /// homeserver can route to the correct tenant namespace.
    pub fn signin(&self, keypair: &pkarr::Keypair) -> anyhow::Result<()> {
        let token_bytes = build_auth_token(keypair)?;
        let session_url = format!("https://{}/session", self.homeserver);

        let response = self
            .client
            .post(&session_url)
            .header("Host", &self.pubkey_z32)
            .body(token_bytes.clone())
            .send()
            .map_err(|e| anyhow::anyhow!("signin request failed: {}", e))?;

        let status = response.status();

        if status == reqwest::StatusCode::NOT_FOUND {
            // First-time user: account not yet registered. Fall back to /signup.
            let signup_url = format!("https://{}/signup", self.homeserver);
            let signup_response = self
                .client
                .post(&signup_url)
                .header("Host", &self.pubkey_z32)
                .body(token_bytes.clone())
                .send()
                .map_err(|e| anyhow::anyhow!("signup request failed: {}", e))?;

            let signup_status = signup_response.status();

            if signup_status == reqwest::StatusCode::CONFLICT {
                // Race condition: user was created between our /session and /signup.
                // Retry /session once.
                let retry_response = self
                    .client
                    .post(&session_url)
                    .header("Host", &self.pubkey_z32)
                    .body(build_auth_token(keypair)?) // Fresh token with current timestamp
                    .send()
                    .map_err(|e| anyhow::anyhow!("signin retry request failed: {}", e))?;

                if !retry_response.status().is_success() {
                    let retry_status = retry_response.status();
                    let body = retry_response.text().unwrap_or_default();
                    anyhow::bail!(
                        "signin retry failed after signup conflict (status {}): {}",
                        retry_status,
                        body.trim()
                    );
                }
            } else if !signup_status.is_success() {
                let body = signup_response.text().unwrap_or_default();
                anyhow::bail!(
                    "signup failed (status {}): {}",
                    signup_status,
                    body.trim()
                );
            }
            // /signup succeeded (or conflict resolved) — session is now established
        } else if !status.is_success() {
            let body = response.text().unwrap_or_default();
            anyhow::bail!(
                "signin failed (status {}): {}",
                status,
                body.trim()
            );
        }

        self.signed_in.set(true);
        Ok(())
    }

    /// Ensure the client is signed in, calling `signin()` at most once per lifetime.
    ///
    /// Callers that need an authenticated session but don't want to force a fresh
    /// signin (e.g., `publish()`) should use this instead of `signin()` directly.
    fn ensure_signed_in(&self, keypair: &pkarr::Keypair) -> anyhow::Result<()> {
        if !self.signed_in.get() {
            self.signin(keypair)?;
        }
        Ok(())
    }

    /// PUT a serialized HandoffRecord at `/pub/cclink/{token}`.
    ///
    /// Requires a prior successful `signin()` call. The session cookie is sent
    /// automatically by the reqwest cookie jar. Includes the `Host` header so
    /// the homeserver routes to the correct tenant namespace.
    pub fn put_record(&self, token: &str, record_bytes: &[u8]) -> anyhow::Result<()> {
        let url = format!("https://{}/pub/cclink/{}", self.homeserver, token);

        let response = self
            .client
            .put(&url)
            .header("Host", &self.pubkey_z32)
            .header("content-type", "application/octet-stream")
            .body(record_bytes.to_vec())
            .send()
            .map_err(|e| anyhow::anyhow!("PUT record request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            anyhow::bail!(
                "PUT record failed (status {}): {}",
                status,
                body.trim()
            );
        }

        Ok(())
    }

    /// PUT the latest.json pointer at `/pub/cclink/latest`.
    ///
    /// Requires a prior successful `signin()` call. Includes the `Host` header
    /// so the homeserver routes to the correct tenant namespace.
    pub fn put_latest(&self, latest_bytes: &[u8]) -> anyhow::Result<()> {
        let url = format!("https://{}/pub/cclink/latest", self.homeserver);

        let response = self
            .client
            .put(&url)
            .header("Host", &self.pubkey_z32)
            .header("content-type", "application/octet-stream")
            .body(latest_bytes.to_vec())
            .send()
            .map_err(|e| anyhow::anyhow!("PUT latest request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            anyhow::bail!(
                "PUT latest failed (status {}): {}",
                status,
                body.trim()
            );
        }

        Ok(())
    }

    /// GET and verify a HandoffRecord at `/pub/cclink/{token}`.
    ///
    /// The record is deserialized from JSON and its Ed25519 signature is verified
    /// against `pubkey` before being returned. Returns an error on 404 or
    /// signature mismatch (hard fail — no bypass).
    ///
    /// Uses the client's own pubkey as the `Host` header (self-operation).
    pub fn get_record(
        &self,
        token: &str,
        pubkey: &pkarr::PublicKey,
    ) -> anyhow::Result<HandoffRecord> {
        let url = format!("https://{}/pub/cclink/{}", self.homeserver, token);
        let bytes = self.get_bytes(&url, None)?;
        self.deserialize_and_verify(&bytes, pubkey)
    }

    /// GET and verify a HandoffRecord using the Host-header-based multi-tenant routing.
    ///
    /// URL path: `/pub/cclink/{token}` (same as self-operation).
    /// Tenant identification is done via the `Host` header set to `pubkey_z32`,
    /// NOT by embedding the pubkey in the URL path. This is the correct Pubky
    /// homeserver virtual hosting API.
    pub fn get_record_by_pubkey(
        &self,
        pubkey_z32: &str,
        token: &str,
        pubkey: &pkarr::PublicKey,
    ) -> anyhow::Result<HandoffRecord> {
        let url = format!("https://{}/pub/cclink/{}", self.homeserver, token);
        let bytes = self.get_bytes(&url, Some(pubkey_z32))?;
        self.deserialize_and_verify(&bytes, pubkey)
    }

    /// GET the latest.json pointer bytes.
    ///
    /// If `pubkey_z32` is `None`, reads own records via the client's pubkey in
    /// the `Host` header. If `Some`, reads another user's records via their
    /// pubkey in the `Host` header.
    ///
    /// URL path is always `/pub/cclink/latest` — tenant differentiation is done
    /// via the `Host` header, not the URL path.
    pub fn get_latest(&self, pubkey_z32: Option<&str>) -> anyhow::Result<Vec<u8>> {
        let url = format!("https://{}/pub/cclink/latest", self.homeserver);
        self.get_bytes(&url, pubkey_z32)
    }

    /// Publish a HandoffRecord: sign in, PUT record, PUT latest pointer.
    ///
    /// Token is the record's `created_at` Unix timestamp (seconds) as a string.
    /// Returns the token string used for the record path.
    pub fn publish(
        &self,
        keypair: &pkarr::Keypair,
        record: &HandoffRecord,
    ) -> anyhow::Result<String> {
        // Token is the Unix timestamp in seconds (from created_at field)
        let token = record.created_at.to_string();

        // Serialize the record to JSON
        let record_bytes = serde_json::to_vec(record)
            .map_err(|e| anyhow::anyhow!("failed to serialize record: {}", e))?;

        // Sign in to the homeserver (lazy — will only POST /session if not already signed in)
        self.ensure_signed_in(keypair)?;

        // PUT the record
        self.put_record(&token, &record_bytes)?;

        // Build and PUT the latest pointer
        let latest = LatestPointer {
            created_at: record.created_at,
            hostname: record.hostname.clone(),
            project: record.project.clone(),
            token: token.clone(),
        };
        let latest_bytes = serde_json::to_vec(&latest)
            .map_err(|e| anyhow::anyhow!("failed to serialize latest pointer: {}", e))?;
        self.put_latest(&latest_bytes)?;

        Ok(token)
    }

    /// DELETE a HandoffRecord at `/pub/cclink/{token}`.
    ///
    /// Treats 404 as success (idempotent — record already deleted).
    /// Must be called AFTER `signin()` — the session cookie is forwarded automatically.
    /// Includes the `Host` header for correct tenant routing.
    pub fn delete_record(&self, token: &str) -> anyhow::Result<()> {
        let url = format!("https://{}/pub/cclink/{}", self.homeserver, token);
        let response = self
            .client
            .delete(&url)
            .header("Host", &self.pubkey_z32)
            .send()
            .map_err(|e| anyhow::anyhow!("DELETE request failed: {}", e))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(()); // Already deleted — idempotent
        }
        if !response.status().is_success() {
            let status = response.status();
            anyhow::bail!("DELETE failed (status {}): {}", status, token);
        }
        Ok(())
    }

    /// List all record tokens under `/pub/cclink/`.
    ///
    /// Returns only numeric timestamp tokens — filters out "latest" and any non-numeric keys.
    /// Must be called AFTER `signin()` — the directory listing requires an authenticated session.
    /// Returns an empty vec if no records are published (404).
    /// Includes the `Host` header for correct tenant routing.
    pub fn list_record_tokens(&self) -> anyhow::Result<Vec<String>> {
        let url = format!("https://{}/pub/cclink/", self.homeserver);
        let response = self
            .client
            .get(&url)
            .header("Host", &self.pubkey_z32)
            .send()
            .map_err(|e| anyhow::anyhow!("LIST request failed: {}", e))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(vec![]);
        }
        if !response.status().is_success() {
            anyhow::bail!("LIST failed (status {})", response.status());
        }
        let body = response.text()
            .map_err(|e| anyhow::anyhow!("failed to read LIST response: {}", e))?;
        // Homeserver returns full pubky:// URIs, e.g.:
        //   pubky://<z32-pubkey>/pub/cclink/1700000000
        //   pubky://<z32-pubkey>/pub/cclink/latest
        // Split on "/pub/cclink/" to extract the token portion.
        // Also handles legacy plain-path format: /pub/cclink/1700000000
        let tokens: Vec<String> = body.lines()
            .filter(|l| !l.is_empty())
            .filter_map(|line| {
                line.split("/pub/cclink/").nth(1)
                    .map(|t| t.trim_end_matches('/').to_string())
            })
            .filter(|t| t.parse::<u64>().is_ok()) // Filter out "latest" and non-numeric keys
            .collect();
        Ok(tokens)
    }

    /// Fetch all HandoffRecords for a given public key in one transport call.
    ///
    /// Calls `list_record_tokens()` to get all token strings, then fetches each
    /// record individually via `get_record()`. Records that fail to fetch or verify
    /// are silently skipped (consistent with list command behavior).
    ///
    /// NOTE: The Pubky homeserver has no native batch-get endpoint — its directory
    /// listing returns only path names, not record content. True single-request
    /// batching is not possible with the current protocol. The optimization here is
    /// architectural: this method encapsulates the 1-listing + N-fetch pattern in the
    /// transport layer so the command layer makes ONE call. If the homeserver later
    /// supports batch GET, only this method changes.
    pub fn get_all_records(
        &self,
        pubkey: &pkarr::PublicKey,
    ) -> anyhow::Result<Vec<(String, HandoffRecord)>> {
        let tokens = self.list_record_tokens()?;
        if tokens.is_empty() {
            return Ok(vec![]);
        }
        let mut results = Vec::new();
        for token in &tokens {
            match self.get_record(token, pubkey) {
                Ok(record) => results.push((token.clone(), record)),
                Err(_) => {
                    // Skip records that fail to fetch or verify — they may have been
                    // tampered with or partially written. Silent skip is correct.
                    continue;
                }
            }
        }
        Ok(results)
    }

    // ── Private helpers ────────────────────────────────────────────────────

    /// Perform a GET request, returning response body bytes.
    ///
    /// `host_pubkey` overrides the `Host` header value. If `None`, uses the
    /// client's own pubkey (self-operation). If `Some(pk)`, uses that pubkey
    /// (cross-user operation targeting a different tenant).
    fn get_bytes(&self, url: &str, host_pubkey: Option<&str>) -> anyhow::Result<Vec<u8>> {
        let host = self.host_header(host_pubkey);
        let response = self
            .client
            .get(url)
            .header("Host", &host)
            .send()
            .map_err(|e| anyhow::anyhow!("GET request failed: {}", e))?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(crate::error::CclinkError::RecordNotFound.into());
        }

        if !response.status().is_success() {
            let status = response.status();
            anyhow::bail!("GET failed (status {}): {}", status, url);
        }

        Ok(response
            .bytes()
            .map_err(|e| anyhow::anyhow!("failed to read response bytes: {}", e))?
            .to_vec())
    }

    /// Deserialize JSON bytes to HandoffRecord and verify its Ed25519 signature.
    fn deserialize_and_verify(
        &self,
        bytes: &[u8],
        pubkey: &pkarr::PublicKey,
    ) -> anyhow::Result<HandoffRecord> {
        let record: HandoffRecord = serde_json::from_slice(bytes)
            .map_err(|e| anyhow::anyhow!("failed to deserialize record: {}", e))?;

        crate::record::verify_record(&record, pubkey)?;

        Ok(record)
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

/// Extract numeric timestamp tokens from a homeserver directory listing body.
///
/// Parses newline-separated lines that may be full pubky:// URIs or plain paths.
/// Filters out "latest" and any non-numeric keys, and removes trailing slashes.
/// This mirrors the parsing logic in `list_record_tokens()` and is exposed here
/// for unit testing.
#[cfg(test)]
fn parse_record_tokens(body: &str) -> Vec<String> {
    body.lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            line.split("/pub/cclink/").nth(1)
                .map(|t| t.trim_end_matches('/').to_string())
        })
        .filter(|t| t.parse::<u64>().is_ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::{sign_record, HandoffRecordSignable};

    fn fixed_keypair() -> pkarr::Keypair {
        pkarr::Keypair::from_secret_key(&[42u8; 32])
    }

    // ── AuthToken tests ────────────────────────────────────────────────────

    #[test]
    fn test_auth_token_has_valid_length() {
        let keypair = fixed_keypair();
        let token = build_auth_token(&keypair).expect("build_auth_token should succeed");

        // Minimum expected length:
        // 1 (varint) + 64 (sig) + 10 (namespace) + 1 (version) + 8 (timestamp) + 32 (pubkey) + min(capabilities)
        // = 65 + 10 + 1 + 8 + 32 + ~5 = ~121 bytes minimum
        assert!(
            token.len() >= 116,
            "AuthToken should be at least 116 bytes, got {}",
            token.len()
        );
    }

    #[test]
    fn test_auth_token_structure() {
        let keypair = fixed_keypair();
        let token = build_auth_token(&keypair).expect("build_auth_token should succeed");

        // Verify byte[0] is the varint length prefix for the 64-byte signature
        // postcard varint for 64: since 64 < 128, it encodes as a single byte = 0x40 (64 decimal)
        assert_eq!(
            token[0], 0x40,
            "first byte should be varint(64) = 0x40, got 0x{:02x}",
            token[0]
        );

        // Verify namespace at bytes[65..75] = b"PUBKY:AUTH"
        assert_eq!(
            &token[65..75],
            b"PUBKY:AUTH",
            "bytes[65..75] should be b\"PUBKY:AUTH\""
        );

        // Verify version at byte[75] = 0
        assert_eq!(token[75], 0, "version byte should be 0");
    }

    #[test]
    fn test_auth_token_signature_verifies() {
        // Build a token and verify the signature covers bytes[65..]
        let keypair = fixed_keypair();
        let token = build_auth_token(&keypair).expect("build_auth_token should succeed");

        // Extract signature from bytes[1..65]
        let sig_bytes: [u8; 64] = token[1..65].try_into().expect("sig must be 64 bytes");
        let sig = ed25519_dalek::Signature::from_bytes(&sig_bytes);

        // Signable region is bytes[65..]
        let signable = &token[65..];

        // Verify using the public key
        keypair
            .public_key()
            .verify(signable, &sig)
            .expect("signature over signable region should verify");
    }

    #[test]
    fn test_auth_token_different_keypairs_produce_different_tokens() {
        let keypair_a = fixed_keypair();
        let keypair_b = pkarr::Keypair::from_secret_key(&[99u8; 32]);

        let token_a = build_auth_token(&keypair_a).expect("token_a build failed");
        let token_b = build_auth_token(&keypair_b).expect("token_b build failed");

        // Pubkeys differ (bytes[84..116]), so tokens must differ
        assert_ne!(
            token_a[84..116],
            token_b[84..116],
            "pubkey region should differ between keypairs"
        );

        // Signatures must also differ
        assert_ne!(
            token_a[1..65],
            token_b[1..65],
            "signatures should differ between keypairs"
        );
    }

    // ── HomeserverClient tests ─────────────────────────────────────────────

    #[test]
    fn test_homeserver_client_new() {
        let keypair = fixed_keypair();
        let pubkey_z32 = keypair.public_key().to_z32();
        // Client should build with cookie_store enabled
        let client = HomeserverClient::new("pubky.app", &pubkey_z32);
        assert!(
            client.is_ok(),
            "HomeserverClient::new should succeed with valid hostname"
        );
    }

    #[test]
    fn test_homeserver_client_strips_https_prefix() {
        let keypair = fixed_keypair();
        let pubkey_z32 = keypair.public_key().to_z32();
        let client = HomeserverClient::new("https://pubky.app", &pubkey_z32).expect("should succeed");
        assert_eq!(
            client.homeserver, "pubky.app",
            "https:// prefix should be stripped"
        );
    }

    #[test]
    fn test_homeserver_client_stores_pubkey_z32() {
        let keypair = fixed_keypair();
        let pubkey_z32 = keypair.public_key().to_z32();
        let client = HomeserverClient::new("pubky.app", &pubkey_z32).expect("should succeed");
        assert_eq!(
            client.pubkey_z32, pubkey_z32,
            "pubkey_z32 should be stored in the client"
        );
    }

    #[test]
    fn test_host_header_self_operation() {
        let keypair = fixed_keypair();
        let pubkey_z32 = keypair.public_key().to_z32();
        let client = HomeserverClient::new("pubky.app", &pubkey_z32).expect("should succeed");
        // None -> uses client's own pubkey
        assert_eq!(
            client.host_header(None),
            pubkey_z32,
            "host_header(None) should return client's own pubkey_z32"
        );
    }

    #[test]
    fn test_host_header_cross_user_operation() {
        let keypair_a = fixed_keypair();
        let keypair_b = pkarr::Keypair::from_secret_key(&[99u8; 32]);
        let pubkey_a = keypair_a.public_key().to_z32();
        let pubkey_b = keypair_b.public_key().to_z32();
        let client = HomeserverClient::new("pubky.app", &pubkey_a).expect("should succeed");
        // Some(pk) -> uses that pubkey (cross-user)
        assert_eq!(
            client.host_header(Some(&pubkey_b)),
            pubkey_b,
            "host_header(Some(pk)) should return the provided pubkey"
        );
    }

    #[test]
    fn test_ensure_signed_in_flag() {
        let keypair = fixed_keypair();
        let pubkey_z32 = keypair.public_key().to_z32();
        // New client should start with signed_in = false
        let client = HomeserverClient::new("pubky.app", &pubkey_z32).expect("client build failed");
        assert!(
            !client.signed_in.get(),
            "signed_in should be false on new client"
        );

        // Manually set the flag to simulate a successful signin
        client.signed_in.set(true);
        assert!(
            client.signed_in.get(),
            "signed_in should be true after setting"
        );

        // Reset and verify
        client.signed_in.set(false);
        assert!(
            !client.signed_in.get(),
            "signed_in should be false after reset"
        );
    }

    #[test]
    fn test_signin_url_construction() {
        // Verify the session and signup URL patterns are constructed correctly.
        // (Cannot test actual network, but verifies the string format.)
        let keypair = fixed_keypair();
        let pubkey_z32 = keypair.public_key().to_z32();
        let client = HomeserverClient::new("pubky.app", &pubkey_z32).expect("should succeed");

        // Verify expected URL format for /session
        let session_url = format!("https://{}/session", client.homeserver);
        assert_eq!(session_url, "https://pubky.app/session");

        // Verify expected URL format for /signup
        let signup_url = format!("https://{}/signup", client.homeserver);
        assert_eq!(signup_url, "https://pubky.app/signup");

        // Verify the Host header will be the pubkey_z32
        assert_eq!(client.pubkey_z32, pubkey_z32);
    }

    #[test]
    fn test_publish_token_is_created_at_timestamp() {
        // publish() should use record.created_at as the token string
        let keypair = fixed_keypair();
        let created_at: u64 = 1_700_000_000;

        // We test the token generation logic directly (without a live server)
        // by verifying that token = created_at.to_string()
        let expected_token = created_at.to_string();
        assert_eq!(expected_token, "1700000000");

        // Build a complete signed record and verify token derivation
        let signable = HandoffRecordSignable {
            blob: "dGVzdA==".to_string(),
            burn: false,
            created_at,
            hostname: "testhost".to_string(),
            pin_salt: None,
            project: "/test".to_string(),
            pubkey: keypair.public_key().to_z32(),
            recipient: None,
            ttl: 3600,
        };
        let signature = sign_record(&signable, &keypair).expect("sign_record failed");
        let record = HandoffRecord {
            blob: signable.blob,
            burn: false,
            created_at: signable.created_at,
            hostname: signable.hostname,
            pin_salt: None,
            project: signable.project,
            pubkey: signable.pubkey,
            recipient: None,
            signature,
            ttl: signable.ttl,
        };

        // Token should be the created_at timestamp as a string
        let token = record.created_at.to_string();
        assert_eq!(
            token, "1700000000",
            "token must be created_at as string"
        );
    }

    #[test]
    fn test_get_record_deserialization_and_verification_pipeline() {
        // Test the deserialization + verify_record pipeline used by get_record
        // without a live homeserver.
        let keypair = fixed_keypair();
        let pubkey_z32 = keypair.public_key().to_z32();
        let created_at: u64 = 1_700_000_000;

        let signable = HandoffRecordSignable {
            blob: "dGVzdA==".to_string(),
            burn: false,
            created_at,
            hostname: "testhost".to_string(),
            pin_salt: None,
            project: "/test".to_string(),
            pubkey: pubkey_z32.clone(),
            recipient: None,
            ttl: 3600,
        };
        let signature = sign_record(&signable, &keypair).expect("sign_record failed");
        let record = HandoffRecord {
            blob: signable.blob,
            burn: false,
            created_at: signable.created_at,
            hostname: signable.hostname,
            pin_salt: None,
            project: signable.project,
            pubkey: signable.pubkey,
            recipient: None,
            signature,
            ttl: signable.ttl,
        };

        // Serialize to JSON bytes (as if received from homeserver)
        let json_bytes = serde_json::to_vec(&record).expect("serialize failed");

        // Simulate what get_record does internally: deserialize + verify
        let client = HomeserverClient::new("pubky.app", &pubkey_z32).expect("client build failed");
        let result = client.deserialize_and_verify(&json_bytes, &keypair.public_key());
        assert!(result.is_ok(), "valid signed record should deserialize and verify: {:?}", result.err());

        // Now test with a tampered record (should fail)
        let mut tampered = record.clone();
        tampered.ttl = tampered.ttl + 9999;
        let tampered_bytes = serde_json::to_vec(&tampered).expect("serialize failed");
        let tampered_result = client.deserialize_and_verify(&tampered_bytes, &keypair.public_key());
        assert!(
            tampered_result.is_err(),
            "tampered record should fail verification"
        );
    }

    #[test]
    fn test_get_record_wrong_pubkey_fails() {
        let keypair_a = fixed_keypair();
        let keypair_b = pkarr::Keypair::from_secret_key(&[99u8; 32]);
        let pubkey_a_z32 = keypair_a.public_key().to_z32();

        let signable = HandoffRecordSignable {
            blob: "dGVzdA==".to_string(),
            burn: false,
            created_at: 1_700_000_000,
            hostname: "testhost".to_string(),
            pin_salt: None,
            project: "/test".to_string(),
            pubkey: pubkey_a_z32.clone(),
            recipient: None,
            ttl: 3600,
        };
        let signature = sign_record(&signable, &keypair_a).expect("sign_record failed");
        let record = HandoffRecord {
            blob: signable.blob,
            burn: false,
            created_at: signable.created_at,
            hostname: signable.hostname,
            pin_salt: None,
            project: signable.project,
            pubkey: signable.pubkey,
            recipient: None,
            signature,
            ttl: signable.ttl,
        };

        let json_bytes = serde_json::to_vec(&record).expect("serialize failed");
        let client = HomeserverClient::new("pubky.app", &pubkey_a_z32).expect("client build failed");

        // Verify with wrong pubkey must fail
        let result = client.deserialize_and_verify(&json_bytes, &keypair_b.public_key());
        assert!(result.is_err(), "wrong pubkey must cause verification failure");
    }

    // ── list_record_tokens parsing tests ───────────────────────────────────

    #[test]
    fn test_parse_record_tokens_full_pubky_uris() {
        // Homeserver returns full pubky:// URIs; "latest" must be filtered out
        let body = "pubky://abc123xyz/pub/cclink/1700000000\npubky://abc123xyz/pub/cclink/latest\n";
        let tokens = parse_record_tokens(body);
        assert_eq!(tokens, vec!["1700000000"]);
    }

    #[test]
    fn test_parse_record_tokens_plain_paths() {
        // Legacy plain-path format also parses correctly
        let body = "/pub/cclink/1700000000\n/pub/cclink/latest\n";
        let tokens = parse_record_tokens(body);
        assert_eq!(tokens, vec!["1700000000"]);
    }

    #[test]
    fn test_parse_record_tokens_mixed_formats() {
        // Mixed pubky:// and plain-path entries in same listing
        let body = "pubky://abc123xyz/pub/cclink/1700000000\n/pub/cclink/1700000001\n";
        let mut tokens = parse_record_tokens(body);
        tokens.sort();
        assert_eq!(tokens, vec!["1700000000", "1700000001"]);
    }

    #[test]
    fn test_parse_record_tokens_empty_body() {
        // Empty body returns empty vec
        let tokens = parse_record_tokens("");
        assert!(tokens.is_empty(), "empty body should yield no tokens");
    }

    #[test]
    fn test_parse_record_tokens_only_latest() {
        // Body with only "latest" pointer yields no numeric tokens
        let body = "pubky://abc123xyz/pub/cclink/latest\n";
        let tokens = parse_record_tokens(body);
        assert!(tokens.is_empty(), "only 'latest' should yield no tokens");
    }

    #[test]
    fn test_parse_record_tokens_trailing_slash() {
        // Trailing slashes on token paths are stripped
        let body = "pubky://abc123xyz/pub/cclink/1700000000/\n";
        let tokens = parse_record_tokens(body);
        assert_eq!(tokens, vec!["1700000000"]);
    }

    #[test]
    fn test_parse_record_tokens_multiple_numeric() {
        // Multiple numeric tokens returned in listing order
        let body = "pubky://abc123xyz/pub/cclink/1700000001\npubky://abc123xyz/pub/cclink/1700000002\npubky://abc123xyz/pub/cclink/latest\n";
        let tokens = parse_record_tokens(body);
        assert_eq!(tokens, vec!["1700000001", "1700000002"]);
    }

    /// Integration test requiring a live Pubky homeserver.
    ///
    /// Run with: cargo test --lib transport::tests::test_integration_signin_put_get -- --ignored
    #[test]
    #[ignore]
    fn test_integration_signin_put_get() {
        let keypair = pkarr::Keypair::random();
        let pubkey_z32 = keypair.public_key().to_z32();

        let client = HomeserverClient::new("pubky.app", &pubkey_z32).expect("client build failed");

        // Sign in (handles signup fallback for new keypair)
        client.signin(&keypair).expect("signin should succeed");

        // Build a signed record
        let signable = HandoffRecordSignable {
            blob: "dGVzdA==".to_string(),
            burn: false,
            created_at: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            hostname: "integration-test".to_string(),
            pin_salt: None,
            project: "/test".to_string(),
            pubkey: pubkey_z32.clone(),
            recipient: None,
            ttl: 3600,
        };
        let signature = sign_record(&signable, &keypair).expect("sign_record failed");
        let record = HandoffRecord {
            blob: signable.blob,
            burn: false,
            created_at: signable.created_at,
            hostname: signable.hostname,
            pin_salt: None,
            project: signable.project,
            pubkey: signable.pubkey,
            recipient: None,
            signature,
            ttl: signable.ttl,
        };

        // Publish (signin + PUT record + PUT latest)
        let token = client.publish(&keypair, &record).expect("publish should succeed");
        assert!(!token.is_empty(), "token should not be empty");

        // GET and verify
        let retrieved = client
            .get_record(&token, &keypair.public_key())
            .expect("get_record should succeed");
        assert_eq!(retrieved.created_at, record.created_at);
        assert_eq!(retrieved.hostname, record.hostname);
    }

    /// Integration test: first-time user signup flow.
    ///
    /// Verifies that a brand-new keypair (never signed up) triggers the /signup
    /// fallback in signin() and can subsequently publish and retrieve a record.
    ///
    /// Run with: cargo test --lib transport::tests::test_integration_signup_new_keypair -- --ignored
    #[test]
    #[ignore]
    fn test_integration_signup_new_keypair() {
        let keypair = pkarr::Keypair::random(); // Brand-new keypair, never registered
        let pubkey_z32 = keypair.public_key().to_z32();

        let client = HomeserverClient::new("pubky.app", &pubkey_z32).expect("client build failed");

        // signin() should trigger the /signup fallback for a new keypair
        client.signin(&keypair).expect("signin (with signup fallback) should succeed for new keypair");
        assert!(client.signed_in.get(), "signed_in should be true after signup");

        // Build and publish a record
        let signable = HandoffRecordSignable {
            blob: "dGVzdA==".to_string(),
            burn: false,
            created_at: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            hostname: "signup-integration-test".to_string(),
            pin_salt: None,
            project: "/signup-test".to_string(),
            pubkey: pubkey_z32.clone(),
            recipient: None,
            ttl: 3600,
        };
        let signature = sign_record(&signable, &keypair).expect("sign_record failed");
        let record = HandoffRecord {
            blob: signable.blob,
            burn: false,
            created_at: signable.created_at,
            hostname: signable.hostname,
            pin_salt: None,
            project: signable.project,
            pubkey: signable.pubkey,
            recipient: None,
            signature,
            ttl: signable.ttl,
        };

        let token = client.publish(&keypair, &record).expect("publish should succeed after signup");
        assert!(!token.is_empty(), "token should not be empty");

        // Retrieve and verify the round-trip
        let retrieved = client
            .get_record(&token, &keypair.public_key())
            .expect("get_record should succeed");
        assert_eq!(retrieved.created_at, record.created_at);
        assert_eq!(retrieved.project, record.project);
    }

    /// Integration test: cross-user GET via Host header routing.
    ///
    /// Verifies that user_b can retrieve user_a's record by using user_a's pubkey
    /// in the Host header (not embedded in the URL path).
    ///
    /// Run with: cargo test --lib transport::tests::test_integration_cross_user_get -- --ignored
    #[test]
    #[ignore]
    fn test_integration_cross_user_get() {
        let keypair_a = pkarr::Keypair::random();
        let keypair_b = pkarr::Keypair::random();
        let pubkey_a_z32 = keypair_a.public_key().to_z32();
        let pubkey_b_z32 = keypair_b.public_key().to_z32();

        // user_a publishes a self-encrypted record
        let client_a = HomeserverClient::new("pubky.app", &pubkey_a_z32).expect("client_a build failed");
        client_a.signin(&keypair_a).expect("user_a signin should succeed");

        let created_at = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let signable = HandoffRecordSignable {
            blob: "dGVzdA==".to_string(),
            burn: false,
            created_at,
            hostname: "cross-user-test".to_string(),
            pin_salt: None,
            project: "/cross-user".to_string(),
            pubkey: pubkey_a_z32.clone(),
            recipient: None,
            ttl: 3600,
        };
        let signature = sign_record(&signable, &keypair_a).expect("sign_record failed");
        let record_a = HandoffRecord {
            blob: signable.blob,
            burn: false,
            created_at: signable.created_at,
            hostname: signable.hostname,
            pin_salt: None,
            project: signable.project,
            pubkey: signable.pubkey,
            recipient: None,
            signature,
            ttl: signable.ttl,
        };
        let token = client_a.publish(&keypair_a, &record_a).expect("user_a publish should succeed");

        // user_b retrieves user_a's record via get_record_by_pubkey (Host header routing)
        let client_b = HomeserverClient::new("pubky.app", &pubkey_b_z32).expect("client_b build failed");
        let retrieved = client_b
            .get_record_by_pubkey(&pubkey_a_z32, &token, &keypair_a.public_key())
            .expect("cross-user get_record_by_pubkey should succeed via Host header routing");

        // Verify the record metadata matches (cannot decrypt — it's self-encrypted by user_a)
        assert_eq!(retrieved.created_at, record_a.created_at);
        assert_eq!(retrieved.pubkey, pubkey_a_z32);
        assert_eq!(retrieved.project, record_a.project);
    }
}
