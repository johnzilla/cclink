//! Transport module: Pubky SDK client for homeserver operations.
//!
//! Uses the official `pubky` crate for all homeserver communication. The SDK handles
//! PKARR-based homeserver discovery, authentication, session management, and CRUD
//! operations internally. All public reads go through `pubky.public_storage()` (no auth
//! needed). Writes and deletes require an authenticated session via `signer.signin()`
//! or `signer.signup()`.

use std::cell::RefCell;

use crate::record::{HandoffRecord, LatestPointer};

// ── HomeserverClient ───────────────────────────────────────────────────────

/// Client for the Pubky homeserver, backed by the official Pubky SDK.
///
/// Uses `tokio::runtime::Runtime` to bridge sync callers to the SDK's async API.
/// The `session` field holds the authenticated session obtained via `signin()`.
/// Uses `RefCell` for interior mutability so `&self` methods can update session state.
pub struct HomeserverClient {
    rt: tokio::runtime::Runtime,
    pubky: pubky::Pubky,
    /// z32-encoded public key of the keypair that owns this client instance.
    pubkey_z32: String,
    /// z32-encoded public key of the homeserver (used for signup fallback).
    homeserver_pk: String,
    /// Authenticated session, set by `signin()`. Uses RefCell so `&self` methods
    /// can take/replace the session across `block_on()` boundaries.
    session: RefCell<Option<pubky::PubkySession>>,
}

