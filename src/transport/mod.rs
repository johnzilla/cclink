//! Transport module: PKARR DHT client for publishing/retrieving handoff records.
//!
//! Publishes HandoffRecord JSON as a DNS TXT record named `_cclink` inside a
//! PKARR SignedPacket on the Mainline DHT. No homeserver, no accounts, no signup
//! tokens — the DHT publish is authenticated by the Ed25519 signature in the
//! SignedPacket itself.

use crate::record::HandoffRecord;

/// DNS TXT record name for cclink handoff records inside a PKARR SignedPacket.
const CCLINK_LABEL: &str = "_cclink";

/// DNS TTL for the TXT record (seconds). This is the DNS-level TTL inside the
/// SignedPacket, not the application-level HandoffRecord TTL.
const DNS_TTL: u32 = 86400;

// ── DhtClient ────────────────────────────────────────────────────────────

/// Client for the PKARR Mainline DHT.
///
/// Uses `pkarr::ClientBlocking` which handles its own async runtime internally.
pub struct DhtClient {
    client: pkarr::ClientBlocking,
}

impl DhtClient {
    /// Create a new DhtClient.
    pub fn new() -> anyhow::Result<Self> {
        let client = pkarr::Client::builder()
            .no_relays()
            .build()
            .map_err(|e| anyhow::anyhow!("failed to create pkarr client: {}", e))?
            .as_blocking();

        Ok(Self { client })
    }

    /// Publish a HandoffRecord to the DHT.
    ///
    /// Serializes the record to JSON, stores it as a DNS TXT record named `_cclink`
    /// inside a SignedPacket, and publishes to the Mainline DHT.
    pub fn publish(&self, keypair: &pkarr::Keypair, record: &HandoffRecord) -> anyhow::Result<()> {
        let json = serde_json::to_string(record)
            .map_err(|e| anyhow::anyhow!("failed to serialize record: {}", e))?;

        let txt = pkarr::dns::rdata::TXT::try_from(json.as_str())
            .map_err(|e| anyhow::anyhow!("failed to create TXT record: {}", e))?;

        let cas = self.current_timestamp(keypair);

        let signed_packet = pkarr::SignedPacket::builder()
            .txt(
                CCLINK_LABEL
                    .try_into()
                    .map_err(|e| anyhow::anyhow!("invalid label: {}", e))?,
                txt,
                DNS_TTL,
            )
            .sign(keypair)
            .map_err(|e| anyhow::anyhow!("failed to sign packet: {}", e))?;

        self.client
            .publish(&signed_packet, cas)
            .map_err(|e| anyhow::anyhow!("DHT publish failed: {}", e))?;

        Ok(())
    }

    /// Resolve a HandoffRecord from the DHT by public key.
    ///
    /// Looks up the SignedPacket for the given z32 public key, extracts the `_cclink`
    /// TXT record, deserializes the JSON, and verifies the inner Ed25519 signature.
    pub fn resolve_record(&self, pubkey_z32: &str) -> anyhow::Result<HandoffRecord> {
        let pubkey = pkarr::PublicKey::try_from(pubkey_z32)
            .map_err(|e| anyhow::anyhow!("invalid public key: {}", e))?;

        let packet = self
            .client
            .resolve(&pubkey)
            .ok_or(crate::error::CclinkError::RecordNotFound)?;

        let json = Self::extract_txt(&packet)?;
        let record: HandoffRecord = serde_json::from_str(&json)
            .map_err(|e| anyhow::anyhow!("failed to deserialize record: {}", e))?;

        crate::record::verify_record(&record, &pubkey)?;

        Ok(record)
    }

    /// Revoke the active handoff by publishing an empty SignedPacket.
    ///
    /// Only the key owner can revoke (same Ed25519 key signs the packet).
    pub fn revoke(&self, keypair: &pkarr::Keypair) -> anyhow::Result<()> {
        let cas = self.current_timestamp(keypair);

        let empty_packet = pkarr::SignedPacket::builder()
            .sign(keypair)
            .map_err(|e| anyhow::anyhow!("failed to sign empty packet: {}", e))?;

        self.client
            .publish(&empty_packet, cas)
            .map_err(|e| anyhow::anyhow!("DHT revoke failed: {}", e))?;

        Ok(())
    }

    // ── Private helpers ──────────────────────────────────────────────────

    /// Get the current packet's timestamp for CAS (compare-and-swap).
    ///
    /// Returns `Some(timestamp)` if there's an existing packet, `None` otherwise.
    /// Used to prevent stale overwrites on the DHT.
    fn current_timestamp(&self, keypair: &pkarr::Keypair) -> Option<pkarr::Timestamp> {
        self.client
            .resolve_most_recent(&keypair.public_key())
            .map(|p| p.timestamp())
    }

