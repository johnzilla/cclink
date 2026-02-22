/// Crypto module: Ed25519-to-X25519 key derivation and age encryption/decryption.
///
/// All functions convert between pkarr's Ed25519 keypairs and the X25519 keys
/// needed by the age encryption format. Key boundaries are always raw [u8; 32]
/// bytes to avoid type conflicts between curve25519-dalek 4 (age) and
/// curve25519-dalek 5 (pkarr).

use bech32::{ToBase32, Variant};
use std::io::Write;

/// Derive the X25519 secret scalar from an Ed25519 keypair.
///
/// Uses SHA-512(seed)[0..32] via ed25519-dalek's `to_scalar_bytes()`.
/// The result is compatible with X25519 ECDH as the static secret scalar.
pub fn ed25519_to_x25519_secret(keypair: &pkarr::Keypair) -> [u8; 32] {
    // keypair.secret_key() returns [u8; 32] (raw Ed25519 seed bytes)
    // Reconstruct SigningKey and call to_scalar_bytes() = SHA-512(seed)[0..32]
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&keypair.secret_key());
    signing_key.to_scalar_bytes()
}

/// Derive the X25519 public Montgomery point from an Ed25519 keypair.
///
/// Uses `VerifyingKey::to_montgomery()` from curve25519-dalek 5 (pkarr's version).
/// Returns raw bytes — never mix these types with age's curve25519-dalek 4 types.
pub fn ed25519_to_x25519_public(keypair: &pkarr::Keypair) -> [u8; 32] {
    keypair.public_key().verifying_key().to_montgomery().to_bytes()
}

/// Construct an age X25519 Identity from derived secret scalar bytes.
///
/// Bech32-encodes the scalar with the "age-secret-key-" HRP as required by
/// the age x25519 format, then parses the string into an Identity.
pub fn age_identity(x25519_secret: &[u8; 32]) -> age::x25519::Identity {
    let encoded = bech32::encode("age-secret-key-", x25519_secret.to_base32(), Variant::Bech32)
        .expect("bech32 encode is infallible for fixed-length input");
    // age parses the identity case-insensitively; uppercase is the canonical form
    encoded
        .to_ascii_uppercase()
        .parse()
        .expect("valid age identity string from correctly encoded X25519 scalar")
}

/// Construct an age X25519 Recipient from derived public key bytes.
///
/// Bech32-encodes the Montgomery point with the "age" HRP as required by
/// the age x25519 recipient format.
pub fn age_recipient(x25519_pubkey: &[u8; 32]) -> age::x25519::Recipient {
    let encoded = bech32::encode("age", x25519_pubkey.to_base32(), Variant::Bech32)
        .expect("bech32 encode is infallible for fixed-length input");
    encoded
        .parse()
        .expect("valid age recipient string from correctly encoded X25519 Montgomery point")
}

/// Convert a z32-encoded Ed25519 public key string to an age X25519 Recipient.
///
/// Uses the same conversion path as `ed25519_to_x25519_public()` but starts from
/// a parsed `pkarr::PublicKey` instead of a full Keypair. This is used for
/// `--share <pubkey>` encryption to encrypt for a specific recipient.
pub fn recipient_from_z32(z32: &str) -> anyhow::Result<age::x25519::Recipient> {
    let pubkey = pkarr::PublicKey::try_from(z32)
        .map_err(|e| anyhow::anyhow!("invalid recipient pubkey '{}': {}", z32, e))?;
    let x25519_bytes: [u8; 32] = pubkey.verifying_key().to_montgomery().to_bytes();
    Ok(age_recipient(&x25519_bytes))
}

/// Encrypt plaintext with an age X25519 Recipient.
///
/// Returns the full age ciphertext including the age header (which contains
/// the ephemeral public key). The complete blob must be stored and passed
/// intact to `age_decrypt`. Do not strip or truncate the header.
pub fn age_encrypt(plaintext: &[u8], recipient: &age::x25519::Recipient) -> anyhow::Result<Vec<u8>> {
    let encryptor = age::Encryptor::with_recipients(std::iter::once(recipient as &dyn age::Recipient))
        .expect("non-empty recipients list");
    let mut ciphertext = vec![];
    let mut writer = encryptor.wrap_output(&mut ciphertext)?;
    writer.write_all(plaintext)?;
    writer.finish()?;
    Ok(ciphertext)
}

