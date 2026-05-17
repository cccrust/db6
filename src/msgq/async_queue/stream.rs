use super::queue::AsyncQueue;
use super::AsyncQueueMessage;
use futures::stream::Stream;
use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{Context, Poll};

pub struct AsyncQueueStream {
    queue: AsyncQueue,
    buffer: VecDeque<AsyncQueueMessage>,
}

impl AsyncQueueStream {
    pub fn new(queue: AsyncQueue) -> Self {
        Self {
            queue,
            buffer: VecDeque::new(),
        }
    }
}

impl Stream for AsyncQueueStream {
    type Item = Result<AsyncQueueMessage, String>;

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
