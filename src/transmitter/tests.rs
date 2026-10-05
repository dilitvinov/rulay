//! Transmitter over real loopback sockets. The test plays both the receiver (connecting to the
//! upstream port) and the clients (connecting to the downstream port).

use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::time::Instant;
use crate::test_utils::*;
use crate::transmitter::crypto::reality_client_hello;
use crate::transmitter::{run_transmitter, DEFAULT_UPSTREAM_WAIT};
use crate::{PING, PONG};

const SERVER_KEY: &str = "uM5Zol5nBgyqDrn2RYGhmTeoONiULxeLMhkeDqMtMUE";
const OTHER_KEY: &str = "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE";

struct Transmitter {
    upstream: String,
    downstream: String,
}

/// Starts a transmitter on fresh ports; unauthenticated clients go to `redirect`.
fn start(redirect: &str) -> Transmitter {
    start_with(redirect, DEFAULT_UPSTREAM_WAIT)
}

/// Like `start`, but authenticated clients give up waiting for an upstream after `upstream_wait`.
fn start_with(redirect: &str, upstream_wait: Duration) -> Transmitter {
    let t = Transmitter { upstream: free_addr(), downstream: free_addr() };
    tokio::spawn(run_transmitter(
        t.upstream.clone(),
        t.downstream.clone(),
        redirect.to_string(),
        SERVER_KEY.to_string(),
        upstream_wait,
    ));
    t
}

/// An authenticated client paired with the pooled receiver connection it was handed.
struct Session {
    client: TcpStream,
    receiver: TcpStream,
}

async fn open_session(t: &Transmitter) -> Session {
    let mut receiver = connect_retry(&t.upstream).await;
    let mut client = connect_retry(&t.downstream).await;
    let hello = reality_client_hello(SERVER_KEY);
    client.write_all(&hello).await.unwrap();

    // the transmitter forwards the hello verbatim as the first bytes of the session
    let head = answer_pings_until_data(&mut receiver).await;
    let rest = read_n(&mut receiver, hello.len() - 4, "rest of the forwarded hello").await;
    assert_eq!([&head[..], &rest[..]].concat(), hello);
    Session { client, receiver }
}

#[tokio::test(flavor = "multi_thread")]
async fn starts_listening_on_upstream_and_downstream_ports() {
    let t = start(&free_addr());
    connect_retry(&t.upstream).await;
    connect_retry(&t.downstream).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn keeps_pinging_accepted_upstream_connection() {
    let t = start(&free_addr());
    let mut receiver = connect_retry(&t.upstream).await;

    // two rounds: the second PING proves the connection went back to the pool after PONG
    for round in 1..=2 {
        let ping = read_n(&mut receiver, 4, &format!("PING #{round}")).await;
        assert_eq!(ping, PING);
        receiver.write_all(PONG).await.unwrap();
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn drops_upstream_connection_that_does_not_answer_ping() {
    let t = start(&free_addr());
    let mut receiver = connect_retry(&t.upstream).await;

    let ping = read_n(&mut receiver, 4, "PING").await;
    assert_eq!(ping, PING);
    // no PONG: the transmitter must give up after its PONG timeout (5s) and close
    expect_closed(&mut receiver, 8, "unanswered upstream to be closed").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn pairs_authenticated_client_with_pooled_upstream() {
    let t = start(&free_addr());
    let Session { mut client, mut receiver } = open_session(&t).await;

    client.write_all(b"from client").await.unwrap();
    assert_eq!(read_n(&mut receiver, 11, "client payload at receiver").await, b"from client");

    receiver.write_all(b"from upstream").await.unwrap();
    assert_eq!(read_n(&mut client, 13, "upstream payload at client").await, b"from upstream");
}

#[tokio::test(flavor = "multi_thread")]
async fn waits_for_upstream_when_client_arrives_first() {
    let t = start(&free_addr());
    let mut client = connect_retry(&t.downstream).await;
    let hello = reality_client_hello(SERVER_KEY);
    client.write_all(&hello).await.unwrap();

    // give the transmitter time to start waiting on the empty pool
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let mut receiver = connect_retry(&t.upstream).await;
    let head = answer_pings_until_data(&mut receiver).await;
    assert_eq!(head, hello[..4]);
}

#[tokio::test(flavor = "multi_thread")]
async fn redirects_unauthenticated_client() {
    let redirect = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let t = start(&redirect.local_addr().unwrap().to_string());
    let mut client = connect_retry(&t.downstream).await;
    let hello = reality_client_hello(OTHER_KEY);
    client.write_all(&hello).await.unwrap();

    let (mut site, _) = within(5, "redirect target to be connected", redirect.accept()).await.unwrap();
    assert_eq!(read_n(&mut site, hello.len(), "hello at redirect target").await, hello);

    site.write_all(b"cover site").await.unwrap();
    assert_eq!(read_n(&mut client, 10, "redirect reply at client").await, b"cover site");
}

#[tokio::test(flavor = "multi_thread")]
async fn closes_upstream_when_client_disconnects() {
    let t = start(&free_addr());
    let Session { client, mut receiver } = open_session(&t).await;

    drop(client);
    expect_closed(&mut receiver, 5, "receiver side to be closed").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn closes_client_when_upstream_disconnects() {
    let t = start(&free_addr());
    let Session { mut client, receiver } = open_session(&t).await;

    drop(receiver);
    expect_closed(&mut client, 5, "client side to be closed").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn closes_client_that_sends_nothing() {
    let t = start(&free_addr());
    let mut client = connect_retry(&t.downstream).await;

    // the transmitter waits 10s for the first TLS record, then gives up
    expect_closed(&mut client, 13, "silent client to be closed").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn drops_client_when_no_upstream_arrives_in_time() {
    let t = start_with(&free_addr(), Duration::from_millis(100));
    let mut client = connect_retry(&t.downstream).await;
    client.write_all(&reality_client_hello(SERVER_KEY)).await.unwrap();

    let sent = Instant::now();
    expect_closed(&mut client, 2, "client without an upstream to be dropped").await;
    assert!(sent.elapsed() >= Duration::from_millis(100), "dropped before the wait ran out");
}

#[tokio::test(flavor = "multi_thread")]
async fn drops_half_open_upstream_instead_of_handing_it_out() {
    let t = start_with(&free_addr(), Duration::from_millis(100));
    let link = LossyLink::start(t.upstream.clone()).await;
    let mut receiver = connect_retry(&link.addr).await;

    // healthy at first: one full PING/PONG round
    assert_eq!(read_n(&mut receiver, 4, "PING").await, PING);
    receiver.write_all(PONG).await.unwrap();

    // the path dies silently: the next PING goes unanswered, the transmitter must drop the connection
    link.lose_existing();
    wait_until(10, "transmitter to drop the half-open upstream", || link.target_closed() == 1).await;

    // and must not hand the dead connection to a client: with an empty pool it is dropped cleanly
    let mut client = connect_retry(&t.downstream).await;
    client.write_all(&reality_client_hello(SERVER_KEY)).await.unwrap();
    expect_closed(&mut client, 2, "client to be dropped, not paired with a dead upstream").await;
}
