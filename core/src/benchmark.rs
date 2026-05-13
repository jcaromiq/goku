use hdrhistogram::Histogram;
use tokio::time::Instant;

// ---------------------------------------------------------------------------
// Traits
// ---------------------------------------------------------------------------

pub trait Metrics {
    fn avg(&self) -> u64;
    fn max(&self) -> u64;
    fn min(&self) -> u64;
}

impl Metrics for Vec<BenchmarkResult> {
    fn avg(&self) -> u64 {
        if self.is_empty() {
            return 0;
        }
        let total: u64 = self.iter().map(|r| r.duration).sum();
        total / self.len() as u64
    }

    fn max(&self) -> u64 {
        self.iter().map(|r| r.duration).max().unwrap_or(0)
    }

    fn min(&self) -> u64 {
        self.iter().map(|r| r.duration).min().unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// BenchmarkResult
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub status: String,
    pub duration: u64,
    /// Which worker produced this result.
    pub num_client: usize,
    /// Sequence number within the worker.
    pub execution: u32,
    /// Unix timestamp (ms) when the request started.
    pub timestamp_ms: u64,
}

// ---------------------------------------------------------------------------
// StatusBreakdown
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct StatusBreakdown {
    pub success: usize,      // 2xx
    pub client_error: usize, // 4xx
    pub server_error: usize, // 5xx
    pub network_error: usize, // timeouts, connection refused, etc.
    pub other: usize,        // 1xx, 3xx or any other
}

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct Report {
    pub clients: u32,
    pub results: Vec<BenchmarkResult>,
    pub hist: Histogram<u64>,
    pub start: Instant,
}

impl Report {
    pub fn new(clients: u32) -> Self {
        Report {
            clients,
            results: vec![],
            // sigfig = 3 gives ~0.1% precision, plenty for latency histograms
            hist: Histogram::<u64>::new(3).expect("Failed to create HDR histogram"),
            start: Instant::now(),
        }
    }

    pub fn add_result(&mut self, result: BenchmarkResult) {
        let duration = result.duration;
        self.results.push(result);
        // Saturate at u64::MAX rather than panic on out-of-range values.
        let _ = self.hist.record(duration);
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

    /// Returns an ASCII-art latency histogram across `n_buckets` buckets.
    /// Each bucket is labelled with the upper bound (ms) and a bar proportional to count.
    pub fn latency_histogram(&self, n_buckets: usize) -> Vec<(String, u64)> {
        if self.results.is_empty() || n_buckets == 0 {
            return vec![];
        }
        let min = self.results.iter().map(|r| r.duration).min().unwrap_or(0);
        let max = self.results.iter().map(|r| r.duration).max().unwrap_or(0);
        if min == max {
            return vec![(format!("{}ms", max), self.results.len() as u64)];
        }

        let range = max - min;
        let bucket_width = (range as f64 / n_buckets as f64).ceil() as u64;
        let bucket_width = bucket_width.max(1);

        let mut counts = vec![0u64; n_buckets];
        for r in &self.results {
            let idx = ((r.duration - min) / bucket_width) as usize;
            let idx = idx.min(n_buckets - 1);
            counts[idx] += 1;
        }

        counts
            .iter()
            .enumerate()
            .map(|(i, &count)| {
                let upper = min + (i as u64 + 1) * bucket_width;
                (format!("{}ms", upper), count)
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_result(duration: u64, status: &str) -> BenchmarkResult {
        BenchmarkResult {
            status: status.to_string(),
            duration,
            num_client: 0,
            execution: 0,
            timestamp_ms: 0,
        }
    }

    // --- Metrics trait ---

    #[test]
    fn avg_empty_vec_returns_zero() {
        let v: Vec<BenchmarkResult> = vec![];
        assert_eq!(v.avg(), 0);
    }

    #[test]
    fn min_empty_vec_returns_zero() {
        let v: Vec<BenchmarkResult> = vec![];
        assert_eq!(v.min(), 0);
    }

    #[test]
    fn max_empty_vec_returns_zero() {
        let v: Vec<BenchmarkResult> = vec![];
        assert_eq!(v.max(), 0);
    }

    #[test]
    fn avg_correct_value() {
        let v = vec![
            make_result(10, "200 OK"),
            make_result(20, "200 OK"),
            make_result(30, "200 OK"),
        ];
        assert_eq!(v.avg(), 20);
    }

    #[test]
    fn min_correct_value() {
        let v = vec![make_result(5, "200 OK"), make_result(15, "200 OK")];
        assert_eq!(v.min(), 5);
    }

    #[test]
    fn max_correct_value() {
        let v = vec![make_result(5, "200 OK"), make_result(15, "200 OK")];
        assert_eq!(v.max(), 15);
    }

    // --- StatusBreakdown ---

    #[test]
    fn status_breakdown_counts_correctly() {
        let mut report = Report::new(1);
        report.add_result(make_result(10, "200 OK"));
        report.add_result(make_result(10, "201 Created"));
        report.add_result(make_result(10, "404 Not Found"));
        report.add_result(make_result(10, "500 Internal Server Error"));
        report.add_result(make_result(10, "Failed to connect"));

        let bd = report.status_breakdown();
        assert_eq!(bd.success, 2);
        assert_eq!(bd.client_error, 1);
        assert_eq!(bd.server_error, 1);
        assert_eq!(bd.network_error, 1);
        assert_eq!(bd.other, 0);
    }

    #[test]
    fn status_breakdown_empty_report() {
        let report = Report::new(1);
        let bd = report.status_breakdown();
        assert_eq!(bd.success, 0);
        assert_eq!(bd.network_error, 0);
    }

    // --- Report ---

    #[test]
    fn add_result_increments_histogram() {
        let mut report = Report::new(2);
        report.add_result(make_result(100, "200 OK"));
        report.add_result(make_result(200, "200 OK"));
        assert_eq!(report.hist.len(), 2);
    }

    #[test]
    fn latency_histogram_empty_report() {
        let report = Report::new(1);
        assert!(report.latency_histogram(5).is_empty());
    }

    #[test]
    fn latency_histogram_produces_correct_bucket_count() {
        let mut report = Report::new(1);
        for d in [10, 20, 30, 40, 50] {
            report.add_result(make_result(d, "200 OK"));
        }
        let hist = report.latency_histogram(5);
        assert_eq!(hist.len(), 5);
        let total: u64 = hist.iter().map(|(_, c)| c).sum();
        assert_eq!(total, 5);
    }
}
