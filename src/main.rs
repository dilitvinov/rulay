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
    upstream_addr: Option<String>,
    #[arg(long)]
    downstream_addr: Option<String>,
}
const PING: &[u8] = &[1, 62, 34, 6];
const PONG: &[u8] = &[6, 34, 62, 1];

const TRANSMITTER_UPSTREAM_ADDR: &str = "0.0.0.0:8444";
const TRANSMITTER_DOWNSTREAM_ADDR: &str = "0.0.0.0:8443";
const RECEIVER_UPSTREAM_ADDR: &str = "94.177.170.43:8443";
const RECEIVER_DOWNSTREAM_ADDR: &str = "0.0.0.0:8444";

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
                args.upstream_addr
                    .unwrap_or_else(|| TRANSMITTER_UPSTREAM_ADDR.to_string()),
                args.downstream_addr
                    .unwrap_or_else(|| TRANSMITTER_DOWNSTREAM_ADDR.to_string()),
            );
        }
        Mode::Receiver => {
            start_receiver(
                args.upstream_addr
                    .unwrap_or_else(|| RECEIVER_UPSTREAM_ADDR.to_string()),
                args.downstream_addr
                    .unwrap_or_else(|| RECEIVER_DOWNSTREAM_ADDR.to_string()),
            );
        }
    }
    println!("Exiting...");
}
