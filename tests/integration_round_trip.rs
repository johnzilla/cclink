/// Integration tests: encryption round-trip for all four code paths, plus v1.1
/// tamper detection for burn and recipient fields.
///
/// Tests cover:
///   1. Self-encrypt  — own key encrypts, own key decrypts
///   2. Shared-encrypt — recipient's key encrypts, recipient's key decrypts; sender cannot decrypt
///   3. Burn round-trip — burn flag is in signed envelope; crypto path is identical to self-encrypt
///   4. Shared+burn    — combines shared encrypt + burn flag; recipient decrypts; sender cannot
///   5. Signed burn tamper detection — tamping burn after signing causes verify_record to fail
///   6. Signed recipient tamper detection — tampering recipient after signing causes verify_record to fail
///
/// All tests are `#[test]` (not `#[tokio::test]`) — no async, no network access.

use cclink::crypto::{
    age_decrypt, age_encrypt, age_identity, age_recipient, ed25519_to_x25519_public,
    ed25519_to_x25519_secret, pin_decrypt, pin_encrypt,
};
use cclink::record::{sign_record, verify_record, HandoffRecord, HandoffRecordSignable};

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

// ── Test 5: Signed burn tamper detection ───────────────────────────────────

/// Build a full signed HandoffRecord with burn=false. Verify it passes.
/// Then tamper burn=true. Verify the signature check fails.
/// Proves that burn is part of the signed envelope in v1.1.
#[test]
fn test_signed_burn_tamper_detection() {
    let keypair = keypair_a();

    // Build a signed record with burn=false
    let signable = HandoffRecordSignable {
        blob: "dGVzdGJsb2I=".to_string(),
        burn: false,
        created_at: 1_700_000_000,
        hostname: "testhost".to_string(),
        pin_salt: None,
        project: "/home/user/project".to_string(),
        pubkey: keypair.public_key().to_z32(),
        recipient: None,
        ttl: 3600,
    };
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
        signature: signature.clone(),
        ttl: signable.ttl,
    };

    // Valid record should verify
    verify_record(&record, &keypair.public_key())
        .expect("valid record should pass signature verification");

    // Tamper: flip burn to true
    let tampered = HandoffRecord {
        burn: true, // tampered!
        ..record
    };

    let result = verify_record(&tampered, &keypair.public_key());
    assert!(
        result.is_err(),
        "tampered burn flag must cause signature verification failure"
    );
}

// ── Test 7: PIN encrypt/decrypt round-trip ────────────────────────────────

/// Encrypt a session ID using pin_encrypt with PIN "1234", then:
///   - Decrypt with same PIN and returned salt — assert plaintext matches.
///   - Decrypt with wrong PIN "9999" — assert Err (wrong PIN rejected).
#[test]
fn test_pin_encrypt_round_trip() {
    let session_id = b"sess-pin-round-trip-test-abc123";

    // Encrypt with PIN "1234"
    let (ciphertext, salt) = pin_encrypt(session_id, "1234")
        .expect("pin_encrypt should succeed");
    assert!(!ciphertext.is_empty(), "ciphertext must not be empty");

    // Correct PIN decrypts successfully
    let decrypted = pin_decrypt(&ciphertext, "1234", &salt)
        .expect("pin_decrypt should succeed with correct PIN");
    assert_eq!(
        decrypted.as_slice(),
        session_id,
        "pin_decrypt with correct PIN must return original session ID"
    );

    // Wrong PIN returns Err
    let result = pin_decrypt(&ciphertext, "9999", &salt);
    assert!(
        result.is_err(),
        "pin_decrypt with wrong PIN must return an error"
    );
}

// ── Test 8: PIN record — owner keypair cannot decrypt ─────────────────────

/// Proves that the owner's age identity (derived from their Ed25519 keypair) cannot
/// decrypt a PIN-protected record. After confirming keypair decryption fails,
/// verify that the correct PIN succeeds — demonstrating PIN-derived key isolation.
#[test]
fn test_pin_record_owner_cannot_decrypt() {
    let session_id = b"sess-pin-owner-isolation-test";

    // Encrypt with PIN "5678"
    let (ciphertext, salt) = pin_encrypt(session_id, "5678")
        .expect("pin_encrypt should succeed");

    // Attempt decryption with owner's keypair identity — must fail
    let owner_keypair = keypair_a();
    let owner_secret = ed25519_to_x25519_secret(&owner_keypair);
    let owner_identity = age_identity(&owner_secret);
    let owner_result = age_decrypt(&ciphertext, &owner_identity);
    assert!(
        owner_result.is_err(),
        "owner keypair alone must NOT decrypt PIN-protected data"
    );

    // Correct PIN decrypts successfully — proving PIN-derived key isolation
    let decrypted = pin_decrypt(&ciphertext, "5678", &salt)
        .expect("pin_decrypt with correct PIN should succeed");
    assert_eq!(
        decrypted.as_slice(),
        session_id,
        "pin_decrypt with correct PIN must return original session ID"
    );
}

// ── Test 6: Signed recipient tamper detection ──────────────────────────────

/// Build a full signed HandoffRecord with recipient=None. Verify it passes.
/// Then tamper recipient=Some("attacker"). Verify the signature check fails.
/// Proves that recipient is part of the signed envelope in v1.1.
#[test]
fn test_signed_recipient_tamper_detection() {
    let keypair = keypair_a();

    // Build a signed record with recipient=None (self-encrypted)
    let signable = HandoffRecordSignable {
        blob: "dGVzdGJsb2I=".to_string(),
        burn: false,
        created_at: 1_700_000_000,
        hostname: "testhost".to_string(),
        pin_salt: None,
        project: "/home/user/project".to_string(),
        pubkey: keypair.public_key().to_z32(),
        recipient: None,
        ttl: 3600,
    };
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
        signature: signature.clone(),
        ttl: signable.ttl,
    };

    // Valid record should verify
    verify_record(&record, &keypair.public_key())
        .expect("valid record should pass signature verification");

    // Tamper: inject a recipient that was not in the original signable
    let tampered = HandoffRecord {
        recipient: Some("attacker-pubkey-z32encoded".to_string()), // tampered!
        ..record
    };

    let result = verify_record(&tampered, &keypair.public_key());
    assert!(
        result.is_err(),
        "tampered recipient must cause signature verification failure"
    );
}
