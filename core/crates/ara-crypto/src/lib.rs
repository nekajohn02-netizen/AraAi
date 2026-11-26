//! Envelope encryption (AES-256-GCM) for at-rest data (T1.8) and Ed25519
//! signature verification for signed skill manifests (T2.2b, ADR-017).
//!
//! Both use `ring`, which is already compiled into the APK today via
//! `ureq`'s `rustls` backend (`ara-cloud`, `ara-model-manager` — HTTPS
//! downloads need it) — so neither feature adds a *new* native dependency
//! or a new cross-compilation target for NDK (ADR-005, ADR-018).
//!
//! What this crate deliberately does NOT do: it never touches Android
//! Keystore. The data-encryption key (DEK) this crate seals/opens with is
//! raw key material the caller already has in memory; wrapping that DEK
//! with a Keystore-held key-encryption key (KEK) is Kotlin's job, not
//! Rust's — `ara-ffi` still passes a fixed placeholder today
//! (`TODO(T1.8)`, same pattern as `NetworkPolicy` in ADR-015) until that
//! wiring exists. Likewise, this crate never signs anything at runtime:
//! signing happens offline, once, when ARA's team reviews and publishes an
//! "ara-verified" skill — the app only ever calls `verify_signature`.

use ara_core_api::{AraError, AraResult};
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM, NONCE_LEN};
use ring::pbkdf2;
use ring::rand::{SecureRandom, SystemRandom};
use ring::signature::{Ed25519KeyPair, KeyPair, UnparsedPublicKey, ED25519};
use std::num::NonZeroU32;

pub const KEY_LEN: usize = 32;
pub type Key = [u8; KEY_LEN];

/// Format version byte, so a future change to the blob layout fails loudly
/// on old data instead of silently garbling it.
const FORMAT_V1: u8 = 1;

/// A fresh, random 256-bit key. Generates key material only — storing it
/// (wrapped by a Keystore KEK) is the caller's responsibility.
pub fn generate_key() -> AraResult<Key> {
    let rng = SystemRandom::new();
    let mut key = [0u8; KEY_LEN];
    rng.fill(&mut key)
        .map_err(|_| AraError::Internal("random generation failed".into()))?;
    Ok(key)
}

/// Encrypts `plaintext` with `key`. Output layout:
/// `[version(1 byte) | nonce(12 bytes) | ciphertext+tag]`.
/// A fresh random nonce is drawn for every call, so encrypting the same
/// plaintext twice never produces the same ciphertext.
pub fn seal(key: &Key, plaintext: &[u8]) -> AraResult<Vec<u8>> {
    let rng = SystemRandom::new();
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rng.fill(&mut nonce_bytes)
        .map_err(|_| AraError::Internal("random generation failed".into()))?;
    let unbound = UnboundKey::new(&AES_256_GCM, key)
        .map_err(|_| AraError::Internal("key setup failed".into()))?;
    let sealing_key = LessSafeKey::new(unbound);
    let mut in_out = plaintext.to_vec();
    sealing_key
        .seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce_bytes),
            Aad::empty(),
            &mut in_out,
        )
        .map_err(|_| AraError::Internal("encryption failed".into()))?;
    let mut out = Vec::with_capacity(1 + NONCE_LEN + in_out.len());
    out.push(FORMAT_V1);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&in_out);
    Ok(out)
}

/// Decrypts a blob produced by `seal`. Fails cleanly (never panics) on a
/// truncated blob, an unknown format version, a tampered ciphertext (the
/// GCM tag check), or the wrong key. The error text never includes the key
/// or any plaintext (same rule as the rest of ARA's storage layer).
pub fn open(key: &Key, blob: &[u8]) -> AraResult<Vec<u8>> {
    if blob.len() < 1 + NONCE_LEN {
        return Err(AraError::InvalidInput("ciphertext too short".into()));
    }
    if blob[0] != FORMAT_V1 {
        return Err(AraError::InvalidInput(
            "unknown encryption format version".into(),
        ));
    }
    let nonce_bytes: [u8; NONCE_LEN] = blob[1..1 + NONCE_LEN]
        .try_into()
        .map_err(|_| AraError::InvalidInput("ciphertext too short".into()))?;
    let mut in_out = blob[1 + NONCE_LEN..].to_vec();
    let unbound = UnboundKey::new(&AES_256_GCM, key)
        .map_err(|_| AraError::Internal("key setup failed".into()))?;
    let opening_key = LessSafeKey::new(unbound);
    let plain = opening_key
        .open_in_place(Nonce::assume_unique_for_key(nonce_bytes), Aad::empty(), &mut in_out)
        .map_err(|_| {
            AraError::InvalidInput("decryption failed: wrong key or corrupted data".into())
        })?;
    Ok(plain.to_vec())
}

