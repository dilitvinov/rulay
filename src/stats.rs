//! Process-wide counters behind the periodic `[state]` log line.
//! Everything is a lock-free atomic so instrumenting a hot path costs next to nothing.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering::Relaxed};
use std::time::{Duration, Instant};

/// How often each mode prints its `[state]` line.
pub const STATE_INTERVAL: Duration = Duration::from_secs(30);

static START: OnceLock<Instant> = OnceLock::new();

/// Pins the process start time; call once at the top of `main`.
pub fn init() {
    START.get_or_init(Instant::now);
}

pub fn uptime() -> Duration {
    START.get_or_init(Instant::now).elapsed()
}

/// Monotonic event counter.
pub struct Counter(AtomicU64);

impl Counter {
    pub const fn new() -> Self {
        Counter(AtomicU64::new(0))
    }
    pub fn inc(&self) {
        self.0.fetch_add(1, Relaxed);
    }
    pub fn get(&self) -> u64 {
        self.0.load(Relaxed)
    }
}

/// Number of things currently in some state; goes up and down.
pub struct Gauge(AtomicI64);

impl Gauge {
    pub const fn new() -> Self {
        Gauge(AtomicI64::new(0))
    }
    /// Counts one more until the returned guard is dropped.
    pub fn enter(&'static self) -> GaugeGuard {
        self.0.fetch_add(1, Relaxed);
        GaugeGuard(self)
    }
    pub fn get(&self) -> i64 {
        self.0.load(Relaxed)
    }
}

pub struct GaugeGuard(&'static Gauge);

impl Drop for GaugeGuard {
    fn drop(&mut self) {
        self.0.0.fetch_sub(1, Relaxed);
    }
}

/// "When did this last happen?"; 0 means never.
pub struct Stamp(AtomicU64);

impl Stamp {
    pub const fn new() -> Self {
        Stamp(AtomicU64::new(0))
    }
    pub fn touch(&self) {
        self.0.store(uptime().as_millis() as u64 + 1, Relaxed);
    }
    /// e.g. `12s ago`, or `never`.
    pub fn ago(&self) -> String {
        match self.0.load(Relaxed) {
            0 => "never".to_string(),
            at => format!("{}s ago", (uptime().as_millis() as u64 + 1).saturating_sub(at) / 1000),
        }
    }
}

/// Open file descriptors vs the soft limit, e.g. `213/1048576` (`n/a` off Linux).
pub fn open_fds() -> String {
    let Ok(dir) = std::fs::read_dir("/proc/self/fd") else {
        return "n/a".to_string();
    };
    let used = dir.count();
    let limit = std::fs::read_to_string("/proc/self/limits")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("Max open files"))
                .and_then(|l| l.split_whitespace().nth(3).map(str::to_string))
        })
        .unwrap_or_else(|| "?".to_string());
    format!("{}/{}", used, limit)
}

/// Tasks still alive on the current tokio runtime; a number that only grows means a leak.
pub fn alive_tasks() -> usize {
    tokio::runtime::Handle::current().metrics().num_alive_tasks()
}

pub mod tx {
    use super::*;

    pub static UP_ACCEPTED: Counter = Counter::new();
    pub static LAST_UP_ACCEPT: Stamp = Stamp::new();
    pub static PONG_OK: Counter = Counter::new();
    pub static LAST_PONG: Stamp = Stamp::new();
    pub static PING_DROPPED: Counter = Counter::new();
    pub static PINGING: Gauge = Gauge::new();

    pub static DOWN_ACCEPTED: Counter = Counter::new();
    pub static AUTH_OK: Counter = Counter::new();
    pub static AUTH_FAILED: Counter = Counter::new();
    pub static NO_UPSTREAM: Counter = Counter::new();
    pub static DEAD_ON_HANDOFF: Counter = Counter::new();
    pub static SESSIONS_STARTED: Counter = Counter::new();
    pub static ACTIVE: Gauge = Gauge::new();

    pub static ACCEPT_ERR: Counter = Counter::new();
}

pub mod rx {
    use super::*;

    pub static CONNECTING: Gauge = Gauge::new();
    pub static CONNECT_OK: Counter = Counter::new();
    pub static CONNECT_ERR: Counter = Counter::new();
    /// Dials to the transmitter abandoned after `CONNECT_TIMEOUT` (also counted in `CONNECT_ERR`).
    pub static CONNECT_TIMED_OUT: Counter = Counter::new();
    /// Connections to the transmitter that are parked waiting for PING / data.
    pub static IDLE: Gauge = Gauge::new();
    pub static PINGS: Counter = Counter::new();
    pub static LAST_PING: Stamp = Stamp::new();
    /// Idle connection ended without any payload (transmitter closed it).
    pub static IDLE_CLOSED: Counter = Counter::new();
    /// Idle connection closed by us after no PING for the idle timeout (half-open).
    pub static IDLE_TIMED_OUT: Counter = Counter::new();
    pub static SESSIONS_STARTED: Counter = Counter::new();
    pub static ACTIVE: Gauge = Gauge::new();
    pub static UPSTREAM_ERR: Counter = Counter::new();
}
