#![deny(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
#![cfg_attr(test, allow(clippy::panic, clippy::unwrap_used, clippy::expect_used))]

use std::{str::FromStr, time::SystemTime};

use color_eyre::eyre::{Result, eyre};
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode},
};
use tracing::debug;

use crate::{cli::CliArgs, payments::Payments, query::account::print_accounts_csv};

pub(crate) mod cli;
mod csv;
mod domain;
mod payments;
mod query;

#[tokio::main]
async fn main() -> Result<()> {
    let cli_args = CliArgs::load()?;

    // We will use event sourcing with sqlite backed event store.
    // There will be a temp sqlite file generated per run.
    // We will also store account projections in that same sqlite db.
    //let sqlite_pool = default_sqlite_pool(&sqlite_uri()).await;
    let sqlite_pool = sqlite_pool(&sqlite_uri()).await?;

    let payments = Payments::new(sqlite_pool.clone()).await;

    let rows = csv::read_input::<csv::CsvPaymentRecord>(&cli_args.input_file_path)?;

    for row_result in rows {
        match row_result {
            Ok(row) => {
                let _ = payments
                    .handle(row)
                    .await
                    .inspect_err(|e| debug!("Error processing row: {}", e));
            }
            Err(e) => debug!("Error parsing row: {}", e),
        }
    }

    print_accounts_csv(&sqlite_pool).await?;

    Ok(())
}

async fn sqlite_pool(sqlite_uri: &str) -> Result<SqlitePool> {
    let opts = SqliteConnectOptions::from_str(sqlite_uri)?
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);
    SqlitePool::connect_with(opts).await.map_err(|e| eyre!(e))
}

#[allow(clippy::unwrap_used)]
fn sqlite_uri() -> String {
    let db_suffix = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("sqlite:es-{}.db?mode=rwc", db_suffix)
}