/// Decrypt age ciphertext with an age X25519 Identity.
///
/// Expects the full age ciphertext blob (including the age header).
/// Returns an error if the identity does not match or the ciphertext is malformed.
pub fn age_decrypt(ciphertext: &[u8], identity: &age::x25519::Identity) -> anyhow::Result<Vec<u8>> {
    let decryptor = age::Decryptor::new(ciphertext)
        .map_err(|e| anyhow::anyhow!("age decryptor error: {}", e))?;
    let mut reader = decryptor
        .decrypt(std::iter::once(identity as &dyn age::Identity))
        .map_err(|e| anyhow::anyhow!("age decrypt error: {}", e))?;
    let mut plaintext = vec![];
    std::io::Read::read_to_end(&mut reader, &mut plaintext)?;
    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_keypair() -> pkarr::Keypair {
        // Use a fixed 32-byte seed for deterministic tests
        let seed = [42u8; 32];
        pkarr::Keypair::from_secret_key(&seed)
    }

    #[test]
    fn test_ed25519_to_x25519_secret_deterministic() {
        let keypair = fixed_keypair();
        let scalar1 = ed25519_to_x25519_secret(&keypair);
        let scalar2 = ed25519_to_x25519_secret(&keypair);
        assert_eq!(scalar1, scalar2, "same keypair must produce same X25519 scalar");
        assert_eq!(scalar1.len(), 32, "scalar must be 32 bytes");
        // Must not be all zeros (would be a degenerate key)
        assert_ne!(scalar1, [0u8; 32], "scalar must not be all zeros");
    }

    #[test]
    fn test_ed25519_to_x25519_public_deterministic() {
        let keypair = fixed_keypair();
        let point1 = ed25519_to_x25519_public(&keypair);
        let point2 = ed25519_to_x25519_public(&keypair);
        assert_eq!(point1, point2, "same keypair must produce same Montgomery point");
        assert_eq!(point1.len(), 32, "Montgomery point must be 32 bytes");
        assert_ne!(point1, [0u8; 32], "Montgomery point must not be all zeros");
    }

    #[test]
    fn test_age_encrypt_decrypt_round_trip() {
        let keypair = fixed_keypair();
        let secret = ed25519_to_x25519_secret(&keypair);
        let pubkey = ed25519_to_x25519_public(&keypair);
        let identity = age_identity(&secret);
        let recipient = age_recipient(&pubkey);

        let plaintext = b"session-abc123";
        let ciphertext = age_encrypt(plaintext, &recipient).expect("encrypt should succeed");
        let decrypted = age_decrypt(&ciphertext, &identity).expect("decrypt should succeed");

        assert_eq!(decrypted, plaintext, "decrypted plaintext must match original");
    }

    #[test]
    fn test_age_encrypt_produces_different_ciphertext() {
        let keypair = fixed_keypair();
        let pubkey = ed25519_to_x25519_public(&keypair);
        let recipient = age_recipient(&pubkey);

        let plaintext = b"session-abc123";
        let ct1 = age_encrypt(plaintext, &recipient).expect("first encrypt should succeed");
        let ct2 = age_encrypt(plaintext, &recipient).expect("second encrypt should succeed");

        assert_ne!(ct1, ct2, "two encryptions must produce different ciphertext (ephemeral keys)");
    }

    #[test]
    fn test_age_decrypt_wrong_key_fails() {
        let keypair_a = fixed_keypair();
        let keypair_b = pkarr::Keypair::from_secret_key(&[99u8; 32]);

        let pubkey_a = ed25519_to_x25519_public(&keypair_a);
        let secret_b = ed25519_to_x25519_secret(&keypair_b);

        let recipient_a = age_recipient(&pubkey_a);
        let identity_b = age_identity(&secret_b);

        let plaintext = b"secret session";
        let ciphertext = age_encrypt(plaintext, &recipient_a).expect("encrypt should succeed");

        // Decrypting with wrong key must fail
        let result = age_decrypt(&ciphertext, &identity_b);
        assert!(result.is_err(), "decryption with wrong key must return an error");
    }

    #[test]
    fn test_recipient_from_z32_round_trip() {
        // Create a keypair, derive z32 pubkey, convert to age Recipient
        let keypair = fixed_keypair();
        let z32 = keypair.public_key().to_z32();

        // Convert z32 to recipient — should succeed
        let recipient = recipient_from_z32(&z32).expect("recipient_from_z32 should succeed with valid z32");

        // Encrypt to the derived recipient
        let plaintext = b"round-trip test";
        let ciphertext = age_encrypt(plaintext, &recipient).expect("encrypt should succeed");

        // Decrypt with the keypair's identity — should succeed, proving the recipient is correct
        let secret = ed25519_to_x25519_secret(&keypair);
        let identity = age_identity(&secret);
        let decrypted = age_decrypt(&ciphertext, &identity).expect("decrypt should succeed");

        assert_eq!(decrypted, plaintext, "decrypted plaintext must match original");
    }

    #[test]
    fn test_recipient_from_z32_invalid_key() {
        let result = recipient_from_z32("not-a-valid-z32-key");
        assert!(result.is_err(), "recipient_from_z32 must return Err for invalid z32 key");

        let err_str = result.unwrap_err().to_string();
        assert!(
            err_str.contains("invalid recipient pubkey"),
            "error should mention invalid recipient pubkey, got: {}",
            err_str
        );
    }
}
