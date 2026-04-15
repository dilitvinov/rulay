mod crypto;
mod ping;
mod downstream;
mod upstream;

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::net::TcpStream;
use tokio::runtime::Builder;
use crate::transmitter::downstream::start_listener_for_downstream;
use crate::transmitter::ping::start_pinging;
use crate::transmitter::upstream::start_upstream_listener;

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
        let upstream_addr_for_listener = upstream_addr.clone();

        // start listener for upstream
        start_upstream_listener(upstream_addr_for_listener, addr_stack_ptr.clone());

        // ping all available streams every 3 sec
        start_pinging(addr_stack_ptr.clone());

        // start listener for new clients, connect them to an upstream, blocking
        start_listener_for_downstream(downstream_addr, redirect_addr, server_priv_b64, addr_stack_ptr).await;
    });
}