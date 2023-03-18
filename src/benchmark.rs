use colored::Colorize;
use hdrhistogram::Histogram;
use std::fmt::{Display, Formatter};
use std::time::Duration;

pub trait Average {
    fn avg(&self) -> u64;
}

impl Average for Vec<BenchmarkResult> {
    fn avg(&self) -> u64 {
        let total: u64 = self.iter().map(|r| r.duration).sum();
        let size: u64 = self.iter().len() as u64;
        total / size
    }
}

impl Display for BenchmarkResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let report = format!(
            "[{} {} {} {}] {} {}{}",
            "Client".bold().green(),
            self.num_client.to_string().bold().green(),
            "Iteration".bold().green(),
            self.execution.to_string().bold().green(),
            self.status.to_string().bold().yellow(),
            self.duration.to_string().cyan(),
            "ms".cyan()
        );
        write!(f, "{}", report)
    }
}

#[derive(Debug)]
pub struct BenchmarkResult {
    pub status: u16,
    pub duration: u64,
    pub execution: usize,
    pub num_client: usize,
}

#[derive(Debug)]
pub struct Report {
    clients: usize,
    results: Vec<BenchmarkResult>,
    hist: Histogram<u64>,
}

impl Report {
    pub fn new(clients: usize) -> Self {
        Report {
            clients,
            results: vec![],
            hist: Histogram::<u64>::new(1).unwrap(),
        }
    }
    pub fn add_result(&mut self, result: BenchmarkResult) {
        let duration = result.duration;
        self.results.push(result);
        self.hist.record(duration).expect("");
    }

    pub fn show_results(&self, elapsed: Duration) {
        println!();
        println!(
            "{} {}",
            "Concurrency level".yellow().bold(),
            self.clients.to_string().purple()
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
            self.hist.len().to_string().purple()
        );
        println!(
            "{} {} {}",
            "Mean request time".yellow().bold(),
            self.hist.mean().to_string().purple(),
            "ms".purple()
        );
        println!(
            "{} {} {}",
            "Max request time".yellow().bold(),
            self.hist.max().to_string().purple(),
            "ms".purple()
        );
        println!(
            "{} {} {}",
            "Min request time".yellow().bold(),
            self.hist.min().to_string().purple(),
            "ms".purple()
        );
        println!(
            "{} {} {}",
            "95'th percentile:".yellow().bold(),
            self.hist.value_at_quantile(0.95).to_string().purple(),
            "ms".purple()
        );
        println!(
            "{} {} {}",
            "99.9'th percentile:".yellow().bold(),
            self.hist.value_at_quantile(0.999).to_string().purple(),
            "ms".purple()
        );
    }
}
