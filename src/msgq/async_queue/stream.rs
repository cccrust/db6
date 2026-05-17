//! AsyncQueue Stream 介面 — 實作 tokio_stream::Stream trait
//!
//! 允許消費者使用 `while let` 語法消費訊息：
//!
//! ```ignore
//! let mut stream = queue.stream();
//! while let Some(msg) = stream.next().await {
//!     process(msg).await;
//! }
//! ```

use super::queue::AsyncQueue;
use super::AsyncQueueMessage;
use futures::stream::Stream;
use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{Context, Poll};

/// 非同步佇列串流
///
/// 包裝 AsyncQueue 並實作 Stream trait，
/// 讓消費者可以像使用迭代器一樣消費訊息。
pub struct AsyncQueueStream {
    /// 底層佇列
    queue: AsyncQueue,
    /// 內部緩衝區（用於非同步邊界）
    buffer: VecDeque<AsyncQueueMessage>,
}

impl AsyncQueueStream {
    /// 建立串流
    pub fn new(queue: AsyncQueue) -> Self {
        Self {
            queue,
            buffer: VecDeque::new(),
        }
    }
}

impl Stream for AsyncQueueStream {
    type Item = Result<AsyncQueueMessage, String>;

    /// 輪詢下一條訊息
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(msg) = self.buffer.pop_front() {
            return Poll::Ready(Some(Ok(msg)));
        }

        let queue = &mut self.queue;
        match futures::executor::block_on(queue.dequeue(0)) {
            Ok(Some(msg)) => Poll::Ready(Some(Ok(msg))),
            Ok(None) => {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            Err(e) => Poll::Ready(Some(Err(e))),
        }
    }
}

impl AsyncQueue {
    pub fn stream(self) -> AsyncQueueStream {
        AsyncQueueStream::new(self)
    }
}
