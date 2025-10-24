use derive_more::Display;

#[derive(Debug, PartialEq, Display)]
pub enum TransactionError {
    DuplicateTransaction,
}

impl std::error::Error for TransactionError {}
