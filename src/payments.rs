use std::sync::Arc;

use color_eyre::eyre::{OptionExt, Result, eyre};
use cqrs_es::{CqrsFramework, EventStore, persist::PersistedEventStore};
use rust_decimal::Decimal;
use sqlite_es::{SqliteEventRepository, SqliteViewRepository, init_tables, sqlite_aggregate_cqrs};
use sqlx::{Pool, Sqlite};

use crate::{
    csv,
    domain::{
        account::{
            aggregate::{Account, AccountServices, acc_aggregate_id},
            command::{
                AccountCommand, ChargebackDisputePayload, DepositAccountPayload,
                DisputeFundsPayload, ResolveDisputePayload, WithdrawAccountPayload,
            },
        },
        props::{Amount, ClientId, TransactionId, TxType},
        transaction::{
            aggregate::{Transaction, TransactionServices, tx_aggregate_id},
            command::{RecordTransactionPayload, TransactionCommand},
        },
    },
    query::account::{AccountQueryRepository, AccountView, init_accounts_table},
};

// This is an orchestrator service coordinating actions between 2 domains - Transaction and Account.
// It should be treated as a naive SAGAs implementation, so should be improved for a production use -
// to have atomic steps and backed by storage for the redundancy.
pub struct Payments {
    account_cqrs: CqrsFramework<Account, PersistedEventStore<SqliteEventRepository, Account>>,
    transaction_cqrs:
        CqrsFramework<Transaction, PersistedEventStore<SqliteEventRepository, Transaction>>,
    transactions_store: PersistedEventStore<SqliteEventRepository, Transaction>,
}

impl Payments {
    pub async fn new(sqlite_pool: Pool<Sqlite>) -> Self {
        #[allow(clippy::expect_used)]
        init_tables(&sqlite_pool)
            .await
            .expect("Failed to initialize DB tables");
        init_accounts_table(&sqlite_pool).await;

        let view_repo =
            SqliteViewRepository::<AccountView, Account>::new("accounts", sqlite_pool.clone());
        let account_query = AccountQueryRepository::new(Arc::new(view_repo));
        let account_cqrs = sqlite_aggregate_cqrs(
            sqlite_pool.clone(),
            vec![Box::new(account_query)],
            AccountServices {},
        );

        let transaction_cqrs =
            sqlite_aggregate_cqrs(sqlite_pool.clone(), vec![], TransactionServices {});
        let transactions_store =
            PersistedEventStore::new_aggregate_store(SqliteEventRepository::new(sqlite_pool));

        Payments {
            account_cqrs,
            transaction_cqrs,
            transactions_store,
        }
    }

    pub async fn handle(&self, r: csv::CsvPaymentRecord) -> Result<()> {
        match r.tx_type {
            csv::TxType::Deposit => self.handle_deposit(r).await?,
            csv::TxType::Withdrawal => self.handle_withdrawal(r).await?,
            csv::TxType::Dispute => self.handle_dispute_funds(r).await?,
            csv::TxType::Resolve => self.handle_resolve_dispute(r).await?,
            csv::TxType::Chargeback => self.handle_chargeback_dispute(r).await?,
        }

        Ok(())
    }

    pub async fn handle_deposit(&self, r: csv::CsvPaymentRecord) -> Result<()> {
        let amount = require_amount(r.amount, &r.tx_id)?;

        // If tx recording fails (e.g. duplicate exists),
        //   then subsequent account operation will not proceed.
        self.transaction_cqrs
            .execute(
                &format!("Transaction-{}", r.tx_id),
                TransactionCommand::RecordTransaction(RecordTransactionPayload {
                    client_id: ClientId(r.client_id.to_owned()),
                    id: TransactionId(r.tx_id.to_owned()),
                    amount: Amount(amount),
                }),
            )
            .await?;

        self.account_cqrs
            .execute(
                &format!("Account-{}", r.client_id),
                AccountCommand::DepositAccount(DepositAccountPayload {
                    client_id: ClientId(r.client_id),
                    transaction_id: TransactionId(r.tx_id.to_owned()),
                    amount: Amount(amount),
                }),
            )
            .await?;

        Ok(())
    }

