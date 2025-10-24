use cqrs_es::DomainEvent;
use serde::{Deserialize, Serialize};

use crate::domain::props::{Amount, ClientId, TransactionId};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AccountEvent {
    AccountDeposited(AccountDepositedPayload),
    AccountWithdrawn(AccountWithdrawnPayload),
    FundsDisputed(FundsDisputedPayload),
    DisputeResolved(DisputeResolvedPayload),
    DisputeChargedback(DisputeChargedbackPayload),
}

impl DomainEvent for AccountEvent {
    fn event_type(&self) -> String {
        let event_type: &str = match self {
            AccountEvent::AccountDeposited(_) => "AccountDeposited",
            AccountEvent::AccountWithdrawn(_) => "AccountWithdrawn",
            AccountEvent::FundsDisputed(_) => "FundsDisputed",
            AccountEvent::DisputeResolved(_) => "DisputeResolved",
            AccountEvent::DisputeChargedback(_) => "DisputeChargedback",
        };
        event_type.to_string()
    }

    fn event_version(&self) -> String {
        "1.0".to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AccountDepositedPayload {
    pub client_id: ClientId,
    pub transaction_id: TransactionId,
    pub amount: Amount,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AccountWithdrawnPayload {
    pub client_id: ClientId,
    pub transaction_id: TransactionId,
    pub amount: Amount,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FundsDisputedPayload {
    pub client_id: ClientId,
    pub transaction_id: TransactionId,
    pub amount: Amount,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DisputeResolvedPayload {
    pub client_id: ClientId,
    pub transaction_id: TransactionId,
    pub amount: Amount,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DisputeChargedbackPayload {
    pub client_id: ClientId,
    pub transaction_id: TransactionId,
    pub amount: Amount,
}
