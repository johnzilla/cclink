/// Record module: HandoffRecord struct with canonical JSON serialization and Ed25519 signing.
///
/// HandoffRecord stores all metadata for a session handoff: hostname, project path,
/// timestamp, TTL, encrypted session blob, and creator pubkey. Signing is performed
/// over a canonical (deterministic, alphabetically-sorted, compact) JSON representation
/// of the signable fields, excluding the signature itself.

use base64::Engine;
use serde::{Deserialize, Serialize};

/// A complete handoff record including the Ed25519 signature.
///
/// Fields are in alphabetical order. This is critical: serde serializes struct fields
/// in declaration order, so alphabetical order ensures deterministic JSON output
/// without enabling the `preserve_order` serde_json feature.
///
/// As of v1.1, `burn` and `recipient` are included in the signed envelope
/// (HandoffRecordSignable), so tampering with either field causes signature
/// verification failure. v1.0 records (signed without these fields) are not
/// supported — they expire via TTL (clean break).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HandoffRecord {
    /// Base64-encoded age ciphertext containing the encrypted session payload.
    pub blob: String,
    /// Burn-after-read flag: if true, the record should be deleted after first successful pickup.
    /// Signed as part of the v1.1 envelope — tampering causes verification failure.
    #[serde(default)]
    pub burn: bool,
    /// Unix timestamp (seconds) when the record was created.
    pub created_at: u64,
    /// Hostname of the machine that created this record.
    pub hostname: String,
    /// Base64-encoded 32-byte random salt used for PIN key derivation (None when no PIN used).
    /// Signed as part of the envelope — tampering causes verification failure.
    #[serde(default)]
    pub pin_salt: Option<String>,
    /// Project path identifier.
    pub project: String,
    /// Creator's z32-encoded Ed25519 public key.
    pub pubkey: String,
    /// Optional z32-encoded public key of the intended recipient (None = self-encrypted).
    /// Signed as part of the v1.1 envelope — tampering causes verification failure.
    #[serde(default)]
    pub recipient: Option<String>,
    /// Base64-encoded Ed25519 signature over canonical JSON of the signable fields.
    pub signature: String,
    /// Record time-to-live in seconds.
    pub ttl: u64,
}

/// The signable subset of HandoffRecord fields (excludes `signature` to avoid circular dependency).
///
/// Fields are in alphabetical order — matching HandoffRecord ordering — for deterministic
/// canonical JSON serialization.
///
/// Field order (alphabetical): blob, burn, created_at, hostname, pin_salt, project, pubkey, recipient, ttl
///
/// v1.1 change: `burn` and `recipient` are now included in the signed envelope.
/// This is a clean break from v1.0 — v1.0 records (signed without burn/recipient) are
/// not supported; they expire via TTL. There is no version negotiation.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HandoffRecordSignable {
    /// Base64-encoded age ciphertext.
    pub blob: String,
    /// Burn-after-read flag: signed into the envelope so tampering is detectable.
    pub burn: bool,
    /// Unix timestamp (seconds) when the record was created.
    pub created_at: u64,
    /// Hostname of the machine that created this record.
    pub hostname: String,
    /// Base64-encoded 32-byte random salt used for PIN key derivation (None when no PIN used).
    /// Signed into the envelope so tampering with the salt is detectable.
    pub pin_salt: Option<String>,
    /// Project path identifier.
    pub project: String,
    /// Creator's z32-encoded Ed25519 public key.
    pub pubkey: String,
    /// Optional z32-encoded public key of the intended recipient: signed into the envelope.
    pub recipient: Option<String>,
    /// Record time-to-live in seconds.
    pub ttl: u64,
}

/// A pointer stored at `latest.json` that references the most recent HandoffRecord.
///
/// Contains summary metadata so consumers can quickly check freshness without
/// fetching the full record.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LatestPointer {
    /// Unix timestamp (seconds) when the record was created.
    pub created_at: u64,
    /// Hostname of the machine that created the referenced record.
    pub hostname: String,
    /// Project path identifier.
    pub project: String,
    /// Unix timestamp token matching the record path (used to locate the full record).
    pub token: String,
}

