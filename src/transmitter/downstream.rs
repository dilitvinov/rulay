use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::io;
use tokio::sync::Mutex;
use crate::transmitter::crypto::verify_reality_auth;
use std::time::Duration;
use crate::utils::copy_bidirectional_with_timeout;

pub async fn start_listener_for_downstream(
    downstream_addr: String,
    redirect_addr: String,
    server_priv_b64: String,
    addr_stack_ptr: Arc<Mutex<Vec<(TcpStream, SocketAddr)>>>
) {
    match TcpListener::bind(&downstream_addr).await {
        Ok(listener) => {
            println!("DOWNSTREAM addr:{:?}", downstream_addr);
            loop {
                if let Ok((mut stream_a, addr)) = listener.accept().await {
                    println!("accepted from downstream {}", addr);
                    let addr_stack = addr_stack_ptr.clone();
                    let server_priv_b64 = server_priv_b64.clone();
                    let redirect_addr = redirect_addr.clone();
                    let _ = tokio::task::Builder::new().name("downstream-client").spawn(async move { // nowait + redirect
                        let buf = match read_tls_record(&mut stream_a).await {
                            Ok(b) => b,
                            Err(e) => {
                                eprintln!("read_tls_record from {}: {}", addr, e);
                                return;
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
                                        let _ = tokio::task::Builder::new().name("copy-bidir-client").spawn(async move {
                                            let _ = copy_bidirectional_with_timeout(stream_a, stream_b.0).await;
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
                            start_redirect(redirect_addr, stream_a, &buf).await;
                        }
                    });
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to bind downstream {}: {}", downstream_addr, e);
        }
    }
}

async fn read_tls_record(stream: &mut TcpStream) -> Result<Vec<u8>, io::Error> {
    let mut header = [0u8; 5];
    let n = tokio::time::timeout(Duration::from_secs(10), stream.read(&mut header)).await??;
    if n != header.len() {
        // empty request, probably http instead of https
        return Ok(Vec::new());
    }
    let body_len = u16::from_be_bytes([header[3], header[4]]) as usize;
    let mut buf = Vec::with_capacity(5 + body_len);
    buf.extend_from_slice(&header);
    buf.resize(5 + body_len, 0); // todo why?
    tokio::time::timeout(Duration::from_secs(10), stream.read_exact(&mut buf[5..])).await??;
    Ok(buf)
}
async fn start_redirect(redirect_addr: String, to_user: TcpStream, read_buffer : &[u8]) {
    let result = async {
        let mut to_server = TcpStream::connect(redirect_addr).await?;
        to_server.write_all(read_buffer).await?;
        Ok::<TcpStream, io::Error>(to_server)
    }.await;
    if let  Ok(to_server) = result {
        let _ = tokio::task::Builder::new().name("copy-bidir-redirect").spawn(async move {
            let _ = copy_bidirectional_with_timeout(to_server, to_user).await;
        });
        return;
    }
    eprintln!("redirect failed: {}", result.unwrap_err());
}