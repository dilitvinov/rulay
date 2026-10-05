use std::sync::Arc;
use tokio::net::TcpListener;
use std::time::Duration;
use crate::stats::tx;
use crate::transmitter::pool::StreamPool;

pub fn start_upstream_listener(upstream_addr: String, pool: Arc<StreamPool>) {
    let _ = tokio::task::Builder::new().name("upstream-listener").spawn(async move {
        match TcpListener::bind(&upstream_addr).await {
            Ok(listener) => {
                println!("UPSTREAM addr:{:?}", upstream_addr);
                loop {
                    match listener.accept().await {
                        Ok(stream) => {
                            println!("accepted from upstream addr:{}", stream.1);
                            tx::UP_ACCEPTED.inc();
                            tx::LAST_UP_ACCEPT.touch();
                            pool.push(stream).await;
                        }
                        Err(e) => {
                            // e.g. EMFILE; back off so a persistent error doesn't spin or flood the log
                            tx::ACCEPT_ERR.inc();
                            eprintln!("accept on upstream failed: {}", e);
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!(
                    "Failed to bind upstream {}: {}",
                    upstream_addr, e
                );
                std::process::exit(1);
            }
        }
    });
}
