//! Transmitter and receiver wired together: client -> transmitter -> receiver -> target and back.

use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use crate::receiver::{run_receiver, CONN_NUM, DEFAULT_IDLE_TIMEOUT};
use crate::test_utils::*;
use crate::transmitter::{reality_client_hello, run_transmitter, DEFAULT_UPSTREAM_WAIT};

const SERVER_KEY: &str = "uM5Zol5nBgyqDrn2RYGhmTeoONiULxeLMhkeDqMtMUE";

#[tokio::test(flavor = "multi_thread")]
async fn client_reaches_target_through_tunnel_and_close_propagates() {
    let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (upstream, downstream) = (free_addr(), free_addr());
    tokio::spawn(run_transmitter(
        upstream.clone(),
        downstream.clone(),
        free_addr(),
        SERVER_KEY.to_string(),
        DEFAULT_UPSTREAM_WAIT,
    ));
    tokio::spawn(run_receiver(target.local_addr().unwrap().to_string(), upstream, DEFAULT_IDLE_TIMEOUT));

    let mut client = connect_retry(&downstream).await;
    let hello = reality_client_hello(SERVER_KEY);
    client.write_all(&hello).await.unwrap();
    client.write_all(b"GET /").await.unwrap();

    let (mut site, _) = within(10, "target to be dialed", target.accept()).await.unwrap();
    let got = read_n(&mut site, hello.len() + 5, "hello and payload at target").await;
    assert_eq!(got, [&hello[..], b"GET /"].concat());

    site.write_all(b"200 OK").await.unwrap();
    assert_eq!(read_n(&mut client, 6, "reply at client").await, b"200 OK");

    drop(client);
    expect_closed(&mut site, 5, "target side to be closed after client left").await;
}

/// The production failure: after a silent outage between receiver and transmitter, clients get
/// "no upstream available" forever, even once the network is back.
#[tokio::test(flavor = "multi_thread")]
async fn tunnel_recovers_after_silent_network_outage() {
    let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (upstream, downstream) = (free_addr(), free_addr());
    tokio::spawn(run_transmitter(
        upstream.clone(),
        downstream.clone(),
        free_addr(),
        SERVER_KEY.to_string(),
        Duration::from_secs(20),
    ));
    let link = LossyLink::start(upstream).await;
    // real transmitter pings every 3s, so the idle timeout must sit comfortably above that
    tokio::spawn(run_receiver(
        target.local_addr().unwrap().to_string(),
        link.addr.clone(),
        Duration::from_secs(5),
    ));
    wait_until(5, "receiver to fill its pool", || link.accepted() == CONN_NUM).await;

    // outage: every pooled connection goes half-open; the transmitter drops them on missed PONGs
    link.lose_existing();
    wait_until(15, "transmitter to drop all pooled connections", || link.target_closed() == CONN_NUM).await;

    // the network is back (new connections pass); a client must get through again
    let mut client = connect_retry(&downstream).await;
    let hello = reality_client_hello(SERVER_KEY);
    client.write_all(&hello).await.unwrap();
    let (mut site, _) = within(25, "target to be dialed after the outage", target.accept()).await.unwrap();
    assert_eq!(read_n(&mut site, hello.len(), "hello at target").await, hello);
}
