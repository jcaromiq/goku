use crate::benchmark::Result;
use crate::settings::Settings;
use futures::stream::FuturesUnordered;
use reqwest::Client;
use tokio::sync::mpsc::Sender;
use tokio::task::JoinHandle;
use tokio::time::Instant;

pub async fn run(settings: &Settings, tx: Sender<Result>) -> FuturesUnordered<JoinHandle<()>> {
    let tasks = FuturesUnordered::new();
    let mut clients = Vec::with_capacity(settings.clients);
    for _ in 0..settings.clients {
        let client = reqwest::Client::builder()
            .tcp_keepalive(settings.keep_alive)
            .build()
            .unwrap();
        clients.push(client);
    }
    for (id, client) in clients.into_iter().enumerate() {
        let task = tokio::spawn(exec_iterator(
            id,
            settings.requests_by_client(),
            client,
            tx.clone(),
        ));
        tasks.push(task);
    }
    return tasks;
}

async fn exec_iterator(num_client: usize, num_requests: usize, client: Client, tx: Sender<Result>) {
    for i in 0..num_requests {
        let r = exec(num_client, i, &client, "http://localhost:3000/").await;
        tx.send(r).await.unwrap();
    }
}

async fn exec(num_client: usize, execution: usize, client: &Client, url: &str) -> Result {
    let begin = Instant::now();
    let response = client.get(url).send().await;
    let duration_ms = begin.elapsed().as_millis();
    println!(
        "[Client {}] Execution {} in Duration {} ms",
        num_client, execution, duration_ms
    );
    match response {
        Ok(r) => Result {
            status: r.status().as_u16(),
            duration: duration_ms,
        },
        Err(_) => Result {
            status: 0,
            duration: duration_ms,
        },
    }
}