    pub async fn handle_withdrawal(&self, r: csv::CsvPaymentRecord) -> Result<()> {
        let amount = require_amount(r.amount, &r.tx_id)?;

        // If tx recording fails (e.g. duplicate exists),
        //   then subsequent account operation will not proceed.
        self.transaction_cqrs
            .execute(
                &format!("Transaction-{}", r.tx_id),
                TransactionCommand::RecordTransaction(RecordTransactionPayload {
                    client_id: ClientId(r.client_id.to_owned()),
                    id: TransactionId(r.tx_id.to_owned()),
                    amount: Amount(amount),
                }),
            )
            .await?;

        let _ = self
            .account_cqrs
            .execute(
                &format!("Account-{}", r.client_id),
                AccountCommand::WithdrawAccount(WithdrawAccountPayload {
                    client_id: ClientId(r.client_id),
                    transaction_id: TransactionId(r.tx_id.to_owned()),
                    amount: Amount(amount),
                }),
            )
            .await;

        Ok(())
    }

    pub async fn handle_dispute_funds(&self, r: csv::CsvPaymentRecord) -> Result<()> {
        let transaction = require_transaction(&self.transactions_store, &r.tx_id).await?;

        #[allow(clippy::collapsible_if)] // collapsable 'if' can be unstable
        if let Some(tx_type) = transaction.tx_type {
            if let TxType::Withdrawal = tx_type {
                return Err(eyre!("Dispute not allowed for type={}", tx_type));
            }
        }

        let amount = transaction.amount;

        self.account_cqrs
            .execute(
                &acc_aggregate_id(&r.client_id),
                AccountCommand::DisputeFunds(DisputeFundsPayload {
                    client_id: ClientId(r.client_id),
                    transaction_id: TransactionId(r.tx_id.to_owned()),
                    amount: Amount(amount),
                }),
            )
            .await?;

        Ok(())
    }

    pub async fn handle_resolve_dispute(&self, r: csv::CsvPaymentRecord) -> Result<()> {
        let _ = require_transaction(&self.transactions_store, &r.tx_id).await?;

        // If there was no open dispute, this will fail as expected.
        let _ = self
            .account_cqrs
            .execute(
                &format!("Account-{}", r.client_id),
                AccountCommand::ResolveDispute(ResolveDisputePayload {
                    client_id: ClientId(r.client_id),
                    transaction_id: TransactionId(r.tx_id.to_owned()),
                }),
            )
            .await;

        Ok(())
    }

    pub async fn handle_chargeback_dispute(&self, r: csv::CsvPaymentRecord) -> Result<()> {
        let _ = require_transaction(&self.transactions_store, &r.tx_id).await?;

        // If there was no open dispute, this will fail as expected.
        let _ = self
            .account_cqrs
            .execute(
                &format!("Account-{}", r.client_id),
                AccountCommand::ChargebackDispute(ChargebackDisputePayload {
                    client_id: ClientId(r.client_id),
                    transaction_id: TransactionId(r.tx_id.to_owned()),
                }),
            )
            .await;

        Ok(())
    }
}

fn require_amount(amount_opt: Option<Decimal>, tx_id: &str) -> Result<Decimal> {
    amount_opt.ok_or_eyre(format!("No amount found in row for tx {}", tx_id))
}

async fn require_transaction(
    transactions_store: &PersistedEventStore<SqliteEventRepository, Transaction>,
    tx_id: &str,
) -> Result<Transaction> {
    Ok(transactions_store
        .load_aggregate(&tx_aggregate_id(tx_id))
        .await
        .map_err(|e| eyre!(e))?
        .aggregate)
}
