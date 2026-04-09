use crate::crypto::verify_reality_auth;
use crate::{PING, PONG};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::{io, spawn};
use tokio::io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::Builder;
use tokio::time::{Duration, sleep};

pub fn start_transmitter(
    upstream_addr: String,
    downstream_addr: String,
    redirect_addr: String,
    server_priv_b64: String,
) {
    let rt = Builder::new_multi_thread().enable_all().build();

    // we ok with panic here
    rt.unwrap().block_on(async {
        let addr_stack_ptr = Arc::new(Mutex::new(Vec::<(TcpStream, SocketAddr)>::new()));
        let addr_stack = addr_stack_ptr.clone();
        let upstream_addr_for_listener = upstream_addr.clone();

        // start listener for upstream
        spawn(async move {
            match TcpListener::bind(&upstream_addr_for_listener).await {
                Ok(listener) => {
                    println!("UPSTREAM addr:{:?}", upstream_addr_for_listener);
                    loop {
                        if let Ok(stream) = listener.accept().await {
                            println!("accepted from upstream addr:{}", stream.1);
                            if let Ok(mut arr) = addr_stack.lock() {
                                arr.push(stream)
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!(
                        "Failed to bind upstream {}: {}",
                        upstream_addr_for_listener, e
                    );
                    std::process::exit(1);
                }
            }
        });

        // ping all available streams every 3 sec
        let addr_stack = addr_stack_ptr.clone();
        spawn(async move {
            loop {
                sleep(Duration::from_secs(3)).await;
                let mut v: Vec<(TcpStream, SocketAddr)> = Vec::new();
                if let Ok(mut arr) = addr_stack.lock() {
                    for stream in arr.drain(..) {
                        v.push(stream);
                    }
                }
                let mut counter = 0;
                for mut stream in v {
                    if let Ok(_) = stream.0.write_all(PING).await {
                        let mut buf: [u8; 4] = [0; 4];
                        match stream.0.read(&mut buf).await {
                            Ok(_) if buf != PONG => {
                                println!("conn closed from upstream {}", stream.1);
                                continue; // close stream
                            }
                            Ok(_) => {}
                            Err(_) => {}
                        }
                        if let Ok(mut arr) = addr_stack.lock() {
                            arr.push(stream);
                            counter = counter + 1;
                        }
                    }
                }
                println!("ping complete. len={}", counter);
            }
        });

        // start listener for new clients, connect them to an upstream
        let addr_stack = addr_stack_ptr.clone();
        match TcpListener::bind(&downstream_addr).await {
            Ok(listener) => {
                println!("DOWNSTREAM addr:{:?}", downstream_addr);
                loop {
                    let mut from_arr = None;
                    if let Ok((mut stream_a, addr)) = listener.accept().await {
                        println!("accepted from downstream {}", addr);

                        let mut buf = [0u8; 1024];
                        let n = stream_a.read(&mut buf).await.unwrap_or_default();
                        match verify_reality_auth(&buf[..n], &server_priv_b64) {
                            Ok(true)  => {
                                println!("reality auth: OK");
                                'inner: loop {
                                    if let Ok(mut guard) = addr_stack.lock() {
                                        from_arr = guard.pop();
                                    }
                                    match from_arr {
                                        Some(mut stream_b) => {
                                            stream_b.0.write_all(&buf[..n]).await.unwrap();
                                            println!(
                                                "starting copy_bidirectional {} <-> {}",
                                                addr, stream_b.1
                                            );
                                            spawn(async move {
                                                let _ = copy_bidirectional(&mut stream_a, &mut stream_b.0).await;
                                            });
                                            break 'inner;
                                        }
                                        None => {
                                            continue 'inner;
                                        }
                                    }
                                }
                            },
                            Ok(false) | Err(_) => {
                                println!("reality auth: FAILED, redirecting...");
                                let redirect_addr = redirect_addr.clone();
                                start_redirect(redirect_addr, stream_a, &buf[..n]).await;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to bind downstream {}: {}", downstream_addr, e);
            }
        }
    });
}

async fn start_redirect(redirect_addr: String, mut to_user: TcpStream, read_buffer : &[u8]) {
    let result = async {
        let mut to_server = TcpStream::connect(redirect_addr).await?;
        to_server.write_all(read_buffer).await?;
        Ok::<TcpStream, io::Error>(to_server)
    }.await;
    if let Ok(mut to_server) = result {
        spawn(async move {
            let _ = copy_bidirectional(&mut to_user, &mut to_server).await;
        });
        return;
    }
    eprintln!("redirect failed: {}", result.unwrap_err());
}