    /// Extract the `_cclink` TXT record from a SignedPacket and reassemble its value.
    fn extract_txt(packet: &pkarr::SignedPacket) -> anyhow::Result<String> {
        use pkarr::dns::rdata::RData;

        let rr = packet
            .resource_records(CCLINK_LABEL)
            .next()
            .ok_or(crate::error::CclinkError::RecordNotFound)?;

        match &rr.rdata {
            RData::TXT(txt) => {
                let json = String::try_from(txt.clone())
                    .map_err(|e| anyhow::anyhow!("failed to reassemble TXT: {}", e))?;
                Ok(json)
            }
            other => anyhow::bail!("expected TXT record, got {:?}", other),
        }
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

    fn sample_record(keypair: &pkarr::Keypair) -> HandoffRecord {
        let signable = HandoffRecordSignable {
            blob: "dGVzdA==".to_string(),
            burn: false,
            created_at: 1_700_000_000,
            hostname: "testhost".to_string(),
            pin_salt: None,
            project: "/test".to_string(),
            pubkey: keypair.public_key().to_z32(),
            recipient: None,
            ttl: 3600,
        };
        let signature = sign_record(&signable, keypair).expect("sign_record failed");
        HandoffRecord {
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
        }
    }

    #[test]
    fn test_dht_client_new() {
        let _keypair = fixed_keypair();
        let client = DhtClient::new();
        assert!(
            client.is_ok(),
            "DhtClient::new should succeed: {:?}",
            client.err()
        );
    }

    #[test]
    fn test_build_signed_packet_fits_budget() {
        let keypair = fixed_keypair();
        let record = sample_record(&keypair);
        let json = serde_json::to_string(&record).expect("serialize");

        let txt = pkarr::dns::rdata::TXT::try_from(json.as_str()).expect("TXT::try_from");

        let signed_packet = pkarr::SignedPacket::builder()
            .txt(CCLINK_LABEL.try_into().expect("label"), txt, DNS_TTL)
            .sign(&keypair)
            .expect("sign");

        // SignedPacket max payload is 1000 bytes
        let encoded = signed_packet.encoded_packet();
        assert!(
            encoded.len() <= 1000,
            "SignedPacket DNS payload must fit in 1000 bytes, got {}",
            encoded.len()
        );
    }

    #[test]
    fn test_extract_record_roundtrip() {
        let keypair = fixed_keypair();
        let record = sample_record(&keypair);
        let json = serde_json::to_string(&record).expect("serialize");

        let txt = pkarr::dns::rdata::TXT::try_from(json.as_str()).expect("TXT::try_from");

        let signed_packet = pkarr::SignedPacket::builder()
            .txt(CCLINK_LABEL.try_into().expect("label"), txt, DNS_TTL)
            .sign(&keypair)
            .expect("sign");

        let extracted = DhtClient::extract_txt(&signed_packet).expect("extract_txt");
        let round_tripped: HandoffRecord = serde_json::from_str(&extracted).expect("deserialize");

        assert_eq!(round_tripped.created_at, record.created_at);
        assert_eq!(round_tripped.hostname, record.hostname);
        assert_eq!(round_tripped.project, record.project);
        assert_eq!(round_tripped.pubkey, record.pubkey);

        // Verify signature still valid
        crate::record::verify_record(&round_tripped, &keypair.public_key())
            .expect("signature should verify after roundtrip");
    }

    #[test]
    fn test_revoke_produces_empty_packet() {
        let keypair = fixed_keypair();

        let empty_packet = pkarr::SignedPacket::builder()
            .sign(&keypair)
            .expect("sign empty packet");

        // No resource records should be present
        let records: Vec<_> = empty_packet.resource_records(CCLINK_LABEL).collect();
        assert!(
            records.is_empty(),
            "revoked (empty) packet should have no _cclink records"
        );
    }

    #[test]
    fn test_extract_txt_fails_on_empty_packet() {
        let keypair = fixed_keypair();

        let empty_packet = pkarr::SignedPacket::builder()
            .sign(&keypair)
            .expect("sign empty packet");

        let result = DhtClient::extract_txt(&empty_packet);
        assert!(result.is_err(), "extract_txt should fail on empty packet");
    }

    /// Integration test requiring DHT connectivity.
    ///
    /// Run with: cargo test --lib transport::tests::test_integration_dht_publish_resolve -- --ignored
    #[test]
    #[ignore]
    fn test_integration_dht_publish_resolve() {
        let keypair = pkarr::Keypair::random();
        let pubkey_z32 = keypair.public_key().to_z32();
        let client = DhtClient::new().expect("client build failed");

        let record = sample_record(&keypair);
        client
            .publish(&keypair, &record)
            .expect("publish should succeed");

        // DHT propagation may take a moment
        std::thread::sleep(std::time::Duration::from_secs(2));

        let resolved = client
            .resolve_record(&pubkey_z32)
            .expect("resolve should succeed");
        assert_eq!(resolved.created_at, record.created_at);
        assert_eq!(resolved.hostname, record.hostname);
    }
}
