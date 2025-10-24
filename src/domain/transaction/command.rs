use serde::Deserialize;

use crate::domain::props::{Amount, ClientId, TransactionId};

#[derive(Debug, Clone, Deserialize)]
pub enum TransactionCommand {
    RecordTransaction(RecordTransactionPayload),
}

#[derive(Debug, Clone, Deserialize)]
pub struct RecordTransactionPayload {
    pub id: TransactionId,
    pub client_id: ClientId,
    pub amount: Amount,
}
