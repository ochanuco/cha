use crate::blob::sha256_hex;
use crate::canonical::{cmp_utf16, parse_strict, to_canonical};
use crate::errors::{CanonicalReason, ChaError, RevisionViolation};
use crate::ids::{AttachmentId, BlobId, ChangeId, DocumentId, RevisionId};
use serde_json::{json, Map, Value};

pub const REVISION_SCHEMA: &str = "cha.revision/1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    pub attachment_id: AttachmentId,
    pub blob_id: BlobId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct State {
    pub tombstone: bool,
    pub host: Map<String, Value>,
}

/// Immutable logical snapshot. Only the fields that define identity live here.
#[derive(Debug, Clone, PartialEq)]
pub struct Revision {
    pub document_id: DocumentId,
    pub change_id: ChangeId,
    pub parents: Vec<RevisionId>,
    pub content_blob_id: Option<BlobId>,
    pub attachments: Vec<Attachment>,
    pub state: State,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedRevision {
    pub revision_id: RevisionId,
    pub canonical: String,
    pub revision: Value,
}

impl PreparedRevision {
    pub fn to_value(&self) -> Value {
        json!({
            "revision_id": self.revision_id.as_str(),
            "canonical": self.canonical,
            "revision": self.revision,
        })
    }
}

impl Revision {
    /// Enforces the tombstone/content and attachment invariants and puts the
    /// unordered collections into canonical order.
    pub fn build(
        document_id: DocumentId,
        change_id: ChangeId,
        mut parents: Vec<RevisionId>,
        content_blob_id: Option<BlobId>,
        mut attachments: Vec<Attachment>,
        state: State,
    ) -> Result<Self, ChaError> {
        let violation = |reason| Err(ChaError::InvalidRevision { reason });
        if state.tombstone {
            if content_blob_id.is_some() {
                return violation(RevisionViolation::TombstoneHasContent);
            }
            if !attachments.is_empty() {
                return violation(RevisionViolation::TombstoneHasAttachments);
            }
        } else if content_blob_id.is_none() {
            return violation(RevisionViolation::MissingContent);
        }
        attachments.sort_by(|a, b| cmp_utf16(a.attachment_id.as_str(), b.attachment_id.as_str()));
        if attachments
            .windows(2)
            .any(|w| w[0].attachment_id == w[1].attachment_id)
        {
            return violation(RevisionViolation::DuplicateAttachment);
        }
        parents.sort();
        Ok(Self {
            document_id,
            change_id,
            parents,
            content_blob_id,
            attachments,
            state,
        })
    }

    pub fn to_value(&self) -> Value {
        let attachments: Vec<Value> = self
            .attachments
            .iter()
            .map(|a| {
                json!({
                    "attachment_id": a.attachment_id.as_str(),
                    "blob_id": a.blob_id.as_str(),
                })
            })
            .collect();
        let parents: Vec<&str> = self.parents.iter().map(RevisionId::as_str).collect();
        json!({
            "schema": REVISION_SCHEMA,
            "document_id": self.document_id.as_str(),
            "change_id": self.change_id.as_str(),
            "parents": parents,
            "content_blob_id": self.content_blob_id.as_ref().map(BlobId::as_str),
            "attachments": attachments,
            "state": {
                "tombstone": self.state.tombstone,
                "host": Value::Object(self.state.host.clone()),
            },
        })
    }

    pub fn canonical(&self) -> String {
        to_canonical(&self.to_value())
    }

    pub fn prepare(&self) -> PreparedRevision {
        let canonical = self.canonical();
        PreparedRevision {
            revision_id: hash_canonical(&canonical),
            revision: self.to_value(),
            canonical,
        }
    }

    /// A forward revision that takes content and attachments from `target`.
    /// The document check on `target` belongs to the caller; a tombstone
    /// target is rejected here.
    pub fn restore(
        change_id: ChangeId,
        parents: Vec<RevisionId>,
        target: &Revision,
        host: Option<Map<String, Value>>,
    ) -> Result<Self, ChaError> {
        if target.state.tombstone {
            return Err(ChaError::InvalidRestoreTarget);
        }
        Self::build(
            target.document_id.clone(),
            change_id,
            parents,
            target.content_blob_id.clone(),
            target.attachments.clone(),
            State {
                tombstone: false,
                host: host.unwrap_or_else(|| target.state.host.clone()),
            },
        )
    }

