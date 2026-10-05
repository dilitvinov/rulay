mod crypto;
mod ping;
mod pool;
mod downstream;
mod upstream;
#[cfg(test)]
mod tests;

#[cfg(test)]
pub use crypto::reality_client_hello;

use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Builder;
use crate::transmitter::downstream::start_listener_for_downstream;
use crate::transmitter::ping::start_pinging;
use crate::transmitter::pool::StreamPool;
use crate::transmitter::upstream::start_upstream_listener;
use crate::stats::{self, tx};

/// How long an authenticated client waits for a free upstream before being dropped, by default.
pub const DEFAULT_UPSTREAM_WAIT: Duration = Duration::from_secs(30);

pub fn start_transmitter(
    upstream_addr: String,
    downstream_addr: String,
    redirect_addr: String,
    server_priv_b64: String,
    upstream_wait: Duration,
) {
    let rt = Builder::new_multi_thread().enable_all().build();

    // we ok with panic here
    rt.unwrap().block_on(run_transmitter(
        upstream_addr,
        downstream_addr,
        redirect_addr,
        server_priv_b64,
        upstream_wait,
    ));
}

/// Runs the transmitter on the current tokio runtime; returns only if the downstream listener fails.
pub async fn run_transmitter(
    upstream_addr: String,
    downstream_addr: String,
    redirect_addr: String,
    server_priv_b64: String,
    upstream_wait: Duration,
) {
    let pool = Arc::new(StreamPool::new());

    // start listener for upstream
    start_upstream_listener(upstream_addr, pool.clone());

    // ping all available streams every 3 sec
    start_pinging(pool.clone());

    start_state_logger(pool.clone());

    // start listener for new clients, connect them to an upstream, blocking
    start_listener_for_downstream(downstream_addr, redirect_addr, server_priv_b64, pool, upstream_wait).await;
}

/// Prints one `[state]` line every `STATE_INTERVAL` so a stalled tunnel can be diagnosed after the fact.
fn start_state_logger(pool: Arc<StreamPool>) {
    let _ = tokio::task::Builder::new().name("state-logger").spawn(async move {
        let mut tick = tokio::time::interval(stats::STATE_INTERVAL);
        loop {
            tick.tick().await;
            println!(
                "[state] transmitter up={}s pool_idle={} pinging={} active_sessions={} | \
                 upstream accepted={} last_accept={} pong_ok={} last_pong={} ping_dropped={} | \
                 downstream accepted={} auth_ok={} auth_failed={} no_upstream={} dead_on_handoff={} sessions_started={} | \
                 accept_err={} fds={} tasks={}",
                stats::uptime().as_secs(),
                pool.len().await,
                tx::PINGING.get(),
                tx::ACTIVE.get(),
                tx::UP_ACCEPTED.get(),
                tx::LAST_UP_ACCEPT.ago(),
                tx::PONG_OK.get(),
                tx::LAST_PONG.ago(),
                tx::PING_DROPPED.get(),
                tx::DOWN_ACCEPTED.get(),
                tx::AUTH_OK.get(),
                tx::AUTH_FAILED.get(),
                tx::NO_UPSTREAM.get(),
                tx::DEAD_ON_HANDOFF.get(),
                tx::SESSIONS_STARTED.get(),
                tx::ACCEPT_ERR.get(),
                stats::open_fds(),
                stats::alive_tasks(),
            );
        }
    });
}
