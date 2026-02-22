/// Plaintext leak detection tests.
///
/// Verify that encrypted blobs produced by the age encryption path never
/// contain the original session ID in any readable form — neither as raw bytes
/// nor as a base64-encoded string.
///
/// These tests guard against regression where a refactor accidentally stores
/// or transmits plaintext session data alongside the ciphertext.

use base64::Engine;
use cclink::crypto::{age_encrypt, age_recipient, ed25519_to_x25519_public};

/// Fixed keypair seed used as the self-encrypt key.
fn keypair_self() -> pkarr::Keypair {
    pkarr::Keypair::from_secret_key(&[42u8; 32])
}

/// Fixed keypair seed used as an external recipient.
fn keypair_recipient() -> pkarr::Keypair {
    pkarr::Keypair::from_secret_key(&[99u8; 32])
}

/// Build an age Recipient for the given keypair.
fn recipient_for(keypair: &pkarr::Keypair) -> age::x25519::Recipient {
    let pubkey = ed25519_to_x25519_public(keypair);
    age_recipient(&pubkey)
}

// ── Test 1: Self-encrypt ciphertext contains no plaintext session ID ────────

/// Encrypt to own key and assert the session ID does not appear in the ciphertext.
#[test]
fn test_encrypted_blob_contains_no_plaintext_session_id() {
    let keypair = keypair_self();
    let recipient = recipient_for(&keypair);

    let known_id = "KNOWN-SESSION-ID-abc123-MUST-NOT-APPEAR";
    let ciphertext = age_encrypt(known_id.as_bytes(), &recipient)
        .expect("age_encrypt should succeed");

    // String scan: ciphertext interpreted as lossy UTF-8 must not contain the session ID
    let ct_lossy = String::from_utf8_lossy(&ciphertext);
    assert!(
        !ct_lossy.contains(known_id),
        "ciphertext (UTF-8 lossy) must not contain the plaintext session ID"
    );

    // Byte-window scan: no contiguous window of bytes matches the session ID bytes
    let id_bytes = known_id.as_bytes();
    let found_in_bytes = ciphertext
        .windows(id_bytes.len())
        .any(|w| w == id_bytes);
    assert!(
        !found_in_bytes,
        "ciphertext bytes must not contain the plaintext session ID byte sequence"
    );
}

// ── Test 2: Shared-encrypt ciphertext contains no plaintext ────────────────

/// Encrypt to a different recipient and assert the session ID is not in the ciphertext.
#[test]
fn test_shared_encrypted_blob_contains_no_plaintext() {
    let recipient_kp = keypair_recipient();
    let recipient = recipient_for(&recipient_kp);

    let known_id = "KNOWN-SESSION-ID-abc123-MUST-NOT-APPEAR";
    let ciphertext = age_encrypt(known_id.as_bytes(), &recipient)
        .expect("age_encrypt to recipient should succeed");

    // String scan
    let ct_lossy = String::from_utf8_lossy(&ciphertext);
    assert!(
        !ct_lossy.contains(known_id),
        "shared-encrypt ciphertext (UTF-8 lossy) must not contain the plaintext session ID"
    );

    // Byte-window scan
    let id_bytes = known_id.as_bytes();
    let found_in_bytes = ciphertext
        .windows(id_bytes.len())
        .any(|w| w == id_bytes);
    assert!(
        !found_in_bytes,
        "shared-encrypt ciphertext bytes must not contain the plaintext session ID byte sequence"
    );
}

// ── Test 3: Base64-encoded blob contains no plaintext ─────────────────────

/// Encrypt a session ID, base64-encode the ciphertext (as the publish path stores in
/// HandoffRecord.blob), and verify the base64 string does not contain the plaintext ID.
///
/// This tests what actually gets stored in the HandoffRecord `blob` field.
#[test]
fn test_base64_encoded_blob_contains_no_plaintext() {
    let keypair = keypair_self();
    let recipient = recipient_for(&keypair);

    let known_id = "KNOWN-SESSION-ID-abc123-MUST-NOT-APPEAR";
    let ciphertext = age_encrypt(known_id.as_bytes(), &recipient)
        .expect("age_encrypt should succeed");

    // Base64-encode as the publish path does
    let blob = base64::engine::general_purpose::STANDARD.encode(&ciphertext);

    // The base64-encoded blob must not contain the plaintext session ID string
    assert!(
        !blob.contains(known_id),
        "base64-encoded blob must not contain the plaintext session ID string"
    );

    // Also verify the plaintext is not present as UTF-8 bytes inside the base64 string
    let id_bytes = known_id.as_bytes();
    let blob_bytes = blob.as_bytes();
    let found_in_base64_bytes = blob_bytes
        .windows(id_bytes.len())
        .any(|w| w == id_bytes);
    assert!(
        !found_in_base64_bytes,
        "base64-encoded blob bytes must not contain the plaintext session ID byte sequence"
    );
}
