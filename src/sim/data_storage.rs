use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataStorage {
    pub capacity: u64,
    pub stored: u64,
}

impl DataStorage {
    pub fn new(capacity: u64) -> Self {
        Self {
            capacity,
            stored: 0,
        }
    }

    pub fn store(&mut self, amount: u64) -> u64 {
        let free = self.free_capacity();
        let to_store = amount.min(free);
        self.stored += to_store;
        to_store
    }

    pub fn free_capacity(&self) -> u64 {
        self.capacity.saturating_sub(self.stored)
    }

    pub fn expand(&mut self, extra: u64) {
        self.capacity += extra;
    }
}
