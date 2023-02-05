use futures::stream::FuturesUnordered;
use reqwest::Client;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio::task::JoinHandle;
use tokio::time::Instant;

struct Settings {
    clients: usize,
    requests: usize,
    keep_alive: Option<Duration>,
}

impl Settings {
    pub fn requests_by_client(&self) -> usize {
        self.requests / self.clients
    }
}

#[tokio::main]
async fn main() {
    let settings = Settings {
        clients: 400,
        requests: 1000000,
        keep_alive: None,
    };

    let (tx, mut rx) = mpsc::channel(settings.requests);
    let begin = Instant::now();

    run(&settings, tx).await;

    let mut results: Vec<Result> = vec![];

    while let Some(value) = rx.recv().await {
        results.push(value);
    }
    let end = begin.elapsed().as_secs();

    println!(
        "Total time: {}s for {} request with a average of {}ms ",
        end,
        results.iter().len(),
        results.avg()
    );
}

pub trait Average {
    fn avg(&self) -> u128;
}

impl Average for Vec<Result> {
    fn avg(&self) -> u128 {
        let total: u128 = self.iter().map(|r| r.duration).sum();
        let size: u128 = self.iter().len() as u128;
        total / size
    }
}

async fn run(
    settings: &Settings,
    tx: Sender<Result>,
) {
    let mut tasks = FuturesUnordered::new();
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
            status: r.status().to_string(),
            duration: duration_ms,
        },
        Err(_) => Result {
            status: "client error".to_string(),
            duration: duration_ms,
        },
    }
}

#[derive(Debug)]
struct Result {
    status: String,
    duration: u128,
}