    /// Parses a stored canonical revision. Anything that would not be
    /// produced by `canonical()` yields `None`.
    pub fn from_canonical(canonical: &str) -> Option<Self> {
        let value = parse_strict(canonical)?;
        let root = exact_object(
            &value,
            &[
                "schema",
                "document_id",
                "change_id",
                "parents",
                "content_blob_id",
                "attachments",
                "state",
            ],
        )?;
        if root["schema"] != REVISION_SCHEMA {
            return None;
        }
        let document_id = DocumentId::parse(root["document_id"].as_str()?, "").ok()?;
        let change_id = ChangeId::parse(root["change_id"].as_str()?, "").ok()?;
        let parents = root["parents"]
            .as_array()?
            .iter()
            .map(|p| RevisionId::parse(p.as_str()?, "").ok())
            .collect::<Option<Vec<_>>>()?;
        if parents.windows(2).any(|w| w[0] >= w[1]) {
            return None;
        }
        let content_blob_id = match &root["content_blob_id"] {
            Value::Null => None,
            v => Some(BlobId::parse(v.as_str()?, "").ok()?),
        };
        let attachments = root["attachments"]
            .as_array()?
            .iter()
            .map(|a| {
                let a = exact_object(a, &["attachment_id", "blob_id"])?;
                Some(Attachment {
                    attachment_id: AttachmentId::parse(a["attachment_id"].as_str()?, "").ok()?,
                    blob_id: BlobId::parse(a["blob_id"].as_str()?, "").ok()?,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        let state = exact_object(&root["state"], &["tombstone", "host"])?;
        let state = State {
            tombstone: state["tombstone"].as_bool()?,
            host: state["host"].as_object()?.clone(),
        };
        let revision = Self::build(
            document_id,
            change_id,
            parents,
            content_blob_id,
            attachments,
            state,
        )
        .ok()?;
        (revision.canonical() == canonical).then_some(revision)
    }
}

fn exact_object<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Map<String, Value>> {
    let map = value.as_object()?;
    (map.len() == keys.len() && keys.iter().all(|k| map.contains_key(*k))).then_some(map)
}

pub fn hash_canonical(canonical: &str) -> RevisionId {
    RevisionId::parse(&format!("sha256:{}", sha256_hex(canonical.as_bytes())), "")
        .expect("sha256_hex yields 64 lowercase hex digits")
}

/// A revision as persisted by the host. `field` is the dotted input path of
/// the stored revision, used for error contexts.
pub fn verify_stored(
    field: &str,
    revision_id: &RevisionId,
    canonical: &str,
) -> Result<Revision, ChaError> {
    let error = |reason| ChaError::InvalidCanonicalState {
        field: format!("{field}.canonical"),
        reason,
    };
    let revision =
        Revision::from_canonical(canonical).ok_or_else(|| error(CanonicalReason::NotCanonical))?;
    if &hash_canonical(canonical) != revision_id {
        return Err(error(CanonicalReason::IdMismatch));
    }
    Ok(revision)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uuid(n: u32) -> String {
        format!("01890a5d-ac96-7001-8000-{n:012x}")
    }

    fn blob(c: char) -> BlobId {
        BlobId::parse(&format!("sha256:{}", c.to_string().repeat(64)), "b").unwrap()
    }

    fn attachment(id: &str, c: char) -> Attachment {
        Attachment {
            attachment_id: AttachmentId::parse(id, "a").unwrap(),
            blob_id: blob(c),
        }
    }

    fn revision(
        attachments: Vec<Attachment>,
        tombstone: bool,
        content: Option<BlobId>,
    ) -> Result<Revision, ChaError> {
        Revision::build(
            DocumentId::parse(&uuid(1), "d").unwrap(),
            ChangeId::parse(&uuid(2), "c").unwrap(),
            vec![],
            content,
            attachments,
            State {
                tombstone,
                host: Map::new(),
            },
        )
    }

    fn reason(r: Result<Revision, ChaError>) -> Value {
        r.unwrap_err().context()["reason"].clone()
    }

    #[test]
    fn invariants() {
        assert_eq!(
            reason(revision(vec![], true, Some(blob('a')))),
            "tombstone_has_content"
        );
        assert_eq!(
            reason(revision(vec![attachment("x", 'a')], true, None)),
            "tombstone_has_attachments"
        );
        assert_eq!(reason(revision(vec![], false, None)), "missing_content");
        assert_eq!(
            reason(revision(
                vec![attachment("x", 'a'), attachment("x", 'b')],
                false,
                Some(blob('c'))
            )),
            "duplicate_attachment"
        );
    }

    #[test]
    fn attachments_sort_by_utf16() {
        let r = revision(
            vec![
                attachment("\u{ff5e}", 'a'),
                attachment("\u{1f600}", 'b'),
                attachment("a", 'c'),
            ],
            false,
            Some(blob('d')),
        )
        .unwrap();
        let ids: Vec<&str> = r
            .attachments
            .iter()
            .map(|a| a.attachment_id.as_str())
            .collect();
        assert_eq!(ids, ["a", "\u{1f600}", "\u{ff5e}"]);
    }

    #[test]
    fn stored_revisions_round_trip_and_reject_tampering() {
        let r = revision(vec![attachment("x", 'a')], false, Some(blob('c'))).unwrap();
        let prepared = r.prepare();
        assert_eq!(
            verify_stored("target", &prepared.revision_id, &prepared.canonical).unwrap(),
            r
        );

        let spaced = prepared.canonical.replace(',', ", ");
        let err = verify_stored("target", &hash_canonical(&spaced), &spaced).unwrap_err();
        assert_eq!(err.context()["reason"], "not_canonical");
        assert_eq!(err.context()["field"], "target.canonical");

        let other = revision(vec![], false, Some(blob('e'))).unwrap().prepare();
        let err = verify_stored("target", &other.revision_id, &prepared.canonical).unwrap_err();
        assert_eq!(err.context()["reason"], "id_mismatch");
    }

    #[test]
    fn restore_rejects_tombstone_targets() {
        let tomb = revision(vec![], true, None).unwrap();
        let err = Revision::restore(ChangeId::parse(&uuid(3), "c").unwrap(), vec![], &tomb, None)
            .unwrap_err();
        assert_eq!(err.code(), "INVALID_RESTORE_TARGET");
    }
}
