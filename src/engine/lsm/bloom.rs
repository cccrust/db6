//! 布隆過濾器 (Bloom Filter) — 機率型資料結構
//!
//! 布隆過濾器用於快速判斷一個元素**是否可能在集合中**。
//!
//! ## 特性
//!
//! - 如果回傳 `false`：元素**絕對不在**集合中
//! - 如果回傳 `true`：元素**可能在**集合中（有偽陽性 false positive 的機率）
//! - 無法刪除元素（標準布隆過濾器不支援刪除）
//!
//! ## 在 LSM-Tree 中的應用
//!
//! 查詢 LSM 引擎時，先檢查 Bloom Filter：
//! - 若 `might_contain(key)` = false，直接回傳 None，避免昂貴的 SSTable 磁碟讀取
//! - 若 = true，再到 SSTable 中搜尋
//!
//! ## 實作細節
//!
//! 使用 3 個雜湊函數與位元陣列。每個元素插入時計算 3 次雜湊，
//! 將對應的位元設為 1。查詢時檢查所有對應位元是否都為 1。
//!
//! 位元陣列儲存在 `Vec<u64>` 中，每個 u64 儲存 64 個位元。

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// 布隆過濾器
///
/// - `bits`: 位元陣列（以 u64 為單位）
/// - `capacity`: 預估元素數量
/// - `hashes`: 雜湊函數數量（固定為 3）
pub struct BloomFilter {
    bits: Vec<u64>,
    capacity: usize,
    hashes: usize,
}

impl BloomFilter {
    /// 建立一個新的布隆過濾器
    ///
    /// `capacity`: 預估要容納的元素數量
    pub fn new(capacity: usize) -> Self {
        let bits = (capacity + 63) / 64;
        Self {
            bits: vec![0; bits],
            capacity,
            hashes: 3,
        }
    }

    /// 使用 seed 計算指定鍵的雜湊值
    ///
    /// 透過不同的 seed 產生多個（近似）獨立的雜湊函數。
    /// 先對鍵內容做 hash，再對 seed 做 hash，最後取模。
    fn hash(&self, key: &[u8], seed: usize) -> usize {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        seed.hash(&mut hasher);
        (hasher.finish() as usize) % self.capacity
    }

    /// 將一個鍵插入布隆過濾器
    ///
    /// 用 3 個雜湊函數計算位置，將對應位元設為 1。
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

    /// 檢查鍵是否可能在集合中
    ///
    /// - `false`: 確定不在集合中
    /// - `true`: 可能在集合中（有偽陽性機率）
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