use crate::errors::{ChaError, OperationViolation};
use crate::ids::{ActorId, ChangeId, DocumentId, OperationId};
use serde_json::{json, Value};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeRef {
    pub document_id: DocumentId,
    pub change_id: ChangeId,
}

/// Groups document-local changes. It is the only place an ActorId appears and
/// it never feeds a RevisionId.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    pub operation_id: OperationId,
    pub actor_id: ActorId,
    pub changes: Vec<ChangeRef>,
}

impl Operation {
    pub fn new(
        operation_id: OperationId,
        actor_id: ActorId,
        mut changes: Vec<ChangeRef>,
    ) -> Result<Self, ChaError> {
        let violation = |reason| Err(ChaError::InvalidOperation { reason });
        if changes.is_empty() {
            return violation(OperationViolation::Empty);
        }
        let mut documents = HashSet::new();
        if !changes.iter().all(|c| documents.insert(&c.document_id)) {
            return violation(OperationViolation::DuplicateDocument);
        }
        let mut change_ids = HashSet::new();
        if !changes.iter().all(|c| change_ids.insert(&c.change_id)) {
            return violation(OperationViolation::DuplicateChange);
        }
        changes.sort_by(|a, b| a.document_id.cmp(&b.document_id));
        Ok(Self {
            operation_id,
            actor_id,
            changes,
        })
    }

    pub fn is_multi_document(&self) -> bool {
        self.changes.len() > 1
    }

    pub fn to_value(&self) -> Value {
        let changes: Vec<Value> = self
            .changes
            .iter()
            .map(|c| {
                json!({
                    "document_id": c.document_id.as_str(),
                    "change_id": c.change_id.as_str(),
                })
            })
            .collect();
        json!({
            "operation_id": self.operation_id.as_str(),
            "actor_id": self.actor_id.as_str(),
            "changes": changes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uuid(n: u32) -> String {
        format!("01890a5d-ac96-7001-8000-{n:012x}")
    }

    fn change(doc: u32, change: u32) -> ChangeRef {
        ChangeRef {
            document_id: DocumentId::parse(&uuid(doc), "d").unwrap(),
            change_id: ChangeId::parse(&uuid(change), "c").unwrap(),
        }
    }

    fn operation(changes: Vec<ChangeRef>) -> Result<Operation, ChaError> {
        Operation::new(
            OperationId::parse(&uuid(99), "o").unwrap(),
            ActorId::parse("actor", "a").unwrap(),
            changes,
        )
    }

    #[test]
    fn sorts_by_document_and_flags_multi_document() {
        let op = operation(vec![change(3, 13), change(1, 11)]).unwrap();
        assert_eq!(op.changes[0], change(1, 11));
        assert!(op.is_multi_document());
        assert!(!operation(vec![change(1, 11)]).unwrap().is_multi_document());
    }

    #[test]
    fn rejects_empty_and_duplicates() {
        let reason = |r: Result<Operation, ChaError>| r.unwrap_err().context()["reason"].clone();
        assert_eq!(reason(operation(vec![])), "empty");
        assert_eq!(
            reason(operation(vec![change(1, 11), change(1, 12)])),
            "duplicate_document"
        );
        assert_eq!(
            reason(operation(vec![change(1, 11), change(2, 11)])),
            "duplicate_change"
        );
    }
}
