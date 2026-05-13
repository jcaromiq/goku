mod args;
mod output;

use std::fmt::{Display, Formatter};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use tokio::sync::{mpsc, watch};

use crate::args::{Cli, Command};
use crate::output::{
    print_comparison, print_csv, print_json, print_text, print_text_colored, write_results_log,
    RunSnapshot,
};
use goku_core::benchmark::{BenchmarkResult, Report};
use goku_core::execution::run;
use goku_core::settings::{OutputFormat, Settings};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle subcommands first
    if let Some(cmd) = &cli.command {
        return handle_subcommand(cmd);
    }

    let settings: Settings = cli.to_settings()?;
    settings.validate()?;

    run_benchmark(settings).await
}

// ---------------------------------------------------------------------------
// Subcommand dispatch
// ---------------------------------------------------------------------------

fn handle_subcommand(cmd: &Command) -> Result<()> {
    match cmd {
        Command::Compare {
            baseline,
            candidate,
        } => {
            let base_raw = std::fs::read_to_string(baseline)
                .map_err(|e| anyhow::anyhow!("Cannot read baseline file '{}': {}", baseline, e))?;
            let cand_raw = std::fs::read_to_string(candidate).map_err(|e| {
                anyhow::anyhow!("Cannot read candidate file '{}': {}", candidate, e)
            })?;

            let base: RunSnapshot = serde_json::from_str(&base_raw).map_err(|e| {
                anyhow::anyhow!("Invalid JSON in baseline file '{}': {}", baseline, e)
            })?;
            let cand: RunSnapshot = serde_json::from_str(&cand_raw).map_err(|e| {
                anyhow::anyhow!("Invalid JSON in candidate file '{}': {}", candidate, e)
            })?;

            print_comparison(&base, &cand);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Benchmark runner
// ---------------------------------------------------------------------------

async fn run_benchmark(settings: Settings) -> Result<()> {
    print_banner(&settings);

    // ── Progress bar ──────────────────────────────────────────────────────
    let pb = build_progress_bar(&settings);

    // ── Channels ──────────────────────────────────────────────────────────
    let (tx_sigint, rx_sigint) = watch::channel(None);
    let channel_capacity = (settings.clients as usize * 2).min(4096);
    let (benchmark_tx, mut benchmark_rx) = mpsc::channel::<BenchmarkResult>(channel_capacity);

    ctrlc::set_handler(move || {
        tx_sigint.send(Some(())).unwrap_or(());
    })?;

    // ── Live stats ─────────────────────────────────────────────────────────
    let live_report: Option<Arc<Mutex<Report>>> = settings.live_stats.map(|interval_secs| {
        let shared = Arc::new(Mutex::new(Report::new(settings.clients)));
        let arc_clone = Arc::clone(&shared);
        let secs = interval_secs;
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(secs)).await;
                if let Ok(r) = arc_clone.lock() {
                    let total = r.hist.len();
                    if total > 0 {
                        eprintln!(
                            "  [live] requests={} rps={:.1} p50={}ms p95={}ms",
                            total,
                            r.requests_per_second(),
                            r.hist.value_at_quantile(0.50),
                            r.hist.value_at_quantile(0.95),
                        );
                    }
                }
            }
        });
        shared
    });

    // ── Spawn workers ──────────────────────────────────────────────────────
    run(settings.clone(), benchmark_tx, Some(rx_sigint)).await?;

    // ── Collect results ────────────────────────────────────────────────────
    let mut report = Report::new(settings.clients);
    while let Some(value) = benchmark_rx.recv().await {
        match settings.verbose {
            true => println!("{}", DisplayableBenchmarkResult(&value)),
            false => {
                if settings.duration.is_none() {
                    pb.inc(1);
                }
            }
        }
        // Update live-stats report if active
        if let Some(live) = &live_report {
            if let Ok(mut r) = live.lock() {
                r.add_result(value.clone());
            }
        }
        report.add_result(value);
    }
    pb.finish_and_clear();

    // ── Output results ─────────────────────────────────────────────────────
    write_output(&settings, &report)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Output writer
// ---------------------------------------------------------------------------

fn write_output(settings: &Settings, report: &Report) -> Result<()> {
    // Determine the output writer (file or stdout)
    let mut file_handle: Option<std::fs::File> = None;
    if let Some(path) = &settings.output_file {
        let f = std::fs::File::create(path)
            .map_err(|e| anyhow::anyhow!("Cannot create output file '{}': {}", path, e))?;
        file_handle = Some(f);
    }

    // Write main results
    match &settings.output {
        OutputFormat::Text => {
            if let Some(f) = &mut file_handle {
                print_text(report, f);
            } else {
                print_text_colored(report);
            }
        }
        OutputFormat::Json => {
            if let Some(f) = &mut file_handle {
                print_json(report, f);
            } else {
                let mut stdout = std::io::stdout();
                print_json(report, &mut stdout);
            }
        }
        OutputFormat::Csv => {
            if let Some(f) = &mut file_handle {
                print_csv(report, f);
            } else {
                let mut stdout = std::io::stdout();
                print_csv(report, &mut stdout);
            }
        }
    }

    // Write per-request log if requested
    if let Some(log_path) = &settings.results_log {
        let mut log_file = std::fs::File::create(log_path)
            .map_err(|e| anyhow::anyhow!("Cannot create results log '{}': {}", log_path, e))?;
        write_results_log(report, &mut log_file);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Banner
// ---------------------------------------------------------------------------

pub fn print_banner(settings: &Settings) {
    let target_display = if settings.steps.is_empty() {
        settings.target.clone()
    } else {
        format!("{} steps", settings.steps.len())
    };

    let banner = match settings.duration {
        None => format!(
            "kamehameha to {} with {} concurrent clients and {} total iterations",
            target_display, settings.clients, settings.requests
        ),
        Some(d) => format!(
            "kamehameha to {} with {} concurrent clients for {} seconds",
            target_display, settings.clients, d
        ),
    };

    let mut extras = vec![];
    if settings.http2 {
        extras.push("HTTP/2".to_string());
    }
    if settings.insecure {
        extras.push("insecure".yellow().to_string());
    }
    if let Some(rps) = settings.rps {
        extras.push(format!("{}rps limit", rps));
    }
    if settings.auth.is_some() {
        extras.push("auth".to_string());
    }

    println!("{}", banner.cyan().bold());
    if !extras.is_empty() {
        println!("  [{}]", extras.join(", "));
    }
}

// ---------------------------------------------------------------------------
// Progress bar builder
// ---------------------------------------------------------------------------

fn build_progress_bar(settings: &Settings) -> ProgressBar {
    match settings.duration {
        None => {
            let bar = ProgressBar::new(settings.requests as u64);
            bar.set_style(
                ProgressStyle::with_template(
                    "{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
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
                    "{spinner:.green} Running for {elapsed_precise} / {msg}",
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
    }
}

// ---------------------------------------------------------------------------
// Verbose display
// ---------------------------------------------------------------------------

struct DisplayableBenchmarkResult<'a>(&'a BenchmarkResult);

impl Display for DisplayableBenchmarkResult<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{} {} {} {}] {} {}{}",
            "Client".bold().green(),
            self.0.num_client.to_string().bold().green(),
            "Iter".bold().green(),
            self.0.execution.to_string().bold().green(),
            self.0.status.to_string().bold().yellow(),
            self.0.duration.to_string().cyan(),
            "ms".cyan(),
        )
    }
}
