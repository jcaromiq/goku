use crate::settings::Operation::Get;
use anyhow::{Context, Result};
use clap::Parser;
use std::fs;
use std::str::FromStr;
use std::time::Duration;
use strum::EnumString;

/// a HTTP benchmarking tool
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// URL to be requested using an operation [default: GET] Ex. GET http://localhost:3000/
    #[arg(short, long)]
    target: String,

    /// File path for the request body
    #[arg(short, long)]
    request_body: Option<String>,

    /// Number of concurrent clients
    #[arg(short, long, default_value_t = 1)]
    clients: usize,

    /// Total number of iterations
    #[arg(short, long, default_value_t = 1)]
    iterations: usize,
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

#[derive(Clone)]
pub struct Settings {
    pub clients: usize,
    pub requests: usize,
    pub target: String,
    pub keep_alive: Option<Duration>,
    pub body: Option<String>,
}

impl Settings {
    pub fn requests_by_client(&self) -> usize {
        self.requests / self.clients
    }
    pub fn from_args(args: Args) -> Result<Self> {
        match args.request_body {
            None => Ok(Settings {
                clients: args.clients,
                requests: args.iterations,
                target: args.target,
                keep_alive: None,
                body: None,
            }),
            Some(file) => {
                let content = fs::read_to_string(&file)
                    .with_context(move || format!("Failed to read file from {}", &file))?;
                Ok(Settings {
                    clients: args.clients,
                    requests: args.iterations,
                    target: args.target,
                    keep_alive: None,
                    body: Some(content),
                })
            }
        }
    }
    pub fn operation(&self) -> Operation {
        let slices: Vec<&str> = self.target.split_whitespace().collect();
        if slices.len() == 1 {
            return Get;
        }
        match Operation::from_str(&slices.first().unwrap().to_uppercase()) {
            Ok(op) => op,
            Err(_) => Get,
        }
    }
    pub fn target(&self) -> String {
        let slices: Vec<&str> = self.target.split_whitespace().collect();
        if slices.len() == 1 {
            return slices.first().unwrap().to_string();
        }
        slices.get(1).unwrap().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Operation::{Delete, Get, Head, Patch, Post, Put};

    #[test]
    fn should_set_get_as_default_operation() -> Result<()> {
        let args = Args {
            target: "https://localhost:3000".to_string(),
            request_body: None,
            clients: 0,
            iterations: 0,
        };

        let settings = Settings::from_args(args)?;
        assert_eq!(Get, settings.operation());
        Ok(())
    }

    #[test]
    fn should_get_operation_from_target() -> Result<()> {
        let args = Args {
            target: "POST https://localhost:3000".to_string(),
            request_body: None,
            clients: 0,
            iterations: 0,
        };

        let settings = Settings::from_args(args)?;
        assert_eq!(Post, settings.operation());
        Ok(())
    }

    #[test]
    fn should_get_target_from_target_without_operation() -> Result<()> {
        let args = Args {
            target: "https://localhost:3000".to_string(),
            request_body: None,
            clients: 0,
            iterations: 0,
        };

        let settings = Settings::from_args(args)?;
        assert_eq!("https://localhost:3000", settings.target());
        Ok(())
    }

    #[test]
    fn should_get_target_from_target_with_operation() -> Result<()> {
        let args = Args {
            target: "POST https://localhost:3000".to_string(),
            request_body: None,
            clients: 0,
            iterations: 0,
        };

        let settings = Settings::from_args(args)?;
        assert_eq!("https://localhost:3000", settings.target());
        Ok(())
    }

    #[test]
    fn should_set_get_operation_if_operation_is_not_allowed() -> Result<()> {
        let args = Args {
            target: "FOO https://localhost:3000".to_string(),
            request_body: None,
            clients: 0,
            iterations: 0,
        };

        let settings = Settings::from_args(args)?;
        assert_eq!(Get, settings.operation());
        Ok(())
    }

    #[test]
    fn should_return_error_if_request_body_file_does_not_exists() -> Result<()> {
        let args = Args {
            target: "FOO https://localhost:3000".to_string(),
            request_body: Some(String::from("foo")),
            clients: 0,
            iterations: 0,
        };
        match Settings::from_args(args) {
            Ok(_) => {}
            Err(e) => {
                assert_eq!(e.to_string(), "Failed to read file from foo")
            }
        }
        Ok(())
    }
}
