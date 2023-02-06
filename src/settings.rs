use std::time::Duration;

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
}
