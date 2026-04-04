mod transmitter;
mod receiver;

use clap::{Parser, ValueEnum};
use crate::receiver::start_receiver;
use crate::transmitter::start_transmitter;

#[derive(Parser, Debug)]
struct Args {
    // #[arg(long)]
    // port: u16,
    #[arg(long, value_enum)]
    mode: Mode,
    // #[arg(long, default_value_t = SocketAddr::from(([127, 0, 0, 1], 8080)))]
    // server: SocketAddr,
}
const PING : &[u8] = &[1, 62, 34, 6];
const PONG : &[u8] = &[6, 34, 62, 1];

#[derive(Copy, Clone, Debug, ValueEnum)]
enum Mode {
    // Receiver establishes connections to Transmitter;
    // exchanges the data between Transmitter and target upstream
    Receiver,
    // Transmitter accepts requests from clients and Receiver and
    // performs bidirectional data copying
    Transmitter
}


fn main() {
    let args = Args::parse();
    match args.mode {
        Mode::Transmitter => {
            start_transmitter();
        },
        Mode::Receiver => {
            start_receiver();
        }
    }
    println!("Exiting...");
}