impl From<&HandoffRecord> for HandoffRecordSignable {
    /// Convert a HandoffRecord to its signable form by copying all fields except `signature`.
    /// `burn`, `pin_salt`, and `recipient` are included — they are signed into the v1.1 envelope.
    fn from(record: &HandoffRecord) -> Self {
        HandoffRecordSignable {
            blob: record.blob.clone(),
            burn: record.burn,
            created_at: record.created_at,
            hostname: record.hostname.clone(),
            pin_salt: record.pin_salt.clone(),
            project: record.project.clone(),
            pubkey: record.pubkey.clone(),
            recipient: record.recipient.clone(),
            ttl: record.ttl,
        }
    }
}

/// Produce canonical JSON for signing: compact (no whitespace), fields in alphabetical order.
///
/// Because HandoffRecordSignable fields are declared in alphabetical order and serde_json
/// serializes struct fields in declaration order, this produces deterministic output.
/// Do NOT enable the `preserve_order` feature on serde_json.
pub fn canonical_json(signable: &HandoffRecordSignable) -> anyhow::Result<String> {
    Ok(serde_json::to_string(signable)?)
}

/// Sign a HandoffRecordSignable with a pkarr Keypair, returning a base64-encoded signature.
///
/// Signs the canonical JSON bytes with the Ed25519 private key. The returned string is
/// suitable for storage in HandoffRecord.signature.
pub fn sign_record(signable: &HandoffRecordSignable, keypair: &pkarr::Keypair) -> anyhow::Result<String> {
    let json = canonical_json(signable)?;
    let sig = keypair.sign(json.as_bytes());
    let encoded = base64::engine::general_purpose::STANDARD.encode(sig.to_bytes());
    Ok(encoded)
}

