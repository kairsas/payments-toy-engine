use async_trait::async_trait;
use cqrs_es::Aggregate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::domain::{
    props::TxType,
    transaction::{
        command::{RecordTransactionPayload, TransactionCommand},
        error::TransactionError,
        event::{TransactionEvent, TransactionRecordedPayload},
    },
};

// Aggregate
#[derive(Serialize, Default, Deserialize)]
pub struct Transaction {
    recorded: bool,
    pub tx_type: Option<TxType>,
    pub amount: Decimal,
}

// Interface to the outside world, not used in this case.
pub struct TransactionServices {}

#[async_trait]
impl Aggregate for Transaction {
    type Command = TransactionCommand;
    type Event = TransactionEvent;
    type Error = TransactionError;
    type Services = TransactionServices;

    fn aggregate_type() -> String {
        "Transaction".to_string()
    }

    async fn handle(
        &self,
        command: Self::Command,
        _services: &Self::Services,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            TransactionCommand::RecordTransaction(p) => self.record(p).await,
        }
    }

    fn apply(&mut self, event: Self::Event) {
        match event {
            TransactionEvent::TransactionRecorded(_) => {
                self.recorded = true;
            }
        }
    }
}

impl Transaction {
    async fn record(
        &self,
        p: RecordTransactionPayload,
    ) -> Result<Vec<<Transaction as Aggregate>::Event>, <Transaction as Aggregate>::Error> {
        debug!("Recording {} with {}", p.id, p.amount);

        require_new(self)?;

        Ok(vec![TransactionEvent::TransactionRecorded(
            TransactionRecordedPayload {
                id: p.id,
                client_id: p.client_id,
                amount: p.amount,
            },
        )])
    }
}

fn require_new(transaction: &Transaction) -> Result<(), <Transaction as Aggregate>::Error> {
    if transaction.recorded {
        return Err(TransactionError::DuplicateTransaction);
    }

    Ok(())
}

pub fn tx_aggregate_id(id: &str) -> String {
    format!("Transaction-{}", id)
}
