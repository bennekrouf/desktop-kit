//! The signature on api0's answer, so a saved answer can't be edited into Pro.
//!
//! api0 sends its answer twice in one reply: as the fields the apps released
//! before the signature read, and as `signed`, the same answer as one JSON
//! text, with `signature`, the base64 of an Ed25519 signature over
//! [`CONTEXT`] followed by the UTF-8 bytes of `signed`. The app checks that
//! signature with [`PUBLIC_KEY`], reads the answer *from `signed`*, and keeps
//! `signed` and `signature` as they came, so the check can be made again each
//! time the saved answer is read. The fields beside them are never trusted.
//! The gateway's side is `src/auth/licence_seal.rs` in gateway-api0, and its
//! `LICENCE_SIGNING.md` says how the key pair is made.
//!
//! An answer that isn't signed counts until [`UNSIGNED_UNTIL`]: the answers
//! saved before api0 signed them, and api0's own until the gateway that signs
//! them is deployed, so nobody on Pro loses it in between. After that date an
//! unsigned answer, like one whose signature doesn't match, counts as no
//! answer: the app asks api0 again, as it does for a licence not checked yet.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

use super::{normalise, Answer};

/// api0's public key, the base64 of its 32 bytes.
///
/// Empty until the production key pair is made (gateway-api0's
/// `LICENCE_SIGNING.md`). While it is, no signature can be checked, and every
/// answer counts as unsigned: Pro until [`UNSIGNED_UNTIL`], then no answer at
/// all. Set it, and ship the apps, before that date.
pub const PUBLIC_KEY: &str = "";

/// Put before the bytes of `signed`, and signed with them, so a signature on a
/// licence answer can't pass for anything else signed with the same key.
pub const CONTEXT: &[u8] = b"api0 licence v1\n";

/// 2027-02-01 00:00 UTC (seconds since 1970): until then, an answer without a
/// signature still counts. The gateway that signs must be deployed before.
pub const UNSIGNED_UNTIL: u64 = 1_801_440_000;

/// An answer that can be trusted, and when api0 says it gave it.
pub(super) struct Opened {
    pub answer: Answer,
    /// `issued_at` from the signed text; `None` for an unsigned answer, whose
    /// time is only this computer's say-so.
    pub issued_at: Option<u64>,
}

/// What `answer` can be trusted to say about `licence` at `now`, checked with
/// `key` (base64; [`PUBLIC_KEY`] outside the tests). `None`: no answer.
pub(super) fn open(answer: &Answer, licence: &str, now: u64, key: &str) -> Option<Opened> {
    let unsigned = || {
        (now < UNSIGNED_UNTIL).then(|| Opened {
            answer: answer.clone(),
            issued_at: None,
        })
    };
    if answer.signature.is_empty() {
        return unsigned();
    }
    let Some(key) = verifying_key(key) else {
        // No key built in yet: nothing can be checked.
        return unsigned();
    };
    let signature = STANDARD.decode(&answer.signature).ok()?;
    let signature = Signature::from_slice(&signature).ok()?;
    key.verify(&[CONTEXT, answer.signed.as_bytes()].concat(), &signature)
        .ok()?;
    let mut signed: Answer = serde_json::from_str(&answer.signed).ok()?;
    // A signed answer about another licence says nothing about this one.
    if normalise(&signed.licence) != normalise(licence) {
        return None;
    }
    signed.signed.clone_from(&answer.signed);
    signed.signature.clone_from(&answer.signature);
    let issued_at = (signed.issued_at > 0).then_some(signed.issued_at);
    Some(Opened {
        answer: signed,
        issued_at,
    })
}

fn verifying_key(key: &str) -> Option<VerifyingKey> {
    let bytes: [u8; 32] = STANDARD.decode(key).ok()?.try_into().ok()?;
    VerifyingKey::from_bytes(&bytes).ok()
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    /// RFC 8032, section 7.1, test 1: a published key pair, never a real one.
    pub const TEST_PUBLIC: &str = "11qYAYKxCrfVS/7TyWQHOg7hcvPapiMlrwIaaPcHURo=";
    const TEST_SEED: &str = "nWGxne/9WmC6hEr0kuwsxERJxWl7MmkZcDusAxyuf2A=";

    fn test_key() -> SigningKey {
        let seed: [u8; 32] = STANDARD.decode(TEST_SEED).unwrap().try_into().unwrap();
        SigningKey::from_bytes(&seed)
    }

    /// A key that isn't api0's, in base64.
    pub fn other_public() -> String {
        STANDARD.encode(
            SigningKey::from_bytes(&[9u8; 32])
                .verifying_key()
                .as_bytes(),
        )
    }

    /// `answer` as api0 sends it: its fields, and the same as `signed`.
    pub fn sealed(answer: &Answer) -> Answer {
        let signed = serde_json::to_string(answer).unwrap();
        let signature = test_key().sign(&[CONTEXT, signed.as_bytes()].concat());
        Answer {
            signed,
            signature: STANDARD.encode(signature.to_bytes()),
            ..answer.clone()
        }
    }

    #[test]
    fn the_built_in_key_is_empty_or_32_bytes() {
        assert!(PUBLIC_KEY.is_empty() || verifying_key(PUBLIC_KEY).is_some());
        assert_eq!(
            STANDARD.encode(test_key().verifying_key().as_bytes()),
            TEST_PUBLIC
        );
    }

    /// The vector gateway-api0's tests sign (src/auth/licence_seal.rs), so the
    /// two sides can't drift apart on what is signed.
    #[test]
    fn the_signature_both_sides_agree_on() {
        let answer = Answer {
            licence: "ABCDE-FGHJK-LMNPQ-RSTUV".into(),
            signed: r#"{"active":true,"issued_at":1800000000,"licence":"ABCDE-FGHJK-LMNPQ-RSTUV","object":"licence","products":["*"]}"#.into(),
            signature: "0sCoUA3SAa4PyuocJLZ4QA8FAGnmONHsAfCvxBiQX0nccti+4XMPQnC+XUy0f9H6lgQq9E/jO+BR10YUd9z8Cg==".into(),
            ..Answer::default()
        };
        let opened = open(&answer, &answer.licence, UNSIGNED_UNTIL + 1, TEST_PUBLIC).unwrap();
        assert!(opened.answer.unlocks_for("anything"));
        assert_eq!(opened.issued_at, Some(1_800_000_000));
    }
}
