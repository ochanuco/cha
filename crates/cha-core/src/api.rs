//! String-in, string-out entry points. Each returns canonical JSON.
//!
//! Every function follows the same phases so the first failure matches the
//! documented order: parse (`INVALID_JSON`), shape (`INVALID_INPUT`),
//! identifiers (`INVALID_ID`), change state, parent set, then domain rules.

use crate::blob;
use crate::canonical::to_canonical;
use crate::change::{Change, ChangeState};
use crate::diff;
use crate::errors::ChaError;
use crate::ids::{ActorId, AttachmentId, BlobId, ChangeId, DocumentId, OperationId, RevisionId};
use crate::input::{self, join, Object, Tagged};
use crate::operation::{ChangeRef, Operation};
use crate::revision::{verify_stored, Attachment, PreparedRevision, Revision, State};
use crate::typed_tree::Node;
use serde_json::{json, Value};
use std::collections::HashSet;

pub const ABI_VERSION: &str = "cha-abi/1";

pub fn abi_version() -> &'static str {
    ABI_VERSION
}

pub fn blob_id(bytes: &[u8]) -> String {
    blob::blob_id(bytes).as_str().to_string()
}

struct RawChange<'a> {
    id: Tagged<'a>,
    state: ChangeState,
}

struct RawAttachment<'a> {
    attachment_id: Tagged<'a>,
    blob_id: Tagged<'a>,
}

struct RawStored<'a> {
    revision_id: Tagged<'a>,
    canonical: &'a str,
    path: String,
}

struct RawState<'a> {
    tombstone: bool,
    host: &'a Object,
}

fn shape_change<'a>(root: &'a Object) -> Result<RawChange<'a>, ChaError> {
    let map = input::object(
        input::field(root, "", "change")?,
        "change",
        &["id", "state"],
    )?;
    let id = input::string(map, "change", "id")?;
    let state = match input::string(map, "change", "state")?.value {
        "open" => ChangeState::Open,
        "closed" => ChangeState::Closed,
        _ => {
            return Err(ChaError::InvalidInput {
                field: "change.state".to_string(),
            })
        }
    };
    Ok(RawChange { id, state })
}

fn shape_content<'a>(root: &'a Object) -> Result<Option<Tagged<'a>>, ChaError> {
    match input::field(root, "", "content_blob_id")? {
        Value::Null => Ok(None),
        Value::String(value) => Ok(Some(Tagged {
            value,
            field: "content_blob_id".to_string(),
        })),
        _ => Err(ChaError::InvalidInput {
            field: "content_blob_id".to_string(),
        }),
    }
}

fn shape_attachments<'a>(root: &'a Object) -> Result<Vec<RawAttachment<'a>>, ChaError> {
    input::array(root, "", "attachments")?
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let path = join("attachments", &i.to_string());
            let map = input::object(item, &path, &["attachment_id", "blob_id"])?;
            Ok(RawAttachment {
                attachment_id: input::string(map, &path, "attachment_id")?,
                blob_id: input::string(map, &path, "blob_id")?,
            })
        })
        .collect()
}

fn shape_state<'a>(root: &'a Object) -> Result<RawState<'a>, ChaError> {
    let map = input::object(
        input::field(root, "", "state")?,
        "state",
        &["tombstone", "host"],
    )?;
    Ok(RawState {
        tombstone: input::boolean(map, "state", "tombstone")?,
        host: input::object_field(map, "state", "host")?,
    })
}

fn shape_stored<'a>(value: &'a Value, path: String) -> Result<RawStored<'a>, ChaError> {
    let map = input::object(value, &path, &["revision_id", "canonical"])?;
    Ok(RawStored {
        revision_id: input::string(map, &path, "revision_id")?,
        canonical: input::string(map, &path, "canonical")?.value,
        path,
    })
}

fn document_id(raw: &Tagged) -> Result<DocumentId, ChaError> {
    DocumentId::parse(raw.value, &raw.field)
}

fn revision_ids(raw: &[Tagged]) -> Result<Vec<RevisionId>, ChaError> {
    raw.iter()
        .map(|t| RevisionId::parse(t.value, &t.field))
        .collect()
}

