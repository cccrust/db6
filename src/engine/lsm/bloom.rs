//! Bloom Filter - probabilistic data structure for membership testing

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub struct BloomFilter {
    bits: Vec<u64>,
    capacity: usize,
    hashes: usize,
}

impl BloomFilter {
    pub fn new(capacity: usize) -> Self {
        let bits = (capacity + 63) / 64;
        Self {
            bits: vec![0; bits],
            capacity,
            hashes: 3, // 3 hash functions
        }
    }

    fn hash(&self, key: &[u8], seed: usize) -> usize {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        seed.hash(&mut hasher);
        (hasher.finish() as usize) % self.capacity
    }

    pub fn insert(&mut self, key: &[u8]) {
        for i in 0..self.hashes {
            let h = self.hash(key, i);
            let idx = h / 64;
            let bit = h % 64;
            if idx < self.bits.len() {
                self.bits[idx] |= 1 << bit;
            }
        }
    }

    pub fn might_contain(&self, key: &[u8]) -> bool {
        for i in 0..self.hashes {
            let h = self.hash(key, i);
            let idx = h / 64;
            let bit = h % 64;
            if idx >= self.bits.len() {
                return false;
            }
            if self.bits[idx] & (1 << bit) == 0 {
                return false;
            }
        }
        true
    }
}

impl Default for BloomFilter {
    fn default() -> Self {
        Self::new(1024)
    }
}