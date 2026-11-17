//! Ed25519 release signature verification.
//!
//! Every update asset is accompanied by `<asset>.sig`, a base64 Ed25519 signature over the
//! canonical message built by [`signed_message`]. The message binds the release tag, the asset
//! name and the asset SHA-256, so a valid signature cannot be replayed for another release,
//! another file or tampered contents. Release signing is done with
//! `cargo run -p graphite-app --example update_signer` (see `docs/en/features/updates-and-issue-reporter.md`).

use base64::Engine;
use ring::signature::{UnparsedPublicKey, ED25519};

/// Public keys trusted to sign Graphite releases. Several keys allow rotation: add the new
/// key, ship a release signed by the old key, then retire the old one.
pub(crate) const TRUSTED_UPDATE_KEYS: &[[u8; 32]] = &[UPDATE_SIGNING_KEY_2026];

/// Primary release-signing public key (Ed25519, raw 32 bytes).
const UPDATE_SIGNING_KEY_2026: [u8; 32] = [
    178, 37, 15, 248, 184, 55, 17, 84, 218, 188, 56, 170, 167, 85, 4, 9, 231, 131, 90, 127, 243,
    240, 246, 171, 55, 249, 122, 104, 194, 56, 120, 79,
];

const MESSAGE_DOMAIN: &str = "graphite-update-v1";

/// Largest signature file accepted (a base64 Ed25519 signature is 88 bytes).
pub(crate) const MAX_SIGNATURE_FILE_BYTES: usize = 4096;

/// Canonical message signed by the release pipeline.
pub(crate) fn signed_message(tag: &str, asset_name: &str, sha256_hex: &str) -> Vec<u8> {
    format!(
        "{MESSAGE_DOMAIN}\n{tag}\n{asset_name}\n{}\n",
        sha256_hex.trim().to_ascii_lowercase()
    )
    .into_bytes()
}

/// Verifies `signature_text` (base64, surrounding whitespace ignored) against any trusted key.
pub(crate) fn verify_release_signature(
    trusted_keys: &[[u8; 32]],
    message: &[u8],
    signature_text: &str,
) -> Result<(), String> {
    if trusted_keys.is_empty() {
        return Err("No trusted update signing keys are configured".into());
    }
    let signature = base64::engine::general_purpose::STANDARD
        .decode(signature_text.trim())
        .map_err(|_| "Update signature is not valid base64".to_owned())?;
    if signature.len() != 64 {
        return Err("Update signature has an invalid length".into());
    }
    let valid = trusted_keys.iter().any(|key| {
        UnparsedPublicKey::new(&ED25519, key)
            .verify(message, &signature)
            .is_ok()
    });
    if valid {
        Ok(())
    } else {
        Err("Update signature verification failed: package is not signed by a trusted key".into())
    }
}

#[cfg(test)]
pub(crate) mod test_keys {
    use base64::Engine;
    use ring::rand::SystemRandom;
    use ring::signature::{Ed25519KeyPair, KeyPair};

    pub(crate) struct TestSigner(Ed25519KeyPair);

    impl TestSigner {
        pub(crate) fn new() -> Self {
            let pkcs8 = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).unwrap();
            Self(Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap())
        }

        pub(crate) fn public_key(&self) -> [u8; 32] {
            self.0.public_key().as_ref().try_into().unwrap()
        }

        pub(crate) fn sign(&self, tag: &str, asset: &str, sha256: &str) -> String {
            let msg = super::signed_message(tag, asset, sha256);
            base64::engine::general_purpose::STANDARD.encode(self.0.sign(&msg).as_ref())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_keys::TestSigner;
    use super::*;

    const HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    #[test]
    fn the_shipped_trust_list_has_real_distinct_keys() {
        assert!(
            !TRUSTED_UPDATE_KEYS.is_empty(),
            "releases could never be verified"
        );
        for (index, key) in TRUSTED_UPDATE_KEYS.iter().enumerate() {
            assert_ne!(key, &[0u8; 32], "an all-zero key is a placeholder");
            assert!(
                !TRUSTED_UPDATE_KEYS[..index].contains(key),
                "the same key is listed twice"
            );
        }
    }

    #[test]
    fn a_signature_by_an_unlisted_key_is_not_accepted_with_the_shipped_list() {
        let signer = TestSigner::new();
        assert!(
            !TRUSTED_UPDATE_KEYS.contains(&signer.public_key()),
            "a throwaway test key must never be trusted by the shipped list"
        );
    }

    #[test]
    fn accepts_valid_signature_from_trusted_key() {
        let signer = TestSigner::new();
        let sig = signer.sign("v1.0.0", "graphite-windows-x64.zip", HASH);
        let msg = signed_message("v1.0.0", "graphite-windows-x64.zip", HASH);
        assert!(verify_release_signature(&[signer.public_key()], &msg, &sig).is_ok());
        assert!(
            verify_release_signature(&[signer.public_key()], &msg, &format!("{sig}\n")).is_ok()
        );
    }

    #[test]
    fn rejects_untrusted_key_wrong_binding_and_malformed_signatures() {
        let signer = TestSigner::new();
        let other = TestSigner::new();
        let sig = signer.sign("v1.0.0", "a.zip", HASH);
        let msg = signed_message("v1.0.0", "a.zip", HASH);

        assert!(verify_release_signature(&[other.public_key()], &msg, &sig).is_err());
        assert!(verify_release_signature(&[], &msg, &sig).is_err());
        for (tag, name, hash) in [
            ("v1.0.1", "a.zip", HASH),
            ("v1.0.0", "b.zip", HASH),
            ("v1.0.0", "a.zip", &HASH.replace('a', "b")),
        ] {
            let tampered = signed_message(tag, name, hash);
            assert!(verify_release_signature(&[signer.public_key()], &tampered, &sig).is_err());
        }
        assert!(verify_release_signature(&[signer.public_key()], &msg, "!!not base64!!").is_err());
        assert!(verify_release_signature(&[signer.public_key()], &msg, "AAAA").is_err());
    }
}
