use std::collections::HashMap;

use async_trait::async_trait;
use cqrs_es::Aggregate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::domain::{
    account::{
        command::{
            AccountCommand, ChargebackDisputePayload, DepositAccountPayload, DisputeFundsPayload,
            ResolveDisputePayload, WithdrawAccountPayload,
        },
        error::AccountError,
        event::{
            AccountDepositedPayload, AccountEvent, AccountWithdrawnPayload,
            DisputeChargedbackPayload, DisputeResolvedPayload, FundsDisputedPayload,
        },
    },
    props::{Amount, TransactionId},
};

// Aggregate
#[derive(Serialize, Default, Deserialize)]
pub struct Account {
    locked: bool,
    funds_available: Decimal,
    funds_held: Decimal,
    disputes: HashMap<TransactionId, Decimal>,
}

// Interface to the outside world, not used in this case.
pub struct AccountServices {}

#[async_trait]
impl Aggregate for Account {
    type Command = AccountCommand;
    type Event = AccountEvent;
    type Error = AccountError;
    type Services = AccountServices;

    fn aggregate_type() -> String {
        "Account".to_string()
    }

    async fn handle(
        &self,
        command: Self::Command,
        _services: &Self::Services,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            AccountCommand::DepositAccount(p) => self.deposit(p).await,
            AccountCommand::WithdrawAccount(p) => self.withdraw(p).await,
            AccountCommand::DisputeFunds(p) => self.dispute(p).await,
            AccountCommand::ResolveDispute(p) => self.resolve_dispute(p).await,
            AccountCommand::ChargebackDispute(p) => self.chargeback_dispute(p).await,
        }
    }

    fn apply(&mut self, event: Self::Event) {
        match event {
            AccountEvent::AccountDeposited(p) => {
                self.funds_available += *p.amount;
            }
            AccountEvent::AccountWithdrawn(p) => {
                self.funds_available -= *p.amount;
            }
            AccountEvent::FundsDisputed(p) => {
                self.disputes.insert(p.transaction_id, *p.amount);
                self.funds_available -= *p.amount;
                self.funds_held += *p.amount;
            }
            AccountEvent::DisputeResolved(p) => {
                self.disputes.remove(&p.transaction_id);
                self.funds_available += *p.amount;
                self.funds_held -= *p.amount;
            }
            AccountEvent::DisputeChargedback(p) => {
                self.locked = true;
                self.funds_held -= *p.amount;
            }
        }
    }
}

impl Account {
    async fn deposit(
        &self,
        p: DepositAccountPayload,
    ) -> Result<Vec<<Account as Aggregate>::Event>, <Account as Aggregate>::Error> {
        debug!("Depositing {} with {}", p.client_id, p.amount);

        require_legal_amount(&p.amount)?;
        require_active_account(self)?;

        Ok(vec![AccountEvent::AccountDeposited(
            AccountDepositedPayload {
                client_id: p.client_id,
                transaction_id: p.transaction_id,
                amount: p.amount,
            },
        )])
    }

    async fn withdraw(
        &self,
        p: WithdrawAccountPayload,
    ) -> Result<Vec<<Account as Aggregate>::Event>, <Account as Aggregate>::Error> {
        debug!("Withdrawing {} from {}", p.amount, p.client_id);

        require_legal_amount(&p.amount)?;
        require_active_account(self)?;
        require_sufficient_funds(self, &p.amount)?;

        Ok(vec![AccountEvent::AccountWithdrawn(
            AccountWithdrawnPayload {
                client_id: p.client_id,
                transaction_id: p.transaction_id,
                amount: p.amount,
            },
        )])
    }

    async fn dispute(
        &self,
        p: DisputeFundsPayload,
    ) -> Result<Vec<<Account as Aggregate>::Event>, <Account as Aggregate>::Error> {
        debug!("Disputing {} from {}", p.amount, p.client_id);

        require_legal_amount(&p.amount)?;
        require_active_account(self)?;
        require_no_active_dispute(self, &p.transaction_id)?;
        require_sufficient_funds(self, &p.amount)?;

        Ok(vec![AccountEvent::FundsDisputed(FundsDisputedPayload {
            client_id: p.client_id,
            transaction_id: p.transaction_id,
            amount: p.amount,
        })])
    }

