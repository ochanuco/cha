use crate::errors::ChaError;

pub fn is_uuid_v7(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 36 {
        return false;
    }
    for (i, &c) in b.iter().enumerate() {
        let ok = match i {
            8 | 13 | 18 | 23 => c == b'-',
            14 => c == b'7',
            19 => matches!(c, b'8' | b'9' | b'a' | b'b'),
            _ => is_lower_hex(c),
        };
        if !ok {
            return false;
        }
    }
    true
}

pub fn is_sha256_id(s: &str) -> bool {
    match s.strip_prefix("sha256:") {
        Some(hex) => hex.len() == 64 && hex.bytes().all(is_lower_hex),
        None => false,
    }
}

fn is_lower_hex(c: u8) -> bool {
    c.is_ascii_digit() || (b'a'..=b'f').contains(&c)
}

fn is_non_empty(s: &str) -> bool {
    !s.is_empty()
}

macro_rules! id_type {
    ($(#[$meta:meta])* $name:ident, $valid:path) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            /// `field` is the dotted input path reported when `value` is malformed.
            pub fn parse(value: &str, field: &str) -> Result<Self, ChaError> {
                if $valid(value) {
                    Ok(Self(value.to_string()))
                } else {
                    Err(ChaError::InvalidId { field: field.to_string() })
                }
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

id_type!(DocumentId, is_uuid_v7);
id_type!(ChangeId, is_uuid_v7);
id_type!(OperationId, is_uuid_v7);
id_type!(BlobId, is_sha256_id);
id_type!(RevisionId, is_sha256_id);
id_type!(ActorId, is_non_empty);
id_type!(AttachmentId, is_non_empty);

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "01890a5d-ac96-774b-bcce-b302099a8057";

    #[test]
    fn accepts_lowercase_uuid_v7() {
        assert!(is_uuid_v7(GOOD));
        assert!(is_uuid_v7("01890a5d-ac96-7001-8000-000000000001"));
        assert!(is_uuid_v7("01890a5d-ac96-7001-9000-000000000001"));
        assert!(is_uuid_v7("01890a5d-ac96-7001-a000-000000000001"));
    }

    #[test]
    fn rejects_malformed_uuids() {
        for bad in [
            "",
            "01890A5D-AC96-774B-BCCE-B302099A8057",
            "01890a5d-ac96-474b-bcce-b302099a8057",
            "01890a5d-ac96-774b-cce-b302099a80570",
            "01890a5d-ac96-774b-7cce-b302099a8057",
            "01890a5d-ac96-774b-cce0-b302099a8057",
            "01890a5dac96774bbcceb302099a8057",
            "01890a5d-ac96-774b-bcce-b302099a805",
            "01890a5d-ac96-774b-bcce-b302099a805g",
            "{01890a5d-ac96-774b-bcce-b302099a8057}",
        ] {
            assert!(!is_uuid_v7(bad), "{bad}");
        }
    }

    #[test]
    fn hash_ids_need_prefix_and_64_lowercase_hex() {
        let hex = "0".repeat(64);
        assert!(is_sha256_id(&format!("sha256:{hex}")));
        assert!(!is_sha256_id(&hex));
        assert!(!is_sha256_id(&format!("sha256:{}", "A".repeat(64))));
        assert!(!is_sha256_id(&format!("sha256:{}", "0".repeat(63))));
        assert!(!is_sha256_id(&format!("sha1:{hex}")));
    }

    #[test]
    fn parse_reports_the_field() {
        let err = DocumentId::parse("nope", "change.id").unwrap_err();
        assert_eq!(err.code(), "INVALID_ID");
        assert_eq!(err.context()["field"], "change.id");
        assert!(ActorId::parse("", "actor_id").is_err());
        assert!(ActorId::parse("é", "actor_id").is_ok());
    }
}
