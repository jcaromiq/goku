use std::io::Write;

use goku_core::benchmark::{Metrics, Report};

// ---------------------------------------------------------------------------
// Text output (stdout or file)
// ---------------------------------------------------------------------------

pub fn print_text(r: &Report, out: &mut dyn Write) {
    let elapsed = r.start.elapsed();
    let bd = r.status_breakdown();

    let _ = writeln!(out);
    let _ = writeln!(out);

    let _ = writeln!(out, "{:<20} {}", "Concurrency level", r.clients);
    let _ = writeln!(
        out,
        "{:<20} {} seconds",
        "Time taken",
        elapsed.as_secs()
    );
    let _ = writeln!(out, "{:<20} {}", "Total requests", r.hist.len());
    let _ = writeln!(
        out,
        "{:<20} {:.2} req/s",
        "Requests/sec",
        r.requests_per_second()
    );
    let _ = writeln!(
        out,
        "{:<20} {:.2} ms",
        "Mean",
        r.hist.mean()
    );
    let _ = writeln!(out, "{:<20} {} ms", "Min", r.results.min());
    let _ = writeln!(out, "{:<20} {} ms", "Max", r.results.max());
    let _ = writeln!(
        out,
        "{:<20} {} ms",
        "p50 (median)",
        r.hist.value_at_quantile(0.50)
    );
    let _ = writeln!(
        out,
        "{:<20} {} ms",
        "p95",
        r.hist.value_at_quantile(0.95)
    );
    let _ = writeln!(
        out,
        "{:<20} {} ms",
        "p99",
        r.hist.value_at_quantile(0.99)
    );
    let _ = writeln!(
        out,
        "{:<20} {} ms",
        "p99.9",
        r.hist.value_at_quantile(0.999)
    );

    let _ = writeln!(out);
    let _ = writeln!(out, "{}", "Status codes");
    let _ = writeln!(out, "  2xx  {}", bd.success);
    if bd.client_error > 0 {
        let _ = writeln!(out, "  4xx  {}", bd.client_error);
    }
    if bd.server_error > 0 {
        let _ = writeln!(out, "  5xx  {}", bd.server_error);
    }
    if bd.other > 0 {
        let _ = writeln!(out, "  other  {}", bd.other);
    }
    if bd.network_error > 0 {
        let _ = writeln!(out, "  network errors  {}", bd.network_error);
    }

    // ASCII latency histogram
    let buckets = r.latency_histogram(10);
    if !buckets.is_empty() {
        let _ = writeln!(out);
        let _ = writeln!(out, "Latency distribution");
        let max_count = buckets.iter().map(|(_, c)| *c).max().unwrap_or(1).max(1);
        let bar_width = 40usize;
        for (label, count) in &buckets {
            let filled = ((*count as f64 / max_count as f64) * bar_width as f64).round() as usize;
            let bar: String = "█".repeat(filled);
            let _ = writeln!(out, "  {:>8}  {:<40}  {}", label, bar, count);
        }
    }
}

/// Print text to stdout with ANSI colors (normal interactive use).
pub fn print_text_colored(r: &Report) {
    use colored::Colorize;

    let elapsed = r.start.elapsed();
    let bd = r.status_breakdown();

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
        "p99             ".yellow().bold(),
        r.hist.value_at_quantile(0.99).to_string().purple(),
        "ms".purple()
    );
    println!(
        "{} {} {}",
        "p99.9           ".yellow().bold(),
        r.hist.value_at_quantile(0.999).to_string().purple(),
        "ms".purple()
    );

    println!();
    println!("{}", "Status codes".yellow().bold());
    println!("  {} {}", "2xx".green().bold(), bd.success.to_string().purple());
    if bd.client_error > 0 {
        println!(
            "  {} {}",
            "4xx".yellow().bold(),
            bd.client_error.to_string().purple()
        );
    }
    if bd.server_error > 0 {
        println!(
            "  {} {}",
            "5xx".red().bold(),
            bd.server_error.to_string().purple()
        );
    }
    if bd.other > 0 {
        println!(
            "  {} {}",
            "other".cyan().bold(),
            bd.other.to_string().purple()
        );
    }
    if bd.network_error > 0 {
        println!(
            "  {} {}",
            "network errors".red().bold(),
            bd.network_error.to_string().purple()
        );
    }

    // ASCII latency histogram
    let buckets = r.latency_histogram(10);
    if !buckets.is_empty() {
        println!();
        println!("{}", "Latency distribution".yellow().bold());
        let max_count = buckets.iter().map(|(_, c)| *c).max().unwrap_or(1).max(1);
        let bar_width = 40usize;
        for (label, count) in &buckets {
            let filled =
                ((*count as f64 / max_count as f64) * bar_width as f64).round() as usize;
            let bar: String = "█".repeat(filled);
            println!(
                "  {:>8}  {}  {}",
                label.cyan(),
                format!("{:<40}", bar).green(),
                count.to_string().purple()
            );
        }
    }
}