pub const SALT_LEN: usize = 16;

/// Iterations for `derive_key_from_passphrase`. 600,000 is OWASP's current
/// PBKDF2-HMAC-SHA256 baseline (2023 cheat sheet); this uses a lower,
/// still-reasonable 210,000 as an explicit mobile-hardware tradeoff — this
/// runs synchronously on a phone CPU with no hardware acceleration for
/// PBKDF2 (unlike AES-GCM), and a user export/restore is a rare, deliberate
/// action, not a login a phone has to do many times a day. Documented here
/// rather than silently chosen so a future change has the reasoning to
/// react to, not just a number.
const PBKDF2_ITERATIONS: u32 = 210_000;

/// A fresh random salt for `derive_key_from_passphrase`. Not secret — it is
/// stored alongside the ciphertext it was used for (same pattern as the
/// nonce inside `seal`'s own output) so the same derivation can be repeated
/// on import.
pub fn random_salt() -> AraResult<[u8; SALT_LEN]> {
    let rng = SystemRandom::new();
    let mut salt = [0u8; SALT_LEN];
    rng.fill(&mut salt)
        .map_err(|_| AraError::Internal("random generation failed".into()))?;
    Ok(salt)
}

/// Derives a 256-bit key from a user-chosen passphrase and a salt
/// (PBKDF2-HMAC-SHA256). For a portable encrypted export the user must be
/// able to re-derive the same key on another device with just the
/// passphrase they remember — unlike `generate_key`'s random DEK (meant to
/// be wrapped by an Android Keystore KEK and never typed by a human), this
/// is deliberately deterministic given the same passphrase and salt.
pub fn derive_key_from_passphrase(passphrase: &str, salt: &[u8; SALT_LEN]) -> Key {
    let mut key = [0u8; KEY_LEN];
    pbkdf2::derive(
        pbkdf2::PBKDF2_HMAC_SHA256,
        NonZeroU32::new(PBKDF2_ITERATIONS).expect("non-zero constant"),
        salt,
        passphrase.as_bytes(),
        &mut key,
    );
    key
}

/// Generates a fresh Ed25519 keypair: `(pkcs8_private_bytes, raw_public_bytes)`.
/// Used only by offline tooling that signs an "ara-verified" skill manifest
/// (T2.2b) and by tests that need a throwaway keypair — never by the app
/// at runtime, which only ever calls `verify_signature`.
pub fn generate_keypair() -> AraResult<(Vec<u8>, Vec<u8>)> {
    let rng = SystemRandom::new();
    let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng)
        .map_err(|_| AraError::Internal("key generation failed".into()))?;
    let pair = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref())
        .map_err(|_| AraError::Internal("key generation failed".into()))?;
    Ok((pkcs8.as_ref().to_vec(), pair.public_key().as_ref().to_vec()))
}

/// Signs `message` with a PKCS#8-encoded Ed25519 private key (as returned
/// by `generate_keypair`). Offline tooling only — see module docs.
pub fn sign(pkcs8_private: &[u8], message: &[u8]) -> AraResult<Vec<u8>> {
    let pair = Ed25519KeyPair::from_pkcs8(pkcs8_private)
        .map_err(|_| AraError::InvalidInput("invalid private key".into()))?;
    Ok(pair.sign(message).as_ref().to_vec())
}

