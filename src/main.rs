mod crypto;
mod receiver;
mod transmitter;

use crate::receiver::start_receiver;
use crate::transmitter::start_transmitter;
use clap::{Parser, ValueEnum};

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, value_enum)]
    mode: Mode,
    #[arg(long)]
    upstream_server: Option<String>,
    #[arg(long)]
    upstream_port: Option<u16>,
    #[arg(long)]
    downstream_server: Option<String>,
    #[arg(long)]
    downstream_port: Option<u16>,
    /// Base64url (no-pad) encoded 32-byte X25519 server private key for REALITY auth
    #[arg(long)]
    server_priv: Option<String>,
    #[arg(long)]
    redirect_server: Option<String>,
}
const PING: &[u8] = &[1, 62, 34, 6];
const PONG: &[u8] = &[6, 34, 62, 1];

const TRANSMITTER_UPSTREAM_SERVER: &str = "0.0.0.0";
const TRANSMITTER_UPSTREAM_PORT: u16 = 8444;
const TRANSMITTER_DOWNSTREAM_SERVER: &str = "0.0.0.0";
const TRANSMITTER_DOWNSTREAM_PORT: u16 = 443;
const TRANSMITTER_PRIV: &str = "uM5Zol5nBgyqDrn2RYGhmTeoONiULxeLMhkeDqMtMUE";
const RECEIVER_UPSTREAM_SERVER: &str = "0.0.0.0";
const RECEIVER_UPSTREAM_PORT: u16 = 8443;
const RECEIVER_DOWNSTREAM_SERVER: &str = "0.0.0.0";
const RECEIVER_DOWNSTREAM_PORT: u16 = 8444;

#[derive(Copy, Clone, Debug, ValueEnum)]
enum Mode {
    // Receiver establishes connections to Transmitter;
    // exchanges the data between Transmitter and target upstream
    Receiver,
    // Transmitter accepts requests from clients and Receiver and
    // performs bidirectional data copying
    Transmitter,
}

fn main() {
    let args = Args::parse();
    match args.mode {
        Mode::Transmitter => {
            start_transmitter(
                format!(
                    "{}:{}",
                    args.upstream_server
                        .unwrap_or_else(|| TRANSMITTER_UPSTREAM_SERVER.to_string()),
                    args.upstream_port.unwrap_or(TRANSMITTER_UPSTREAM_PORT)
                ),
                format!(
                    "{}:{}",
                    args.downstream_server
                        .unwrap_or_else(|| TRANSMITTER_DOWNSTREAM_SERVER.to_string()),
                    args.downstream_port.unwrap_or(TRANSMITTER_DOWNSTREAM_PORT)
                ),
                args.redirect_server.unwrap_or_default(),
                args.server_priv.unwrap_or_else(|| TRANSMITTER_PRIV.to_string()),
            );
        }
        Mode::Receiver => {
            start_receiver(
                format!(
                    "{}:{}",
                    args.upstream_server
                        .unwrap_or_else(|| RECEIVER_UPSTREAM_SERVER.to_string()),
                    args.upstream_port.unwrap_or(RECEIVER_UPSTREAM_PORT)
                ),
                format!(
                    "{}:{}",
                    args.downstream_server
                        .unwrap_or_else(|| RECEIVER_DOWNSTREAM_SERVER.to_string()),
                    args.downstream_port.unwrap_or(RECEIVER_DOWNSTREAM_PORT)
                ),
            );
        }
    }
    println!("Exiting...");
}
