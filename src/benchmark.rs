use colored::Colorize;
use std::fmt::{Display, Formatter};

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
    results: Vec<BenchmarkResult>,
}

impl Report {
    pub fn new() -> Self {
        Report { results: vec![] }
    }
    pub fn add_result(&mut self, result: BenchmarkResult) {
        self.results.push(result);
    }
}
