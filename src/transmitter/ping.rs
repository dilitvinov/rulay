use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::spawn;
use tokio::sync::Mutex;
use tokio::time::sleep;
use crate::{PING, PONG};

pub fn start_pinging(addr_stack: Arc<Mutex<Vec<(TcpStream, SocketAddr)>>>) {
    spawn(async move {
        loop {
            sleep(Duration::from_secs(3)).await;
            let mut v: Vec<(TcpStream, SocketAddr)> = Vec::new();
            let mut arr = addr_stack.lock().await;
            for stream in arr.drain(..) {
                v.push(stream);
            }
            drop(arr);
            let mut counter = 0;
            for mut stream in v {
                if let Ok(_) = stream.0.write_all(PING).await {
                    let mut buf: [u8; 4] = [0; 4];
                    if let Ok(_) = stream.0.read_exact(&mut buf).await && buf == PONG {} else {
                        println!("conn closed from upstream {}", stream.1);
                        continue; // close stream
                    }
                    let mut arr = addr_stack.lock().await;
                    arr.push(stream);
                    counter = counter + 1;
                }
            }
        }
    });
}
