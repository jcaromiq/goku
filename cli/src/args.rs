use std::fs;
use std::time::Duration;
use anyhow::Context;
use clap::Parser;
use goku_core::settings::{Header, Settings};

// a HTTP benchmarking tool
#[derive(Parser, Debug, Default)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Runs in verbose mode
    #[arg(short, long)]
    verbose: bool,

    /// URL to be requested using an operation [default: GET] Ex. GET http://localhost:3000/
    #[arg(
        short,
        long,
        conflicts_with = "scenario",
        required_unless_present = "scenario"
    )]
    target: Option<String>,

    /// File path for the request body
    #[arg(short, long, conflicts_with = "scenario")]
    request_body: Option<String>,

    /// Number of concurrent clients
    #[arg(short, long, default_value_t = 1, conflicts_with = "scenario")]
    clients: usize,

    /// Total number of iterations
    #[arg(short, long, default_value_t = 1, conflicts_with_all = ["duration", "scenario"])]
    iterations: usize,

    /// Duration of the test in seconds
    #[arg(short, long, conflicts_with_all = ["iterations", "scenario"])]
    duration: Option<u64>,

    /// Headers, multi value in format headerName:HeaderValue
    #[arg(long, conflicts_with = "scenario")]
    headers: Option<Vec<String>>,

    /// Scenario file
    #[arg(long, conflicts_with = "target")]
    scenario: Option<String>,

    /// Timeout in milliseconds
    #[arg(long, default_value_t = 30000)]
    timeout: u64,
}

impl Args {
    pub fn to_settings(self) -> anyhow::Result<Settings> {
        match self.scenario {
            None => Self::from_args(self),
            Some(file) => Settings::from_file(file),
        }
    }

    pub fn from_args(args: Args) -> anyhow::Result<Settings> {
        let headers = match args.headers {
            None => None,
            Some(headers_string) => {
                let headers: Vec<Header> = headers_string
                    .iter()
                    .map(|v| {
                        let split: Vec<&str> = v.split(':').collect();
                        let key = split[0].to_string();
                        let value = split[1].to_string();
                        Header { key, value }
                    })
                    .collect();
                Some(headers)
            }
        };

        match args.request_body {
            None => Ok(Settings {
                clients: args.clients,
                requests: args.iterations,
                target: args.target.unwrap(),
                keep_alive: None,
                body: None,
                headers,
                duration: args.duration,
                verbose: args.verbose,
                timeout: Duration::from_millis(args.timeout)
            }),
            Some(file) => {
                let content = fs::read_to_string(&file)
                    .with_context(move || format!("Failed to read file from {}", &file))?;
                Ok(Settings {
                    clients: args.clients,
                    requests: args.iterations,
                    target: args.target.unwrap(),
                    keep_alive: None,
                    body: Some(content),
                    headers,
                    duration: args.duration,
                    verbose: args.verbose,
                    timeout: Duration::from_millis(args.timeout)
                })
            }
        }
    }
}