use crate::benchmark::Report;
use crate::execution::run;
use crate::settings::{Args, Settings};
use tokio::sync::{mpsc, watch};
use tokio::time::Instant;

mod benchmark;
mod execution;
mod settings;

use anyhow::Result;
use clap::Parser;
use hdrhistogram::Histogram;

use colored::Colorize;

#[tokio::main]
async fn main() -> Result<()> {
    let mut hist = Histogram::<u64>::new(1).unwrap();
    let arguments = Args::parse();
    let settings: Settings = arguments.to_settings()?;

    let (tx_sigint, rx_sigint) = watch::channel(None);

    ctrlc::set_handler(move || {
        tx_sigint
            .send(Some("kill".to_string()))
            .expect("TODO: panic message");
    })
    .expect("Error setting Ctrl-C handler");

    let mut report = Report::new();

    let (benchmark_tx, mut benchmark_rx) = mpsc::channel(settings.requests);

    let begin = Instant::now();
    run(settings.clone(), benchmark_tx, rx_sigint).await?;

    while let Some(value) = benchmark_rx.recv().await {
        println!("{}", value);
        hist.record(value.duration).expect("");
        report.add_result(value);
    }
    let elapsed = begin.elapsed().as_secs();

    println!();
    println!(
        "{} {}",
        "Concurrency level".yellow().bold(),
        settings.clients.clone().to_string().purple()
    );
    println!(
        "{} {} {}",
        "Time taken".yellow().bold(),
        elapsed.to_string().purple(),
        "seconds".purple()
    );
    println!(
        "{} {}",
        "Total requests ".yellow().bold(),
        hist.len().to_string().purple()
    );
    println!(
        "{} {} {}",
        "Mean request time".yellow().bold(),
        hist.mean().to_string().purple(),
        "ms".purple()
    );
    println!(
        "{} {} {}",
        "Max request time".yellow().bold(),
        hist.max().to_string().purple(),
        "ms".purple()
    );
    println!(
        "{} {} {}",
        "Min request time".yellow().bold(),
        hist.min().to_string().purple(),
        "ms".purple()
    );
    println!(
        "{} {} {}",
        "95'th percentile:".yellow().bold(),
        hist.value_at_quantile(0.95).to_string().purple(),
        "ms".purple()
    );
    println!(
        "{} {} {}",
        "99.9'th percentile:".yellow().bold(),
        hist.value_at_quantile(0.999).to_string().purple(),
        "ms".purple()
    );
    Ok(())
}
