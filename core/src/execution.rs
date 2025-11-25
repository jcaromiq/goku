use std::str::FromStr;

use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::Client;
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
    let mut clients = Vec::with_capacity(settings.clients as usize);
    for _ in 0..settings.clients {
        let client = Client::builder()
            .timeout(settings.timeout)
            .danger_accept_invalid_certs(true)
            .tcp_keepalive(settings.keep_alive)
            .build()
            .with_context(|| "Can not create http Client".to_string())?;
        clients.push(client);
    }
    for (id, client) in clients.into_iter().enumerate() {
        tokio::spawn(exec_iterator(
            id,
            settings.clone(),
            client,
            tx.clone(),
            rx_sigint.clone(),
        ));
    }
    Ok(())
}

async fn exec_iterator(
    num_client: usize,
    settings: Settings,
    client: Client,
    tx: Sender<BenchmarkResult>,
    mut rx_sigint: Option<Receiver<Option<()>>>,
) {
    match settings.duration {
        None => {
            by_iterations(num_client, &settings, &client, &tx, &mut rx_sigint).await;
        }
        Some(duration) => {
            by_time(num_client, &settings, &client, tx, &mut rx_sigint, duration).await;
        }
    }
}

async fn by_time(
    num_client: usize,
    settings: &Settings,
    client: &Client,
    tx: Sender<BenchmarkResult>,
    rx_sigint: &mut Option<Receiver<Option<()>>>,
    duration: u64,
) {
    let begin = Instant::now();
    let mut execution_number = 0;
    while begin.elapsed().as_secs() < duration {
        match rx_sigint {
            None => {
                let benchmark_result = exec(num_client, execution_number, client, settings);
                let _ = tx.send(benchmark_result.await).await;
                execution_number += 1;
            }
            Some(rx) => {
                let stop_signal = rx.changed();
                let benchmark_result = exec(num_client, execution_number, client, settings);
                let ack_send_result = tx.send(benchmark_result.await);
                execution_number += 1;
                match tokio::select! {
                _ = ack_send_result =>  None,
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
    tx: &Sender<BenchmarkResult>,
    rx_sigint: &mut Option<Receiver<Option<()>>>,
) {
    for execution_number in 0..settings.requests_by_client() {
        match rx_sigint {
            None => {
                let benchmark_result = exec(num_client, execution_number, client, settings).await;
                let _ = tx.send(benchmark_result).await;
            }
            Some(rx) => {
                let benchmark_result = exec(num_client, execution_number, client, settings).await;
                let stop_signal = rx.changed();
                let ack_send_result = tx.send(benchmark_result);

                match tokio::select! {
                    _ = ack_send_result =>  None,
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
    settings: &Settings,
) -> BenchmarkResult {
    let request_builder = match settings.operation() {
        Operation::Get => client.get(settings.target()),
        Operation::Post => client.post(settings.target()),
        Operation::Head => client.head(settings.target()),
        Operation::Patch => client.patch(settings.target()),
        Operation::Put => client.put(settings.target()),
        Operation::Delete => client.delete(settings.target()),
    };
    let headers_map: HeaderMap = match &settings.headers {
        None => HeaderMap::new(),
        Some(headers) => {
            let mut headers_map: HeaderMap = HeaderMap::new();
            headers.iter().for_each(|h| {
                let name = h.key.as_str();
                let value = h.value.as_str();

                let name = HeaderName::from_str(name).unwrap();
                let value = HeaderValue::from_str(value).unwrap();
                headers_map.insert(name, value);
            });
            headers_map
        }
    };
    let request_builder = match &settings.body {
        None => request_builder,
        Some(body) => request_builder.body(body.to_string()),
    };
    let request = request_builder.headers(headers_map);
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
