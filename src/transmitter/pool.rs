use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::{Mutex, Notify};
use tokio::time::{Instant, timeout_at};

/// Pool of idle upstream connections waiting to be handed to a downstream client.
pub struct StreamPool {
    streams: Mutex<Vec<(TcpStream, SocketAddr)>>,
    notify: Notify,
}

impl StreamPool {
    pub fn new() -> Self {
        StreamPool {
            streams: Mutex::new(Vec::new()),
            notify: Notify::new(),
        }
    }

    pub async fn push(&self, stream: (TcpStream, SocketAddr)) {
        self.streams.lock().await.push(stream);
        // notify_one keeps a permit even with no waiter, so a wakeup is never lost
        self.notify.notify_one();
    }

    pub async fn len(&self) -> usize {
        self.streams.lock().await.len()
    }

    pub async fn drain(&self) -> Vec<(TcpStream, SocketAddr)> {
        std::mem::take(&mut *self.streams.lock().await)
    }

    /// Waits (without burning CPU) until a stream is available or `wait` elapses.
    pub async fn pop_wait(&self, wait: Duration) -> Option<(TcpStream, SocketAddr)> {
        let deadline = Instant::now() + wait;
        loop {
            if let Some(stream) = self.streams.lock().await.pop() {
                return Some(stream);
            }
            timeout_at(deadline, self.notify.notified()).await.ok()?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::net::TcpListener;

    async fn pair() -> (TcpStream, SocketAddr) {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        let s = TcpStream::connect(addr).await.unwrap();
        (s, addr)
    }

    #[tokio::test]
    async fn empty_pool_waits_without_spinning() {
        let pool = StreamPool::new();
        let cpu_before = std::time::Instant::now();
        let waited = tokio::time::Instant::now();
        assert!(pool.pop_wait(Duration::from_millis(300)).await.is_none());
        // the wait must have actually elapsed on the timer, not been burned in a loop
        assert!(waited.elapsed() >= Duration::from_millis(300));
        assert!(cpu_before.elapsed() < Duration::from_secs(2));
    }

    #[tokio::test]
    async fn push_wakes_a_waiter() {
        let pool = Arc::new(StreamPool::new());
        let writer = pool.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            writer.push(pair().await).await;
        });
        assert!(pool.pop_wait(Duration::from_secs(5)).await.is_some());
    }
}