/// Verifies an Ed25519 `signature` over `message` against a raw 32-byte
/// public key. Returns `false` (never panics, never errors) for any bad
/// input — a malformed key or signature is just "not verified", exactly
/// like a wrong one.
pub fn verify_signature(public_key: &[u8], message: &[u8], signature: &[u8]) -> bool {
    UnparsedPublicKey::new(&ED25519, public_key)
        .verify(message, signature)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_open_round_trip() {
        let key = generate_key().unwrap();
        let msg = "محادثة سرية بين المستخدم وARA".as_bytes();
        let blob = seal(&key, msg).unwrap();
        assert_ne!(blob, msg, "ciphertext must not equal plaintext");
        assert_eq!(open(&key, &blob).unwrap(), msg);
    }

    #[test]
    fn empty_plaintext_round_trips() {
        let key = generate_key().unwrap();
        let blob = seal(&key, b"").unwrap();
        assert_eq!(open(&key, &blob).unwrap(), b"");
    }

    #[test]
    fn same_plaintext_twice_gives_different_ciphertext() {
        let key = generate_key().unwrap();
        let a = seal(&key, b"hello").unwrap();
        let b = seal(&key, b"hello").unwrap();
        assert_ne!(a, b, "nonce must not repeat");
    }

    #[test]
    fn wrong_key_fails_to_decrypt() {
        let key = generate_key().unwrap();
        let other = generate_key().unwrap();
        let blob = seal(&key, b"secret").unwrap();
        assert!(open(&other, &blob).is_err());
    }

    #[test]
    fn tampered_ciphertext_is_rejected() {
        let key = generate_key().unwrap();
        let mut blob = seal(&key, b"secret conversation").unwrap();
        let last = blob.len() - 1;
        blob[last] ^= 1;
        assert!(open(&key, &blob).is_err(), "GCM tag must catch tampering");
    }

    #[test]
    fn truncated_or_unknown_version_blob_is_rejected_cleanly() {
        let key = generate_key().unwrap();
        assert!(open(&key, b"").is_err());
        assert!(open(&key, &[1, 2, 3]).is_err());
        let mut blob = seal(&key, b"x").unwrap();
        blob[0] = 99; // unknown format version
        assert!(open(&key, &blob).is_err());
    }

    #[test]
    fn passphrase_derivation_is_deterministic_and_salt_and_passphrase_both_matter() {
        let salt = random_salt().unwrap();
        let k1 = derive_key_from_passphrase("correct horse battery staple", &salt);
        let k2 = derive_key_from_passphrase("correct horse battery staple", &salt);
        assert_eq!(k1, k2, "same passphrase + salt must give the same key");

        let other_salt = random_salt().unwrap();
        assert_ne!(other_salt, salt, "random_salt must not repeat");
        let k3 = derive_key_from_passphrase("correct horse battery staple", &other_salt);
        assert_ne!(k1, k3, "a different salt must give a different key");

        let k4 = derive_key_from_passphrase("a different passphrase", &salt);
        assert_ne!(k1, k4, "a different passphrase must give a different key");
    }

    #[test]
    fn passphrase_derived_key_works_with_seal_and_open() {
        let salt = random_salt().unwrap();
        let key = derive_key_from_passphrase("my recovery passphrase", &salt);
        let blob = seal(&key, b"exported data").unwrap();
        assert_eq!(open(&key, &blob).unwrap(), b"exported data");
        let wrong_key = derive_key_from_passphrase("wrong passphrase", &salt);
        assert!(open(&wrong_key, &blob).is_err());
    }

    #[test]
    fn signature_round_trip_and_tamper_detection() {
        let (sk, pk) = generate_keypair().unwrap();
        let msg = b"skill-manifest-bytes-to-sign";
        let sig = sign(&sk, msg).unwrap();
        assert!(verify_signature(&pk, msg, &sig));
        assert!(!verify_signature(&pk, b"different message", &sig));

        let (_, other_pk) = generate_keypair().unwrap();
        assert!(!verify_signature(&other_pk, msg, &sig));

        let mut bad_sig = sig.clone();
        bad_sig[0] ^= 1;
        assert!(!verify_signature(&pk, msg, &bad_sig));
    }

    #[test]
    fn malformed_key_or_signature_never_panics() {
        assert!(!verify_signature(b"not a key", b"msg", b"not a sig"));
        // A real (non-degenerate) public key with a garbage signature must
        // fail. Note deliberately NOT tested here: an all-zero public key
        // with an all-zero signature verifies as "valid" under Ed25519's
        // standard cofactored check (both decode to a low-order point, so
        // the equation is trivially satisfied) — a known property of the
        // scheme, not a bug in this crate. It is not a real weakness for
        // ARA because `verify_signature` is only ever called with ARA's
        // own fixed, embedded, known-good public key (ADR-017): an
        // attacker who can choose the *public key* being checked against
        // has already won by a different route, and no ARA trust tier
        // lets a skill supply its own "trusted" key.
        let (_, pk) = generate_keypair().unwrap();
        assert!(!verify_signature(&pk, b"msg", &[0u8; 64]));
    }
}
