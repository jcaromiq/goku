use std::future::Future;
use std::ops::Div;
use std::time::Duration;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use reqwest::{Client, RequestBuilder};
use tokio::runtime;
use tokio::runtime::Runtime;
use tokio::sync::{mpsc, oneshot, Semaphore};
use tokio::task::JoinHandle;
use tokio::time::{Instant, sleep};


fn main() {
    // let threads = std::cmp::min(num_cpus::get(), config.concurrency as usize);
    let rt = runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(10)
        .build().unwrap();
    let iterations = 400;
    let clients = 400;
    let mut tasks = FuturesUnordered::new();
    rt.block_on(async {
        by_iteration(&rt, clients, iterations, &mut tasks).await;
        let mut patata: Vec<Vec<R>> = vec![];
        while let Some(finished_task) = tasks.next().await {
            match finished_task {
                Err(e) => { /* e is a JoinError - the task has panicked */ }
                Ok(result) => {
                    patata.push(result);
                }
            }
        }
        let patata = patata.into_iter().flatten().collect::<Vec<R>>();
        let v: f64 = patata
            .iter()
            .map(|r| {
                r.duration
            }).sum();
        println!("Total time: {} ms avg {} ", v, v / iterations as f64);
    });
}


async fn by_iteration(rt: &Runtime, num_clients: u64, iterations: u64, mut tasks: &mut FuturesUnordered<JoinHandle<Vec<R>>>) {
    let mut clients = Vec::with_capacity(num_clients as usize);
    for x in 0..num_clients {
        let client = reqwest::Client::builder().tcp_keepalive(None).build().unwrap();
        clients.push(client);
    }
    for (id, c) in clients.into_iter().enumerate() {
        let task = rt.spawn({
            exec_iterator(id, iterations / num_clients, c)
        });

        tasks.push(task);
    }
}

async fn exec_iterator(iteration: usize, num_requests: u64, client: Client) -> Vec<R> {
    let mut results = vec![];
    for i in 0..num_requests {
        let r = exec(iteration, i, &client).await;
        results.push(r);
    }
    results
}

async fn exec(num_client: usize, execution: u64, client: &Client) -> R {
    let begin = Instant::now();
    let response = client.get("http://localhost:3000/users").send().await;
    let duration_ms = begin.elapsed().as_secs_f64() * 1000.0;
    println!("Client {}, Execution {} Duration {}", num_client, execution, duration_ms);
    match response {
        Ok(r) => {
            R {
                status: r.status().to_string(),
                duration: duration_ms,
            }
        }
        Err(e) => {
            R {
                status: "client error".to_string(),
                duration: duration_ms,
            }
        }
    }
}

async fn execute(iteration: usize, execution: i32, url: &str) -> (Option<String>, f64, ) {
    println!("Iteration {}, Execution {}", iteration, execution);
    let begin = Instant::now();
    let response = reqwest::get(url).await;
    let duration_ms = begin.elapsed().as_secs_f64() * 1000.0;
    match response {
        Ok(r) => { (Some(r.status().to_string()), duration_ms) }
        Err(e) => { (None, duration_ms) }
    }
}

#[derive(Debug)]
struct R {
    status: String,
    duration: f64,
}