fn content_id(raw: Option<&Tagged>) -> Result<Option<BlobId>, ChaError> {
    raw.map(|t| BlobId::parse(t.value, &t.field)).transpose()
}

fn attachment_list(raw: &[RawAttachment]) -> Result<Vec<Attachment>, ChaError> {
    raw.iter()
        .map(|a| {
            Ok(Attachment {
                attachment_id: AttachmentId::parse(a.attachment_id.value, &a.attachment_id.field)?,
                blob_id: BlobId::parse(a.blob_id.value, &a.blob_id.field)?,
            })
        })
        .collect()
}

fn open_change(raw: &RawChange, document_id: DocumentId) -> Result<Change, ChaError> {
    Ok(Change::new(
        ChangeId::parse(raw.id.value, &raw.id.field)?,
        document_id,
        raw.state,
    ))
}

/// `count` must be within `min..=max` and every id distinct.
fn check_parent_set(ids: &[RevisionId], min: usize, max: usize) -> Result<(), ChaError> {
    let distinct: HashSet<&RevisionId> = ids.iter().collect();
    if ids.len() < min || ids.len() > max || distinct.len() != ids.len() {
        return Err(ChaError::InvalidParentSet { count: ids.len() });
    }
    Ok(())
}

fn respond(prepared: &PreparedRevision) -> String {
    to_canonical(&prepared.to_value())
}

pub fn prepare_revision(input: &str) -> Result<String, ChaError> {
    let value = input::parse(input)?;
    let root = input::object(
        &value,
        "",
        &[
            "document_id",
            "change",
            "parents",
            "content_blob_id",
            "attachments",
            "state",
        ],
    )?;
    let raw_document = input::string(root, "", "document_id")?;
    let raw_change = shape_change(root)?;
    let raw_parents = input::strings(root, "", "parents")?;
    let raw_content = shape_content(root)?;
    let raw_attachments = shape_attachments(root)?;
    let raw_state = shape_state(root)?;

    let document_id = document_id(&raw_document)?;
    let change = open_change(&raw_change, document_id.clone())?;
    let parents = revision_ids(&raw_parents)?;
    let content = content_id(raw_content.as_ref())?;
    let attachments = attachment_list(&raw_attachments)?;

    change.ensure_open()?;
    check_parent_set(&parents, 0, 1)?;

    let revision = Revision::build(
        document_id,
        change.id,
        parents,
        content,
        attachments,
        State {
            tombstone: raw_state.tombstone,
            host: raw_state.host.clone(),
        },
    )?;
    Ok(respond(&revision.prepare()))
}

pub fn prepare_restore(input: &str) -> Result<String, ChaError> {
    let value = input::parse(input)?;
    let root = input::object(
        &value,
        "",
        &["document_id", "change", "parents", "target", "host"],
    )?;
    let raw_document = input::string(root, "", "document_id")?;
    let raw_change = shape_change(root)?;
    let raw_parents = input::strings(root, "", "parents")?;
    let raw_target = shape_stored(input::field(root, "", "target")?, "target".to_string())?;
    let host = match root.get("host") {
        None => None,
        Some(v) => Some(v.as_object().ok_or_else(|| ChaError::InvalidInput {
            field: "host".to_string(),
        })?),
    };

    let document_id = document_id(&raw_document)?;
    let change = open_change(&raw_change, document_id.clone())?;
    let parents = revision_ids(&raw_parents)?;
    let target_id = RevisionId::parse(raw_target.revision_id.value, &raw_target.revision_id.field)?;

    change.ensure_open()?;
    check_parent_set(&parents, 1, usize::MAX)?;

    let target = verify_stored(&raw_target.path, &target_id, raw_target.canonical)?;
    if target.document_id != document_id {
        return Err(ChaError::InvalidDocument {
            field: raw_target.path,
        });
    }
    let revision = Revision::restore(change.id, parents, &target, host.cloned())?;
    Ok(respond(&revision.prepare()))
}

