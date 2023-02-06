
pub trait Average {
    fn avg(&self) -> u128;
}

impl Average for Vec<Result> {
    fn avg(&self) -> u128 {
        let total: u128 = self.iter().map(|r| r.duration).sum();
        let size: u128 = self.iter().len() as u128;
        total / size
    }
}

#[derive(Debug)]
pub struct Result {
    pub(crate) status: u16,
    pub(crate) duration: u128,
}

#[derive(Debug)]
pub struct Report {
    results: Vec<Result>,
}

impl Report {
    pub fn new() -> Self {
        Report { results: vec![] }
    }
    pub fn total(&self) -> usize {
        self.results.len()
    }

    pub fn avg(&self) -> u128 {
        self.results.avg()
    }

    pub fn add_result(&mut self, result: Result) {
        self.results.push(result);
    }
}
