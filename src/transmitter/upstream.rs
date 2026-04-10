use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::spawn;
use tokio::sync::Mutex;

pub fn start_upstream_listener(upstream_addr: String, addr_stack: Arc<Mutex<Vec<(TcpStream, SocketAddr)>>>) {
    spawn(async move {
        match TcpListener::bind(&upstream_addr).await {
            Ok(listener) => {
                println!("UPSTREAM addr:{:?}", upstream_addr);
                loop {
                    if let Ok(stream) = listener.accept().await {
                        println!("accepted from upstream addr:{}", stream.1);
                        let mut arr = addr_stack.lock().await;
                        arr.push(stream)
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