// ---------------------------------------------------------------------------
// JSON output
// ---------------------------------------------------------------------------

pub fn print_json(r: &Report, out: &mut dyn Write) {
    let elapsed = r.start.elapsed().as_secs_f64();
    let bd = r.status_breakdown();
    let min = r.results.iter().map(|x| x.duration).min().unwrap_or(0);
    let max = r.results.iter().map(|x| x.duration).max().unwrap_or(0);

    let data = serde_json::json!({
        "concurrency": r.clients,
        "duration_secs": format!("{:.3}", elapsed).parse::<f64>().unwrap_or(0.0),
        "total_requests": r.hist.len(),
        "requests_per_sec": format!("{:.2}", r.requests_per_second()).parse::<f64>().unwrap_or(0.0),
        "mean_ms": format!("{:.2}", r.hist.mean()).parse::<f64>().unwrap_or(0.0),
        "min_ms": min,
        "max_ms": max,
        "p50_ms": r.hist.value_at_quantile(0.50),
        "p95_ms": r.hist.value_at_quantile(0.95),
        "p99_ms": r.hist.value_at_quantile(0.99),
        "p999_ms": r.hist.value_at_quantile(0.999),
        "status_2xx": bd.success,
        "status_4xx": bd.client_error,
        "status_5xx": bd.server_error,
        "status_other": bd.other,
        "network_errors": bd.network_error,
    });

    let json_str = serde_json::to_string_pretty(&data)
        .unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"));
    let _ = writeln!(out, "{}", json_str);
}

// ---------------------------------------------------------------------------
// CSV output
// ---------------------------------------------------------------------------

pub fn print_csv(r: &Report, out: &mut dyn Write) {
    let elapsed = r.start.elapsed().as_secs_f64();
    let bd = r.status_breakdown();
    let min = r.results.iter().map(|x| x.duration).min().unwrap_or(0);
    let max = r.results.iter().map(|x| x.duration).max().unwrap_or(0);

    let _ = writeln!(
        out,
        "concurrency,duration_secs,total_requests,requests_per_sec,mean_ms,min_ms,max_ms,\
p50_ms,p95_ms,p99_ms,p999_ms,status_2xx,status_4xx,status_5xx,status_other,network_errors"
    );
    let _ = writeln!(
        out,
        "{},{:.3},{},{:.2},{:.2},{},{},{},{},{},{},{},{},{},{},{}",
        r.clients,
        elapsed,
        r.hist.len(),
        r.requests_per_second(),
        r.hist.mean(),
        min,
        max,
        r.hist.value_at_quantile(0.50),
        r.hist.value_at_quantile(0.95),
        r.hist.value_at_quantile(0.99),
        r.hist.value_at_quantile(0.999),
        bd.success,
        bd.client_error,
        bd.server_error,
        bd.other,
        bd.network_error,
    );
}

// ---------------------------------------------------------------------------
// Results log (per-request CSV)
// ---------------------------------------------------------------------------

pub fn write_results_log(r: &Report, out: &mut dyn Write) {
    let _ = writeln!(out, "timestamp_ms,num_client,execution,status,latency_ms");
    for result in &r.results {
        let _ = writeln!(
            out,
            "{},{},{},{},{}",
            result.timestamp_ms,
            result.num_client,
            result.execution,
            result.status,
            result.duration,
        );
    }
}

// ---------------------------------------------------------------------------
// Run comparison
// ---------------------------------------------------------------------------

#[allow(dead_code)]
#[derive(Debug, serde::Deserialize)]
pub struct RunSnapshot {
    pub concurrency: Option<u64>,
    pub duration_secs: Option<f64>,
    pub total_requests: Option<u64>,
    pub requests_per_sec: Option<f64>,
    pub mean_ms: Option<f64>,
    pub min_ms: Option<u64>,
    pub max_ms: Option<u64>,
    pub p50_ms: Option<u64>,
    pub p95_ms: Option<u64>,
    pub p99_ms: Option<u64>,
    pub p999_ms: Option<u64>,
    pub status_2xx: Option<u64>,
    pub status_4xx: Option<u64>,
    pub status_5xx: Option<u64>,
    pub network_errors: Option<u64>,
}

