//! Receiver over real loopback sockets. The test plays the transmitter (a listener the receiver
//! dials into) and the target upstream the receiver forwards sessions to.

use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use crate::receiver::{run_receiver, CONN_NUM, DEFAULT_IDLE_TIMEOUT};
use crate::test_utils::*;
use crate::{PING, PONG};

struct Receiver {
    /// Where the receiver keeps its idle connections (the transmitter's upstream port).
    transmitter: TcpListener,
    /// Where the receiver forwards sessions to.
    target: TcpListener,
}

async fn start() -> Receiver {
    start_with(DEFAULT_IDLE_TIMEOUT).await
}

/// Like `start`, but idle connections that hear no PING for `idle_timeout` get closed.
async fn start_with(idle_timeout: Duration) -> Receiver {
    let transmitter = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
    tokio::spawn(run_receiver(
        target.local_addr().unwrap().to_string(),
        transmitter.local_addr().unwrap().to_string(),
        idle_timeout,
    ));
    Receiver { transmitter, target }
}

async fn accept(listener: &TcpListener, what: &str) -> TcpStream {
    within(5, what, listener.accept()).await.unwrap().0
}

async fn expect_no_connection(listener: &TcpListener) {
    let extra = tokio::time::timeout(Duration::from_millis(500), listener.accept()).await;
    assert!(extra.is_err(), "receiver opened more than {} idle connections", CONN_NUM);
}

/// Starts a session on one idle connection and returns (transmitter side, target side).
async fn open_session(r: &Receiver) -> (TcpStream, TcpStream) {
    let mut tx_side = accept(&r.transmitter, "idle connection from receiver").await;
    tx_side.write_all(b"hello world").await.unwrap();
    let mut target_side = accept(&r.target, "receiver to dial the target").await;
    // the 4 bytes the receiver peeked at must not be lost
    assert_eq!(read_n(&mut target_side, 11, "session payload at target").await, b"hello world");
    (tx_side, target_side)
}

#[tokio::test(flavor = "multi_thread")]
async fn opens_full_pool_of_connections_on_start() {
    let r = start().await;
    let mut idle = Vec::new();
    for i in 0..CONN_NUM {
        idle.push(accept(&r.transmitter, &format!("idle connection #{i}")).await);
    }
    expect_no_connection(&r.transmitter).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn answers_ping_with_pong() {
    let r = start().await;
    let mut conn = accept(&r.transmitter, "idle connection").await;

    for _ in 0..2 {
        conn.write_all(PING).await.unwrap();
        assert_eq!(read_n(&mut conn, 4, "PONG").await, PONG);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn forwards_session_to_target_both_ways() {
    let r = start().await;
    let (mut tx_side, mut target_side) = open_session(&r).await;

    target_side.write_all(b"reply").await.unwrap();
    assert_eq!(read_n(&mut tx_side, 5, "target reply at transmitter").await, b"reply");

    tx_side.write_all(b"more").await.unwrap();
    assert_eq!(read_n(&mut target_side, 4, "follow-up at target").await, b"more");
}

#[tokio::test(flavor = "multi_thread")]
async fn replaces_connection_taken_by_session() {
    let r = start().await;
    let mut idle = Vec::new();
    for i in 0..CONN_NUM - 1 {
        idle.push(accept(&r.transmitter, &format!("idle connection #{i}")).await);
    }
    let _session = open_session(&r).await;

    let _replacement = accept(&r.transmitter, "replacement for the connection now in a session").await;
    expect_no_connection(&r.transmitter).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn reconnects_when_transmitter_closes_idle_connection() {
    let r = start().await;
    let mut idle = Vec::new();
    for i in 0..CONN_NUM {
        idle.push(accept(&r.transmitter, &format!("idle connection #{i}")).await);
    }

    drop(idle.pop());
    let _replacement = accept(&r.transmitter, "replacement for the closed connection").await;
    expect_no_connection(&r.transmitter).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn closes_transmitter_side_when_target_disconnects() {
    let r = start().await;
    let (mut tx_side, target_side) = open_session(&r).await;

    drop(target_side);
    expect_closed(&mut tx_side, 5, "transmitter side to be closed").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn closes_target_side_when_transmitter_disconnects() {
    let r = start().await;
    let (tx_side, mut target_side) = open_session(&r).await;

    drop(tx_side);
    expect_closed(&mut target_side, 5, "target side to be closed").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn closes_session_when_target_is_unreachable() {
    let transmitter = TcpListener::bind("127.0.0.1:0").await.unwrap();
    tokio::spawn(run_receiver(
        free_addr(),
        transmitter.local_addr().unwrap().to_string(),
        DEFAULT_IDLE_TIMEOUT,
    ));

    let mut conn = accept(&transmitter, "idle connection").await;
    conn.write_all(b"hello world").await.unwrap();
    expect_closed(&mut conn, 5, "session without a target to be closed").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn closes_idle_connection_that_hears_no_ping() {
    let r = start_with(Duration::from_millis(300)).await;
    let mut conn = accept(&r.transmitter, "idle connection").await;

    expect_closed(&mut conn, 2, "silent idle connection to be closed").await;
    let _replacement = accept(&r.transmitter, "replacement for the timed-out connection").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn keeps_idle_connection_that_keeps_getting_pings() {
    let r = start_with(Duration::from_millis(300)).await;
    let mut conn = accept(&r.transmitter, "idle connection").await;

    // ping well within the timeout for several timeouts' worth of time: the connection must survive
    for _ in 0..10 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        conn.write_all(PING).await.unwrap();
        assert_eq!(read_n(&mut conn, 4, "PONG").await, PONG);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn replaces_half_open_idle_connections() {
    let transmitter = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let link = LossyLink::start(transmitter.local_addr().unwrap().to_string()).await;
    tokio::spawn(run_receiver(free_addr(), link.addr.clone(), Duration::from_secs(1)));

    let mut idle = Vec::new();
    for i in 0..CONN_NUM {
        idle.push(accept(&transmitter, &format!("idle connection #{i}")).await);
    }

    // the path dies silently and the transmitter gives up on all of them; the receiver hears nothing
    link.lose_existing();
    drop(idle);
    wait_until(10, "transmitter-side closes to reach the link", || link.target_closed() == CONN_NUM).await;

    // no PINGs arrive any more: the receiver must give up on the dead connections and dial again
    wait_until(5, "receiver to replace half-open connections", || link.accepted() > CONN_NUM).await;
}
