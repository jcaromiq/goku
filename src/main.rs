use crate::benchmark::Report;
use crate::execution::run;
use crate::settings::{Args, Settings};
use tokio::sync::{mpsc, watch};

mod benchmark;
mod execution;
mod settings;

use anyhow::Result;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    let settings: Settings = Args::parse().to_settings()?;
    let mut report = Report::new(settings.clients);

    let (tx_sigint, rx_sigint) = watch::channel(None);
    let (benchmark_tx, mut benchmark_rx) = mpsc::channel(settings.requests);

    ctrlc::set_handler(move || {
        tx_sigint.send(Some(())).unwrap_or(());
    })?;

    run(settings.clone(), benchmark_tx, rx_sigint).await?;
    while let Some(value) = benchmark_rx.recv().await {
        println!("{}", value);
        report.add_result(value);
    }

    report.show_results();
    Ok(())
}
