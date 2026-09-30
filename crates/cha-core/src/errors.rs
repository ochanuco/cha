use serde_json::{json, Value};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevisionViolation {
    TombstoneHasContent,
    TombstoneHasAttachments,
    MissingContent,
    DuplicateAttachment,
}

impl RevisionViolation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TombstoneHasContent => "tombstone_has_content",
            Self::TombstoneHasAttachments => "tombstone_has_attachments",
            Self::MissingContent => "missing_content",
            Self::DuplicateAttachment => "duplicate_attachment",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalReason {
    NotCanonical,
    IdMismatch,
}

impl CanonicalReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotCanonical => "not_canonical",
            Self::IdMismatch => "id_mismatch",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationViolation {
    Empty,
    DuplicateDocument,
    DuplicateChange,
}

impl OperationViolation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::DuplicateDocument => "duplicate_document",
            Self::DuplicateChange => "duplicate_change",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChaError {
    InvalidJson,
    InvalidInput {
        field: String,
    },
    InvalidId {
        field: String,
    },
    InvalidParentSet {
        count: usize,
    },
    ChangeClosed {
        change_id: String,
    },
    InvalidRevision {
        reason: RevisionViolation,
    },
    InvalidCanonicalState {
        field: String,
        reason: CanonicalReason,
    },
    InvalidDocument {
        field: String,
    },
    ConflictState {
        change_id: String,
    },
    InvalidRestoreTarget,
    InvalidTypedTree {
        field: String,
    },
    InvalidOperation {
        reason: OperationViolation,
    },
    HashingError,
}

impl ChaError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidJson => "INVALID_JSON",
            Self::InvalidInput { .. } => "INVALID_INPUT",
            Self::InvalidId { .. } => "INVALID_ID",
            Self::InvalidParentSet { .. } => "INVALID_PARENT_SET",
            Self::ChangeClosed { .. } => "CHANGE_CLOSED",
            Self::InvalidRevision { .. } => "INVALID_REVISION",
            Self::InvalidCanonicalState { .. } => "INVALID_CANONICAL_STATE",
            Self::InvalidDocument { .. } => "INVALID_DOCUMENT",
            Self::ConflictState { .. } => "CONFLICT_STATE",
            Self::InvalidRestoreTarget => "INVALID_RESTORE_TARGET",
            Self::InvalidTypedTree { .. } => "INVALID_TYPED_TREE",
            Self::InvalidOperation { .. } => "INVALID_OPERATION",
            Self::HashingError => "HASHING_ERROR",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::InvalidJson => {
                "input is not JSON or contains a number that is not a safe integer".to_string()
            }
            Self::InvalidInput { field } => {
                format!("missing, unknown, or mistyped field `{field}`")
            }
            Self::InvalidId { field } => format!("malformed identifier in `{field}`"),
            Self::InvalidParentSet { count } => format!("invalid parent set of {count} entries"),
            Self::ChangeClosed { change_id } => format!("change {change_id} is closed"),
            Self::InvalidRevision { reason } => {
                format!("revision breaks an invariant: {}", reason.as_str())
            }
            Self::InvalidCanonicalState { field, reason } => {
                format!(
                    "stored revision in `{field}` is invalid: {}",
                    reason.as_str()
                )
            }
            Self::InvalidDocument { field } => {
                format!("`{field}` belongs to a different document")
            }
            Self::ConflictState { change_id } => {
                format!("conflict resolution reuses change {change_id}")
            }
            Self::InvalidRestoreTarget => "restore target is a tombstone".to_string(),
            Self::InvalidTypedTree { field } => format!("invalid typed tree node at `{field}`"),
            Self::InvalidOperation { reason } => {
                format!("invalid operation: {}", reason.as_str())
            }
            Self::HashingError => "hashing failed".to_string(),
        }
    }

    pub fn context(&self) -> Value {
        match self {
            Self::InvalidJson | Self::InvalidRestoreTarget | Self::HashingError => json!({}),
            Self::InvalidInput { field }
            | Self::InvalidId { field }
            | Self::InvalidDocument { field }
            | Self::InvalidTypedTree { field } => json!({ "field": field }),
            Self::InvalidParentSet { count } => json!({ "count": count }),
            Self::ChangeClosed { change_id } | Self::ConflictState { change_id } => {
                json!({ "change_id": change_id })
            }
            Self::InvalidRevision { reason } => json!({ "reason": reason.as_str() }),
            Self::InvalidCanonicalState { field, reason } => {
                json!({ "field": field, "reason": reason.as_str() })
            }
            Self::InvalidOperation { reason } => json!({ "reason": reason.as_str() }),
        }
    }
}

impl fmt::Display for ChaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code(), self.message())
    }
}

impl std::error::Error for ChaError {}