/// Verify the Ed25519 signature on a HandoffRecord using the given public key.
///
/// Extracts the signable fields, computes canonical JSON, decodes the base64 signature,
/// and verifies with the provided pkarr PublicKey.
///
/// Returns an error if the signature is invalid, the base64 is malformed, or the
/// signature bytes cannot be interpreted as a valid Ed25519 signature.
pub fn verify_record(record: &HandoffRecord, pubkey: &pkarr::PublicKey) -> anyhow::Result<()> {
    use crate::error::CclinkError;

    let signable = HandoffRecordSignable::from(record);
    let json = canonical_json(&signable)?;

    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(&record.signature)
        .map_err(|e| anyhow::anyhow!("invalid base64 signature: {}", e))?;

    let sig_array: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("signature must be exactly 64 bytes"))?;

    let sig = ed25519_dalek::Signature::from_bytes(&sig_array);

    pubkey
        .verify(json.as_bytes(), &sig)
        .map_err(|e| CclinkError::SignatureVerificationFailed(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_keypair() -> pkarr::Keypair {
        pkarr::Keypair::from_secret_key(&[42u8; 32])
    }

    fn sample_signable() -> HandoffRecordSignable {
        HandoffRecordSignable {
            blob: "dGVzdGJsb2I=".to_string(),
            burn: false,
            created_at: 1_700_000_000,
            hostname: "testhost".to_string(),
            pin_salt: None,
            project: "/home/user/project".to_string(),
            pubkey: "testpubkey".to_string(),
            recipient: None,
            ttl: 3600,
        }
    }

    #[test]
    fn test_handoff_record_signable_serializes_alphabetical_keys() {
        // Use a signable with recipient set so its position is testable
        let signable = HandoffRecordSignable {
            blob: "dGVzdGJsb2I=".to_string(),
            burn: false,
            created_at: 1_700_000_000,
            hostname: "testhost".to_string(),
            pin_salt: None,
            project: "/home/user/project".to_string(),
            pubkey: "testpubkey".to_string(),
            recipient: Some("recipientkey".to_string()),
            ttl: 3600,
        };
        let json = canonical_json(&signable).expect("canonical_json should succeed");

        // Find positions of each key in the JSON string
        // Expected order: blob, burn, created_at, hostname, pin_salt, project, pubkey, recipient, ttl
        let blob_pos = json.find("\"blob\"").expect("blob key missing");
        let burn_pos = json.find("\"burn\"").expect("burn key missing");
        let created_at_pos = json.find("\"created_at\"").expect("created_at key missing");
        let hostname_pos = json.find("\"hostname\"").expect("hostname key missing");
        let pin_salt_pos = json.find("\"pin_salt\"").expect("pin_salt key missing");
        let project_pos = json.find("\"project\"").expect("project key missing");
        let pubkey_pos = json.find("\"pubkey\"").expect("pubkey key missing");
        let recipient_pos = json.find("\"recipient\"").expect("recipient key missing");
        let ttl_pos = json.find("\"ttl\"").expect("ttl key missing");

        assert!(blob_pos < burn_pos, "blob must come before burn");
        assert!(burn_pos < created_at_pos, "burn must come before created_at");
        assert!(
            created_at_pos < hostname_pos,
            "created_at must come before hostname"
        );
        assert!(
            hostname_pos < pin_salt_pos,
            "hostname must come before pin_salt"
        );
        assert!(pin_salt_pos < project_pos, "pin_salt must come before project");
        assert!(project_pos < pubkey_pos, "project must come before pubkey");
        assert!(pubkey_pos < recipient_pos, "pubkey must come before recipient");
        assert!(recipient_pos < ttl_pos, "recipient must come before ttl");
    }

    #[test]
    fn test_canonical_json_is_compact_no_whitespace() {
        let signable = sample_signable();
        let json = canonical_json(&signable).expect("canonical_json should succeed");

        assert!(
            !json.contains('\n'),
            "canonical JSON must not contain newlines"
        );
        assert!(
            !json.contains("  "),
            "canonical JSON must not contain double spaces"
        );
        // Compact JSON: no space after colon or comma
        assert!(
            !json.contains(": "),
            "canonical JSON must not have space after colon"
        );
        assert!(
            !json.contains(", "),
            "canonical JSON must not have space after comma"
        );
    }

    #[test]
    fn test_canonical_json_deterministic() {
        let signable = sample_signable();
        let json1 = canonical_json(&signable).expect("first canonical_json should succeed");
        let json2 = canonical_json(&signable).expect("second canonical_json should succeed");
        assert_eq!(
            json1, json2,
            "canonical JSON must be identical for identical structs"
        );
    }

    #[test]
    fn test_sign_and_verify_round_trip() {
        let keypair = fixed_keypair();
        let signable = sample_signable();
        let signature = sign_record(&signable, &keypair).expect("sign_record should succeed");

        let record = HandoffRecord {
            blob: signable.blob.clone(),
            burn: false,
            created_at: signable.created_at,
            hostname: signable.hostname.clone(),
            pin_salt: None,
            project: signable.project.clone(),
            pubkey: signable.pubkey.clone(),
            recipient: None,
            signature,
            ttl: signable.ttl,
        };

        verify_record(&record, &keypair.public_key())
            .expect("verify_record should succeed with correct key");
    }

    #[test]
    fn test_verify_fails_wrong_pubkey() {
        let keypair_a = fixed_keypair();
        let keypair_b = pkarr::Keypair::from_secret_key(&[99u8; 32]);

        let signable = sample_signable();
        let signature = sign_record(&signable, &keypair_a).expect("sign_record should succeed");

        let record = HandoffRecord {
            blob: signable.blob.clone(),
            burn: false,
            created_at: signable.created_at,
            hostname: signable.hostname.clone(),
            pin_salt: None,
            project: signable.project.clone(),
            pubkey: signable.pubkey.clone(),
            recipient: None,
            signature,
            ttl: signable.ttl,
        };

        let result = verify_record(&record, &keypair_b.public_key());
        assert!(result.is_err(), "verify_record must fail with wrong public key");

        let err_str = result.unwrap_err().to_string();
        assert!(
            err_str.contains("Signature verification failed") || err_str.contains("verification"),
            "error should mention signature verification failure, got: {}",
            err_str
        );
    }

    #[test]
    fn test_verify_fails_tampered_json() {
        let keypair = fixed_keypair();
        let signable = sample_signable();
        let signature = sign_record(&signable, &keypair).expect("sign_record should succeed");

        // Tamper with the TTL field
        let tampered = HandoffRecord {
            blob: signable.blob.clone(),
            burn: false,
            created_at: signable.created_at,
            hostname: signable.hostname.clone(),
            pin_salt: None,
            project: signable.project.clone(),
            pubkey: signable.pubkey.clone(),
            recipient: None,
            signature,
            ttl: signable.ttl + 9999, // tampered!
        };

        let result = verify_record(&tampered, &keypair.public_key());
        assert!(result.is_err(), "verify_record must fail with tampered content");
    }

    #[test]
    fn test_latest_pointer_serialization() {
        let pointer = LatestPointer {
            created_at: 1_700_000_000,
            hostname: "testhost".to_string(),
            project: "/home/user/project".to_string(),
            token: "1700000000".to_string(),
        };

        let json = serde_json::to_string(&pointer).expect("LatestPointer should serialize");
        let deserialized: LatestPointer =
            serde_json::from_str(&json).expect("LatestPointer should deserialize");

        assert_eq!(deserialized.created_at, pointer.created_at);
        assert_eq!(deserialized.hostname, pointer.hostname);
        assert_eq!(deserialized.project, pointer.project);
        assert_eq!(deserialized.token, pointer.token);
    }

    #[test]
    fn test_signable_includes_burn_field() {
        let signable = HandoffRecordSignable {
            blob: "dGVzdGJsb2I=".to_string(),
            burn: true,
            created_at: 1_700_000_000,
            hostname: "testhost".to_string(),
            pin_salt: None,
            project: "/home/user/project".to_string(),
            pubkey: "testpubkey".to_string(),
            recipient: None,
            ttl: 3600,
        };
        let json = canonical_json(&signable).expect("canonical_json should succeed");
        assert!(
            json.contains("\"burn\":true"),
            "canonical JSON must contain burn:true, got: {}",
            json
        );
    }

    #[test]
    fn test_signable_includes_recipient_field() {
        let signable = HandoffRecordSignable {
            blob: "dGVzdGJsb2I=".to_string(),
            burn: false,
            created_at: 1_700_000_000,
            hostname: "testhost".to_string(),
            pin_salt: None,
            project: "/home/user/project".to_string(),
            pubkey: "testpubkey".to_string(),
            recipient: Some("abc123".to_string()),
            ttl: 3600,
        };
        let json = canonical_json(&signable).expect("canonical_json should succeed");
        assert!(
            json.contains("\"recipient\":\"abc123\""),
            "canonical JSON must contain recipient:\"abc123\", got: {}",
            json
        );
    }

    #[test]
    fn test_tampered_burn_fails_verification() {
        let keypair = fixed_keypair();
        // Sign with burn: false
        let signable = HandoffRecordSignable {
            blob: "dGVzdGJsb2I=".to_string(),
            burn: false,
            created_at: 1_700_000_000,
            hostname: "testhost".to_string(),
            pin_salt: None,
            project: "/home/user/project".to_string(),
            pubkey: "testpubkey".to_string(),
            recipient: None,
            ttl: 3600,
        };
        let signature = sign_record(&signable, &keypair).expect("sign_record should succeed");

        // Tamper: construct record with burn: true (different from what was signed)
        let tampered = HandoffRecord {
            blob: signable.blob.clone(),
            burn: true, // tampered!
            created_at: signable.created_at,
            hostname: signable.hostname.clone(),
            pin_salt: signable.pin_salt.clone(),
            project: signable.project.clone(),
            pubkey: signable.pubkey.clone(),
            recipient: signable.recipient.clone(),
            signature,
            ttl: signable.ttl,
        };

        let result = verify_record(&tampered, &keypair.public_key());
        assert!(
            result.is_err(),
            "verify_record must fail when burn field is tampered after signing"
        );
    }
}
