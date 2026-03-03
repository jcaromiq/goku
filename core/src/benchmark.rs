use hdrhistogram::Histogram;
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
    pub execution: i32,
    pub num_client: usize,
}

#[derive(Debug, Default)]
pub struct StatusBreakdown {
    pub success: usize,    // 2xx
    pub client_error: usize, // 4xx
    pub server_error: usize, // 5xx
    pub network_error: usize, // timeouts, connection refused, etc.
    pub other: usize,      // 1xx, 3xx, o cualquier otro
}

#[derive(Debug)]
pub struct Report {
    pub clients: i32,
    pub results: Vec<BenchmarkResult>,
    pub hist: Histogram<u64>,
    pub start: Instant,
}

impl Report {
    pub fn new(clients: i32) -> Self {
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

    pub fn requests_per_second(&self) -> f64 {
        let elapsed = self.start.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.hist.len() as f64 / elapsed
        } else {
            0.0
        }
    }

    pub fn status_breakdown(&self) -> StatusBreakdown {
        let mut breakdown = StatusBreakdown::default();
        for r in &self.results {
            match r.status.chars().next().and_then(|c| c.to_digit(10)) {
                Some(2) => breakdown.success += 1,
                Some(4) => breakdown.client_error += 1,
                Some(5) => breakdown.server_error += 1,
                Some(1) | Some(3) => breakdown.other += 1,
                _ => breakdown.network_error += 1, 
            }
        }
        breakdown
    }
}