pub fn print_comparison(baseline: &RunSnapshot, candidate: &RunSnapshot) {
    use colored::Colorize;

    fn fmt_pct(base: f64, cand: f64, lower_is_better: bool) -> String {
        if base == 0.0 {
            return "N/A".dimmed().to_string();
        }
        let pct = (cand - base) / base * 100.0;
        let label = format!("{:+.1}%", pct);
        let improvement = if lower_is_better { pct < 0.0 } else { pct > 0.0 };
        let regression = if lower_is_better { pct > 5.0 } else { pct < -5.0 };
        if improvement {
            label.green().to_string()
        } else if regression {
            label.red().to_string()
        } else {
            label.yellow().to_string()
        }
    }

    println!();
    println!("{}", "Benchmark Comparison".bold().cyan());
    println!("{}", "═".repeat(72).dimmed());
    println!(
        "{:<22} {:>12} {:>12} {:>12}",
        "Metric".bold(),
        "Baseline".bold(),
        "Candidate".bold(),
        "Change".bold()
    );
    println!("{}", "─".repeat(72).dimmed());

    macro_rules! row_f64 {
        ($label:expr, $field:ident, $lower:expr) => {
            let base = baseline.$field.unwrap_or(0.0);
            let cand = candidate.$field.unwrap_or(0.0);
            println!(
                "{:<22} {:>12.2} {:>12.2} {:>12}",
                $label,
                base,
                cand,
                fmt_pct(base, cand, $lower)
            );
        };
    }
    macro_rules! row_u64 {
        ($label:expr, $field:ident, $lower:expr) => {
            let base = baseline.$field.unwrap_or(0) as f64;
            let cand = candidate.$field.unwrap_or(0) as f64;
            println!(
                "{:<22} {:>12.0} {:>12.0} {:>12}",
                $label,
                base,
                cand,
                fmt_pct(base, cand, $lower)
            );
        };
    }

    row_f64!("Requests/sec", requests_per_sec, false);
    row_f64!("Mean (ms)", mean_ms, true);
    row_u64!("p50 (ms)", p50_ms, true);
    row_u64!("p95 (ms)", p95_ms, true);
    row_u64!("p99 (ms)", p99_ms, true);
    row_u64!("p99.9 (ms)", p999_ms, true);
    row_u64!("Min (ms)", min_ms, true);
    row_u64!("Max (ms)", max_ms, true);
    row_u64!("Total requests", total_requests, false);
    row_u64!("2xx", status_2xx, false);
    row_u64!("4xx", status_4xx, true);
    row_u64!("5xx", status_5xx, true);
    row_u64!("Network errors", network_errors, true);

    println!("{}", "═".repeat(72).dimmed());
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use goku_core::benchmark::{BenchmarkResult, Report};

    fn make_report() -> Report {
        let mut r = Report::new(2);
        for (d, s) in [
            (10, "200 OK"),
            (20, "200 OK"),
            (50, "200 OK"),
            (100, "500 Internal Server Error"),
        ] {
            r.add_result(BenchmarkResult {
                status: s.to_string(),
                duration: d,
                num_client: 0,
                execution: 0,
                timestamp_ms: 0,
            });
        }
        r
    }

    #[test]
    fn json_output_is_valid_json() {
        let r = make_report();
        let mut out = Vec::new();
        print_json(&r, &mut out);
        let s = String::from_utf8(out).unwrap();
        assert!(serde_json::from_str::<serde_json::Value>(&s).is_ok());
    }

    #[test]
    fn json_output_contains_expected_keys() {
        let r = make_report();
        let mut out = Vec::new();
        print_json(&r, &mut out);
        let s = String::from_utf8(out).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert!(v.get("p95_ms").is_some());
        assert!(v.get("status_5xx").is_some());
        assert!(v.get("p99_ms").is_some());
    }

    #[test]
    fn csv_output_has_header_row() {
        let r = make_report();
        let mut out = Vec::new();
        print_csv(&r, &mut out);
        let s = String::from_utf8(out).unwrap();
        assert!(s.starts_with("concurrency,"));
        let lines: Vec<&str> = s.trim().lines().collect();
        assert_eq!(lines.len(), 2); // header + data row
    }

    #[test]
    fn results_log_has_header_and_data() {
        let r = make_report();
        let mut out = Vec::new();
        write_results_log(&r, &mut out);
        let s = String::from_utf8(out).unwrap();
        assert!(s.starts_with("timestamp_ms,"));
        let lines: Vec<&str> = s.trim().lines().collect();
        assert_eq!(lines.len(), 5); // header + 4 results
    }
}
