use goku_core::benchmark::Report;


pub fn print_json(r: &Report) {
    let elapsed = r.start.elapsed().as_secs_f64();
    let bd = r.status_breakdown();
    let min = r.results.iter().map(|x| x.duration).min().unwrap_or(0);
    let max = r.results.iter().map(|x| x.duration).max().unwrap_or(0);

    let data = serde_json::json!({
        "concurrency": r.clients,
        "duration_secs": elapsed,
        "total_requests": r.hist.len(),
        "requests_per_sec": format!("{:.2}", r.requests_per_second()).parse::<f64>().unwrap_or(0.0),
        "mean_ms": format!("{:.2}", r.hist.mean()).parse::<f64>().unwrap_or(0.0),
        "min_ms": min,
        "max_ms": max,
        "p50_ms": r.hist.value_at_quantile(0.50),
        "p95_ms": r.hist.value_at_quantile(0.95),
        "p999_ms": r.hist.value_at_quantile(0.999),
        "status_2xx": bd.success,
        "status_4xx": bd.client_error,
        "status_5xx": bd.server_error,
        "status_other": bd.other,
        "network_errors": bd.network_error,
    });
    println!("{}", serde_json::to_string_pretty(&data).unwrap());
}

pub fn print_csv(r: &Report) {
    let elapsed = r.start.elapsed().as_secs_f64();
    let bd = r.status_breakdown();
    let min = r.results.iter().map(|x| x.duration).min().unwrap_or(0);
    let max = r.results.iter().map(|x| x.duration).max().unwrap_or(0);
    // Header
    println!(
        "concurrency,duration_secs,total_requests,requests_per_sec,mean_ms,min_ms,max_ms,\
p50_ms,p95_ms,p999_ms,status_2xx,status_4xx,status_5xx,status_other,network_errors"
    );
    // Data row
    println!(
        "{},{:.3},{},{:.2},{:.2},{},{},{},{},{},{},{},{},{},{}",
        r.clients,
        elapsed,
        r.hist.len(),
        r.requests_per_second(),
        r.hist.mean(),
        min,
        max,
        r.hist.value_at_quantile(0.50),
        r.hist.value_at_quantile(0.95),
        r.hist.value_at_quantile(0.999),
        bd.success,
        bd.client_error,
        bd.server_error,
        bd.other,
        bd.network_error,
    );
}
