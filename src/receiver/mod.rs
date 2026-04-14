use crate::{PING, PONG};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::runtime::Builder;
use tokio::sync::Semaphore;
use crate::utils::copy_bidirectional_with_timeout;

const CONN_NUM: usize = 50;
static SEM: Semaphore = Semaphore::const_new(CONN_NUM);

pub fn start_receiver(upstream_addr: String, downstream_addr: String) {
    let rt = Builder::new_multi_thread().enable_all().build();

    rt.unwrap().block_on(async {
        loop {
            let permit = SEM.acquire().await.unwrap();
            let downstream_addr = downstream_addr.clone();
            let upstream_addr = upstream_addr.clone();
            println!(
                "Connecting to {}. available permits={}",
                downstream_addr,
                SEM.available_permits()
            );
            let _ = tokio::task::Builder::new().name("rcvr-conn").spawn(async move {
                match TcpStream::connect(&downstream_addr).await {
                    Ok(mut stream) => {
                        // ping pong
                        let _ = tokio::task::Builder::new().name("png-loop").spawn(async move {
                            loop {
                                let mut buf: [u8; 4] = [0; 4];
                                let _ = stream.read_exact(&mut buf).await;
                                if buf == PING {
                                    let _ = stream.write_all(PONG).await;
                                    continue;
                                }
                                drop(permit);
                                if buf != [0; 4] {
                                    let _ = tokio::task::Builder::new().name("bi-cpy").spawn(async move {
                                        let _ = start_new_upstream(stream, buf, &upstream_addr).await;
                                        println!("connection to downstream is closed");
                                    });
                                }
                                return;
                            }
                        });
                    }
                    Err(e) => {
                        drop(permit);
                        eprintln!("Connection downstream {} err: {}", downstream_addr, e);
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            });
        }
    });
}

async fn start_new_upstream(mut downstream: TcpStream, buf: [u8; 4], upstream_addr: &str) {
    match TcpStream::connect(upstream_addr).await {
        Ok(mut upstream) => {
            println!("Connected to {}\nStart copy_bidirectional", upstream_addr);
            let _ = upstream.write(&buf).await;
            let _ = copy_bidirectional_with_timeout(&mut upstream, &mut downstream).await;
            println!("copy_bidirectional is closing");
        }
        Err(e) => {
            eprintln!("Connection upstream {} err: {}", upstream_addr, e);
        }
    }
}
