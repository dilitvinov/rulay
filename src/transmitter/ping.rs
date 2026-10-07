use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout};
use crate::stats::tx;
use crate::transmitter::pool::StreamPool;
use crate::{PING, PONG};

pub const PING_INTERVAL: Duration = Duration::from_secs(3);

pub fn start_pinging(pool: Arc<StreamPool>, pong_timeout: Duration) {
    let _ = tokio::task::Builder::new().name("ping-loop").spawn(async move {
        loop {
            sleep(PING_INTERVAL).await;
            // each stream is pinged on its own and goes back to the pool as soon as it answers:
            // a slow PONG (retransmits on a lossy path) keeps only that stream out of the pool,
            // and never delays the next round for all the others
            for stream in pool.drain().await {
                let pool = pool.clone();
                let _ = tokio::task::Builder::new().name("ping").spawn(async move {
                    let _pinging = tx::PINGING.enter();
                    if let Some(stream) = ping_once(stream, pong_timeout).await {
                        pool.push(stream).await;
                    }
                });
            }
        }
    });
}

/// Returns the stream back if it answered PONG in time, otherwise drops (closes) it.
async fn ping_once(
    mut stream: (TcpStream, SocketAddr),
    pong_timeout: Duration,
) -> Option<(TcpStream, SocketAddr)> {
    let mut buf: [u8; 4] = [0; 4];
    let exchange = async {
        stream.0.write_all(PING).await?;
        stream.0.read_exact(&mut buf).await
    };
    match timeout(pong_timeout, exchange).await {
        Ok(Ok(_)) if buf == PONG => {
            tx::PONG_OK.inc();
            tx::LAST_PONG.touch();
            Some(stream)
        }
        _ => {
            tx::PING_DROPPED.inc();
            println!("conn closed from upstream {}", stream.1);
            None
        }
    }
}
