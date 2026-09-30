use crate::errors::ChaError;
use crate::ids::{ChangeId, DocumentId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeState {
    Open,
    Closed,
}

/// One semantic editing session for one document. The state and description
/// are host bookkeeping and never enter a revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub id: ChangeId,
    pub document_id: DocumentId,
    pub state: ChangeState,
    pub description: Option<String>,
}

impl Change {
    pub fn new(id: ChangeId, document_id: DocumentId, state: ChangeState) -> Self {
        Self {
            id,
            document_id,
            state,
            description: None,
        }
    }

    pub fn close(&mut self) {
        self.state = ChangeState::Closed;
    }

    pub fn ensure_open(&self) -> Result<(), ChaError> {
        match self.state {
            ChangeState::Open => Ok(()),
            ChangeState::Closed => Err(ChaError::ChangeClosed {
                change_id: self.id.as_str().to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change() -> Change {
        Change::new(
            ChangeId::parse("01890a5d-ac96-7001-8000-000000000001", "c").unwrap(),
            DocumentId::parse("01890a5d-ac96-7001-8000-000000000002", "d").unwrap(),
            ChangeState::Open,
        )
    }

    #[test]
    fn open_change_accepts_revisions_until_closed() {
        let mut c = change();
        assert!(c.ensure_open().is_ok());
        c.close();
        let err = c.ensure_open().unwrap_err();
        assert_eq!(err.code(), "CHANGE_CLOSED");
        assert_eq!(err.context()["change_id"], c.id.as_str());
    }
}
