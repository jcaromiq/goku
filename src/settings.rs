use std::time::Duration;

pub struct Settings {
    pub(crate) clients: usize,
    pub(crate) requests: usize,
    pub(crate) keep_alive: Option<Duration>,
}

impl Settings {
    pub fn requests_by_client(&self) -> usize {
        self.requests / self.clients
    }
}
