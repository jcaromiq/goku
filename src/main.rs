use crate::benchmark::Report;
use crate::execution::run;
use crate::settings::Settings;
use tokio::sync::mpsc;
use tokio::time::Instant;

mod benchmark;
mod execution;
mod settings;

use clap::Parser;

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
    run(settings, tx).await;

    while let Some(value) = rx.recv().await {
        report.add_result(value);
    }
    let elapsed = begin.elapsed().as_secs();

    println!(
        "Total time: {}s for {} request with a average of {}ms ",
        elapsed,
        &report.total(),
        &report.avg()
    );
}
