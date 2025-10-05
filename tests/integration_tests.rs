use std::path::PathBuf;

use itertools::Itertools;
use payment_engine::PaymentEngine;

#[test]
fn basic() {
    run_test("./tests/test_cases/basic");
}

#[test]
fn decimal_places() {
    run_test("./tests/test_cases/decimal_places");
}

#[test]
fn flow() {
    run_test("./tests/test_cases/flow");
}

fn run_test(dir: impl Into<PathBuf>) {
    let dir = dir.into();

    let wanted = std::fs::read(dir.join("output.csv")).unwrap();

    let accounts = PaymentEngine::process_csv(dir.join("input.csv")).unwrap();

    // we sort the accounts by client to be simplify the comparison
    let accounts = accounts
        .into_iter()
        .sorted_by_key(|(client, _)| *client)
        .map(|(_, acc)| acc)
        .collect_vec();

    let mut output = Vec::new();
    {
        let mut writer = csv::Writer::from_writer(&mut output);
        for account in accounts.into_iter() {
            writer.serialize(account).unwrap();
        }

        writer.flush().unwrap();
    }

    let wanted = String::from_utf8(wanted);
    let output = String::from_utf8(output);

    assert_eq!(wanted, output);
}
