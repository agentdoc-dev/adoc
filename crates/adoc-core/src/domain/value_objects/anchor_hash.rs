//! `AnchorHash` value object (V8.5.1, ADR-0048).
//!
//! The validated form of an Evidence Anchor: `sha256:` followed by exactly
//! 64 lowercase hex characters — byte-identical to what
//! `application::hashing::sha256_prefixed` emits, so `shasum -a 256`
//! output is accepted verbatim.

const PREFIX: &str = "sha256:";
const HEX_LEN: usize = 64;

/// A validated Evidence Anchor hash value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AnchorHash(String);

/// Why a `hash` field value is not a valid Evidence Anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AnchorHashError;

impl AnchorHash {
    pub(crate) fn try_new(value: &str) -> Result<Self, AnchorHashError> {
        let hex = value.strip_prefix(PREFIX).ok_or(AnchorHashError)?;
        let valid = hex.len() == HEX_LEN
            && hex
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'));
        if valid {
            Ok(Self(value.to_string()))
        } else {
            Err(AnchorHashError)
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn try_new_accepts_sha256_prefix_with_64_lowercase_hex() {
        let anchor = AnchorHash::try_new(VALID).expect("valid anchor");
        assert_eq!(anchor.as_str(), VALID);
    }

    #[test]
    fn try_new_rejects_missing_prefix() {
        assert!(AnchorHash::try_new(&VALID[PREFIX.len()..]).is_err());
    }

    #[test]
    fn try_new_rejects_wrong_length() {
        assert!(AnchorHash::try_new("sha256:abc123").is_err());
        assert!(AnchorHash::try_new(&format!("{VALID}0")).is_err());
    }

    #[test]
    fn try_new_rejects_uppercase_and_non_hex() {
        assert!(AnchorHash::try_new(&VALID.to_uppercase()).is_err());
        let non_hex = format!("sha256:{}", "g".repeat(HEX_LEN));
        assert!(AnchorHash::try_new(&non_hex).is_err());
    }

    #[test]
    fn try_new_rejects_other_algorithms() {
        assert!(AnchorHash::try_new(&format!("sha512:{}", "a".repeat(HEX_LEN))).is_err());
    }
}
