use derive_more::Display;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use shrinkwraprs::Shrinkwrap;

#[derive(Shrinkwrap, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Display, Hash)]
pub struct ClientId(pub String);

#[derive(Shrinkwrap, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Display, Hash)]
pub struct TransactionId(pub String);

#[derive(Shrinkwrap, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Display, Hash)]
pub struct Amount(pub Decimal);

#[derive(Debug, Serialize, Deserialize, Display, PartialEq)]
pub enum TxType {
    Deposit,
    Withdrawal,
}
