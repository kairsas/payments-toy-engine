use assert_cmd::prelude::*; // Add methods on commands
use predicates::prelude::*; // Used for writing assertions
use std::{fs, process::Command}; // Run programs

const BIN_NAME: &str = "payments-toy-engine";

#[test]
fn sample_input_output() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin(BIN_NAME)?;

    cmd.arg("sample/transactions.csv");

    cmd.assert()
        .stdout(fs::read_to_string("sample/accounts.csv").unwrap())
        .stderr("");

    Ok(())
}

#[test]
fn duplicate_tx_ids_ignored() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin(BIN_NAME)?;

    cmd.arg("sample/transactions_with_duplicates.csv");

    cmd.assert()
        .stdout(fs::read_to_string("sample/accounts.csv").unwrap())
        .stderr("");

    Ok(())
}

#[test]
fn cli_non_existing_input_file() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin(BIN_NAME)?;

    cmd.arg("sample/accounts_non_existing.csv");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Could not read input file"))
        .stdout("");

    Ok(())
}

#[test]
fn cli_no_input_file_passed() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin(BIN_NAME)?;

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Input file not passed"))
        .stdout("");

    Ok(())
}

#[test]
fn dispute_reflecting() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin(BIN_NAME)?;

    cmd.arg("sample/transaction_dispute.csv");

    cmd.assert()
        .stdout(
            r#"client,available,held,total,locked
1,0.0,1.0,1.0,false
"#,
        )
        .stderr("");

    Ok(())
}
