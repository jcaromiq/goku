use crate::settings::Operation::Get;
use anyhow::{Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::fs;
use std::str::FromStr;
use std::time::Duration;
use strum::EnumString;

/// a HTTP benchmarking tool
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
}

#[derive(Eq, PartialEq, Debug, EnumString)]
pub enum Operation {
    #[strum(serialize = "GET")]
    Get,
    #[strum(serialize = "POST")]
    Post,
    Head,
    Patch,
    Put,
    Delete,
}

impl Args {
    pub fn to_settings(self) -> Result<Settings> {
        match self.scenario {
            None => Settings::from_args(self),
            Some(file) => Settings::from_file(file),
        }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub clients: usize,
    pub requests: usize,
    pub target: String,
    pub keep_alive: Option<Duration>,
    pub body: Option<String>,
    pub headers: Option<Vec<Header>>,
    pub duration: Option<u64>,
    pub verbose: bool,
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct Header {
    pub key: String,
    pub value: String,
}

impl Settings {
    pub fn print_banner(&self) {
        let banner = match &self.duration {
            None => format!(
                "kamehameha to {} with {} concurrent clients and {} total iterations",
                &self.target, &self.clients, &self.requests
            ),
            Some(d) => format!(
                "kamehameha to {} with {} concurrent clients for {} seconds",
                &self.target, &self.clients, d
            ),
        };
        println!("{banner}");
    }

    pub fn requests_by_client(&self) -> usize {
        self.requests / self.clients
    }
    pub fn from_file(file: String) -> Result<Self> {
        let content = fs::read_to_string(&file)
            .with_context(move || format!("Failed to read file from {}", &file))?;
        let settings: Settings = serde_yaml::from_str(&content)
            .with_context(move || "Invalid yaml format".to_string())?;
        Ok(settings)
    }
    pub fn from_args(args: Args) -> Result<Self> {
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
                })
            }
        }
    }
    pub fn operation(&self) -> Operation {
        let slices: Vec<&str> = self.target.split_whitespace().collect();
        if slices.len() == 1 {
            return Get;
        }
        match slices.first() {
            None => Get,
            Some(op) => match Operation::from_str(&op.to_uppercase()) {
                Ok(op) => op,
                Err(_) => Get,
            },
        }
    }
    pub fn target(&self) -> String {
        let slices: Vec<&str> = self.target.split_whitespace().collect();
        if slices.len() == 1 {
            return slices
                .first()
                .expect("target is not well formatted")
                .to_string();
        }
        slices
            .get(1)
            .expect("target is not well formatted")
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Operation::{Get, Post};

    #[test]
    fn should_set_get_as_default_operation() -> Result<()> {
        let args = Args {
            target: Some("https://localhost:3000".to_string()),
            ..Default::default()
        };

        let settings = Settings::from_args(args)?;
        assert_eq!(Get, settings.operation());
        Ok(())
    }

    #[test]
    fn should_get_operation_from_target() -> Result<()> {
        let args = Args {
            target: Some("POST https://localhost:3000".to_string()),
            ..Default::default()
        };

        let settings = Settings::from_args(args)?;
        assert_eq!(Post, settings.operation());
        Ok(())
    }

    #[test]
    fn should_get_target_from_target_without_operation() -> Result<()> {
        let args = Args {
            target: Some("https://localhost:3000".to_string()),
            ..Default::default()
        };

        let settings = Settings::from_args(args)?;
        assert_eq!("https://localhost:3000", settings.target());
        Ok(())
    }

    #[test]
    fn should_get_target_from_target_with_operation() -> Result<()> {
        let args = Args {
            target: Some("POST https://localhost:3000".to_string()),
            ..Default::default()
        };

        let settings = Settings::from_args(args)?;
        assert_eq!("https://localhost:3000", settings.target());
        Ok(())
    }

    #[test]
    fn should_set_get_operation_if_operation_is_not_allowed() -> Result<()> {
        let args = Args {
            target: Some("FOO https://localhost:3000".to_string()),
            ..Default::default()
        };

        let settings = Settings::from_args(args)?;
        assert_eq!(Get, settings.operation());
        Ok(())
    }

    #[test]
    fn should_return_error_if_request_body_file_does_not_exists() -> Result<()> {
        let args = Args {
            target: Some("POST https://localhost:3000".to_string()),
            request_body: Some(String::from("foo")),
            ..Default::default()
        };
        match Settings::from_args(args) {
            Ok(_) => {}
            Err(e) => {
                assert_eq!(e.to_string(), "Failed to read file from foo")
            }
        }
        Ok(())
    }

    #[test]
    fn should_set_none_headers_if_not_present() -> Result<()> {
        let args = Args {
            target: Some("FOO https://localhost:3000".to_string()),
            request_body: None,
            ..Default::default()
        };
        let settings = Settings::from_args(args)?;
        assert_eq!(settings.headers, None);
        Ok(())
    }

    #[test]
    fn should_set_headers() -> Result<()> {
        let args = Args {
            target: Some("FOO https://localhost:3000".to_string()),
            headers: Some(vec![
                "bar:foo".to_string(),
                "Content-Type:application/json".to_string(),
            ]),
            ..Default::default()
        };
        let settings = Settings::from_args(args)?;
        assert_eq!(
            settings.headers,
            Some(vec![
                Header {
                    key: "bar".to_string(),
                    value: "foo".to_string(),
                },
                Header {
                    key: "Content-Type".to_string(),
                    value: "application/json".to_string(),
                },
            ])
        );
        Ok(())
    }
}
