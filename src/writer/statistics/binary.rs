use super::common::BaseStatistics;

#[derive(Debug, Clone)]
pub struct BinaryStatistics {
    pub num_values: u64,
    pub num_present: u64,
    pub sum_lengths: u64,
}

impl BinaryStatistics {
    pub fn new() -> Self {
        Self {
            num_values: 0,
            num_present: 0,
            sum_lengths: 0,
        }
    }

    pub fn update(&mut self, x: Option<&[u8]>) {
        self.num_values += 1;
        self.num_present += x.is_some() as u64;
        if let Some(xv) = x {
            self.sum_lengths += xv.len() as u64;
        }   
    }
}

impl BaseStatistics for BinaryStatistics {
    fn num_values(&self) -> u64 { self.num_values }

    fn num_present(&self) -> u64 { self.num_present }

    fn merge(&mut self, rhs: &Self) {
        self.num_values += rhs.num_values;
        self.num_present += rhs.num_present;
        self.sum_lengths += rhs.sum_lengths;
    }
}