    async fn resolve_dispute(
        &self,
        p: ResolveDisputePayload,
    ) -> Result<Vec<<Account as Aggregate>::Event>, <Account as Aggregate>::Error> {
        debug!(
            "Resolving dispute for {} from {}",
            p.transaction_id, p.client_id
        );

        require_active_account(self)?;

        let dispute = require_dispute(self, &p.transaction_id)?;

        Ok(vec![AccountEvent::DisputeResolved(
            DisputeResolvedPayload {
                client_id: p.client_id,
                transaction_id: p.transaction_id,
                amount: Amount(dispute),
            },
        )])
    }

    async fn chargeback_dispute(
        &self,
        p: ChargebackDisputePayload,
    ) -> Result<Vec<<Account as Aggregate>::Event>, <Account as Aggregate>::Error> {
        debug!(
            "Carging back dispute for {} from {}",
            p.transaction_id, p.client_id
        );

        require_active_account(self)?;

        let dispute = require_dispute(self, &p.transaction_id)?;

        Ok(vec![AccountEvent::DisputeChargedback(
            DisputeChargedbackPayload {
                client_id: p.client_id,
                transaction_id: p.transaction_id,
                amount: Amount(dispute),
            },
        )])
    }
}

fn require_legal_amount(amount: &Amount) -> Result<(), <Account as Aggregate>::Error> {
    if amount.0 <= Decimal::ZERO {
        return Err(AccountError::IllegalAmount);
    }

    if amount.scale() > 4 {
        return Err(AccountError::IllegalAmount);
    }

    Ok(())
}

fn require_active_account(account: &Account) -> Result<(), <Account as Aggregate>::Error> {
    if account.locked {
        return Err(AccountError::AccountLocked);
    }

    Ok(())
}

fn require_sufficient_funds(
    account: &Account,
    amount: &Amount,
) -> Result<(), <Account as Aggregate>::Error> {
    if account.funds_available < amount.0 {
        return Err(AccountError::InsufficientFunds);
    }

    Ok(())
}

fn require_dispute(
    account: &Account,
    transaction_id: &TransactionId,
) -> Result<Decimal, <Account as Aggregate>::Error> {
    account
        .disputes
        .get(transaction_id)
        .map(|x| x.to_owned())
        .ok_or(AccountError::DisputeNotFound)
}

fn require_no_active_dispute(
    account: &Account,
    transaction_id: &TransactionId,
) -> Result<(), <Account as Aggregate>::Error> {
    if account.disputes.contains_key(transaction_id) {
        return Err(AccountError::DuplicateDispute);
    }

    Ok(())
}

pub fn acc_aggregate_id(id: &str) -> String {
    format!("Account-{}", id)
}

#[cfg(test)]
mod tests {
    use cqrs_es::test::TestFramework;
    use rust_decimal::dec;

    use crate::domain::{
        account::{
            aggregate::{Account, AccountServices},
            command::{
                AccountCommand, ChargebackDisputePayload, DepositAccountPayload,
                DisputeFundsPayload, ResolveDisputePayload, WithdrawAccountPayload,
            },
            error::AccountError,
            event::{
                AccountDepositedPayload, AccountEvent, AccountWithdrawnPayload,
                DisputeChargedbackPayload, DisputeResolvedPayload, FundsDisputedPayload,
            },
        },
        props::{Amount, ClientId, TransactionId},
    };

    type AccountTestFramework = TestFramework<Account>;

    #[test]
    fn test_deposit_fresh_account() {
        AccountTestFramework::with(AccountServices {})
            .given_no_previous_events()
            .when(AccountCommand::DepositAccount(DepositAccountPayload {
                client_id: ClientId("cl-1".to_owned()),
                transaction_id: TransactionId("tx-1".to_owned()),
                amount: Amount(dec!(1.2345)),
            }))
            .then_expect_events(vec![AccountEvent::AccountDeposited(
                AccountDepositedPayload {
                    client_id: ClientId("cl-1".to_owned()),
                    transaction_id: TransactionId("tx-1".to_owned()),
                    amount: Amount(dec!(1.2345)),
                },
            )]);
    }

    #[test]
    fn test_deposit_zero_amount() {
        AccountTestFramework::with(AccountServices {})
            .given_no_previous_events()
            .when(AccountCommand::DepositAccount(DepositAccountPayload {
                client_id: ClientId("cl-1".to_owned()),
                transaction_id: TransactionId("tx-1".to_owned()),
                amount: Amount(dec!(0)),
            }))
            .then_expect_error(AccountError::IllegalAmount);
    }

    #[test]
    fn test_deposit_overscale_amount() {
        AccountTestFramework::with(AccountServices {})
            .given_no_previous_events()
            .when(AccountCommand::DepositAccount(DepositAccountPayload {
                client_id: ClientId("cl-1".to_owned()),
                transaction_id: TransactionId("tx-1".to_owned()),
                amount: Amount(dec!(0.12345)),
            }))
            .then_expect_error(AccountError::IllegalAmount);
    }

