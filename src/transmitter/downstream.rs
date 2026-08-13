use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::io;
use tokio::time::Instant;
use crate::transmitter::crypto::verify_reality_auth;
use crate::transmitter::pool::StreamPool;
use std::time::Duration;
use crate::utils::copy_bidirectional_with_timeout;

/// How long an authenticated client waits for a free upstream before being dropped.
const UPSTREAM_WAIT: Duration = Duration::from_secs(30);

pub async fn start_listener_for_downstream(
    downstream_addr: String,
    redirect_addr: String,
    server_priv_b64: String,
    pool_ptr: Arc<StreamPool>
) {
    match TcpListener::bind(&downstream_addr).await {
        Ok(listener) => {
            println!("DOWNSTREAM addr:{:?}", downstream_addr);
            loop {
                if let Ok((mut stream_a, addr)) = listener.accept().await {
                    println!("accepted from downstream {}", addr);
                    let pool = pool_ptr.clone();
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
                            let deadline = Instant::now() + UPSTREAM_WAIT;
                            'inner: loop {
                                let wait = deadline.saturating_duration_since(Instant::now());
                                match pool.pop_wait(wait).await {
                                    Some(mut stream_b) => {
                                        // a pooled stream may have died while idle: try the next one
                                        if let Err(e) = stream_b.0.write_all(&buf).await {
                                            eprintln!("upstream {} is dead: {}", stream_b.1, e);
                                            continue 'inner;
                                        }
                                        println!(
                                            "starting copy_bidirectional {} <-> {}",
                                            addr, stream_b.1
                                        );
                                        let _ = tokio::task::Builder::new().name("copy-bidir-client").spawn(async move {
                                            let _ = copy_bidirectional_with_timeout(&mut stream_a, &mut stream_b.0).await;
                                        });
                                        break 'inner;
                                    }
                                    None => {
                                        eprintln!("no upstream available for {}, dropping", addr);
                                        break 'inner;
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
async fn start_redirect(redirect_addr: String, mut to_user: TcpStream, read_buffer : &[u8]) {
    let result = async {
        let mut to_server = TcpStream::connect(redirect_addr).await?;
        to_server.write_all(read_buffer).await?;
        Ok::<TcpStream, io::Error>(to_server)
    }.await;
    if let  Ok(mut to_server) = result {
        let _ = tokio::task::Builder::new().name("copy-bidir-redirect").spawn(async move {
            let _ = copy_bidirectional_with_timeout(&mut to_server, &mut to_user).await;
        });
        return;
    }
    eprintln!("redirect failed: {}", result.unwrap_err());
}