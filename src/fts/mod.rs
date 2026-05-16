//! FTS5 Full-Text Search module.

use crate::error::Result;

pub trait FtsIndex {
    fn insert(&mut self, doc_id: u64, text: &str) -> Result<()>;
    fn search(&self, query: &str) -> Result<Vec<u64>>;
}

pub trait FtsTokenizer {
    fn tokenize(&self, text: &str) -> Vec<String>;
}

pub struct CjkTokenizer;

impl FtsTokenizer for CjkTokenizer {
    fn tokenize(&self, text: &str) -> Vec<String> {
        text.chars()
            .collect::<Vec<_>>()
            .windows(2)
            .map(|w| w.iter().collect::<String>())
            .collect()
    }
}