    #[test]
    fn test_deposit_negative_amount() {
        AccountTestFramework::with(AccountServices {})
            .given_no_previous_events()
            .when(AccountCommand::DepositAccount(DepositAccountPayload {
                client_id: ClientId("cl-1".to_owned()),
                transaction_id: TransactionId("tx-1".to_owned()),
                amount: Amount(dec!(-1.04)),
            }))
            .then_expect_error(AccountError::IllegalAmount);
    }

    #[test]
    fn test_deposit_locked_account() {
        AccountTestFramework::with(AccountServices {})
            .given(vec![
                AccountEvent::FundsDisputed(FundsDisputedPayload {
                    client_id: ClientId("cl-1".to_owned()),
                    transaction_id: TransactionId("tx-1".to_owned()),
                    amount: Amount(dec!(1.23)),
                }),
                AccountEvent::DisputeChargedback(DisputeChargedbackPayload {
                    client_id: ClientId("cl-1".to_owned()),
                    transaction_id: TransactionId("tx-1".to_owned()),
                    amount: Amount(dec!(1.23)),
                }),
            ])
            .when(AccountCommand::DepositAccount(DepositAccountPayload {
                client_id: ClientId("cl-1".to_owned()),
                transaction_id: TransactionId("tx-1".to_owned()),
                amount: Amount(dec!(1.23)),
            }))
            .then_expect_error(AccountError::AccountLocked);
    }

    #[test]
    fn test_withdraw_full_amount() {
        AccountTestFramework::with(AccountServices {})
            .given(vec![AccountEvent::AccountDeposited(
                AccountDepositedPayload {
                    client_id: ClientId("cl-1".to_owned()),
                    transaction_id: TransactionId("tx-1".to_owned()),
                    amount: Amount(dec!(1.23)),
                },
            )])
            .when(AccountCommand::WithdrawAccount(WithdrawAccountPayload {
                client_id: ClientId("cl-1".to_owned()),
                transaction_id: TransactionId("tx-2".to_owned()),
                amount: Amount(dec!(1.23)),
            }))
            .then_expect_events(vec![AccountEvent::AccountWithdrawn(
                AccountWithdrawnPayload {
                    client_id: ClientId("cl-1".to_owned()),
                    transaction_id: TransactionId("tx-2".to_owned()),
                    amount: Amount(dec!(1.23)),
                },
            )]);
    }

    #[test]
    fn test_withdraw_partial_amount() {
        AccountTestFramework::with(AccountServices {})
            .given(vec![AccountEvent::AccountDeposited(
                AccountDepositedPayload {
                    client_id: ClientId("cl-1".to_owned()),
                    transaction_id: TransactionId("tx-1".to_owned()),
                    amount: Amount(dec!(1.23)),
                },
            )])
            .when(AccountCommand::WithdrawAccount(WithdrawAccountPayload {
                client_id: ClientId("cl-1".to_owned()),
                transaction_id: TransactionId("tx-2".to_owned()),
                amount: Amount(dec!(0.23)),
            }))
            .then_expect_events(vec![AccountEvent::AccountWithdrawn(
                AccountWithdrawnPayload {
                    client_id: ClientId("cl-1".to_owned()),
                    transaction_id: TransactionId("tx-2".to_owned()),
                    amount: Amount(dec!(0.23)),
                },
            )]);
    }

    #[test]
    fn test_withdraw_insufficient_funds() {
        AccountTestFramework::with(AccountServices {})
            .given(vec![AccountEvent::AccountDeposited(
                AccountDepositedPayload {
                    client_id: ClientId("cl-1".to_owned()),
                    transaction_id: TransactionId("tx-1".to_owned()),
                    amount: Amount(dec!(1.23)),
                },
            )])
            .when(AccountCommand::WithdrawAccount(WithdrawAccountPayload {
                client_id: ClientId("cl-1".to_owned()),
                transaction_id: TransactionId("tx-2".to_owned()),
                amount: Amount(dec!(1.2301)),
            }))
            .then_expect_error(AccountError::InsufficientFunds);
    }

    #[test]
    fn test_withdraw_zero_amount() {
        AccountTestFramework::with(AccountServices {})
            .given_no_previous_events()
            .when(AccountCommand::WithdrawAccount(WithdrawAccountPayload {
                client_id: ClientId("cl-1".to_owned()),
                transaction_id: TransactionId("tx-1".to_owned()),
                amount: Amount(dec!(0)),
            }))
            .then_expect_error(AccountError::IllegalAmount);
    }

