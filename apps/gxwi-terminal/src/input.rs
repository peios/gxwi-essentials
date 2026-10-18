//! Ordered terminal input with a byte budget retained until the write completes.

use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use axum::body::Bytes;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot};

/// The aggregate queued and currently written input, shared by every page.
pub const MAX_INPUT_BYTES: usize = 1024 * 1024;
const MAX_INPUT_MESSAGES: usize = 64;

struct Pending {
    bytes: Bytes,
    _budget: OwnedSemaphorePermit,
    completed: oneshot::Sender<io::Result<()>>,
}

pub struct Input {
    sender: Mutex<Option<mpsc::Sender<Pending>>>,
    budget: Arc<Semaphore>,
    capacity_bytes: usize,
}

fn closed() -> io::Error {
    io::Error::new(
        io::ErrorKind::BrokenPipe,
        "the terminal no longer accepts input",
    )
}

impl Input {
    /// Only the dedicated writer blocks on the pseudoterminal; async callers wait for capacity.
    pub fn new(mut writer: impl Write + Send + 'static) -> Self {
        let (input, mut incoming) = Self::channel(MAX_INPUT_BYTES);
        let budget = Arc::clone(&input.budget);
        std::thread::spawn(move || {
            while let Some(pending) = incoming.blocking_recv() {
                let result = writer.write_all(&pending.bytes);
                let failed = result.is_err();
                let _ = pending.completed.send(result);
                if failed {
                    budget.close();
                    break;
                }
            }
        });
        input
    }

    fn channel(capacity_bytes: usize) -> (Self, mpsc::Receiver<Pending>) {
        let (sender, incoming) = mpsc::channel(MAX_INPUT_MESSAGES);
        (
            Self {
                sender: Mutex::new(Some(sender)),
                budget: Arc::new(Semaphore::new(capacity_bytes)),
                capacity_bytes,
            },
            incoming,
        )
    }

    /// Wait for admission and then for this whole message to be written, in queue order.
    ///
    /// Dropping this future after admission does not retract bytes already queued for the child.
    /// A disconnected page must not replay them: their delivery may already have happened.
    pub async fn send(&self, bytes: Bytes) -> io::Result<()> {
        if bytes.len() > self.capacity_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "terminal input exceeds its byte limit",
            ));
        }
        if bytes.is_empty() {
            return Ok(());
        }
        let sender = self
            .sender
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .ok_or_else(closed)?;
        let budget = Arc::clone(&self.budget)
            .acquire_many_owned(bytes.len() as u32)
            .await
            .map_err(|_| closed())?;
        let (completed, result) = oneshot::channel();
        sender
            .send(Pending {
                bytes,
                _budget: budget,
                completed,
            })
            .await
            .map_err(|_| closed())?;
        result.await.map_err(|_| closed())?
    }

    #[cfg(test)]
    pub fn pending_bytes(&self) -> usize {
        self.capacity_bytes - self.budget.available_permits()
    }

    /// Wake blocked senders and stop retaining the writer when the child has ended.
    pub fn close(&self) {
        self.budget.close();
        self.sender
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
    }
}

#[cfg(test)]
mod tests {
    use std::future::{Future, poll_fn};
    use std::pin::Pin;
    use std::task::Poll;

    use super::*;

    async fn pending(future: Pin<&mut impl Future>) {
        let mut future = future;
        poll_fn(|context| {
            assert!(future.as_mut().poll(context).is_pending());
            Poll::Ready(())
        })
        .await;
    }

    #[tokio::test]
    async fn budget_includes_the_current_write_and_close_wakes_waiters() {
        let (input, mut incoming) = Input::channel(4);
        let first = input.send(Bytes::from_static(b"abcd"));
        tokio::pin!(first);
        pending(first.as_mut()).await;
        let writing = incoming.recv().await.unwrap();
        assert_eq!(input.budget.available_permits(), 0);
        let second = input.send(Bytes::from_static(b"e"));
        tokio::pin!(second);
        pending(second.as_mut()).await;
        assert!(incoming.try_recv().is_err());
        input.close();
        assert_eq!(second.await.unwrap_err().kind(), io::ErrorKind::BrokenPipe);
        writing.completed.send(Ok(())).unwrap();
        first.await.unwrap();
    }

    #[tokio::test]
    async fn accepted_messages_keep_order_and_finish_only_after_writing() {
        let (input, mut incoming) = Input::channel(8);
        let first = input.send(Bytes::from_static(b"one"));
        let second = input.send(Bytes::from_static(b"two"));
        tokio::pin!(first, second);
        pending(first.as_mut()).await;
        pending(second.as_mut()).await;
        let one = incoming.recv().await.unwrap();
        let two = incoming.recv().await.unwrap();
        assert_eq!(&one.bytes[..], b"one");
        assert_eq!(&two.bytes[..], b"two");
        pending(first.as_mut()).await;
        one.completed.send(Ok(())).unwrap();
        two.completed.send(Ok(())).unwrap();
        first.await.unwrap();
        second.await.unwrap();
    }

    #[tokio::test]
    async fn oversized_messages_are_refused_before_admission() {
        let (input, mut incoming) = Input::channel(4);
        assert_eq!(
            input
                .send(Bytes::from_static(b"12345"))
                .await
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
        assert!(incoming.try_recv().is_err());
        assert_eq!(input.budget.available_permits(), 4);
    }

    struct Failing;

    impl Write for Failing {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(closed())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn writer_failure_refuses_later_input() {
        let input = Input::new(Failing);
        assert_eq!(
            input
                .send(Bytes::from_static(b"one"))
                .await
                .unwrap_err()
                .kind(),
            io::ErrorKind::BrokenPipe
        );
        assert_eq!(
            input
                .send(Bytes::from_static(b"two"))
                .await
                .unwrap_err()
                .kind(),
            io::ErrorKind::BrokenPipe
        );
    }

    #[tokio::test]
    async fn dropping_an_admitted_sender_does_not_retract_its_bytes() {
        let (input, mut incoming) = Input::channel(4);
        let mut sending = Box::pin(input.send(Bytes::from_static(b"one")));
        pending(sending.as_mut()).await;
        drop(sending);
        let admitted = incoming.recv().await.unwrap();
        assert_eq!(&admitted.bytes[..], b"one");
        assert!(admitted.completed.send(Ok(())).is_err());
    }

    struct ClosedWriter(std::sync::mpsc::Sender<()>);

    impl Drop for ClosedWriter {
        fn drop(&mut self) {
            let _ = self.0.send(());
        }
    }

    impl Write for ClosedWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn closing_idle_input_releases_the_writer_even_while_routes_retain_input() {
        let (closed, heard) = std::sync::mpsc::channel();
        let input = Input::new(ClosedWriter(closed));
        input.close();
        heard
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        assert!(input.budget.is_closed());
    }
}
