//! 非同步佇列設定 — 佇列配置、重試策略、退避演算法

/// 非同步佇列設定
///
/// - `max_delivery_count`: 最大傳遞次數（超過進 DLQ）
/// - `dlq_name`: 死信佇列名稱（None 表示不啟用 DLQ）
/// - `message_ttl_secs`: 訊息存活時間（秒）
/// - `priority_enabled`: 是否啟用優先級排序
#[derive(Debug, Clone)]
pub struct AsyncQueueConfig {
    pub max_delivery_count: u32,
    pub dlq_name: Option<String>,
    pub message_ttl_secs: Option<u64>,
    pub priority_enabled: bool,
}

impl Default for AsyncQueueConfig {
    fn default() -> Self {
        Self {
            max_delivery_count: 3,
            dlq_name: None,
            message_ttl_secs: None,
            priority_enabled: false,
        }
    }
}

/// 重試設定
///
/// - `max_retries`: 最大重試次數
/// - `initial_delay_ms`: 初始延遲（毫秒）
/// - `max_delay_ms`: 最大延遲（毫秒）
/// - `backoff_multiplier`: 退避倍數（每次延遲乘以此值）
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 100,
            max_delay_ms: 5000,
            backoff_multiplier: 2.0,
        }
    }
}

/// 使用指數退避 (Exponential Backoff) 重試一個非同步操作
///
/// 流程：
/// 1. 執行操作
/// 2. 如果失敗且未達最大重試次數，等待 delay 毫秒
/// 3. delay = delay × backoff_multiplier（但不會超過 max_delay_ms）
/// 4. 重複直到成功或達最大重試次數
pub async fn with_retry<T, F, E>(
    config: RetryConfig,
    mut operation: F,
) -> Result<T, E>
where
    F: FnMut() -> Result<T, E>,
{
    let mut delay = config.initial_delay_ms;
    let mut attempts = 0;

    loop {
        match operation() {
            Ok(result) => return Ok(result),
            Err(e) => {
                attempts += 1;
                if attempts >= config.max_retries {
                    return Err(e);
                }
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                delay = (delay as f64 * config.backoff_multiplier) as u64;
                delay = delay.min(config.max_delay_ms);
            }
        }
    }
}
