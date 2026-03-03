use std::str::FromStr;
use std::sync::Arc;

use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Client, Url};
use tokio::sync::mpsc::Sender;
use tokio::sync::watch::Receiver;
use tokio::time::Instant;

use crate::benchmark::BenchmarkResult;
use crate::settings::{Operation, Settings};

pub async fn run(
    settings: Settings,
    tx: Sender<BenchmarkResult>,
    rx_sigint: Option<Receiver<Option<()>>>,
) -> Result<()> {
    let mut builder = Client::builder()
        .timeout(settings.timeout)
        .danger_accept_invalid_certs(true)
        .tcp_keepalive(settings.keep_alive)
        .pool_max_idle_per_host(settings.clients as usize)
        .pool_idle_timeout(std::time::Duration::from_secs(90));

    if settings.http2 {
        builder = builder.http2_prior_knowledge();
    }

    let client = Arc::new(
        builder
            .build()
            .with_context(|| "Can not create http Client".to_string())?,
    );

    let settings = Arc::new(settings);

    let headers_map: Arc<HeaderMap> = Arc::new(build_headers(&settings));

    let url: Arc<Url> = Arc::new(
        settings
            .target()
            .parse::<Url>()
            .with_context(|| format!("Invalid URL: {}", settings.target()))?,
    );

    for id in 0..settings.clients {
        tokio::spawn(exec_iterator(
            id as usize,
            Arc::clone(&settings),
            Arc::clone(&client),
            Arc::clone(&url),
            Arc::clone(&headers_map),
            tx.clone(),
            rx_sigint.clone(),
        ));
    }
    Ok(())
}

fn build_headers(settings: &Settings) -> HeaderMap {
    match &settings.headers {
        None => HeaderMap::new(),
        Some(headers) => {
            let mut map = HeaderMap::with_capacity(headers.len());
            for h in headers {
                let name = HeaderName::from_str(h.key.as_str()).unwrap();
                let value = HeaderValue::from_str(h.value.as_str()).unwrap();
                map.insert(name, value);
            }
            map
        }
    }
}

async fn exec_iterator(
    num_client: usize,
    settings: Arc<Settings>,
    client: Arc<Client>,
    url: Arc<Url>,
    headers_map: Arc<HeaderMap>,
    tx: Sender<BenchmarkResult>,
    mut rx_sigint: Option<Receiver<Option<()>>>,
) {
    match settings.duration {
        None => {
            by_iterations(num_client, &settings, &client, &url, &headers_map, &tx, &mut rx_sigint)
                .await;
        }
        Some(duration) => {
            by_time(
                num_client,
                &settings,
                &client,
                &url,
                &headers_map,
                tx,
                &mut rx_sigint,
                duration,
            )
            .await;
        }
    }
}

async fn by_time(
    num_client: usize,
    settings: &Settings,
    client: &Client,
    url: &Url,
    headers_map: &HeaderMap,
    tx: Sender<BenchmarkResult>,
    rx_sigint: &mut Option<Receiver<Option<()>>>,
    duration: u64,
) {
    let begin = Instant::now();
    let mut execution_number = 0;
    while begin.elapsed().as_secs() < duration {
        match rx_sigint {
            None => {
                let benchmark_result = exec(num_client, execution_number, client, url, headers_map, settings);
                let _ = tx.send(benchmark_result.await).await;
                execution_number += 1;
            }
            Some(rx) => {
                let stop_signal = rx.changed();
                let benchmark_result = exec(num_client, execution_number, client, url, headers_map, settings);
                let ack_send_result = tx.send(benchmark_result.await);
                execution_number += 1;
                match tokio::select! {
                    _ = ack_send_result => None,
                    _ = stop_signal => Some(())
                } {
                    None => {}
                    Some(_) => break,
                }
            }
        }
    }
}

async fn by_iterations(
    num_client: usize,
    settings: &Settings,
    client: &Client,
    url: &Url,
    headers_map: &HeaderMap,
    tx: &Sender<BenchmarkResult>,
    rx_sigint: &mut Option<Receiver<Option<()>>>,
) {
    for execution_number in 0..settings.requests_by_client() {
        match rx_sigint {
            None => {
                let benchmark_result =
                    exec(num_client, execution_number, client, url, headers_map, settings).await;
                let _ = tx.send(benchmark_result).await;
            }
            Some(rx) => {
                let benchmark_result =
                    exec(num_client, execution_number, client, url, headers_map, settings).await;
                let stop_signal = rx.changed();
                let ack_send_result = tx.send(benchmark_result);

                match tokio::select! {
                    _ = ack_send_result => None,
                    _ = stop_signal => Some(())
                } {
                    None => {}
                    Some(_) => break,
                }
            }
        }
    }
}

async fn exec(
    num_client: usize,
    execution: i32,
    client: &Client,
    url: &Url,
    headers_map: &HeaderMap,
    settings: &Settings,
) -> BenchmarkResult {
    let request_builder = match settings.operation() {
        Operation::Get => client.get(url.clone()),
        Operation::Post => client.post(url.clone()),
        Operation::Head => client.head(url.clone()),
        Operation::Patch => client.patch(url.clone()),
        Operation::Put => client.put(url.clone()),
        Operation::Delete => client.delete(url.clone()),
    };

    let request_builder = match &settings.body {
        None => request_builder,
        Some(body) => request_builder.body(body.clone()),
    };

    let request = request_builder.headers(headers_map.clone());
    let begin = Instant::now();
    let response = request.send().await;
    let duration_ms = begin.elapsed().as_millis() as u64;
    match response {
        Ok(r) => BenchmarkResult {
            status: r.status().to_string(),
            duration: duration_ms,
            num_client,
            execution,
        },
        Err(e) => {
            let status = match e.status() {
                None => "Failed to connect".to_string(),
                Some(s) => s.to_string(),
            };
            BenchmarkResult {
                status,
                duration: duration_ms,
                num_client,
                execution,
            }
        }
    }
}
