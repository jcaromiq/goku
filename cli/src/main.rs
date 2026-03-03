mod args;
mod output;

use indicatif::{ProgressBar, ProgressStyle};
use std::fmt::{Display, Formatter};
use std::time::Duration;
use tokio::sync::{mpsc, watch};

use crate::args::Args;
use crate::output::{OutputFormat, print_csv, print_json};
use anyhow::Result;
use clap::Parser;
use colored::Colorize;
use goku_core::benchmark::{BenchmarkResult, Metrics, Report};
use goku_core::execution::run;
use goku_core::settings::Settings;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let output_format = args.output_format();
    let settings: Settings = args.to_settings()?;

    settings.validate()?;

    let mut report = Report::new(settings.clients);
    print_banner(&settings);

    let pb = match settings.duration {
        None => {
            let bar = ProgressBar::new(settings.requests as u64);
            bar.set_style(
                ProgressStyle::with_template(
                    "{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} ({eta})"
                )
                .unwrap()
                .progress_chars("=>-"),
            );
            bar
        }
        Some(secs) => {
            let bar = ProgressBar::new(secs);
            bar.set_style(
                ProgressStyle::with_template(
                    "{spinner:.green} Running for {elapsed_precise} / {msg}"
                )
                .unwrap(),
            );
            bar.set_message(format!("{secs}s"));
            let pb_clone = bar.clone();
            tokio::spawn(async move {
                let mut elapsed = 0u64;
                while elapsed < secs {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    elapsed += 1;
                    pb_clone.set_position(elapsed);
                }
            });
            bar
        }
    };

    let (tx_sigint, rx_sigint) = watch::channel(None);
    let channel_capacity = (settings.clients as usize * 2).min(4096);
    let (benchmark_tx, mut benchmark_rx) = mpsc::channel(channel_capacity);

    ctrlc::set_handler(move || {
        tx_sigint.send(Some(())).unwrap_or(());
    })?;

    run(settings.clone(), benchmark_tx, Some(rx_sigint)).await?;
    while let Some(value) = benchmark_rx.recv().await {
        match settings.verbose {
            true => println!("{}", DisplayableBenchmarkResult(&value)),
            false => {
                if settings.duration.is_none() {
                    pb.inc(1);
                }
            }
        }
        report.add_result(value);
    }
    pb.finish_and_clear();

    match output_format {
        OutputFormat::Json => print_json(&report),
        OutputFormat::Csv => print_csv(&report),
        OutputFormat::Text => show_results(report),
    }

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
        "Time taken      ".yellow().bold(),
        elapsed.as_secs().to_string().purple(),
        "seconds".purple()
    );
    println!(
        "{} {}",
        "Total requests  ".yellow().bold(),
        r.hist.len().to_string().purple()
    );
    println!(
        "{} {} {}",
        "Requests/sec    ".yellow().bold(),
        format!("{:.2}", r.requests_per_second()).purple(),
        "req/s".purple()
    );
    println!(
        "{} {} {}",
        "Mean            ".yellow().bold(),
        format!("{:.2}", r.hist.mean()).purple(),
        "ms".purple()
    );
    println!(
        "{} {} {}",
        "Min             ".yellow().bold(),
        r.results.min().to_string().purple(),
        "ms".purple()
    );
    println!(
        "{} {} {}",
        "Max             ".yellow().bold(),
        r.results.max().to_string().purple(),
        "ms".purple()
    );
    println!(
        "{} {} {}",
        "p50 (median)    ".yellow().bold(),
        r.hist.value_at_quantile(0.50).to_string().purple(),
        "ms".purple()
    );
    println!(
        "{} {} {}",
        "p95             ".yellow().bold(),
        r.hist.value_at_quantile(0.95).to_string().purple(),
        "ms".purple()
    );
    println!(
        "{} {} {}",
        "p99.9           ".yellow().bold(),
        r.hist.value_at_quantile(0.999).to_string().purple(),
        "ms".purple()
    );

    println!();
    let bd = r.status_breakdown();
    println!("{}", "Status codes".yellow().bold());
    println!("  {} {}", "2xx".green().bold(), bd.success.to_string().purple());
    if bd.client_error > 0 {
        println!("  {} {}", "4xx".yellow().bold(), bd.client_error.to_string().purple());
    }
    if bd.server_error > 0 {
        println!("  {} {}", "5xx".red().bold(), bd.server_error.to_string().purple());
    }
    if bd.other > 0 {
        println!("  {} {}", "other".cyan().bold(), bd.other.to_string().purple());
    }
    if bd.network_error > 0 {
        println!("  {} {}", "network errors".red().bold(), bd.network_error.to_string().purple());
    }
}

struct DisplayableBenchmarkResult<'a>(&'a BenchmarkResult);

impl Display for DisplayableBenchmarkResult<'_> {
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
