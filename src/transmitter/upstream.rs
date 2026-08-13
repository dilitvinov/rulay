use std::sync::Arc;
use tokio::net::TcpListener;
use crate::transmitter::pool::StreamPool;

pub fn start_upstream_listener(upstream_addr: String, pool: Arc<StreamPool>) {
    let _ = tokio::task::Builder::new().name("upstream-listener").spawn(async move {
        match TcpListener::bind(&upstream_addr).await {
            Ok(listener) => {
                println!("UPSTREAM addr:{:?}", upstream_addr);
                loop {
                    if let Ok(stream) = listener.accept().await {
                        println!("accepted from upstream addr:{}", stream.1);
                        pool.push(stream).await;
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