    #[test]
    fn test_withdraw_negative_amount() {
        AccountTestFramework::with(AccountServices {})
            .given_no_previous_events()
            .when(AccountCommand::WithdrawAccount(WithdrawAccountPayload {
                client_id: ClientId("cl-1".to_owned()),
                transaction_id: TransactionId("tx-1".to_owned()),
                amount: Amount(dec!(-1.04)),
            }))
            .then_expect_error(AccountError::IllegalAmount);
    }

    #[test]
    fn test_dispute_funds() {
        AccountTestFramework::with(AccountServices {})
            .given(vec![AccountEvent::AccountDeposited(
                AccountDepositedPayload {
                    client_id: ClientId("cl-1".to_owned()),
                    transaction_id: TransactionId("tx-1".to_owned()),
                    amount: Amount(dec!(1.23)),
                },
            )])
            .when(AccountCommand::DisputeFunds(DisputeFundsPayload {
                client_id: ClientId("cl-1".to_owned()),
                transaction_id: TransactionId("tx-1".to_owned()),
                amount: Amount(dec!(1.0)),
            }))
            .then_expect_events(vec![AccountEvent::FundsDisputed(FundsDisputedPayload {
                client_id: ClientId("cl-1".to_owned()),
                transaction_id: TransactionId("tx-1".to_owned()),
                amount: Amount(dec!(1.0)),
            })]);
    }

    #[test]
    fn test_dispute_insufficient_funds() {
        AccountTestFramework::with(AccountServices {})
            .given(vec![AccountEvent::AccountDeposited(
                AccountDepositedPayload {
                    client_id: ClientId("cl-1".to_owned()),
                    transaction_id: TransactionId("tx-1".to_owned()),
                    amount: Amount(dec!(1.23)),
                },
            )])
            .when(AccountCommand::DisputeFunds(DisputeFundsPayload {
                client_id: ClientId("cl-1".to_owned()),
                transaction_id: TransactionId("tx-2".to_owned()),
                amount: Amount(dec!(1.2302)),
            }))
            .then_expect_error(AccountError::InsufficientFunds);
    }

    #[test]
    fn test_dispute_duplicate() {
        AccountTestFramework::with(AccountServices {})
            .given(vec![
                AccountEvent::AccountDeposited(AccountDepositedPayload {
                    client_id: ClientId("cl-1".to_owned()),
                    transaction_id: TransactionId("tx-1".to_owned()),
                    amount: Amount(dec!(1.23)),
                }),
                AccountEvent::FundsDisputed(FundsDisputedPayload {
                    client_id: ClientId("cl-1".to_owned()),
                    transaction_id: TransactionId("tx-1".to_owned()),
                    amount: Amount(dec!(1.0)),
                }),
            ])
            .when(AccountCommand::DisputeFunds(DisputeFundsPayload {
                client_id: ClientId("cl-1".to_owned()),
                transaction_id: TransactionId("tx-1".to_owned()),
                amount: Amount(dec!(0.23)),
            }))
            .then_expect_error(AccountError::DuplicateDispute);
    }

    #[test]
    fn test_resolve_dispute() {
        AccountTestFramework::with(AccountServices {})
            .given(vec![
                AccountEvent::AccountDeposited(AccountDepositedPayload {
                    client_id: ClientId("cl-1".to_owned()),
                    transaction_id: TransactionId("tx-1".to_owned()),
                    amount: Amount(dec!(1.23)),
                }),
                AccountEvent::FundsDisputed(FundsDisputedPayload {
                    client_id: ClientId("cl-1".to_owned()),
                    transaction_id: TransactionId("tx-1".to_owned()),
                    amount: Amount(dec!(1.0)),
                }),
            ])
            .when(AccountCommand::ResolveDispute(ResolveDisputePayload {
                client_id: ClientId("cl-1".to_owned()),
                transaction_id: TransactionId("tx-1".to_owned()),
            }))
            .then_expect_events(vec![AccountEvent::DisputeResolved(
                DisputeResolvedPayload {
                    client_id: ClientId("cl-1".to_owned()),
                    transaction_id: TransactionId("tx-1".to_owned()),
                    amount: Amount(dec!(1.0)),
                },
            )]);
    }

