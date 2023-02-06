use crate::settings::Operation::Get;
use clap::Parser;
use std::str::FromStr;
use std::time::Duration;
use strum::EnumString;

/// a HTTP benchmarking tool
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Url to be request with operation Ej, GET http://localhost:3000/ if operation is empty, will be GET by default
    #[arg(short, long)]
    target: String,

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
}

impl Settings {
    pub fn requests_by_client(&self) -> usize {
        self.requests / self.clients
    }
    pub fn from_args(args: Args) -> Self {
        Settings {
            clients: args.clients,
            requests: args.iterations,
            target: args.target,
            keep_alive: None,
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
    fn should_set_get_as_default_operation() {
        let args = Args {
            target: "https://localhost:3000".to_string(),
            clients: 0,
            iterations: 0,
        };

        let settings = Settings::from_args(args);
        assert_eq!(Get, settings.operation());
    }

    #[test]
    fn should_get_operation_from_target() {
        let args = Args {
            target: "POST https://localhost:3000".to_string(),
            clients: 0,
            iterations: 0,
        };

        let settings = Settings::from_args(args);
        assert_eq!(Post, settings.operation());
    }

    #[test]
    fn should_get_target_from_target_without_operation() {
        let args = Args {
            target: "https://localhost:3000".to_string(),
            clients: 0,
            iterations: 0,
        };

        let settings = Settings::from_args(args);
        assert_eq!("https://localhost:3000", settings.target());
    }

    #[test]
    fn should_get_target_from_target_with_operation() {
        let args = Args {
            target: "POST https://localhost:3000".to_string(),
            clients: 0,
            iterations: 0,
        };

        let settings = Settings::from_args(args);
        assert_eq!("https://localhost:3000", settings.target());
    }

    #[test]
    fn should_set_get_operation_if_operation_is_not_allowed() {
        let args = Args {
            target: "FOO https://localhost:3000".to_string(),
            clients: 0,
            iterations: 0,
        };

        let settings = Settings::from_args(args);
        assert_eq!(Get, settings.operation());
    }
}
