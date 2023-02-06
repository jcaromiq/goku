use crate::benchmark::Report;
use crate::execution::run;
use crate::settings::Settings;
use tokio::sync::mpsc;
use tokio::time::Instant;

mod benchmark;
mod execution;
mod settings;

use clap::Parser;
use hdrhistogram::Histogram;

use colored::Colorize;

/// a HTTP benchmarking tool
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Url to be request
    #[arg(short, long)]
    target: String,

    /// Number of concurrent clients
    #[arg(short, long, default_value_t = 1)]
    clients: usize,

    /// Total number of iterations
    #[arg(short, long, default_value_t = 1)]
    iterations: usize,
}

#[tokio::main]
async fn main() {
    let mut hist = Histogram::<u64>::new(1).unwrap();
    let args = Args::parse();

    let settings = Settings {
        clients: args.clients,
        requests: args.iterations,
        target: args.target,
        keep_alive: None,
    };

    let mut report = Report::new();

    let (tx, mut rx) = mpsc::channel(settings.requests);

    let begin = Instant::now();
    run(settings.clone(), tx).await;

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
        "50'th percentile:".yellow().bold(),
        hist.value_at_quantile(0.50).to_string().purple(),
        "ms".purple()
    );
    println!(
        "{} {} {}",
        "90'th percentile:".yellow().bold(),
        hist.value_at_quantile(0.90).to_string().purple(),
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
}
