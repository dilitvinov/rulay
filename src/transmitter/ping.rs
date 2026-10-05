use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::task::JoinSet;
use tokio::time::{sleep, timeout};
use crate::stats::tx;
use crate::transmitter::pool::StreamPool;
use crate::{PING, PONG};

const PONG_TIMEOUT: Duration = Duration::from_secs(5);

pub fn start_pinging(pool: Arc<StreamPool>) {
    let _ = tokio::task::Builder::new().name("ping-loop").spawn(async move {
        loop {
            sleep(Duration::from_secs(3)).await;
            // ping every stream in parallel: a single unresponsive peer must not
            // hold the whole pool hostage
            let mut checks = JoinSet::new();
            for stream in pool.drain().await {
                let pool = pool.clone();
                checks.spawn(async move {
                    let _pinging = tx::PINGING.enter();
                    if let Some(stream) = ping_once(stream).await {
                        pool.push(stream).await;
                    }
                });
            }
            checks.join_all().await;
        }
    });
}

/// Returns the stream back if it answered PONG in time, otherwise drops (closes) it.
async fn ping_once(mut stream: (TcpStream, SocketAddr)) -> Option<(TcpStream, SocketAddr)> {
    let mut buf: [u8; 4] = [0; 4];
    let exchange = async {
        stream.0.write_all(PING).await?;
        stream.0.read_exact(&mut buf).await
    };
    match timeout(PONG_TIMEOUT, exchange).await {
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
