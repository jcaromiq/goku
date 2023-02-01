use std::future::Future;
use std::time::Duration;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use tokio::runtime;
use tokio::runtime::Runtime;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::{Instant, sleep};


fn main() {
    // let threads = std::cmp::min(num_cpus::get(), config.concurrency as usize);
    let threads = 1;//std::cmp::min(1, 80 as usize);
    let rt = runtime::Builder::new_multi_thread().enable_all().worker_threads(threads).build().unwrap();
    let iterations = 100;
    let seconds = 0.3;
    let mut tasks = FuturesUnordered::new();
    rt.block_on(async {
        by_iteration(&rt, iterations, &mut tasks);
        // by_time(&rt, seconds, &mut tasks);
        while let Some(finished_task) = tasks.next().await {
            match finished_task {
                Err(e) => { /* e is a JoinError - the task has panicked */ }
                Ok(result) => {}
            }
        }
    });
}

fn by_time(rt: &Runtime, seconds: f64, mut tasks: &mut FuturesUnordered<JoinHandle<R>>) {
    let begin = Instant::now();
    let mut x = 0;
    loop {
        println!("segundos! {}", begin.elapsed().as_secs_f64());
        if begin.elapsed().as_secs_f64() > seconds {
            break;
        } else {
            let task = rt.spawn(exec_iterator(x));
            tasks.push(task);
            x = x + 1;
        }

    }
}

fn by_iteration(rt: &Runtime, iterations: i32, mut tasks: &mut FuturesUnordered<JoinHandle<R>>) {
    for x in 0..iterations {
        let task = rt.spawn(exec_iterator(x));
        tasks.push(task);
    }
}

async fn exec_iterator(iteration: i32) -> R {
    execute(iteration, 1, "http://localhost:3000/").await;
    execute(iteration, 2, "http://localhost:3000/users").await;
    execute(iteration, 3, "http://localhost:3000/").await;
    execute(iteration, 4, "http://localhost:3000/users").await;
    R {
        status: "".to_string(),
        duration: 0.0,
        iteration: 0,
    }
}

async fn execute(iteration: i32, execution: i32, url: &str) -> (Option<String>, f64, ) {
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
    iteration: i32,
}
