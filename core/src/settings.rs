use std::fs;
use std::str::FromStr;
use std::time::Duration;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use strum::EnumString;
use crate::settings::Operation::Get;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub clients: i32,
    pub requests: i32,
    pub target: String,
    pub keep_alive: Option<Duration>,
    pub body: Option<String>,
    pub headers: Option<Vec<Header>>,
    pub duration: Option<u64>,
    pub verbose: bool,
    pub timeout: Duration,
    #[serde(default)]
    pub http2: bool,
    #[serde(default)]
    pub ramp_up: Option<u64>,
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct Header {
    pub key: String,
    pub value: String,
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

impl Settings {
    
    pub fn requests_by_client(&self) -> i32 {
        self.requests / self.clients
    }
    pub fn from_file(file: String) -> anyhow::Result<Self> {
        let content = fs::read_to_string(&file)
            .with_context(move || format!("Failed to read file from {}", &file))?;
        let settings: Settings = serde_yaml::from_str(&content)
            .with_context(move || "Invalid yaml format".to_string())?;
        Ok(settings)
    }
    
    pub fn operation(&self) -> Operation {
        let slices: Vec<&str> = self.target.split_whitespace().collect();
        if slices.len() == 1 {
            return Get;
        }
        match slices.first() {
            None => Get,
            Some(op) => Operation::from_str(&op.to_uppercase()).unwrap_or(Get),
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

    pub fn validate(&self) -> anyhow::Result<()> {
        if self.clients <= 0 {
            anyhow::bail!("--clients must be greater than 0, got {}", self.clients);
        }
        if self.duration.is_none() && self.requests <= 0 {
            anyhow::bail!("--iterations must be greater than 0, got {}", self.requests);
        }
        if self.target.trim().is_empty() {
            anyhow::bail!("--target cannot be empty");
        }
        if let Some(ramp_up) = self.ramp_up {
            if let Some(dur) = self.duration {
                if ramp_up >= dur {
                    anyhow::bail!(
                        "--ramp-up ({ramp_up}s) must be shorter than --duration ({dur}s)"
                    );
                }
            }
        }
        Ok(())
    }
}
