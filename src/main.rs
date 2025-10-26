#![deny(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
#![cfg_attr(test, allow(clippy::panic, clippy::unwrap_used, clippy::expect_used))]

use std::{
    fs,
    path::Path,
    str::FromStr,
    thread::{self, available_parallelism},
    time::SystemTime,
};

use color_eyre::eyre::{OptionExt, Result, eyre};
use crossbeam::channel::{Receiver, Sender, bounded};
use murmur2::{KAFKA_SEED, murmur2};
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode},
};
use tokio::task::JoinSet;
use tracing::debug;

use crate::{
    cli::CliArgs, csv::CsvPaymentRecord, payments::PaymentsService,
    query::account::print_accounts_csv,
};

pub(crate) mod cli;
mod csv;
mod domain;
mod payments;
mod query;

// Event sourcing with sqlite backed event store will be used.
// There will be a temp sqlite file generated per core like 'XDB-1761491588862857000-0.db'
// Result account projections will also be stored in that same sqlite dbs.
#[tokio::main]
async fn main() -> Result<()> {
    let cli_args = CliArgs::load()?;

    // We will have a channel per cpy core and will distribute processing in parallel.
    // There will be 1 sender thread which will read csv and send each csv row to one of the channels (see: get_channel_by_client_id).
    // After the processing, the results from all processors will be printed out in csv format.
    let cpu_cores = available_parallelism()
        .map_err(|_| eyre!("unable to get core count"))?
        .get();
    let channels: Vec<(Sender<CsvPaymentRecord>, Receiver<CsvPaymentRecord>)> =
        (0..cpu_cores).map(|_| bounded(100)).collect();
    let senders = channels
        .clone()
        .into_iter()
        .map(|(s, _)| s)
        .collect::<Vec<_>>();
    let receivers = channels.into_iter().map(|(_, r)| r).collect::<Vec<_>>();

    // Start sender thread which reads csv and distributes rows to channels by client_id
    let sender_thread = start_sender_thread(cli_args, senders, cpu_cores);

    // Start receiver threads, one per core
    let receiver_threads = start_receiver_threads(&receivers, cpu_cores)?;

    sender_thread
        .join()
        .map_err(|_| eyre!("Error waiting for senders to finish"))?;

    let receiver_results = receiver_threads.join_all().await;

    // print out all resulting csvs
    println!("client,available,held,total,locked");
    for result_db in &receiver_results {
        print_accounts_csv(result_db).await?;
    }

    cleanup_temp_dbs(&receiver_results)?;

    Ok(())
}

/// Starts sender thread which reads csv and distributes rows to channels by client_id for receivers to process
fn start_sender_thread(
    cli_args: CliArgs,
    senders: Vec<Sender<CsvPaymentRecord>>,
    cpu_cores: usize,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        #[allow(clippy::unwrap_used)]
        let csv_rows = csv::read_input::<csv::CsvPaymentRecord>(&cli_args.input_file_path).unwrap();
        for row_result in csv_rows {
            match row_result {
                Ok(row) => {
                    if row.client_id.is_empty() {
                        debug!("No client_id in a row: {:?}, skipping", row);
                        continue;
                    }
                    let client_worker = get_channel_by_client_id(cpu_cores as u32, &row.client_id);
                    let sender = &senders[client_worker];
                    #[allow(clippy::unwrap_used)]
                    sender.send(row).unwrap();
                }
                Err(e) => debug!("Error parsing row: {}", e),
            }
        }
    })
}

/// Starts receiver threads, one per core, reads csv rows and passes for processing to PaymentService.
/// Returns a reference to resulting sqlite db.
fn start_receiver_threads(
    receivers: &[Receiver<CsvPaymentRecord>],
    cpu_cores: usize,
) -> Result<JoinSet<SqlitePool>> {
    let mut receiver_threads = JoinSet::new();
    let db_file_suffix = epoch_nanos()?;
    for core_idx in 0..cpu_cores {
        let receivers = receivers.to_owned();
        receiver_threads.spawn(async move {
            let receiver = &receivers[core_idx];

            #[allow(clippy::unwrap_used)]
            let pool = sqlite_pool(&sqlite_uri(db_file_suffix, core_idx))
                .await
                .unwrap();
            let payments = PaymentsService::new(pool.clone()).await;

            while let Ok(row) = receiver.recv() {
                let _ = &payments
                    .handle(row)
                    .await
                    .inspect_err(|e| debug!("Error processing row: {}", e));
            }
            pool
        });
    }

    Ok(receiver_threads)
}

async fn sqlite_pool(sqlite_uri: &str) -> Result<SqlitePool> {
    let opts = SqliteConnectOptions::from_str(sqlite_uri)?
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Off);
    SqlitePool::connect_with(opts).await.map_err(|e| eyre!(e))
}

#[allow(clippy::unwrap_used)]
fn sqlite_uri(suffix: u128, core_idx: usize) -> String {
    format!("sqlite:XDB-{}-{}.db?mode=rwc", suffix, core_idx)
}

fn epoch_nanos() -> Result<u128> {
    Ok(SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| eyre!(e))?
        .as_nanos())
}

/// Calculate partition/channel for parallelising work and keeping the same client in the same work partition/channel
fn get_channel_by_client_id(partition_count: u32, client_id: &str) -> usize {
    (murmur2(client_id.as_bytes(), KAFKA_SEED) % partition_count) as usize
}

fn cleanup_temp_dbs(pools: &[SqlitePool]) -> Result<()> {
    for pool in pools {
        let options = pool.connect_options();
        let db_path = options
            .get_filename()
            .to_str()
            .ok_or_eyre("no db file name")?;
        let _ = fs::remove_file(Path::new(db_path));
        let _ = fs::remove_file(Path::new(&format!("{}-shm", db_path)));
        let _ = fs::remove_file(Path::new(&format!("{}-wal", db_path)));
    }

    Ok(())
}
