use std::str::FromStr;

use crate::benchmark::BenchmarkResult;
use crate::settings::{Operation, Settings};
use anyhow::{Context, Result};
use colored::Colorize;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::Client;
use tokio::sync::mpsc::Sender;
use tokio::time::Instant;

pub async fn run(settings: Settings, tx: Sender<BenchmarkResult>) -> Result<()> {
    let mut clients = Vec::with_capacity(settings.clients);
    for _ in 0..settings.clients {
        let client = Client::builder()
            .tcp_keepalive(settings.keep_alive)
            .build()
            .with_context(|| "Can not create http Client".to_string())?;
        clients.push(client);
    }
    for (id, client) in clients.into_iter().enumerate() {
        tokio::spawn(exec_iterator(id, settings.clone(), client, tx.clone()));
    }
    Ok(())
}

async fn exec_iterator(
    num_client: usize,
    settings: Settings,
    client: Client,
    tx: Sender<BenchmarkResult>,
) {
    for i in 0..settings.requests_by_client() {
        let r = exec(num_client, i, &client, &settings).await;
        tx.send(r).await.unwrap();
    }
}

async fn exec(
    num_client: usize,
    execution: usize,
    client: &Client,
    settings: &Settings,
) -> BenchmarkResult {
    let begin = Instant::now();
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
                // si las declaro aqui las str funciona
                // let name = "key";
                // let value = "value";

                // en cambio si las asigno desde h me da el error
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
        Some(b) => request_builder.body(b.to_string()),
    };
    let response = request_builder.headers(headers_map).send().await;
    let duration_ms = begin.elapsed().as_millis() as u64;
    match response {
        Ok(r) => {
            let status = r.status().as_u16();
            println!(
                "[{} {} {} {}] {} {}{}",
                "Client".bold().green(),
                num_client.to_string().bold().green(),
                "Iteration".bold().green(),
                execution.to_string().bold().green(),
                status.to_string().bold().yellow(),
                duration_ms.to_string().cyan(),
                "ms".cyan()
            );
            BenchmarkResult {
                status: r.status().as_u16(),
                duration: duration_ms,
            }
        }
        Err(e) => {
            let status = match e.status() {
                None => "client error".to_string(),
                Some(s) => s.as_u16().to_string(),
            };
            println!(
                "[{} {} {} {}] {} {}{}",
                "Client".bold().green(),
                num_client.to_string().bold().green(),
                "Iteration".bold().green(),
                execution.to_string().bold().green(),
                status.bold().yellow(),
                duration_ms.to_string().cyan(),
                "ms".cyan()
            );
            BenchmarkResult {
                status: 0,
                duration: duration_ms,
            }
        }
    }
}
