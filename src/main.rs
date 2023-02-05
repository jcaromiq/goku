use futures::stream::FuturesUnordered;
use futures::StreamExt;
use reqwest::Client;
use std::time::Duration;
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
    let begin = Instant::now();
    let settings = Settings {
        clients: 100,
        requests: 200,
        keep_alive: None,
    };

    let mut tasks = FuturesUnordered::new();

    by_iteration(&settings, &mut tasks).await;
    let mut results: Vec<Vec<Result>> = vec![];
    while let Some(finished_task) = tasks.next().await {
        match finished_task {
            Err(e) => { /* e is a JoinError - the task has panicked */ }
            Ok(result) => {
                results.push(result);
            }
        }
    }
    let results = results.into_iter().flatten().collect::<Vec<Result>>();

    println!(
        "Total time: {}s for {} request with a average of {}ms ",
        begin.elapsed().as_secs(),
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

async fn by_iteration(settings: &Settings, tasks: &mut FuturesUnordered<JoinHandle<Vec<Result>>>) {
    let mut clients = Vec::with_capacity(settings.clients);
    for _ in 0..settings.clients {
        let client = reqwest::Client::builder()
            .tcp_keepalive(settings.keep_alive)
            .build()
            .unwrap();
        clients.push(client);
    }
    for (id, c) in clients.into_iter().enumerate() {
        let task = tokio::spawn(exec_iterator(id, settings.requests_by_client(), c));

        tasks.push(task);
    }
}

async fn exec_iterator(num_client: usize, num_requests: usize, client: Client) -> Vec<Result> {
    let mut results = vec![];
    for i in 0..num_requests {
        let r = exec(num_client, i, &client, "http://localhost:3000/").await;
        results.push(r);
    }
    results
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
        Err(e) => Result {
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
