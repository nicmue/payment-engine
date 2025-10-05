use anyhow::bail;
use payment_engine::PaymentEngine;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        bail!(
            "Invalid arguments: expect exact one argument, the path to the transactions CSV file!"
        )
    }

    let accounts = PaymentEngine::process_csv(&args[1])?;

    let mut writer = csv::Writer::from_writer(std::io::stdout());
    for (_, account) in accounts.into_iter() {
        writer.serialize(account)?;
    }

    Ok(())
}
