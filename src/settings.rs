use clap::Parser;
use std::time::Duration;

/// a HTTP benchmarking tool
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Url to be request
    #[arg(short, long)]
    target: String,

    /// Number of concurrent clients
    #[arg(short, long, default_value_t = 1)]
    clients: usize,

    /// Total number of iterations
    #[arg(short, long, default_value_t = 1)]
    iterations: usize,
}

#[derive(Clone)]
pub struct Settings {
    pub clients: usize,
    pub requests: usize,
    pub target: String,
    pub keep_alive: Option<Duration>,
}

impl Settings {
    pub fn requests_by_client(&self) -> usize {
        self.requests / self.clients
    }
    pub fn from_args() -> Self {
        let args = Args::parse();
        Settings {
            clients: args.clients,
            requests: args.iterations,
            target: args.target,
            keep_alive: None,
        }
    }
}
