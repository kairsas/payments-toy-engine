use serde::Deserialize;

use crate::domain::props::{Amount, ClientId, TransactionId};

#[derive(Debug, Clone, Deserialize)]
pub enum AccountCommand {
    DepositAccount(DepositAccountPayload),
    WithdrawAccount(WithdrawAccountPayload),
    DisputeFunds(DisputeFundsPayload),
    ResolveDispute(ResolveDisputePayload),
    ChargebackDispute(ChargebackDisputePayload),
}

#[derive(Debug, Clone, Deserialize)]
pub struct DepositAccountPayload {
    pub client_id: ClientId,
    pub transaction_id: TransactionId,
    pub amount: Amount,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WithdrawAccountPayload {
    pub client_id: ClientId,
    pub transaction_id: TransactionId,
    pub amount: Amount,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DisputeFundsPayload {
    pub client_id: ClientId,
    pub transaction_id: TransactionId,
    pub amount: Amount,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResolveDisputePayload {
    pub client_id: ClientId,
    pub transaction_id: TransactionId,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChargebackDisputePayload {
    pub client_id: ClientId,
    pub transaction_id: TransactionId,
}