    #[test]
    fn test_resolve_dispute_tx_not_found() {
        AccountTestFramework::with(AccountServices {})
            .given(vec![
                AccountEvent::AccountDeposited(AccountDepositedPayload {
                    client_id: ClientId("cl-1".to_owned()),
                    transaction_id: TransactionId("tx-1".to_owned()),
                    amount: Amount(dec!(1.23)),
                }),
                AccountEvent::FundsDisputed(FundsDisputedPayload {
                    client_id: ClientId("cl-1".to_owned()),
                    transaction_id: TransactionId("tx-1".to_owned()),
                    amount: Amount(dec!(1.0)),
                }),
            ])
            .when(AccountCommand::ResolveDispute(ResolveDisputePayload {
                client_id: ClientId("cl-1".to_owned()),
                transaction_id: TransactionId("tx-2".to_owned()),
            }))
            .then_expect_error(AccountError::DisputeNotFound);
    }

    #[test]
    fn test_resolve_dispute_account_locked() {
        AccountTestFramework::with(AccountServices {})
            .given(vec![
                AccountEvent::AccountDeposited(AccountDepositedPayload {
                    client_id: ClientId("cl-1".to_owned()),
                    transaction_id: TransactionId("tx-1".to_owned()),
                    amount: Amount(dec!(1.23)),
                }),
                AccountEvent::AccountDeposited(AccountDepositedPayload {
                    client_id: ClientId("cl-1".to_owned()),
                    transaction_id: TransactionId("tx-2".to_owned()),
                    amount: Amount(dec!(1.0)),
                }),
                AccountEvent::FundsDisputed(FundsDisputedPayload {
                    client_id: ClientId("cl-1".to_owned()),
                    transaction_id: TransactionId("tx-1".to_owned()),
                    amount: Amount(dec!(1.23)),
                }),
                AccountEvent::FundsDisputed(FundsDisputedPayload {
                    client_id: ClientId("cl-1".to_owned()),
                    transaction_id: TransactionId("tx-2".to_owned()),
                    amount: Amount(dec!(1.0)),
                }),
                AccountEvent::DisputeChargedback(DisputeChargedbackPayload {
                    client_id: ClientId("cl-1".to_owned()),
                    transaction_id: TransactionId("tx-1".to_owned()),
                    amount: Amount(dec!(1.23)),
                }),
            ])
            .when(AccountCommand::ResolveDispute(ResolveDisputePayload {
                client_id: ClientId("cl-1".to_owned()),
                transaction_id: TransactionId("tx-2".to_owned()),
            }))
            .then_expect_error(AccountError::AccountLocked);
    }

    #[test]
    fn test_chargeback_dispute() {
        AccountTestFramework::with(AccountServices {})
            .given(vec![
                AccountEvent::AccountDeposited(AccountDepositedPayload {
                    client_id: ClientId("cl-1".to_owned()),
                    transaction_id: TransactionId("tx-1".to_owned()),
                    amount: Amount(dec!(1.23)),
                }),
                AccountEvent::FundsDisputed(FundsDisputedPayload {
                    client_id: ClientId("cl-1".to_owned()),
                    transaction_id: TransactionId("tx-1".to_owned()),
                    amount: Amount(dec!(1.0)),
                }),
            ])
            .when(AccountCommand::ChargebackDispute(
                ChargebackDisputePayload {
                    client_id: ClientId("cl-1".to_owned()),
                    transaction_id: TransactionId("tx-1".to_owned()),
                },
            ))
            .then_expect_events(vec![AccountEvent::DisputeChargedback(
                DisputeChargedbackPayload {
                    client_id: ClientId("cl-1".to_owned()),
                    transaction_id: TransactionId("tx-1".to_owned()),
                    amount: Amount(dec!(1.0)),
                },
            )]);
    }

    #[test]
    fn test_chargeback_dispute_tx_not_found() {
        AccountTestFramework::with(AccountServices {})
            .given(vec![
                AccountEvent::AccountDeposited(AccountDepositedPayload {
                    client_id: ClientId("cl-1".to_owned()),
                    transaction_id: TransactionId("tx-1".to_owned()),
                    amount: Amount(dec!(1.23)),
                }),
                AccountEvent::FundsDisputed(FundsDisputedPayload {
                    client_id: ClientId("cl-1".to_owned()),
                    transaction_id: TransactionId("tx-1".to_owned()),
                    amount: Amount(dec!(1.0)),
                }),
            ])
            .when(AccountCommand::ChargebackDispute(
                ChargebackDisputePayload {
                    client_id: ClientId("cl-1".to_owned()),
                    transaction_id: TransactionId("tx-2".to_owned()),
                },
            ))
            .then_expect_error(AccountError::DisputeNotFound);
    }
}
