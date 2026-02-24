//! Crypto module: Ed25519-to-X25519 key derivation and age encryption/decryption.
//!
//! All functions convert between pkarr's Ed25519 keypairs and the X25519 keys
//! needed by the age encryption format. Key boundaries are always raw [u8; 32]
//! bytes to avoid type conflicts between curve25519-dalek 4 (age) and
//! curve25519-dalek 5 (pkarr).

use argon2::{Algorithm, Argon2, Params, Version};
use bech32::{ToBase32, Variant};
use hkdf::Hkdf;
use rand::Rng;
use sha2::Sha256;
use std::io::Write;
use zeroize::Zeroizing;

// ── CCLINKEK binary envelope constants ──────────────────────────────────────
// Phase 16 wires these into init/load; suppress dead_code until then.

/// Magic header bytes identifying the CCLINKEK key envelope format.
#[allow(dead_code)]
const ENVELOPE_MAGIC: &[u8; 8] = b"CCLINKEK";

/// Current version byte for the CCLINKEK envelope format.
#[allow(dead_code)]
const ENVELOPE_VERSION: u8 = 0x01;

/// Fixed header length: 8 magic + 1 version + 4 m_cost + 4 t_cost + 4 p_cost + 32 salt = 53 bytes.
#[allow(dead_code)]
const ENVELOPE_HEADER_LEN: usize = 53;

/// HKDF info string for key envelope derivation (distinct from cclink-pin-v1).
#[allow(dead_code)]
const KEY_HKDF_INFO: &[u8] = b"cclink-key-v1";

/// Default Argon2id memory cost (64 MB) — stored in envelope header on encryption.
#[allow(dead_code)]
const KDF_M_COST: u32 = 65536;

/// Default Argon2id iteration count — stored in envelope header on encryption.
#[allow(dead_code)]
const KDF_T_COST: u32 = 3;

/// Default Argon2id parallelism — stored in envelope header on encryption.
#[allow(dead_code)]
const KDF_P_COST: u32 = 1;

/// Derive the X25519 secret scalar from an Ed25519 keypair.
///
/// Uses SHA-512(seed)[0..32] via ed25519-dalek's `to_scalar_bytes()`.
/// The result is compatible with X25519 ECDH as the static secret scalar.
pub fn ed25519_to_x25519_secret(keypair: &pkarr::Keypair) -> Zeroizing<[u8; 32]> {
    // keypair.secret_key() returns [u8; 32] (raw Ed25519 seed bytes)
    // Reconstruct SigningKey and call to_scalar_bytes() = SHA-512(seed)[0..32]
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&keypair.secret_key());
    Zeroizing::new(signing_key.to_scalar_bytes())
}

/// Derive the X25519 public Montgomery point from an Ed25519 keypair.
///
/// Uses `VerifyingKey::to_montgomery()` from curve25519-dalek 5 (pkarr's version).
/// Returns raw bytes — never mix these types with age's curve25519-dalek 4 types.
pub fn ed25519_to_x25519_public(keypair: &pkarr::Keypair) -> [u8; 32] {
    keypair
        .public_key()
        .verifying_key()
        .to_montgomery()
        .to_bytes()
}

