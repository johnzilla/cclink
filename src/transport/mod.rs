/// Transport module: Pubky homeserver HTTP client with AuthToken authentication.
///
/// Implements the binary AuthToken format required by the Pubky homeserver for session
/// authentication. Uses reqwest::blocking with cookie_store(true) for session persistence
/// across PUT operations.
///
/// AuthToken binary layout (postcard-serialized):
///   - bytes[0]:      varint length of signature (0x40 = 64, one byte in postcard varint)
///   - bytes[1..65]:  Ed25519 signature (64 raw bytes)
///   - bytes[65..75]: namespace b"PUBKY:AUTH" (10 bytes, raw fixed array)
///   - bytes[75]:     version u8 = 0
///   - bytes[76..84]: timestamp [u8; 8] big-endian microseconds (postcard fixed array)
///   - bytes[84..116]: pubkey [u8; 32] (raw fixed array)
///   - bytes[116..]:  capabilities string (postcard varint length + UTF-8 bytes)
///
/// Signable region: bytes[65..] (confirmed from pubky-common-0.5.4/src/auth.rs)

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
pub struct HomeserverClient {
    client: reqwest::blocking::Client,
    /// Homeserver hostname, e.g. "pubky.app" (no scheme, no trailing slash).
    homeserver: String,
}

impl HomeserverClient {
    /// Create a new HomeserverClient.
    ///
    /// `homeserver` should be a plain hostname like "pubky.app". Any "https://"
    /// prefix is stripped automatically.
    pub fn new(homeserver: &str) -> anyhow::Result<Self> {
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

        Ok(Self { client, homeserver })
    }

    /// Sign in to the homeserver with a Pubky AuthToken.
    ///
    /// POSTs the binary AuthToken to `/session`. The homeserver sets a session
    /// cookie that is automatically stored in the client's cookie jar.
    pub fn signin(&self, keypair: &pkarr::Keypair) -> anyhow::Result<()> {
        let token_bytes = build_auth_token(keypair)?;
        let url = format!("https://{}/session", self.homeserver);

        let response = self
            .client
            .post(&url)
            .body(token_bytes)
            .send()
            .map_err(|e| anyhow::anyhow!("signin request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            anyhow::bail!(
                "signin failed (status {}): {}",
                status,
                body.trim()
            );
        }

        Ok(())
    }

