//! FTS5 Full-Text Search module.
//!
//! Based on KV interface - works with all storage engines (Memory/BTree/LSM).

use std::collections::BTreeMap;
use crate::error::Result;
use crate::engine::StorageEngine;

const FTS_TABLE_ID: u32 = 255;

/// Tokenizer trait
pub trait FtsTokenizer: Send + Sync {
    fn tokenize(&self, text: &str) -> Vec<String>;
}

/// CJK Tokenizer - bigram segmentation
pub struct CjkTokenizer;

impl CjkTokenizer {
    pub fn new() -> Self {
        Self
    }
}

impl FtsTokenizer for CjkTokenizer {
    fn tokenize(&self, text: &str) -> Vec<String> {
        let chars: Vec<char> = text.chars().collect();
        if chars.len() < 2 {
            return vec![text.to_string()];
        }
        chars
            .windows(2)
            .map(|w| w.iter().collect::<String>())
            .collect()
    }
}

/// English Tokenizer - lowercase + whitespace tokenization
pub struct EnglishTokenizer;

impl EnglishTokenizer {
    pub fn new() -> Self {
        Self
    }
}

impl FtsTokenizer for EnglishTokenizer {
    fn tokenize(&self, text: &str) -> Vec<String> {
        text.to_lowercase()
            .split_whitespace()
            .map(|s| s.to_string())
            .collect()
    }
}

/// FTS Index - inverted index stored via KV interface
pub struct FtsIndex<E: StorageEngine> {
    engine: E,
    doc_count: u64,
}

impl<E: StorageEngine> FtsIndex<E> {
    pub fn new(engine: E) -> Self {
        Self {
            engine,
            doc_count: 0,
        }
    }

    pub fn insert(&mut self, doc_id: u64, text: &str) -> Result<()> {
        let tokenizer = CjkTokenizer::new();
        let terms = tokenizer.tokenize(text);
        
        let doc_key = format!("D:{}", doc_id);
        self.engine.put(FTS_TABLE_ID, doc_key.as_bytes(), text.as_bytes())?;
        
        let mut term_map: BTreeMap<String, u32> = BTreeMap::new();
        for term in &terms {
            *term_map.entry(term.clone()).or_insert(0) += 1;
        }
        
        for (term, tf) in term_map {
            let term_key = format!("T:{}:{}", term, doc_id);
            self.engine.put(FTS_TABLE_ID, term_key.as_bytes(), &tf.to_le_bytes())?;
        }
        
        self.doc_count += 1;
        Ok(())
    }

    pub fn search(&self, query: &str) -> Result<Vec<u64>> {
        let tokenizer = CjkTokenizer::new();
        let terms = tokenizer.tokenize(query);
        
        if terms.is_empty() {
            return Ok(vec![]);
        }
        
        let mut results: BTreeMap<u64, ()> = BTreeMap::new();
        
        for term in &terms {
            let prefix = format!("T:{}:", term);
            let scan_start = format!("T:{}:", term);
            let scan_end = format!("T:{}~\0", term);
            
            if let Ok(matches) = self.engine.scan(FTS_TABLE_ID, scan_start.as_bytes(), scan_end.as_bytes()) {
                for (key, _) in matches {
                    let key_str = String::from_utf8_lossy(&key);
                    if let Some(pos) = key_str.strip_prefix(&prefix) {
                        if let Ok(doc_id) = pos.parse::<u64>() {
                            results.insert(doc_id, ());
                        }
                    }
                }
            }
        }
        
        Ok(results.into_iter().map(|(k, _)| k).collect())
    }

    pub fn doc_count(&self) -> u64 {
        self.doc_count
    }

    pub fn get_doc(&self, doc_id: u64) -> Result<Option<Vec<u8>>> {
        let doc_key = format!("D:{}", doc_id);
        self.engine.get(FTS_TABLE_ID, doc_key.as_bytes())
    }
}

pub struct FtsQuery {
    pub terms: Vec<String>,
    pub AND: bool,
}

impl FtsQuery {
    pub fn parse(query: &str) -> Self {
        let terms: Vec<String> = query
            .split_whitespace()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_lowercase())
            .collect();
        
        Self {
            terms,
            AND: false,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_cjk_tokenizer() {
        use super::FtsTokenizer;
        let tokenizer = super::CjkTokenizer::new();
        let tokens = tokenizer.tokenize("資料庫系統");
        assert_eq!(tokens, vec!["資料", "料庫", "庫系", "系統"]);
    }

    #[test]
    fn test_english_tokenizer() {
        use super::FtsTokenizer;
        let tokenizer = super::EnglishTokenizer::new();
        let tokens = tokenizer.tokenize("Hello World");
        assert_eq!(tokens, vec!["hello", "world"]);
    }
}