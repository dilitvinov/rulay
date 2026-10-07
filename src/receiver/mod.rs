#[cfg(test)]
mod tests;

use crate::{PING, PONG};
use std::io;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::runtime::Builder;
use tokio::sync::Semaphore;
use tokio::time::timeout;
use crate::stats::{self, rx};
use crate::utils::copy_bidirectional_with_timeout;

pub const CONN_NUM: usize = 50;

/// How long an idle connection may go without a PING before it is considered dead, by default.
/// A live connection can legitimately stay silent for the transmitter's ping interval (3s) plus
/// its PONG timeout (15s) when the previous PONG was slow, so this must stay above 18s.
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

/// How long dialing the transmitter may take. Without it a SYN lost on a bad path parks a pool
/// slot for the OS default (~2 min of SYN retries on Linux) instead of being retried right away.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

pub fn start_receiver(upstream_addr: String, downstream_addr: String, idle_timeout: Duration) {
    let rt = Builder::new_multi_thread().enable_all().build();

    rt.unwrap().block_on(run_receiver(upstream_addr, downstream_addr, idle_timeout));
}

/// Runs the receiver on the current tokio runtime; never returns.
/// Keeps up to `CONN_NUM` idle connections open to the transmitter at `downstream_addr`;
/// one that hears no PING for `idle_timeout` is closed and replaced.
pub async fn run_receiver(upstream_addr: String, downstream_addr: String, idle_timeout: Duration) {
    // one permit per idle connection; released once the connection carries a session or dies
    let sem = Arc::new(Semaphore::new(CONN_NUM));
    start_state_logger(sem.clone());
    loop {
        let permit = sem.clone().acquire_owned().await.unwrap();
        let downstream_addr = downstream_addr.clone();
        let upstream_addr = upstream_addr.clone();
        println!(
            "Connecting to {}. available permits={}",
            downstream_addr,
            sem.available_permits()
        );
        let _ = tokio::task::Builder::new().name("rcvr-conn").spawn(async move {
            let connected = {
                let _connecting = rx::CONNECTING.enter();
                match timeout(CONNECT_TIMEOUT, TcpStream::connect(&downstream_addr)).await {
                    Ok(connected) => connected,
                    Err(_) => {
                        rx::CONNECT_TIMED_OUT.inc();
                        Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            format!("no answer in {:?}", CONNECT_TIMEOUT),
                        ))
                    }
                }
            };
            match connected {
                Ok(mut stream) => {
                    rx::CONNECT_OK.inc();
                    // ping pong
                    let _ = tokio::task::Builder::new().name("png-loop").spawn(async move {
                        let idle = rx::IDLE.enter();
                        loop {
                            let mut buf: [u8; 4] = [0; 4];
                            // Silence means the connection is half-open: the transmitter dropped it but
                            // the close never reached us. Without this the slot would be held forever.
                            if timeout(idle_timeout, stream.read_exact(&mut buf)).await.is_err() {
                                rx::IDLE_TIMED_OUT.inc();
                                println!("no PING for {:?}, closing idle connection", idle_timeout);
                                return; // releases the permit, so a fresh connection gets dialed
                            }
                            if buf == PING {
                                rx::PINGS.inc();
                                rx::LAST_PING.touch();
                                let _ = stream.write_all(PONG).await;
                                continue;
                            }
                            drop(permit);
                            drop(idle);
                            if buf != [0; 4] {
                                rx::SESSIONS_STARTED.inc();
                                let _ = tokio::task::Builder::new().name("bi-cpy").spawn(async move {
                                    let _active = rx::ACTIVE.enter();
                                    let _ = start_new_upstream(stream, buf, &upstream_addr).await;
                                    println!("connection to downstream is closed");
                                });
                            } else {
                                rx::IDLE_CLOSED.inc();
                            }
                            return;
                        }
                    });
                }
                Err(e) => {
                    drop(permit);
                    rx::CONNECT_ERR.inc();
                    eprintln!("Connection downstream {} err: {}", downstream_addr, e);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        });
    }
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
            rx::UPSTREAM_ERR.inc();
            eprintln!("Connection upstream {} err: {}", upstream_addr, e);
        }
    }
}

/// Prints one `[state]` line every `STATE_INTERVAL` so a stalled tunnel can be diagnosed after the fact.
fn start_state_logger(sem: Arc<Semaphore>) {
    let _ = tokio::task::Builder::new().name("state-logger").spawn(async move {
        let mut tick = tokio::time::interval(stats::STATE_INTERVAL);
        loop {
            tick.tick().await;
            println!(
                "[state] receiver up={}s permits_free={}/{} idle={} connecting={} active_sessions={} | \
                 connect ok={} err={} timed_out={} | pings={} last_ping={} idle_closed={} idle_timed_out={} | \
                 sessions_started={} upstream_err={} | fds={} tasks={}",
                stats::uptime().as_secs(),
                sem.available_permits(),
                CONN_NUM,
                rx::IDLE.get(),
                rx::CONNECTING.get(),
                rx::ACTIVE.get(),
                rx::CONNECT_OK.get(),
                rx::CONNECT_ERR.get(),
                rx::CONNECT_TIMED_OUT.get(),
                rx::PINGS.get(),
                rx::LAST_PING.ago(),
                rx::IDLE_CLOSED.get(),
                rx::IDLE_TIMED_OUT.get(),
                rx::SESSIONS_STARTED.get(),
                rx::UPSTREAM_ERR.get(),
                stats::open_fds(),
                stats::alive_tasks(),
            );
        }
    });
}
