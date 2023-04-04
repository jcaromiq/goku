use std::str::FromStr;

use crate::benchmark::BenchmarkResult;
use crate::settings::{Operation, Settings};
use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::Client;
use tokio::sync::mpsc::Sender;
use tokio::sync::watch::Receiver;
use tokio::time::Instant;

pub async fn run(
    settings: Settings,
    tx: Sender<BenchmarkResult>,
    rx_sigint: Receiver<Option<()>>,
) -> Result<()> {
    let mut clients = Vec::with_capacity(settings.clients);
    for _ in 0..settings.clients {
        let client = Client::builder()
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
    mut rx_sigint: Receiver<Option<()>>,
) {
    for i in 0..settings.requests_by_client() {
        let stop_signal = rx_sigint.changed();
        let benchmark_result = exec(num_client, i, &client, &settings);
        let ack_send_result = tx.send(benchmark_result.await);

        match tokio::select! {
        _ = ack_send_result =>  None,
        _ = stop_signal => Some(())
        } {
            None => {}
            Some(_) => break,
        }
    }
}

async fn exec(
    num_client: usize,
    execution: usize,
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
            status: r.status().as_u16(),
            duration: duration_ms,
            num_client,
            execution,
        },
        Err(e) => {
            let status = match e.status() {
                None => 0,
                Some(s) => s.as_u16(),
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
