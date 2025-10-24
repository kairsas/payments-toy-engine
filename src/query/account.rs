use std::io;

use color_eyre::eyre::{Result, eyre};
use cqrs_es::{EventEnvelope, View, persist::GenericQuery};
use csv::WriterBuilder;
use futures::TryStreamExt;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlite_es::SqliteViewRepository;
use sqlx::{Pool, Row, Sqlite, SqlitePool};

use crate::domain::account::{aggregate::Account, event::AccountEvent};

pub(crate) type AccountQueryRepository =
    GenericQuery<SqliteViewRepository<AccountView, Account>, AccountView, Account>;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub(crate) struct AccountView {
    #[serde(rename = "client")]
    pub client_id: String,
    #[serde(rename = "available")]
    pub available_funds: Decimal,
    #[serde(rename = "held")]
    pub held_funds: Decimal,
    #[serde(rename = "total")]
    pub total_funds: Decimal,
    #[serde(rename = "locked")]
    pub is_locked: bool,
}

impl View<Account> for AccountView {
    fn update(&mut self, event: &EventEnvelope<Account>) {
        match &event.payload {
            AccountEvent::AccountDeposited(p) => {
                self.client_id = p.client_id.to_string();
                self.available_funds += *p.amount;
                self.total_funds += *p.amount;
            }
            AccountEvent::AccountWithdrawn(p) => {
                self.available_funds -= *p.amount;
                self.total_funds -= *p.amount;
            }
            AccountEvent::FundsDisputed(p) => {
                self.available_funds -= *p.amount;
                self.held_funds += *p.amount;
            }
            AccountEvent::DisputeResolved(p) => {
                self.available_funds += *p.amount;
                self.held_funds -= *p.amount;
            }
            AccountEvent::DisputeChargedback(p) => {
                self.held_funds -= *p.amount;
                self.total_funds -= *p.amount;
                self.is_locked = true;
            }
        }
    }
}

#[allow(clippy::expect_used)] // without this working, it's a show over
pub async fn init_accounts_table(sqlite_pool: &Pool<Sqlite>) {
    let _ = sqlx::query(
        "CREATE TABLE accounts
            (
                view_id text                        NOT NULL,
                version bigint CHECK (version >= 0) NOT NULL,
                payload json                        NOT NULL,
                PRIMARY KEY (view_id)
            );",
    )
    .execute(&sqlite_pool.clone())
    .await
    .expect("Failed to initialize accounts table");
}

pub async fn print_accounts_csv(sqlite_pool: &SqlitePool) -> Result<()> {
    let mut csv_writer = WriterBuilder::new().from_writer(io::stdout());

    let mut query = sqlx::query("select payload from accounts").fetch(sqlite_pool);
    while let Some(row) = query.try_next().await.map_err(|e| eyre!(e))? {
        let s: String = row.get("payload");
        if let Ok(obj) = serde_json::from_str::<AccountView>(&s) {
            let _ = csv_writer.serialize(obj);
        }
    }

    Ok(())
}
