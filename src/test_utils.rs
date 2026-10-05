//! Helpers shared by the networking tests.

use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use crate::{PING, PONG};

/// A loopback address nobody is listening on right now.
pub fn free_addr() -> String {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().to_string()
}

/// Awaits `fut`, failing the test with `what` if it takes longer than `secs`.
pub async fn within<F: Future>(secs: u64, what: &str, fut: F) -> F::Output {
    tokio::time::timeout(Duration::from_secs(secs), fut)
        .await
        .unwrap_or_else(|_| panic!("timed out after {}s: {}", secs, what))
}

/// Connects to a listener that may still be starting up.
pub async fn connect_retry(addr: &str) -> TcpStream {
    within(5, &format!("connect to {}", addr), async {
        loop {
            match TcpStream::connect(addr).await {
                Ok(s) => return s,
                Err(_) => tokio::time::sleep(Duration::from_millis(20)).await,
            }
        }
    })
    .await
}

pub async fn read_n(stream: &mut TcpStream, n: usize, what: &str) -> Vec<u8> {
    let mut buf = vec![0u8; n];
    within(5, what, stream.read_exact(&mut buf)).await.unwrap();
    buf
}

/// Asserts the peer closed the connection (EOF or reset) without sending anything else.
pub async fn expect_closed(stream: &mut TcpStream, secs: u64, what: &str) {
    let mut buf = [0u8; 64];
    match within(secs, what, stream.read(&mut buf)).await {
        Ok(0) | Err(_) => {}
        Ok(n) => panic!("{}: expected close, got {} bytes: {:?}", what, n, &buf[..n]),
    }
}

/// Plays the receiver on an idle pooled connection: answers PINGs until the first
/// non-PING bytes arrive, and returns those 4 bytes.
pub async fn answer_pings_until_data(stream: &mut TcpStream) -> [u8; 4] {
    within(10, "first payload bytes on pooled connection", async {
        loop {
            let mut buf = [0u8; 4];
            stream.read_exact(&mut buf).await.unwrap();
            if buf != PING {
                return buf;
            }
            stream.write_all(PONG).await.unwrap();
        }
    })
    .await
}

/// Polls `cond` until it holds, failing the test with `what` after `secs`.
pub async fn wait_until(secs: u64, what: &str, cond: impl Fn() -> bool) {
    within(secs, what, async {
        while !cond() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
}

/// A TCP proxy standing in for the network between two peers. `lose_existing` makes the path
/// silently drop every packet of the connections open at that moment, FIN and RST included:
/// neither side is told anything, so each one is left with a half-open connection. Connections
/// opened afterwards are forwarded normally, as after a NAT/conntrack reset or a route flap.
pub struct LossyLink {
    pub addr: String,
    accepted: Arc<AtomicUsize>,
    target_closed: Arc<AtomicUsize>,
    cuts: Arc<Mutex<Vec<oneshot::Sender<()>>>>,
}

impl LossyLink {
    /// Proxies every connection made to `self.addr` on to `target`.
    pub async fn start(target: String) -> LossyLink {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let link = LossyLink {
            addr: listener.local_addr().unwrap().to_string(),
            accepted: Arc::new(AtomicUsize::new(0)),
            target_closed: Arc::new(AtomicUsize::new(0)),
            cuts: Arc::new(Mutex::new(Vec::new())),
        };
        let (accepted, target_closed, cuts) =
            (link.accepted.clone(), link.target_closed.clone(), link.cuts.clone());
        tokio::spawn(async move {
            loop {
                let (mut client, _) = listener.accept().await.unwrap();
                accepted.fetch_add(1, SeqCst);
                let (cut_tx, cut_rx) = oneshot::channel();
                cuts.lock().unwrap().push(cut_tx);
                let target = target.clone();
                let target_closed = target_closed.clone();
                tokio::spawn(async move {
                    let Ok(mut server) = TcpStream::connect(&target).await else { return };
                    tokio::select! {
                        _ = tokio::io::copy_bidirectional(&mut client, &mut server) => {}
                        Ok(()) = cut_rx => {
                            // everything sent from now on is lost; notice when the target gives up
                            let mut buf = [0u8; 1024];
                            while let Ok(1..) = server.read(&mut buf).await {}
                            target_closed.fetch_add(1, SeqCst);
                            // ...but never tell the client: keep its socket open for good
                            std::future::pending::<()>().await;
                        }
                    }
                });
            }
        });
        link
    }

    /// Connections the link has accepted so far.
    pub fn accepted(&self) -> usize {
        self.accepted.load(SeqCst)
    }

    /// Lost connections that the target side has since closed (the closure was not delivered).
    pub fn target_closed(&self) -> usize {
        self.target_closed.load(SeqCst)
    }

    /// From now on, silently drop all packets of the connections open right now.
    pub fn lose_existing(&self) {
        for cut in self.cuts.lock().unwrap().drain(..) {
            let _ = cut.send(());
        }
    }
}
