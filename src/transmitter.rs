use crate::crypto::verify_reality_auth;
use crate::{PING, PONG};
use std::net::SocketAddr;
use std::sync::{Arc};
use tokio::{io, spawn};
use tokio::sync::Mutex;
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
                            let mut arr = addr_stack.lock().await;
                            arr.push(stream)
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

        // start listener for new clients, connect them to an upstream
        let addr_stack = addr_stack_ptr.clone();
        match TcpListener::bind(&downstream_addr).await {
            Ok(listener) => {
                println!("DOWNSTREAM addr:{:?}", downstream_addr);
                loop {
                    if let Ok((mut stream_a, addr)) = listener.accept().await {
                        println!("accepted from downstream {}", addr);

                        let buf = match read_tls_record(&mut stream_a).await {
                            Ok(b) => b,
                            Err(e) => {
                                eprintln!("read_tls_record from {}: {}", addr, e);
                                continue;
                            }
                        };
                        if let Ok(true) = verify_reality_auth(&buf, &server_priv_b64) {
                            println!("reality auth: OK");
                            'inner: loop {
                                let mut guard = addr_stack.lock().await;
                                let from_arr = guard.pop();
                                drop(guard);
                                match from_arr {
                                    Some(mut stream_b) => {
                                        stream_b.0.write_all(&buf).await.unwrap();
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
                                        tokio::task::yield_now().await;
                                        continue 'inner;
                                    }
                                }
                            }
                        } else {
                            println!("reality auth: FAILED, redirecting...");
                            let redirect_addr = redirect_addr.clone();
                            start_redirect(redirect_addr, stream_a, &buf).await;
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

/// Reads exactly one TLS record from the stream.
/// Returns the full raw bytes: 5-byte header + body.
async fn read_tls_record(stream: &mut TcpStream) -> Result<Vec<u8>, io::Error> {
    let mut header = [0u8; 5];
    stream.read_exact(&mut header).await?;
    let body_len = u16::from_be_bytes([header[3], header[4]]) as usize;
    let mut buf = Vec::with_capacity(5 + body_len);
    buf.extend_from_slice(&header);
    buf.resize(5 + body_len, 0);
    stream.read_exact(&mut buf[5..]).await?;
    Ok(buf)
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