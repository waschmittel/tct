//! Identifiers for Boards, Lists, Cards and Labels.
//!
//! There is one type per entity rather than one shared id type, because the
//! four are not interchangeable:
//!
//! * [`BoardId`] and [`CardId`] are *locations* — a Board's id is the directory
//!   holding its `board.json`, a Card's id is the `<id>` in `card-<id>.json`
//!   (ADR-0007). `storage::paths` accepts only these two, so a path cannot be
//!   built from an id that names nothing on disk.
//! * [`ListId`] and [`LabelId`] are *references* — Lists and Labels live inline
//!   in `board.json` and have no file, so their ids are pointers into it
//!   (`card.list_id`, `card.label_ids`).
//!
//! All four share one charset: ASCII alphanumerics, `-` and `_`, at most
//! [`MAX_LEN`]. Ids arrive from JSON and from directory entries that the
//! process does not control (a checked-out repository may carry its own `.tct`),
//! and they become path components, so the charset is enforced at the parse
//! boundary — a value containing `/`, `\` or `..` cannot exist.

use std::borrow::Borrow;
use std::fmt;
use std::ops::Deref;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize};

/// Longest accepted id. Generated ids are 8 chars; the cap only bounds what a
/// foreign file or a directory name may declare.
pub const MAX_LEN: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid {kind} id {value:?}: {reason}")]
pub struct InvalidId {
    pub kind: &'static str,
    pub value: String,
    pub reason: &'static str,
}

fn validate(kind: &'static str, value: &str) -> Result<(), InvalidId> {
    let invalid = |reason| InvalidId { kind, value: value.to_string(), reason };
    if value.len() > MAX_LEN {
        return Err(invalid("longer than 64 characters"));
    }
    if !value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(invalid("only ASCII letters, digits, '-' and '_' are allowed"));
    }
    Ok(())
}

fn random_id() -> String {
    uuid::Uuid::new_v4().to_string()[..8].to_string()
}

macro_rules! define_id {
    ($name:ident, $kind:literal, $doc:literal) => {
        #[doc = $doc]
        ///
        /// The empty value is the "unset" default. It is inert as a path
        /// component but does not name one — see `storage::paths::board_dir`.
        #[derive(Debug, Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn generate() -> Self {
                Self(random_id())
            }

            /// Validate an untrusted string into an id.
            pub fn parse(value: &str) -> Result<Self, InvalidId> {
                validate($kind, value)?;
                Ok(Self(value.to_string()))
            }

            /// Each id type gets the same surface; only the two that name
            /// paths currently need this one.
            #[allow(dead_code)]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let raw = String::deserialize(deserializer)?;
                Self::parse(&raw).map_err(serde::de::Error::custom)
            }
        }

        impl FromStr for $name {
            type Err = InvalidId;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::parse(s)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl Deref for $name {
            type Target = str;

            fn deref(&self) -> &str {
                &self.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        /// Lets `HashMap<$name, _>` be probed with a `&str`.
        impl Borrow<str> for $name {
            fn borrow(&self) -> &str {
                &self.0
            }
        }

        impl PartialEq<str> for $name {
            fn eq(&self, other: &str) -> bool {
                self.0 == other
            }
        }

        impl PartialEq<&str> for $name {
            fn eq(&self, other: &&str) -> bool {
                self.0 == *other
            }
        }

        /// Fixtures spell ids as literals. A malformed literal is a bug in the
        /// test, not input to validate, so this panics — and it stays test-only
        /// so no production path can turn an arbitrary string into an id.
        #[cfg(test)]
        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self::parse(value).expect(concat!("test fixture ", $kind, " id is not valid"))
            }
        }
    };
}

define_id!(BoardId, "board", "Identifies a Board — and names the directory its `board.json` lives in.");
define_id!(CardId, "card", "Identifies a Card — and names its `card-<id>.json` file.");
define_id!(ListId, "list", "Identifies a List within its Board's inline `lists`.");
define_id!(LabelId, "label", "Identifies a Label within its Board's inline `labels`.");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_ids_are_8_chars_and_unique() {
        let a = CardId::generate();
        let b = CardId::generate();
        assert_eq!(a.len(), 8);
        assert_ne!(a, b);
    }

    #[test]
    fn generated_ids_round_trip_through_parse() {
        let id = BoardId::generate();
        assert_eq!(BoardId::parse(&id).unwrap(), id);
    }

    #[test]
    fn parse_accepts_id_charset() {
        for ok in ["abc123", "with-dash", "with_underscore", "A1", "", &"x".repeat(MAX_LEN)] {
            assert!(CardId::parse(ok).is_ok(), "should accept {ok:?}");
        }
    }

    #[test]
    fn parse_rejects_path_traversal() {
        for bad in [
            "..",
            "../../etc",
            "x/../../../tmp/evil",
            "/etc/passwd",
            r"..\windows",
            "a/b",
            "with space",
            "with.dot",
            "nul\0byte",
            "emoji🙂",
        ] {
            assert!(BoardId::parse(bad).is_err(), "should reject {bad:?}");
            assert!(CardId::parse(bad).is_err(), "should reject {bad:?}");
            assert!(ListId::parse(bad).is_err(), "should reject {bad:?}");
            assert!(LabelId::parse(bad).is_err(), "should reject {bad:?}");
        }
    }

    #[test]
    fn parse_rejects_overlong_id() {
        assert!(CardId::parse(&"x".repeat(MAX_LEN + 1)).is_err());
    }

    #[test]
    fn deserialize_rejects_traversal_and_names_the_kind() {
        let err = serde_json::from_str::<BoardId>("\"../../evil\"").unwrap_err();
        assert!(err.to_string().contains("invalid board id"), "{err}");
        let err = serde_json::from_str::<LabelId>("\"../../evil\"").unwrap_err();
        assert!(err.to_string().contains("invalid label id"), "{err}");
    }

    #[test]
    fn deserialize_rejects_non_string() {
        assert!(serde_json::from_str::<CardId>("42").is_err());
    }

    #[test]
    fn serde_round_trip_is_transparent() {
        let id = ListId::generate();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, format!("\"{id}\""));
        assert_eq!(serde_json::from_str::<ListId>(&json).unwrap(), id);
    }
}
