pub use payment::*;

pub mod account;
pub mod operation;

mod payment;

// helper function to ensure we always have the same configuration for the csv reader
pub fn csv_reader_builder() -> csv::ReaderBuilder {
    let mut builder = csv::ReaderBuilder::new();
    builder
        .trim(csv::Trim::All)
        .has_headers(true)
        .flexible(true);
    builder
}