/// Construct an age X25519 Identity from derived secret scalar bytes.
///
/// Bech32-encodes the scalar with the "age-secret-key-" HRP as required by
/// the age x25519 format, then parses the string into an Identity.
pub fn age_identity(x25519_secret: &[u8; 32]) -> age::x25519::Identity {
    let encoded = bech32::encode(
        "age-secret-key-",
        x25519_secret.to_base32(),
        Variant::Bech32,
    )
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
pub fn age_encrypt(
    plaintext: &[u8],
    recipient: &age::x25519::Recipient,
) -> anyhow::Result<Vec<u8>> {
    let encryptor =
        age::Encryptor::with_recipients(std::iter::once(recipient as &dyn age::Recipient))
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

/// Derive a 32-byte key from a PIN and 32-byte salt using Argon2id + HKDF-SHA256.
///
/// Parameters: t_cost=3 (time), m_cost=65536 (64 MB memory), p_cost=1 (parallelism).
/// HKDF expand uses info="cclink-pin-v1" to domain-separate the output.
///
/// The result is deterministic: same PIN + same salt always produces the same 32-byte key.
pub fn pin_derive_key(pin: &str, salt: &[u8; 32]) -> anyhow::Result<Zeroizing<[u8; 32]>> {
    // Argon2id with explicit parameters
    let params = Params::new(65536, 3, 1, Some(32))
        .map_err(|e| anyhow::anyhow!("argon2 params error: {}", e))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    // Hash the PIN into 32 bytes using the salt; Zeroizing ensures zeroing on drop
    let mut argon2_output = Zeroizing::new([0u8; 32]);
    argon2
        .hash_password_into(pin.as_bytes(), salt, argon2_output.as_mut())
        .map_err(|e| anyhow::anyhow!("argon2 hash error: {}", e))?;

    // Expand via HKDF-SHA256 with domain-separation info; Zeroizing ensures zeroing on drop
    let hkdf = Hkdf::<Sha256>::new(None, &*argon2_output);
    let mut okm = Zeroizing::new([0u8; 32]);
    hkdf.expand(b"cclink-pin-v1", okm.as_mut())
        .map_err(|e| anyhow::anyhow!("hkdf expand error: {}", e))?;

    Ok(okm)
}

/// Encrypt plaintext using a PIN-derived X25519 key.
///
/// Generates a random 32-byte salt, derives an X25519 key from the PIN+salt via
/// Argon2id+HKDF, constructs an age Recipient from the derived key, and encrypts
/// with age. Returns (ciphertext, salt) — the salt must be stored alongside the
/// ciphertext for decryption.
pub fn pin_encrypt(plaintext: &[u8], pin: &str) -> anyhow::Result<(Vec<u8>, [u8; 32])> {
    // Generate a fresh random 32-byte salt
    let salt: [u8; 32] = rand::thread_rng().gen();

    // Derive the X25519 secret scalar from PIN+salt
    let derived_key = pin_derive_key(pin, &salt)?;

    // The derived key bytes are the X25519 secret scalar; build an age Identity from it
    let identity = age_identity(&derived_key);

    // Get the corresponding public key (Recipient)
    let recipient = identity.to_public();

    // Encrypt the plaintext to the PIN-derived Recipient
    let ciphertext = age_encrypt(plaintext, &recipient)?;

    Ok((ciphertext, salt))
}

/// Decrypt PIN-encrypted ciphertext using the original PIN and salt.
///
/// Re-derives the X25519 secret from PIN+salt, constructs an age Identity, and
/// decrypts. Returns an error if the PIN is wrong or the ciphertext is malformed —
/// never panics or returns incorrect plaintext silently.
pub fn pin_decrypt(ciphertext: &[u8], pin: &str, salt: &[u8; 32]) -> anyhow::Result<Vec<u8>> {
    // Re-derive the X25519 secret scalar using the same PIN and salt
    let derived_key = pin_derive_key(pin, salt)?;

    // Build the age Identity from the derived key
    let identity = age_identity(&derived_key);

    // Decrypt — wrong PIN produces an age decryption error, not a panic
    age_decrypt(ciphertext, &identity)
}

// ── CCLINKEK binary envelope functions ──────────────────────────────────────

/// Derive a 32-byte key-encryption key from a passphrase and 32-byte salt using Argon2id + HKDF-SHA256.
///
/// Accepts Argon2 parameters as arguments so that `decrypt_key_envelope` can pass the
/// values decoded from the envelope header (enabling forward compatibility on param upgrades).
/// HKDF info string `b"cclink-key-v1"` domain-separates this derivation from `pin_derive_key`.
#[allow(dead_code)]
fn key_derive_key(
    passphrase: &str,
    salt: &[u8; 32],
    m_cost: u32,
    t_cost: u32,
    p_cost: u32,
) -> anyhow::Result<Zeroizing<[u8; 32]>> {
    let params = Params::new(m_cost, t_cost, p_cost, Some(32))
        .map_err(|e| anyhow::anyhow!("argon2 params error: {}", e))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    // Hash the passphrase into 32 bytes using the salt; Zeroizing ensures zeroing on drop
    let mut argon2_output = Zeroizing::new([0u8; 32]);
    argon2
        .hash_password_into(passphrase.as_bytes(), salt, argon2_output.as_mut())
        .map_err(|e| anyhow::anyhow!("argon2 hash error: {}", e))?;

    // Expand via HKDF-SHA256 with domain-separation info; Zeroizing ensures zeroing on drop
    let hkdf = Hkdf::<Sha256>::new(None, &*argon2_output);
    let mut okm = Zeroizing::new([0u8; 32]);
    hkdf.expand(KEY_HKDF_INFO, okm.as_mut())
        .map_err(|e| anyhow::anyhow!("hkdf expand error: {}", e))?;

    Ok(okm)
}

/// Encrypt an Ed25519 seed (32 bytes) into a self-describing CCLINKEK binary envelope.
// Phase 16 wires this into `cclink init`; suppress dead_code until then.
#[allow(dead_code)]
///
/// Generates a random 32-byte salt, derives a key-encryption key from the passphrase
/// using Argon2id + HKDF-SHA256 (with `"cclink-key-v1"` domain separation), and encrypts
/// the seed with age. Returns the complete envelope:
///
/// ```text
/// Offset  Size  Field
/// 0       8     Magic: b"CCLINKEK"
/// 8       1     Version: 0x01
/// 9       4     m_cost (Argon2, u32 big-endian)
/// 13      4     t_cost (Argon2, u32 big-endian)
/// 17      4     p_cost (Argon2, u32 big-endian)
/// 21      32    Salt (random bytes)
/// 53      N     Age ciphertext (variable length)
/// ```
pub fn encrypt_key_envelope(seed: &[u8; 32], passphrase: &str) -> anyhow::Result<Vec<u8>> {
    // Generate a fresh random 32-byte salt
    let salt: [u8; 32] = rand::thread_rng().gen();

    let m_cost = KDF_M_COST;
    let t_cost = KDF_T_COST;
    let p_cost = KDF_P_COST;

    // Derive the key-encryption key from passphrase + salt
    let kek = key_derive_key(passphrase, &salt, m_cost, t_cost, p_cost)?;

    // Build age Identity from the derived key and get the Recipient
    let identity = age_identity(&kek);
    let recipient = identity.to_public();

    // Encrypt the 32-byte seed with age
    let ciphertext = age_encrypt(seed, &recipient)?;

    // Serialize the binary envelope
    let mut envelope = Vec::with_capacity(ENVELOPE_HEADER_LEN + ciphertext.len());
    envelope.extend_from_slice(ENVELOPE_MAGIC);
    envelope.push(ENVELOPE_VERSION);
    envelope.extend_from_slice(&m_cost.to_be_bytes());
    envelope.extend_from_slice(&t_cost.to_be_bytes());
    envelope.extend_from_slice(&p_cost.to_be_bytes());
    envelope.extend_from_slice(&salt);
    envelope.extend_from_slice(&ciphertext);

    Ok(envelope)
}

/// Decrypt a CCLINKEK binary envelope back to the original 32-byte Ed25519 seed.
// Phase 16 wires this into key loading; suppress dead_code until then.
#[allow(dead_code)]
///
/// Validates the magic header and version byte, decodes Argon2 parameters from the
/// envelope header (NOT from hardcoded constants — enables forward compatibility),
/// re-derives the key-encryption key, and decrypts the age ciphertext.
///
/// Returns a clear error (not a panic) when the passphrase is wrong or the envelope
/// is malformed. The recovered seed is wrapped in `Zeroizing<[u8;32]>` for automatic
/// zeroing on drop.
pub fn decrypt_key_envelope(
    envelope: &[u8],
    passphrase: &str,
) -> anyhow::Result<Zeroizing<[u8; 32]>> {
    // Validate minimum length (full fixed header must be present)
    if envelope.len() < ENVELOPE_HEADER_LEN {
        anyhow::bail!(
            "Invalid key envelope: too short ({} bytes, need {})",
            envelope.len(),
            ENVELOPE_HEADER_LEN
        );
    }

    // Validate magic bytes
    if &envelope[..8] != ENVELOPE_MAGIC {
        anyhow::bail!("Invalid key envelope: wrong magic bytes");
    }

    // Validate version byte
    if envelope[8] != ENVELOPE_VERSION {
        anyhow::bail!("Unsupported key envelope version: {}", envelope[8]);
    }

    // Decode Argon2 params from header bytes (NOT from constants — forward compat)
    // Safety: unwrap is safe here because length check above guarantees bytes exist
    let m_cost = u32::from_be_bytes(envelope[9..13].try_into().unwrap());
    let t_cost = u32::from_be_bytes(envelope[13..17].try_into().unwrap());
    let p_cost = u32::from_be_bytes(envelope[17..21].try_into().unwrap());

    // Extract 32-byte salt from header
    // Safety: unwrap is safe here because length check above guarantees bytes exist
    let salt: [u8; 32] = envelope[21..53].try_into().unwrap();

    // Age ciphertext is the remainder after the fixed header
    let ciphertext = &envelope[ENVELOPE_HEADER_LEN..];

    // Re-derive key-encryption key from passphrase + header-decoded params
    let kek = key_derive_key(passphrase, &salt, m_cost, t_cost, p_cost)?;
    let identity = age_identity(&kek);

    // Decrypt the seed — map age errors to a user-friendly message (not raw age internals)
    let plaintext = age_decrypt(ciphertext, &identity)
        .map_err(|_| anyhow::anyhow!("Wrong passphrase or corrupted key envelope"))?;

    // Validate recovered plaintext is exactly 32 bytes
    if plaintext.len() != 32 {
        anyhow::bail!(
            "Decrypted key envelope has wrong size: {} bytes (expected 32)",
            plaintext.len()
        );
    }

    // Copy into Zeroizing<[u8;32]> — automatically zeroed on drop
    let mut seed = Zeroizing::new([0u8; 32]);
    seed.copy_from_slice(&plaintext);
    Ok(seed)
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
        assert_eq!(
            scalar1, scalar2,
            "same keypair must produce same X25519 scalar"
        );
        assert_eq!(scalar1.len(), 32, "scalar must be 32 bytes");
        // Must not be all zeros (would be a degenerate key)
        assert_ne!(*scalar1, [0u8; 32], "scalar must not be all zeros");
    }

    #[test]
    fn test_ed25519_to_x25519_public_deterministic() {
        let keypair = fixed_keypair();
        let point1 = ed25519_to_x25519_public(&keypair);
        let point2 = ed25519_to_x25519_public(&keypair);
        assert_eq!(
            point1, point2,
            "same keypair must produce same Montgomery point"
        );
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

        assert_eq!(
            decrypted, plaintext,
            "decrypted plaintext must match original"
        );
    }

    #[test]
    fn test_age_encrypt_produces_different_ciphertext() {
        let keypair = fixed_keypair();
        let pubkey = ed25519_to_x25519_public(&keypair);
        let recipient = age_recipient(&pubkey);

        let plaintext = b"session-abc123";
        let ct1 = age_encrypt(plaintext, &recipient).expect("first encrypt should succeed");
        let ct2 = age_encrypt(plaintext, &recipient).expect("second encrypt should succeed");

        assert_ne!(
            ct1, ct2,
            "two encryptions must produce different ciphertext (ephemeral keys)"
        );
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
        assert!(
            result.is_err(),
            "decryption with wrong key must return an error"
        );
    }

    #[test]
    fn test_recipient_from_z32_round_trip() {
        // Create a keypair, derive z32 pubkey, convert to age Recipient
        let keypair = fixed_keypair();
        let z32 = keypair.public_key().to_z32();

        // Convert z32 to recipient — should succeed
        let recipient =
            recipient_from_z32(&z32).expect("recipient_from_z32 should succeed with valid z32");

        // Encrypt to the derived recipient
        let plaintext = b"round-trip test";
        let ciphertext = age_encrypt(plaintext, &recipient).expect("encrypt should succeed");

        // Decrypt with the keypair's identity — should succeed, proving the recipient is correct
        let secret = ed25519_to_x25519_secret(&keypair);
        let identity = age_identity(&secret);
        let decrypted = age_decrypt(&ciphertext, &identity).expect("decrypt should succeed");

        assert_eq!(
            decrypted, plaintext,
            "decrypted plaintext must match original"
        );
    }

    #[test]
    fn test_recipient_from_z32_invalid_key() {
        let result = recipient_from_z32("not-a-valid-z32-key");
        assert!(
            result.is_err(),
            "recipient_from_z32 must return Err for invalid z32 key"
        );

        let err_str = result.unwrap_err().to_string();
        assert!(
            err_str.contains("invalid recipient pubkey"),
            "error should mention invalid recipient pubkey, got: {}",
            err_str
        );
    }

    // ── PIN key derivation tests ─────────────────────────────────────────────

    #[test]
    fn test_pin_derive_key_deterministic() {
        // Same PIN + same salt must always produce the same 32-byte key
        let salt = [1u8; 32];
        let key1 = pin_derive_key("1234", &salt).expect("pin_derive_key should succeed");
        let key2 = pin_derive_key("1234", &salt).expect("pin_derive_key should succeed");
        assert_eq!(key1, key2, "same PIN + salt must produce identical keys");
        assert_eq!(key1.len(), 32, "derived key must be 32 bytes");
        assert_ne!(*key1, [0u8; 32], "derived key must not be all zeros");
    }

    #[test]
    fn test_pin_derive_key_different_pins_produce_different_keys() {
        // Different PINs with the same salt must produce different keys
        let salt = [1u8; 32];
        let key_1234 =
            pin_derive_key("1234", &salt).expect("pin_derive_key should succeed for 1234");
        let key_5678 =
            pin_derive_key("5678", &salt).expect("pin_derive_key should succeed for 5678");
        assert_ne!(
            key_1234, key_5678,
            "different PINs must produce different keys"
        );
    }

    #[test]
    fn test_pin_derive_key_different_salts_produce_different_keys() {
        // Same PIN with different salts must produce different keys
        let salt_a = [1u8; 32];
        let salt_b = [2u8; 32];
        let key_a =
            pin_derive_key("1234", &salt_a).expect("pin_derive_key should succeed for salt_a");
        let key_b =
            pin_derive_key("1234", &salt_b).expect("pin_derive_key should succeed for salt_b");
        assert_ne!(key_a, key_b, "different salts must produce different keys");
    }

    #[test]
    fn test_pin_encrypt_decrypt_round_trip() {
        // Encrypt with a PIN, decrypt with the same PIN and returned salt
        let plaintext = b"session-id-abc123";
        let (ciphertext, salt) =
            pin_encrypt(plaintext, "1234").expect("pin_encrypt should succeed");
        assert!(!ciphertext.is_empty(), "ciphertext must not be empty");

        let decrypted = pin_decrypt(&ciphertext, "1234", &salt)
            .expect("pin_decrypt should succeed with correct PIN");
        assert_eq!(
            decrypted, plaintext,
            "decrypted plaintext must match original"
        );
    }

    #[test]
    fn test_pin_decrypt_wrong_pin_fails() {
        // Decrypting with the wrong PIN must return an error, not a panic or wrong result
        let plaintext = b"session-id-abc123";
        let (ciphertext, salt) =
            pin_encrypt(plaintext, "1234").expect("pin_encrypt should succeed");

        let result = pin_decrypt(&ciphertext, "9999", &salt);
        assert!(
            result.is_err(),
            "pin_decrypt with wrong PIN must return an error"
        );
    }

    #[test]
    fn test_owner_keypair_cannot_decrypt_pin_encrypted_data() {
        // The owner's X25519 identity (from Ed25519 keypair) must not be able to decrypt
        // data that was encrypted with a PIN-derived key
        let plaintext = b"session-id-abc123";
        let (ciphertext, _salt) =
            pin_encrypt(plaintext, "1234").expect("pin_encrypt should succeed");

        // Try to decrypt with a regular Ed25519-derived identity
        let keypair = fixed_keypair();
        let secret = ed25519_to_x25519_secret(&keypair);
        let identity = age_identity(&secret);

        let result = age_decrypt(&ciphertext, &identity);
        assert!(
            result.is_err(),
            "owner keypair alone must not be able to decrypt PIN-encrypted data"
        );
    }

    // ── Key envelope tests ───────────────────────────────────────────────────

    #[test]
    fn test_key_envelope_round_trip() {
        let seed = [42u8; 32];
        let passphrase = "correct-horse-battery-staple";
        let blob =
            encrypt_key_envelope(&seed, passphrase).expect("encrypt_key_envelope should succeed");
        let recovered = decrypt_key_envelope(&blob, passphrase)
            .expect("decrypt_key_envelope should round-trip");
        assert_eq!(*recovered, seed, "recovered seed must match original");
    }

    #[test]
    fn test_key_envelope_magic_and_version() {
        let seed = [42u8; 32];
        let blob = encrypt_key_envelope(&seed, "test-passphrase")
            .expect("encrypt_key_envelope should succeed");
        assert_eq!(&blob[..8], b"CCLINKEK", "magic bytes must be CCLINKEK");
        assert_eq!(blob[8], 0x01, "version byte must be 0x01");
        assert!(
            blob.len() > 53,
            "envelope must be longer than 53-byte header"
        );
    }

    #[test]
    fn test_key_envelope_params_stored_in_header() {
        let blob = encrypt_key_envelope(&[1u8; 32], "test").expect("encrypt should succeed");
        let m_cost = u32::from_be_bytes(blob[9..13].try_into().unwrap());
        let t_cost = u32::from_be_bytes(blob[13..17].try_into().unwrap());
        let p_cost = u32::from_be_bytes(blob[17..21].try_into().unwrap());
        assert_eq!(m_cost, 65536, "m_cost must be 65536 in header");
        assert_eq!(t_cost, 3, "t_cost must be 3 in header");
        assert_eq!(p_cost, 1, "p_cost must be 1 in header");
    }

    #[test]
    fn test_key_envelope_wrong_passphrase() {
        let seed = [42u8; 32];
        let blob =
            encrypt_key_envelope(&seed, "correct-passphrase").expect("encrypt should succeed");
        let result = decrypt_key_envelope(&blob, "wrong-passphrase");
        assert!(result.is_err(), "wrong passphrase must return Err");
        let msg = result.unwrap_err().to_string().to_lowercase();
        assert!(
            msg.contains("passphrase") || msg.contains("envelope"),
            "error should mention passphrase or envelope, got: {}",
            msg
        );
    }

    #[test]
    fn test_key_envelope_too_short() {
        let short = vec![0u8; 52];
        let result = decrypt_key_envelope(&short, "passphrase");
        assert!(
            result.is_err(),
            "too-short envelope must return Err, not panic"
        );
    }

    #[test]
    fn test_key_envelope_wrong_magic() {
        let mut buf = vec![0u8; 60];
        buf[..8].copy_from_slice(b"WRONGMAG");
        let result = decrypt_key_envelope(&buf, "passphrase");
        assert!(result.is_err(), "wrong magic must return Err");
    }

    #[test]
    fn test_key_hkdf_info_distinct_from_pin() {
        let salt = [7u8; 32];
        let key_kek = key_derive_key("same-passphrase", &salt, 65536, 3, 1)
            .expect("key derivation should succeed");
        let pin_key =
            pin_derive_key("same-passphrase", &salt).expect("pin derivation should succeed");
        assert_ne!(
            *key_kek, *pin_key,
            "key and PIN derivation must produce different keys (domain separation)"
        );
    }

    #[test]
    fn test_key_derive_key_deterministic() {
        let salt = [5u8; 32];
        let key1 = key_derive_key("my-passphrase", &salt, 65536, 3, 1)
            .expect("first derivation should succeed");
        let key2 = key_derive_key("my-passphrase", &salt, 65536, 3, 1)
            .expect("second derivation should succeed");
        assert_eq!(
            *key1, *key2,
            "same inputs must produce same key (deterministic)"
        );
        assert_ne!(*key1, [0u8; 32], "derived key must not be all zeros");
    }
}
