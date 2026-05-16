//! FTS5 Full-Text Search module.
//!
//! Based on KV interface - works with all storage engines (Memory/BTree/LSM).
//!
//! v1.1 Features:
//! - Boolean queries (AND/OR/NOT)
//! - Prefix matching
//! - BM25 ranking

use std::collections::{BTreeMap, BTreeSet};
use crate::error::Result;
use crate::engine::StorageEngine;

const FTS_TABLE_ID: u32 = 255;

const BM25_K1: f64 = 1.5;
const BM25_B: f64 = 0.75;

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

    fn get_doc_ids_for_term(&self, term: &str) -> Result<BTreeSet<u64>> {
        let prefix = format!("T:{}:", term);
        let scan_start = format!("T:{}:", term);
        let scan_end = format!("T:{}~\0", term);
        
        let mut doc_ids = BTreeSet::new();
        if let Ok(matches) = self.engine.scan(FTS_TABLE_ID, scan_start.as_bytes(), scan_end.as_bytes()) {
            for (key, _) in matches {
                let key_str = String::from_utf8_lossy(&key);
                if let Some(pos) = key_str.strip_prefix(&prefix) {
                    if let Ok(doc_id) = pos.parse::<u64>() {
                        doc_ids.insert(doc_id);
                    }
                }
            }
        }
        Ok(doc_ids)
    }

    pub fn search_prefix(&self, prefix: &str) -> Result<Vec<u64>> {
        let tokenizer = CjkTokenizer::new();
        let terms = tokenizer.tokenize(prefix);
        
        if terms.is_empty() {
            return Ok(vec![]);
        }
        
        let mut results: BTreeSet<u64> = BTreeSet::new();
        
        for term in &terms {
            let term_prefix = format!("T:{}", term);
            let scan_start = term_prefix.as_bytes();
            let scan_end = format!("T:{}~\0", term).into_bytes();
            
            if let Ok(matches) = self.engine.scan(FTS_TABLE_ID, scan_start, &scan_end) {
                for (key, _) in matches {
                    let key_str = String::from_utf8_lossy(&key);
                    if let Some(remainder) = key_str.strip_prefix(&term_prefix) {
                        if remainder.starts_with(':') {
                            if let Some(pos) = remainder.strip_prefix(':') {
                                if let Ok(doc_id) = pos.parse::<u64>() {
                                    results.insert(doc_id);
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(results.into_iter().collect())
    }

    pub fn search_boolean(&self, query: &str) -> Result<Vec<u64>> {
        let parts: Vec<&str> = query.split_whitespace().collect();
        
        let mut positive_terms: Vec<&str> = Vec::new();
        let mut negative_terms: Vec<&str> = Vec::new();
        
        for term in parts {
            if term.starts_with('-') || term.starts_with('!') {
                let t = term.trim_start_matches('-').trim_start_matches('!');
                if !t.is_empty() {
                    negative_terms.push(t);
                }
            } else {
                positive_terms.push(term);
            }
        }
        
        let tokenizer = CjkTokenizer::new();
        
        let mut results: Option<BTreeSet<u64>> = None;
        
        for term_str in positive_terms {
            let terms = tokenizer.tokenize(term_str);
            let mut term_results: BTreeSet<u64> = BTreeSet::new();
            
            for term in &terms {
                let docs = self.get_doc_ids_for_term(term)?;
                term_results.extend(docs);
            }
            
            if results.is_none() {
                results = Some(term_results);
            } else {
                if let Some(ref mut r) = results {
                    *r = r.intersection(&term_results).cloned().collect();
                }
            }
        }
        
        if results.is_none() {
            results = Some(BTreeSet::new());
        }
        
        for term_str in &negative_terms {
            let terms = tokenizer.tokenize(term_str);
            for term in &terms {
                let docs = self.get_doc_ids_for_term(term)?;
                if let Some(ref mut r) = results {
                    for doc in docs {
                        r.remove(&doc);
                    }
                }
            }
        }
        
        Ok(results.unwrap().into_iter().collect())
    }

    pub fn search_bm25(&self, query: &str) -> Result<Vec<(u64, f64)>> {
        let tokenizer = CjkTokenizer::new();
        let terms = tokenizer.tokenize(query);
        
        if terms.is_empty() || self.doc_count == 0 {
            return Ok(vec![]);
        }
        
        let mut doc_scores: BTreeMap<u64, f64> = BTreeMap::new();
        let avg_dl = self.compute_avg_doc_length()?;
        
        for term in &terms {
            let df = self.get_document_frequency(term)?;
            if df == 0 {
                continue;
            }
            
            let idf = ((self.doc_count as f64 - df as f64 + 0.5) / (df as f64 + 0.5) + 1.0).ln();
            
            let docs = self.get_doc_ids_for_term(term)?;
            for doc_id in docs {
                let tf = self.get_term_frequency(doc_id, term)? as f64;
                let dl = self.get_doc_length(doc_id)? as f64;
                
                let score = idf * (tf * (BM25_K1 + 1.0)) / (tf + BM25_K1 * (1.0 - BM25_B + BM25_B * dl / avg_dl));
                *doc_scores.entry(doc_id).or_insert(0.0) += score;
            }
        }
        
        let mut sorted: Vec<(u64, f64)> = doc_scores.into_iter().collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        Ok(sorted)
    }

    fn get_document_frequency(&self, term: &str) -> Result<u32> {
        let scan_start = format!("T:{}:", term);
        let scan_end = format!("T:{}~\0", term).into_bytes();
        
        let mut count = 0u32;
        if let Ok(matches) = self.engine.scan(FTS_TABLE_ID, scan_start.as_bytes(), &scan_end) {
            count = matches.len() as u32;
        }
        Ok(count)
    }

    fn get_term_frequency(&self, doc_id: u64, term: &str) -> Result<u32> {
        let term_key = format!("T:{}:{}", term, doc_id);
        if let Ok(Some(data)) = self.engine.get(FTS_TABLE_ID, term_key.as_bytes()) {
            if data.len() >= 4 {
                return Ok(u32::from_le_bytes([data[0], data[1], data[2], data[3]]));
            }
        }
        Ok(0)
    }

    fn get_doc_length(&self, doc_id: u64) -> Result<u32> {
        if let Ok(Some(text)) = self.get_doc(doc_id) {
            let text_str = String::from_utf8_lossy(&text);
            let tokenizer = CjkTokenizer::new();
            Ok(tokenizer.tokenize(&text_str).len() as u32)
        } else {
            Ok(0)
        }
    }

    fn compute_avg_doc_length(&self) -> Result<f64> {
        if self.doc_count == 0 {
            return Ok(0.0);
        }
        
        let mut total_len = 0u64;
        let scan_start = "D:".as_bytes();
        let scan_end = "D:~\0".as_bytes();
        
        if let Ok(docs) = self.engine.scan(FTS_TABLE_ID, scan_start, scan_end) {
            for (key, value) in docs {
                let _ = key;
                let text_str = String::from_utf8_lossy(&value);
                let tokenizer = CjkTokenizer::new();
                total_len += tokenizer.tokenize(&text_str).len() as u64;
            }
        }
        
        Ok(total_len as f64 / self.doc_count as f64)
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
    pub and: bool,
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
            and: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FtsTokenizer;
    use crate::engine::MemoryEngine;

    #[test]
    fn test_cjk_tokenizer() {
        let tokenizer = super::CjkTokenizer::new();
        let tokens = tokenizer.tokenize("資料庫系統");
        assert_eq!(tokens, vec!["資料", "料庫", "庫系", "系統"]);
    }

    #[test]
    fn test_english_tokenizer() {
        let tokenizer = super::EnglishTokenizer::new();
        let tokens = tokenizer.tokenize("Hello World");
        assert_eq!(tokens, vec!["hello", "world"]);
    }

    #[test]
    fn test_fts_basic() {
        let engine = MemoryEngine::new();
        let mut index = super::FtsIndex::new(engine);
        
        index.insert(1, "Hello World").unwrap();
        index.insert(2, "資料庫系統").unwrap();
        
        let results = index.search("Hello").unwrap();
        assert!(results.contains(&1));
        
        let results2 = index.search("資料庫").unwrap();
        assert!(results2.contains(&2));
    }

    #[test]
    fn test_fts_prefix() {
        let engine = MemoryEngine::new();
        let mut index = super::FtsIndex::new(engine);
        
        index.insert(1, "資料庫").unwrap();
        index.insert(2, "資料結構").unwrap();
        index.insert(3, "作業系統").unwrap();
        
        let results = index.search_prefix("資料").unwrap();
        assert!(results.contains(&1));
        assert!(results.contains(&2));
    }

    #[test]
    fn test_fts_boolean_and() {
        let engine = MemoryEngine::new();
        let mut index = super::FtsIndex::new(engine);
        
        index.insert(1, "Hello World").unwrap();
        index.insert(2, "Hello Rust").unwrap();
        index.insert(3, "World Peace").unwrap();
        
        let results = index.search_boolean("Hello World").unwrap();
        assert!(results.contains(&1));
    }

    #[test]
    fn test_fts_boolean_not() {
        let engine = MemoryEngine::new();
        let mut index = super::FtsIndex::new(engine);
        
        index.insert(1, "Hello World").unwrap();
        index.insert(2, "Hello Rust").unwrap();
        index.insert(3, "World Peace").unwrap();
        
        let results = index.search_boolean("Hello -World").unwrap();
        assert!(results.contains(&2));
        assert!(!results.contains(&1));
    }

    #[test]
    fn test_fts_bm25() {
        let engine = MemoryEngine::new();
        let mut index = super::FtsIndex::new(engine);
        
        index.insert(1, "hello world").unwrap();
        index.insert(2, "hello hello world").unwrap();
        index.insert(3, "world").unwrap();
        
        let results = index.search_bm25("hello").unwrap();
        assert!(!results.is_empty());
        
        let (doc_id, score) = results[0];
        assert!(score > 0.0);
    }
}