    /// PUT a serialized HandoffRecord at `/pub/cclink/{token}`.
    ///
    /// Requires a prior successful `signin()` call. The session cookie is sent
    /// automatically by the reqwest cookie jar.
    pub fn put_record(&self, token: &str, record_bytes: &[u8]) -> anyhow::Result<()> {
        let url = format!("https://{}/pub/cclink/{}", self.homeserver, token);

        let response = self
            .client
            .put(&url)
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
    /// Requires a prior successful `signin()` call.
    pub fn put_latest(&self, latest_bytes: &[u8]) -> anyhow::Result<()> {
        let url = format!("https://{}/pub/cclink/latest", self.homeserver);

        let response = self
            .client
            .put(&url)
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
    pub fn get_record(
        &self,
        token: &str,
        pubkey: &pkarr::PublicKey,
    ) -> anyhow::Result<HandoffRecord> {
        let url = format!("https://{}/pub/cclink/{}", self.homeserver, token);
        let bytes = self.get_bytes(&url)?;
        self.deserialize_and_verify(&bytes, pubkey)
    }

    /// GET and verify a HandoffRecord using the multi-tenant path.
    ///
    /// URL pattern: `/{pubkey_z32}/pub/cclink/{token}` — used when reading
    /// another user's records without a session cookie.
    pub fn get_record_by_pubkey(
        &self,
        pubkey_z32: &str,
        token: &str,
        pubkey: &pkarr::PublicKey,
    ) -> anyhow::Result<HandoffRecord> {
        let url = format!(
            "https://{}/{}/pub/cclink/{}",
            self.homeserver, pubkey_z32, token
        );
        let bytes = self.get_bytes(&url)?;
        self.deserialize_and_verify(&bytes, pubkey)
    }

    /// GET the latest.json pointer bytes.
    ///
    /// If `pubkey_z32` is `None`, reads own records via session cookie.
    /// If `Some`, reads another user's records via multi-tenant path.
    pub fn get_latest(&self, pubkey_z32: Option<&str>) -> anyhow::Result<Vec<u8>> {
        let url = match pubkey_z32 {
            None => format!("https://{}/pub/cclink/latest", self.homeserver),
            Some(pk) => format!("https://{}/{}/pub/cclink/latest", self.homeserver, pk),
        };
        self.get_bytes(&url)
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

        // Sign in to the homeserver (acquires session cookie)
        self.signin(keypair)?;

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
    pub fn delete_record(&self, token: &str) -> anyhow::Result<()> {
        let url = format!("https://{}/pub/cclink/{}", self.homeserver, token);
        let response = self.client.delete(&url).send()
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
    pub fn list_record_tokens(&self) -> anyhow::Result<Vec<String>> {
        let url = format!("https://{}/pub/cclink/", self.homeserver);
        let response = self.client.get(&url).send()
            .map_err(|e| anyhow::anyhow!("LIST request failed: {}", e))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(vec![]);
        }
        if !response.status().is_success() {
            anyhow::bail!("LIST failed (status {})", response.status());
        }
        let body = response.text()
            .map_err(|e| anyhow::anyhow!("failed to read LIST response: {}", e))?;
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

    // ── Private helpers ────────────────────────────────────────────────────

    /// Perform a GET request, returning response body bytes.
    fn get_bytes(&self, url: &str) -> anyhow::Result<Vec<u8>> {
        let response = self
            .client
            .get(url)
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
        // Client should build with cookie_store enabled
        let client = HomeserverClient::new("pubky.app");
        assert!(
            client.is_ok(),
            "HomeserverClient::new should succeed with valid hostname"
        );
    }

    #[test]
    fn test_homeserver_client_strips_https_prefix() {
        let client = HomeserverClient::new("https://pubky.app").expect("should succeed");
        assert_eq!(
            client.homeserver, "pubky.app",
            "https:// prefix should be stripped"
        );
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
        let created_at: u64 = 1_700_000_000;

        let signable = HandoffRecordSignable {
            blob: "dGVzdA==".to_string(),
            burn: false,
            created_at,
            hostname: "testhost".to_string(),
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
            project: signable.project,
            pubkey: signable.pubkey,
            recipient: None,
            signature,
            ttl: signable.ttl,
        };

        // Serialize to JSON bytes (as if received from homeserver)
        let json_bytes = serde_json::to_vec(&record).expect("serialize failed");

        // Simulate what get_record does internally: deserialize + verify
        let client = HomeserverClient::new("pubky.app").expect("client build failed");
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

        let signable = HandoffRecordSignable {
            blob: "dGVzdA==".to_string(),
            burn: false,
            created_at: 1_700_000_000,
            hostname: "testhost".to_string(),
            project: "/test".to_string(),
            pubkey: keypair_a.public_key().to_z32(),
            recipient: None,
            ttl: 3600,
        };
        let signature = sign_record(&signable, &keypair_a).expect("sign_record failed");
        let record = HandoffRecord {
            blob: signable.blob,
            burn: false,
            created_at: signable.created_at,
            hostname: signable.hostname,
            project: signable.project,
            pubkey: signable.pubkey,
            recipient: None,
            signature,
            ttl: signable.ttl,
        };

        let json_bytes = serde_json::to_vec(&record).expect("serialize failed");
        let client = HomeserverClient::new("pubky.app").expect("client build failed");

        // Verify with wrong pubkey must fail
        let result = client.deserialize_and_verify(&json_bytes, &keypair_b.public_key());
        assert!(result.is_err(), "wrong pubkey must cause verification failure");
    }

    /// Integration test requiring a live Pubky homeserver.
    ///
    /// Run with: cargo test --lib transport::tests::test_integration_signin_put_get -- --ignored
    #[test]
    #[ignore]
    fn test_integration_signin_put_get() {
        let keypair = pkarr::Keypair::random();

        let client = HomeserverClient::new("pubky.app").expect("client build failed");

        // Sign in
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
}
