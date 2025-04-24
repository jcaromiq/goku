mod args;

use indicatif::ProgressBar;
use std::fmt::{Display, Formatter};
use tokio::sync::{mpsc, watch};

use crate::args::Args;
use anyhow::Result;
use clap::Parser;
use colored::Colorize;
use goku_core::benchmark::{BenchmarkResult, Metrics, Report};
use goku_core::execution::run;
use goku_core::settings::Settings;

#[tokio::main]
async fn main() -> Result<()> {
    let settings: Settings = Args::parse().to_settings()?;
    let mut report = Report::new(settings.clients);
    print_banner(&settings);

    let pb = ProgressBar::new(settings.requests as u64);

    let (tx_sigint, rx_sigint) = watch::channel(None);
    let (benchmark_tx, mut benchmark_rx) = mpsc::channel(settings.requests);

    ctrlc::set_handler(move || {
        tx_sigint.send(Some(())).unwrap_or(());
    })?;

    run(settings.clone(), benchmark_tx, rx_sigint).await?;
    while let Some(value) = benchmark_rx.recv().await {
        match settings.verbose {
            true => println!("{}", DisplayableBenchmarkResult(&value)),
            false => pb.inc(1),
        }
        report.add_result(value);
    }
    show_results(report);
    Ok(())
}

pub fn print_banner(settings: &Settings) {
    let banner = match settings.duration {
        None => format!(
            "kamehameha to {} with {} concurrent clients and {} total iterations",
            settings.target, settings.clients, settings.requests
        ),
        Some(d) => format!(
            "kamehameha to {} with {} concurrent clients for {} seconds",
            settings.target, settings.clients, d
        ),
    };
    println!("{banner}");
}

pub fn show_results(r: Report) {
    let elapsed = r.start.elapsed();

    println!();
    println!();
    println!();
    println!(
        "{} {}",
        "Concurrency level".yellow().bold(),
        r.clients.to_string().purple()
    );
    println!(
        "{} {} {}",
        "Time taken".yellow().bold(),
        elapsed.as_secs().to_string().purple(),
        "seconds".purple()
    );
    println!(
        "{} {}",
        "Total requests ".yellow().bold(),
        r.hist.len().to_string().purple()
    );
    println!(
        "{} {} {}",
        "Mean request time".yellow().bold(),
        r.hist.mean().to_string().purple(),
        "ms".purple()
    );
    println!(
        "{} {} {}",
        "Max request time".yellow().bold(),
        r.results.max().to_string().purple(),
        "ms".purple()
    );
    println!(
        "{} {} {}",
        "Min request time".yellow().bold(),
        r.results.min().to_string().purple(),
        "ms".purple()
    );
    println!(
        "{} {} {}",
        "95'th percentile:".yellow().bold(),
        r.hist.value_at_quantile(0.95).to_string().purple(),
        "ms".purple()
    );
    println!(
        "{} {} {}",
        "99.9'th percentile:".yellow().bold(),
        r.hist.value_at_quantile(0.999).to_string().purple(),
        "ms".purple()
    );

    for (word, count) in &r.oks() {
        println!("{} {}", word.yellow().bold(), count.to_string().purple(),);
    }
}

struct DisplayableBenchmarkResult<'a>(&'a BenchmarkResult);

impl<'a> Display for DisplayableBenchmarkResult<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let report = format!(
            "[{} {} {} {}] {} {}{}",
            "Client".bold().green(),
            self.0.num_client.to_string().bold().green(),
            "Iteration".bold().green(),
            self.0.execution.to_string().bold().green(),
            self.0.status.to_string().bold().yellow(),
            self.0.duration.to_string().cyan(),
            "ms".cyan()
        );
        write!(f, "{}", report)
    }
}
