use core::str;

use color_eyre::eyre::{Result, eyre};
use csv::{ReaderBuilder, Trim};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CsvPaymentRecord {
    #[serde(rename = "type")]
    pub tx_type: TxType,
    #[serde(rename = "client")]
    pub client_id: String,
    #[serde(rename = "tx")]
    pub tx_id: String,
    pub amount: Option<Decimal>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TxType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

pub fn read_input<D: serde::de::DeserializeOwned>(
    file_path: &str,
) -> Result<impl Iterator<Item = Result<D>>> {
    let reader = ReaderBuilder::new()
        .trim(Trim::All)
        .from_path(file_path)
        .map_err(|e| eyre!("Could not read input file: {}", e))?;

    Ok(reader
        .into_deserialize()
        .map(|r| r.map_err(|ee| eyre!("Error parsing row: {}", ee))))
}

#[cfg(test)]
mod tests {
    use color_eyre::eyre::{Result, eyre};
    use csv::{ReaderBuilder, Trim, WriterBuilder};
    use rust_decimal::{Decimal, dec};

    use crate::csv::{CsvPaymentRecord, TxType};

    #[test]
    fn parses_data() {
        let data = r#"
            type, client, tx, amount
            deposit, 1, tx-1,     1.0
            withdrawal , cl-1,4,1.5
            dispute, 2, 5,
            resolve, c-2, 5,
            chargeback , 2 , 5 ,
            unrecognized , cl-1,4,1.5
            "#;

        let reader = ReaderBuilder::new()
            .trim(Trim::All)
            .from_reader(data.as_bytes());

        let records: Vec<Result<CsvPaymentRecord>> = reader
            .into_deserialize()
            .map(|r| r.map_err(|ee| eyre!("Error parsing row: {}", ee)))
            .collect();

        assert_record(&records, 0, TxType::Deposit, "1", "tx-1", Some(dec!(1.0)));
        assert_record(
            &records,
            1,
            TxType::Withdrawal,
            "cl-1",
            "4",
            Some(dec!(1.5)),
        );
        assert_record(&records, 2, TxType::Dispute, "2", "5", None);
        assert_record(&records, 3, TxType::Resolve, "c-2", "5", None);
        assert_record(&records, 4, TxType::Chargeback, "2", "5", None);

        assert_record_not_parsable(&records, 5);
    }

    fn assert_record(
        records: &[std::result::Result<CsvPaymentRecord, color_eyre::eyre::Error>],
        idx: usize,
        tx_type: TxType,
        client_id: &str,
        tx_id: &str,
        amount: Option<Decimal>,
    ) {
        let rec = records
            .get(idx)
            .unwrap_or_else(|| panic!("{} record not found", idx + 1))
            .as_ref()
            .unwrap_or_else(|_| panic!("{} record not parsed", idx + 1));

        assert_eq!(rec.tx_type, tx_type);
        assert_eq!(rec.client_id, client_id);
        assert_eq!(rec.tx_id, tx_id);
        assert_eq!(rec.amount, amount);
    }

    fn assert_record_not_parsable(
        records: &[std::result::Result<CsvPaymentRecord, color_eyre::eyre::Error>],
        idx: usize,
    ) {
        records
            .get(idx)
            .unwrap_or_else(|| panic!("{} record not found", idx + 1))
            .as_ref()
            .expect_err("Not parsable entry not found");
    }

    #[test]
    #[ignore]
    fn generate_csv() {
        let mut csv_writer = WriterBuilder::new().from_path("generated.csv").unwrap();

        let n_clients = 1000;

        for i in 1..200000 {
            let client_id = (i % n_clients + 1).to_string();

            let deposit = CsvPaymentRecord {
                tx_type: TxType::Deposit,
                client_id: client_id.clone(),
                tx_id: format!("c{}-{}-dps", client_id, i),
                amount: dec!(1.2345).into(),
            };
            csv_writer.serialize(deposit).unwrap();

            let withdrawal = CsvPaymentRecord {
                tx_type: TxType::Withdrawal,
                client_id: client_id.clone(),
                tx_id: format!("c{}-{}-wthr", client_id, i),
                amount: dec!(0.2345).into(),
            };
            csv_writer.serialize(withdrawal).unwrap();
        }
    }
}
