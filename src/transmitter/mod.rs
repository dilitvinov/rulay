mod crypto;
mod ping;
mod pool;
mod downstream;
mod upstream;

use std::sync::Arc;
use tokio::runtime::Builder;
use crate::transmitter::downstream::start_listener_for_downstream;
use crate::transmitter::ping::start_pinging;
use crate::transmitter::pool::StreamPool;
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
        let pool = Arc::new(StreamPool::new());
        let upstream_addr_for_listener = upstream_addr.clone();

        // start listener for upstream
        start_upstream_listener(upstream_addr_for_listener, pool.clone());

        // ping all available streams every 3 sec
        start_pinging(pool.clone());

        // start listener for new clients, connect them to an upstream, blocking
        start_listener_for_downstream(downstream_addr, redirect_addr, server_priv_b64, pool).await;
    });
}