pub fn prepare_conflict_resolution(input: &str) -> Result<String, ChaError> {
    let value = input::parse(input)?;
    let root = input::object(
        &value,
        "",
        &[
            "document_id",
            "change",
            "heads",
            "content_blob_id",
            "attachments",
            "state",
        ],
    )?;
    let raw_document = input::string(root, "", "document_id")?;
    let raw_change = shape_change(root)?;
    let raw_heads = input::array(root, "", "heads")?
        .iter()
        .enumerate()
        .map(|(i, item)| shape_stored(item, join("heads", &i.to_string())))
        .collect::<Result<Vec<_>, _>>()?;
    let raw_content = shape_content(root)?;
    let raw_attachments = shape_attachments(root)?;
    let raw_state = shape_state(root)?;

    let document_id = document_id(&raw_document)?;
    let change = open_change(&raw_change, document_id.clone())?;
    let head_ids = raw_heads
        .iter()
        .map(|h| RevisionId::parse(h.revision_id.value, &h.revision_id.field))
        .collect::<Result<Vec<_>, _>>()?;
    let content = content_id(raw_content.as_ref())?;
    let attachments = attachment_list(&raw_attachments)?;

    change.ensure_open()?;
    check_parent_set(&head_ids, 2, usize::MAX)?;

    let mut heads = Vec::with_capacity(raw_heads.len());
    for (raw, id) in raw_heads.iter().zip(&head_ids) {
        let head = verify_stored(&raw.path, id, raw.canonical)?;
        if head.document_id != document_id {
            return Err(ChaError::InvalidDocument {
                field: raw.path.clone(),
            });
        }
        heads.push(head);
    }
    if heads.iter().any(|head| head.change_id == change.id) {
        return Err(ChaError::ConflictState {
            change_id: change.id.as_str().to_string(),
        });
    }

    let revision = Revision::build(
        document_id,
        change.id,
        head_ids,
        content,
        attachments,
        State {
            tombstone: raw_state.tombstone,
            host: raw_state.host.clone(),
        },
    )?;
    Ok(respond(&revision.prepare()))
}

pub fn semantic_diff(input: &str) -> Result<String, ChaError> {
    let value = input::parse(input)?;
    let root = input::object(&value, "", &["old", "new"])?;
    let old = Node::from_value(input::field(root, "", "old")?, "old")?;
    let new = Node::from_value(input::field(root, "", "new")?, "new")?;
    old.validate("old")?;
    new.validate("new")?;

    let events: Vec<Value> = diff::diff(&old, &new)
        .iter()
        .map(diff::Event::to_value)
        .collect();
    Ok(to_canonical(&json!({ "events": events })))
}

pub fn prepare_operation(input: &str) -> Result<String, ChaError> {
    let value = input::parse(input)?;
    let root = input::object(&value, "", &["operation_id", "actor_id", "changes"])?;
    let raw_operation = input::string(root, "", "operation_id")?;
    let raw_actor = input::string(root, "", "actor_id")?;
    let raw_changes = input::array(root, "", "changes")?
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let path = join("changes", &i.to_string());
            let map = input::object(item, &path, &["document_id", "change_id"])?;
            Ok((
                input::string(map, &path, "document_id")?,
                input::string(map, &path, "change_id")?,
            ))
        })
        .collect::<Result<Vec<_>, ChaError>>()?;

    let operation_id = OperationId::parse(raw_operation.value, &raw_operation.field)?;
    let actor_id = ActorId::parse(raw_actor.value, &raw_actor.field)?;
    let changes = raw_changes
        .iter()
        .map(|(doc, change)| {
            Ok(ChangeRef {
                document_id: DocumentId::parse(doc.value, &doc.field)?,
                change_id: ChangeId::parse(change.value, &change.field)?,
            })
        })
        .collect::<Result<Vec<_>, ChaError>>()?;

    let operation = Operation::new(operation_id, actor_id, changes)?;
    Ok(to_canonical(&json!({
        "operation": operation.to_value(),
        "multi_document": operation.is_multi_document(),
    })))
}
