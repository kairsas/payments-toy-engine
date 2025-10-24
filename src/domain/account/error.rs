use derive_more::Display;

#[derive(Debug, PartialEq, Display)]
pub enum AccountError {
    InsufficientFunds,
    IllegalAmount,
    AccountLocked,
    DisputeNotFound,
    DuplicateDispute,
}

impl std::error::Error for AccountError {}
