use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::io;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::runtime::{Builder};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{sleep, Duration};
use crate::{PING, PONG};

const UPSTREAM_ADDR: &str = "0.0.0.0:8444";
const DOWNSTREAM_ADDR: &str = "0.0.0.0:8443";

pub fn start_transmitter() {
    let rt = Builder::new_multi_thread()
        .enable_all()
        .build();

    // we ok with panic here
    rt.unwrap().block_on(async {
        let addr_stack_ptr = Arc::new(Mutex::new(Vec::<(TcpStream, SocketAddr)>::new()));
        let addr_stack = addr_stack_ptr.clone();

        // start listener for upstream
        tokio::spawn( async move {
            match TcpListener::bind(UPSTREAM_ADDR).await {
                Ok(listener) => {
                    println!("UPSTREAM addr:{:?}", UPSTREAM_ADDR);
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
                    eprintln!("Failed to bind upstream {}: {}", UPSTREAM_ADDR, e);
                    std::process::exit(1);
                }
            }
        });

        // ping all available streams every 3 sec
        let addr_stack = addr_stack_ptr.clone();
        tokio::spawn( async move {
            loop {
                sleep(Duration::from_secs(3)).await;
                let mut v : Vec<(TcpStream, SocketAddr)> = Vec::new();
                if let Ok(mut arr) = addr_stack.lock() {
                    for stream in arr.drain(..) {
                        v.push(stream);
                    }
                }
                let mut counter = 0;
                for mut stream in v {
                    if let Ok(_) = stream.0.write_all(PING).await {
                        let mut buf : [u8; 4] = [0;4];
                        if let Ok(_) = stream.0.read(&mut buf).await && buf != PONG {
                            println!("conn closed from upstream {}", stream.1);
                            continue; // close stream
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
        match TcpListener::bind(DOWNSTREAM_ADDR).await {
            Ok(listener) => {
                println!("DOWNSTREAM addr:{:?}", DOWNSTREAM_ADDR);
                loop {
                    let mut from_arr = None;
                    if let Ok((mut stream_a, addr)) = listener.accept().await {
                        println!("accepted from downstream {}", addr);
                        'inner: loop {
                            if let Ok(mut guard) = addr_stack.lock() { // TODO попытаться найти какой-нибудь try_fetch
                                from_arr = guard.pop();
                            }
                            match from_arr {
                                Some(mut stream_b) => {
                                    println!("starting copy_bidirectional {} <-> {}", addr, stream_b.1);
                                    tokio::spawn(async move {
                                        let _ = io::copy_bidirectional(&mut stream_a, &mut stream_b.0).await;
                                    });
                                    break 'inner;
                                }
                                None => {
                                    continue 'inner;
                                }
                            }
                        }

                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to bind downstream {}: {}", DOWNSTREAM_ADDR, e);
                std::process::exit(1);
            }
        }
    });
}