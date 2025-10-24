use cqrs_es::DomainEvent;
use serde::{Deserialize, Serialize};

use crate::domain::props::{Amount, ClientId, TransactionId};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionEvent {
    TransactionRecorded(TransactionRecordedPayload),
}

impl DomainEvent for TransactionEvent {
    fn event_type(&self) -> String {
        let event_type: &str = match self {
            TransactionEvent::TransactionRecorded(_) => "TransactionRecorded",
        };
        event_type.to_string()
    }

    fn event_version(&self) -> String {
        "1.0".to_string()
    }
}

// This implementation is naive for the purpose of exercise.
// In real world scenario transaction event should include 2 accounts - debit & credit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransactionRecordedPayload {
    pub id: TransactionId,
    pub client_id: ClientId,
    pub amount: Amount,
}