impl HomeserverClient {
    /// Create a new HomeserverClient.
    ///
    /// `homeserver_pk` is the z32-encoded public key of the homeserver.
    /// `pubkey_z32` is the z32-encoded public key of the keypair that owns this client.
    pub fn new(homeserver_pk: &str, pubkey_z32: &str) -> anyhow::Result<Self> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| anyhow::anyhow!("failed to create tokio runtime: {}", e))?;
        let pubky = pubky::Pubky::new()
            .map_err(|e| anyhow::anyhow!("failed to create Pubky client: {}", e))?;

        Ok(Self {
            rt,
            pubky,
            pubkey_z32: pubkey_z32.to_string(),
            homeserver_pk: homeserver_pk.to_string(),
            session: RefCell::new(None),
        })
    }

    /// Sign in to the homeserver with a pkarr Keypair.
    ///
    /// Tries `signer.signin()` first (existing account). If that fails, falls back
    /// to `signer.signup(&homeserver_pk, None)` for first-time users. Sets the
    /// session on success so that subsequent write/delete operations are authenticated.
    ///
    /// No-op if a session is already established.
    pub fn signin(&self, keypair: &pkarr::Keypair) -> anyhow::Result<()> {
        if self.session.borrow().is_some() {
            return Ok(());
        }

        // Convert pkarr::Keypair (v5) to pubky::Keypair (pubky-common) via raw secret bytes
        let pubky_keypair = pubky::Keypair::from_secret(&keypair.secret_key());
        let signer = self.pubky.signer(pubky_keypair);

        let session = self.rt.block_on(async {
            // Try signin first (existing account with PKDNS record)
            match signer.signin().await {
                Ok(session) => Ok(session),
                Err(_signin_err) => {
                    // Signin failed — try signup (first-time user)
                    let homeserver_pk = pubky::PublicKey::try_from_z32(&self.homeserver_pk)
                        .map_err(|e| anyhow::anyhow!("invalid homeserver public key: {}", e))?;
                    signer.signup(&homeserver_pk, None).await
                        .map_err(|e| anyhow::anyhow!("signup failed: {}", e))
                }
            }
        })?;

        *self.session.borrow_mut() = Some(session);
        Ok(())
    }

    /// Ensure the client is signed in, calling `signin()` at most once per lifetime.
    fn ensure_signed_in(&self, keypair: &pkarr::Keypair) -> anyhow::Result<()> {
        if self.session.borrow().is_none() {
            self.signin(keypair)?;
        }
        Ok(())
    }

    /// PUT a serialized HandoffRecord at `/pub/cclink/{token}`.
    ///
    /// Requires a prior successful `signin()` call.
    pub fn put_record(&self, token: &str, record_bytes: &[u8]) -> anyhow::Result<()> {
        let path = format!("/pub/cclink/{}", token);
        let data = record_bytes.to_vec();

        let session = self.session.borrow_mut().take()
            .ok_or_else(|| anyhow::anyhow!("not signed in — call signin() first"))?;

        let result = self.rt.block_on(async {
            session.storage().put(&path, data).await
        });

        *self.session.borrow_mut() = Some(session);
        result.map_err(|e| anyhow::anyhow!("PUT record failed: {}", e))?;
        Ok(())
    }

    /// PUT the latest.json pointer at `/pub/cclink/latest`.
    ///
    /// Requires a prior successful `signin()` call.
    pub fn put_latest(&self, latest_bytes: &[u8]) -> anyhow::Result<()> {
        let path = "/pub/cclink/latest";
        let data = latest_bytes.to_vec();

        let session = self.session.borrow_mut().take()
            .ok_or_else(|| anyhow::anyhow!("not signed in — call signin() first"))?;

        let result = self.rt.block_on(async {
            session.storage().put(path, data).await
        });

        *self.session.borrow_mut() = Some(session);
        result.map_err(|e| anyhow::anyhow!("PUT latest failed: {}", e))?;
        Ok(())
    }

    /// GET and verify a HandoffRecord at `/pub/cclink/{token}`.
    ///
    /// Reads from the client's own namespace. The record is deserialized from JSON
    /// and its Ed25519 signature is verified against `pubkey` before being returned.
    pub fn get_record(
        &self,
        token: &str,
        pubkey: &pkarr::PublicKey,
    ) -> anyhow::Result<HandoffRecord> {
        let addr = format!("{}/pub/cclink/{}", self.pubkey_z32, token);
        let bytes = self.get_public_bytes(&addr)?;
        self.deserialize_and_verify(&bytes, pubkey)
    }

    /// GET and verify a HandoffRecord from another user's namespace.
    ///
    /// Uses `pubkey_z32` to address the target user's public storage.
    pub fn get_record_by_pubkey(
        &self,
        pubkey_z32: &str,
        token: &str,
        pubkey: &pkarr::PublicKey,
    ) -> anyhow::Result<HandoffRecord> {
        let addr = format!("{}/pub/cclink/{}", pubkey_z32, token);
        let bytes = self.get_public_bytes(&addr)?;
        self.deserialize_and_verify(&bytes, pubkey)
    }

    /// GET the latest.json pointer bytes.
    ///
    /// If `pubkey_z32` is `None`, reads own records. If `Some`, reads another
    /// user's records.
    pub fn get_latest(&self, pubkey_z32: Option<&str>) -> anyhow::Result<Vec<u8>> {
        let pk = pubkey_z32.unwrap_or(&self.pubkey_z32);
        let addr = format!("{}/pub/cclink/latest", pk);
        self.get_public_bytes(&addr)
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
        let token = record.created_at.to_string();

        let record_bytes = serde_json::to_vec(record)
            .map_err(|e| anyhow::anyhow!("failed to serialize record: {}", e))?;

        self.ensure_signed_in(keypair)?;

        self.put_record(&token, &record_bytes)?;

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
    /// Treats not-found as success (idempotent — record already deleted).
    /// Must be called AFTER `signin()`.
    pub fn delete_record(&self, token: &str) -> anyhow::Result<()> {
        let path = format!("/pub/cclink/{}", token);

        let session = self.session.borrow_mut().take()
            .ok_or_else(|| anyhow::anyhow!("not signed in — call signin() first"))?;

        let result = self.rt.block_on(async {
            session.storage().delete(&path).await
        });

        *self.session.borrow_mut() = Some(session);

        match result {
            Ok(_) => Ok(()),
            Err(e) => {
                let err_str = e.to_string();
                // Treat not-found as success (idempotent)
                if err_str.contains("404") || err_str.to_lowercase().contains("not found") {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!("DELETE failed: {}", e))
                }
            }
        }
    }

    /// List all record tokens under `/pub/cclink/`.
    ///
    /// Returns only numeric timestamp tokens — filters out "latest" and any non-numeric keys.
    /// Returns an empty vec if no records are published.
    pub fn list_record_tokens(&self) -> anyhow::Result<Vec<String>> {
        let addr = format!("{}/pub/cclink/", self.pubkey_z32);

        let entries = self.rt.block_on(async {
            match self.pubky.public_storage().list(&addr) {
                Ok(builder) => builder.send().await,
                Err(e) => Err(e),
            }
        });

        match entries {
            Ok(resources) => {
                let tokens: Vec<String> = resources
                    .iter()
                    .filter_map(|entry| {
                        let path = entry.path.as_str();
                        // Extract the last path segment after /pub/cclink/
                        path.split("/pub/cclink/")
                            .nth(1)
                            .map(|t| t.trim_end_matches('/').to_string())
                    })
                    .filter(|t| t.parse::<u64>().is_ok())
                    .collect();
                Ok(tokens)
            }
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("404") || err_str.to_lowercase().contains("not found") {
                    Ok(vec![])
                } else {
                    Err(anyhow::anyhow!("LIST failed: {}", e))
                }
            }
        }
    }

    /// Fetch all HandoffRecords for a given public key in one transport call.
    ///
    /// Calls `list_record_tokens()` to get all token strings, then fetches each
    /// record individually via `get_record()`. Records that fail to fetch or verify
    /// are silently skipped.
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
                Err(_) => continue,
            }
        }
        Ok(results)
    }

    // ── Private helpers ────────────────────────────────────────────────────

    /// Perform a public GET, returning response body bytes.
    ///
    /// Uses `pubky.public_storage().get()` — no authentication needed.
    fn get_public_bytes(&self, addr: &str) -> anyhow::Result<Vec<u8>> {
        let addr = addr.to_string();
        self.rt.block_on(async {
            let response = self.pubky.public_storage().get(&addr).await
                .map_err(|e| {
                    let err_str = e.to_string();
                    if err_str.contains("404") || err_str.to_lowercase().contains("not found") {
                        crate::error::CclinkError::RecordNotFound.into()
                    } else {
                        anyhow::anyhow!("GET failed: {}", e)
                    }
                })?;
            let bytes = response.bytes().await
                .map_err(|e| anyhow::anyhow!("failed to read response bytes: {}", e))?;
            Ok(bytes.to_vec())
        })
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
    use std::time::SystemTime;

    fn fixed_keypair() -> pkarr::Keypair {
        pkarr::Keypair::from_secret_key(&[42u8; 32])
    }

    // ── HomeserverClient tests ─────────────────────────────────────────────

    #[test]
    fn test_homeserver_client_new() {
        let keypair = fixed_keypair();
        let pubkey_z32 = keypair.public_key().to_z32();
        let client = HomeserverClient::new(
            "8um71us3fyw6h8wbcxb5ar3rwusy1a6u49956ikzojg3gcwd1dty",
            &pubkey_z32,
        );
        assert!(
            client.is_ok(),
            "HomeserverClient::new should succeed with valid homeserver pk"
        );
    }

    #[test]
    fn test_homeserver_client_stores_pubkey_z32() {
        let keypair = fixed_keypair();
        let pubkey_z32 = keypair.public_key().to_z32();
        let client = HomeserverClient::new(
            "8um71us3fyw6h8wbcxb5ar3rwusy1a6u49956ikzojg3gcwd1dty",
            &pubkey_z32,
        )
        .expect("should succeed");
        assert_eq!(
            client.pubkey_z32, pubkey_z32,
            "pubkey_z32 should be stored in the client"
        );
    }

    #[test]
    fn test_session_starts_as_none() {
        let keypair = fixed_keypair();
        let pubkey_z32 = keypair.public_key().to_z32();
        let client = HomeserverClient::new(
            "8um71us3fyw6h8wbcxb5ar3rwusy1a6u49956ikzojg3gcwd1dty",
            &pubkey_z32,
        )
        .expect("client build failed");
        assert!(
            client.session.borrow().is_none(),
            "session should be None on new client"
        );
    }

    #[test]
    fn test_publish_token_is_created_at_timestamp() {
        let keypair = fixed_keypair();
        let created_at: u64 = 1_700_000_000;

        let expected_token = created_at.to_string();
        assert_eq!(expected_token, "1700000000");

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

        let token = record.created_at.to_string();
        assert_eq!(token, "1700000000", "token must be created_at as string");
    }

    #[test]
    fn test_get_record_deserialization_and_verification_pipeline() {
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

        let json_bytes = serde_json::to_vec(&record).expect("serialize failed");

        let client = HomeserverClient::new(
            "8um71us3fyw6h8wbcxb5ar3rwusy1a6u49956ikzojg3gcwd1dty",
            &pubkey_z32,
        )
        .expect("client build failed");
        let result = client.deserialize_and_verify(&json_bytes, &keypair.public_key());
        assert!(
            result.is_ok(),
            "valid signed record should deserialize and verify: {:?}",
            result.err()
        );

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
        let client = HomeserverClient::new(
            "8um71us3fyw6h8wbcxb5ar3rwusy1a6u49956ikzojg3gcwd1dty",
            &pubkey_a_z32,
        )
        .expect("client build failed");

        let result = client.deserialize_and_verify(&json_bytes, &keypair_b.public_key());
        assert!(
            result.is_err(),
            "wrong pubkey must cause verification failure"
        );
    }

    /// Integration test requiring a live Pubky homeserver.
    ///
    /// Run with: cargo test --lib transport::tests::test_integration_signin_put_get -- --ignored
    #[test]
    #[ignore]
    fn test_integration_signin_put_get() {
        let keypair = pkarr::Keypair::random();
        let pubkey_z32 = keypair.public_key().to_z32();

        let client = HomeserverClient::new(
            "8um71us3fyw6h8wbcxb5ar3rwusy1a6u49956ikzojg3gcwd1dty",
            &pubkey_z32,
        )
        .expect("client build failed");

        client
            .signin(&keypair)
            .expect("signin should succeed");

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

        let token = client
            .publish(&keypair, &record)
            .expect("publish should succeed");
        assert!(!token.is_empty(), "token should not be empty");

        let retrieved = client
            .get_record(&token, &keypair.public_key())
            .expect("get_record should succeed");
        assert_eq!(retrieved.created_at, record.created_at);
        assert_eq!(retrieved.hostname, record.hostname);
    }

    /// Integration test: first-time user signup flow.
    ///
    /// Run with: cargo test --lib transport::tests::test_integration_signup_new_keypair -- --ignored
    #[test]
    #[ignore]
    fn test_integration_signup_new_keypair() {
        let keypair = pkarr::Keypair::random();
        let pubkey_z32 = keypair.public_key().to_z32();

        let client = HomeserverClient::new(
            "8um71us3fyw6h8wbcxb5ar3rwusy1a6u49956ikzojg3gcwd1dty",
            &pubkey_z32,
        )
        .expect("client build failed");

        client
            .signin(&keypair)
            .expect("signin (with signup fallback) should succeed for new keypair");
        assert!(
            client.session.borrow().is_some(),
            "session should be Some after signup"
        );

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

        let token = client
            .publish(&keypair, &record)
            .expect("publish should succeed after signup");
        assert!(!token.is_empty(), "token should not be empty");

        let retrieved = client
            .get_record(&token, &keypair.public_key())
            .expect("get_record should succeed");
        assert_eq!(retrieved.created_at, record.created_at);
        assert_eq!(retrieved.project, record.project);
    }

    /// Integration test: cross-user GET via public storage.
    ///
    /// Run with: cargo test --lib transport::tests::test_integration_cross_user_get -- --ignored
    #[test]
    #[ignore]
    fn test_integration_cross_user_get() {
        let keypair_a = pkarr::Keypair::random();
        let keypair_b = pkarr::Keypair::random();
        let pubkey_a_z32 = keypair_a.public_key().to_z32();
        let pubkey_b_z32 = keypair_b.public_key().to_z32();

        let client_a = HomeserverClient::new(
            "8um71us3fyw6h8wbcxb5ar3rwusy1a6u49956ikzojg3gcwd1dty",
            &pubkey_a_z32,
        )
        .expect("client_a build failed");
        client_a
            .signin(&keypair_a)
            .expect("user_a signin should succeed");

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
        let token = client_a
            .publish(&keypair_a, &record_a)
            .expect("user_a publish should succeed");

        let client_b = HomeserverClient::new(
            "8um71us3fyw6h8wbcxb5ar3rwusy1a6u49956ikzojg3gcwd1dty",
            &pubkey_b_z32,
        )
        .expect("client_b build failed");
        let retrieved = client_b
            .get_record_by_pubkey(&pubkey_a_z32, &token, &keypair_a.public_key())
            .expect("cross-user get_record_by_pubkey should succeed");

        assert_eq!(retrieved.created_at, record_a.created_at);
        assert_eq!(retrieved.pubkey, pubkey_a_z32);
        assert_eq!(retrieved.project, record_a.project);
    }
}
