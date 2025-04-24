use hdrhistogram::Histogram;
use std::collections::HashMap;
use tokio::time::Instant;

pub trait Metrics {
    fn avg(&self) -> u64;
    fn max(&self) -> u64;
    fn min(&self) -> u64;
}

impl Metrics for Vec<BenchmarkResult> {
    fn avg(&self) -> u64 {
        let total: u64 = self.iter().map(|r| r.duration).sum();
        let size: u64 = self.iter().len() as u64;
        total / size
    }
    fn max(&self) -> u64 {
        self.iter().map(|r| r.duration).max().unwrap_or(0)
    }
    fn min(&self) -> u64 {
        self.iter().map(|r| r.duration).min().unwrap_or(0)
    }
}

#[derive(Debug)]
pub struct BenchmarkResult {
    pub status: String,
    pub duration: u64,
    pub execution: usize,
    pub num_client: usize,
}

#[derive(Debug)]
pub struct Report {
    pub clients: usize,
    pub results: Vec<BenchmarkResult>,
    pub hist: Histogram<u64>,
    pub start: Instant,
}

impl Report {
    pub fn new(clients: usize) -> Self {
        Report {
            clients,
            results: vec![],
            hist: Histogram::<u64>::new(5).unwrap(),
            start: Instant::now(),
        }
    }
    pub fn add_result(&mut self, result: BenchmarkResult) {
        let duration = result.duration;
        self.results.push(result);
        self.hist.record(duration).expect("");
    }
    
    pub fn oks(&self) -> HashMap<&String, usize> {
        let mut frequencies = HashMap::new();
        for r in &self.results {
            *frequencies.entry(&r.status).or_insert(0) += 1;
        }
        frequencies
        
    }
}
