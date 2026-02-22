/// Integration tests: encryption round-trip for all four code paths.
///
/// Tests cover:
///   1. Self-encrypt  — own key encrypts, own key decrypts
///   2. Shared-encrypt — recipient's key encrypts, recipient's key decrypts; sender cannot decrypt
///   3. Burn round-trip — burn flag is metadata only; crypto path is identical to self-encrypt
///   4. Shared+burn    — combines shared encrypt + burn flag; recipient decrypts; sender cannot
///
/// All tests are `#[test]` (not `#[tokio::test]`) — no async, no network access.

use cclink::crypto::{
    age_decrypt, age_encrypt, age_identity, age_recipient, ed25519_to_x25519_public,
    ed25519_to_x25519_secret,
};

/// Fixed keypair with seed [42u8; 32] — used for the "self" / sender role.
fn keypair_a() -> pkarr::Keypair {
    pkarr::Keypair::from_secret_key(&[42u8; 32])
}

/// Fixed keypair with seed [99u8; 32] — used as the recipient in shared-encrypt tests.
fn keypair_b() -> pkarr::Keypair {
    pkarr::Keypair::from_secret_key(&[99u8; 32])
}

/// Build an age Identity for the given keypair.
fn identity_for(keypair: &pkarr::Keypair) -> age::x25519::Identity {
    let secret = ed25519_to_x25519_secret(keypair);
    age_identity(&secret)
}

/// Build an age Recipient for the given keypair.
fn recipient_for(keypair: &pkarr::Keypair) -> age::x25519::Recipient {
    let pubkey = ed25519_to_x25519_public(keypair);
    age_recipient(&pubkey)
}

// ── Test 1: Self-encrypt round-trip ────────────────────────────────────────

/// Encrypt a session ID to own key, decrypt with own identity, verify equality.
#[test]
fn test_self_encrypt_round_trip() {
    let keypair = keypair_a();

    let x25519_secret = ed25519_to_x25519_secret(&keypair);
    let x25519_public = ed25519_to_x25519_public(&keypair);

    let identity = age_identity(&x25519_secret);
    let recipient = age_recipient(&x25519_public);

    let session_id = b"sess-abc123-round-trip-test";

    let ciphertext = age_encrypt(session_id, &recipient)
        .expect("age_encrypt should succeed for self-encrypt");

    let decrypted = age_decrypt(&ciphertext, &identity)
        .expect("age_decrypt should succeed with own identity");

    assert_eq!(
        decrypted.as_slice(),
        session_id,
        "decrypted plaintext must exactly match the original session ID"
    );
}

// ── Test 2: Shared-encrypt round-trip ─────────────────────────────────────

/// Encrypt to recipient's key, decrypt with recipient's identity.
/// Also verify that the sender CANNOT decrypt the ciphertext.
#[test]
fn test_shared_encrypt_round_trip() {
    let sender = keypair_a();
    let recipient_kp = keypair_b();

    // Encrypt to the recipient's public key
    let recipient = recipient_for(&recipient_kp);
    let session_id = b"sess-shared-recipient-round-trip";

    let ciphertext = age_encrypt(session_id, &recipient)
        .expect("age_encrypt to recipient key should succeed");

    // Recipient can decrypt
    let recipient_identity = identity_for(&recipient_kp);
    let decrypted = age_decrypt(&ciphertext, &recipient_identity)
        .expect("recipient should be able to decrypt");

    assert_eq!(
        decrypted.as_slice(),
        session_id,
        "recipient-decrypted plaintext must match original session ID"
    );

    // Sender CANNOT decrypt — they only have the recipient's public key, not private key
    let sender_identity = identity_for(&sender);
    let sender_result = age_decrypt(&ciphertext, &sender_identity);
    assert!(
        sender_result.is_err(),
        "sender must NOT be able to decrypt a message encrypted for the recipient"
    );
}

// ── Test 3: Burn-after-read round-trip ─────────────────────────────────────

/// The burn flag is a metadata field on HandoffRecord — not a crypto concern.
/// This test confirms the encryption path is identical whether burn is true or false.
/// Encrypt session ID to own key (same as self-encrypt path), decrypt, compare.
#[test]
fn test_burn_encrypt_round_trip() {
    let keypair = keypair_a();

    // burn = true is handled at the HandoffRecord metadata level; the crypto path
    // is the same self-encrypt path as test_self_encrypt_round_trip.
    let identity = identity_for(&keypair);
    let recipient = recipient_for(&keypair);

    let session_id = b"sess-burn-after-read-test-xyz789";

    // Simulate what publish does when burn=true: encrypt normally to own key
    let ciphertext = age_encrypt(session_id, &recipient)
        .expect("age_encrypt should succeed for burn path");

    let decrypted = age_decrypt(&ciphertext, &identity)
        .expect("age_decrypt should succeed for burn path");

    assert_eq!(
        decrypted.as_slice(),
        session_id,
        "burn round-trip: decrypted plaintext must match original session ID"
    );
}

// ── Test 4: Shared + burn round-trip ──────────────────────────────────────

/// Combine shared-encrypt with burn flag: encrypt to recipient key, recipient decrypts.
/// Sender cannot decrypt. (burn flag is metadata only — does not affect crypto.)
#[test]
fn test_shared_burn_encrypt_round_trip() {
    let sender = keypair_a();
    let recipient_kp = keypair_b();

    // Encrypt to recipient (same as shared-encrypt path; burn flag is metadata)
    let recipient = recipient_for(&recipient_kp);
    let session_id = b"sess-shared-burn-round-trip-abc";

    let ciphertext = age_encrypt(session_id, &recipient)
        .expect("age_encrypt should succeed for shared+burn path");

    // Recipient decrypts successfully
    let recipient_identity = identity_for(&recipient_kp);
    let decrypted = age_decrypt(&ciphertext, &recipient_identity)
        .expect("recipient should decrypt shared+burn ciphertext");

    assert_eq!(
        decrypted.as_slice(),
        session_id,
        "shared+burn round-trip: decrypted plaintext must match original session ID"
    );

    // Sender cannot decrypt
    let sender_identity = identity_for(&sender);
    let sender_result = age_decrypt(&ciphertext, &sender_identity);
    assert!(
        sender_result.is_err(),
        "sender must NOT decrypt a shared+burn message encrypted for the recipient"
    );
}
