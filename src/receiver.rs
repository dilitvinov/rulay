use std::time::Duration;
use tokio::io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream};
use tokio::runtime::Builder;
use tokio::sync::Semaphore;
use crate::{PING, PONG};

const UPSTREAM_ADDR: &str = "94.177.170.43:8443";
const DOWNSTREAM_ADDR: &str = "0.0.0.0:8444";
const CONN_NUM: usize = 10;
static SEM: Semaphore = Semaphore::const_new(CONN_NUM);

pub fn start_receiver() {
    let rt = Builder::new_multi_thread()
        .enable_all()
        .build();

    rt.unwrap().block_on(async {
        loop {
            let permit = SEM.acquire().await.unwrap();
            println!("Connecting to {}. available permits={}", DOWNSTREAM_ADDR, SEM.available_permits());
            tokio::spawn(async move {
                match TcpStream::connect(DOWNSTREAM_ADDR).await {
                    Ok(mut stream) => {
                        // ping pong
                        tokio::spawn(async move {
                            loop {
                                let mut buf: [u8; 4] = [0; 4];
                                let _ = stream.read_exact(&mut buf).await;
                                if buf == PING {
                                    let _ = stream.write_all(PONG).await;
                                    continue;
                                }
                                if buf != [0; 4] {
                                    drop(permit);
                                    start_new_upstream(stream, buf).await;
                                }
                                println!("connection to downstream is closed");
                                return;
                            }
                        });
                    },
                    Err(e) => {
                        eprintln!("Connection downstream {} err: {}", DOWNSTREAM_ADDR, e);
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            });
        }
    });
}

async fn start_new_upstream(mut downstream: TcpStream, buf: [u8; 4]) {
    match TcpStream::connect(UPSTREAM_ADDR).await {
        Ok(mut upstream) => {
            println!("Connected to {}\nStart copy_bidirectional", UPSTREAM_ADDR);
            let _ = upstream.write(&buf).await;
            let _ = copy_bidirectional(& mut upstream, & mut downstream).await;
            println!("copy_bidirectional is closing");
        },
        Err(e) => {
            eprintln!("Connection upstream {} err: {}", UPSTREAM_ADDR, e);
        }
    }
}
