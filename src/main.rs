use crate::benchmark::Report;
use crate::execution::run;
use crate::settings::{Args, Settings};
use tokio::sync::mpsc;
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
    let settings = Settings::from_args(Args::parse())?;

    let mut report = Report::new();

    let (tx, mut rx) = mpsc::channel(settings.requests);

    let begin = Instant::now();
    run(settings.clone(), tx).await?;

    while let Some(value) = rx.recv().await {
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
