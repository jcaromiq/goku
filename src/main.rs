use crate::benchmark::{Report};
use crate::execution::run;
use crate::settings::Settings;
use tokio::sync::mpsc;
use tokio::time::Instant;

mod benchmark;
mod execution;
mod settings;

#[tokio::main]
async fn main() {
    let settings = Settings {
        clients: 10,
        requests: 100,
        keep_alive: None,
    };

    let mut report = Report::new();

    let (tx, mut rx) = mpsc::channel(settings.requests);

    let begin = Instant::now();
    run(&settings, tx).await;

    while let Some(value) = rx.recv().await {
        report.add_result(value);
    }
    let elapsed = begin.elapsed().as_secs();

    println!(
        "Total time: {}s for {} request with a average of {}ms ",
        elapsed,
        &report.total(),
        &report.avg()
    );
}
