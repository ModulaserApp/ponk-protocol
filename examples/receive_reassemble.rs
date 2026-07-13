//! Join the PONK multicast group and reassemble incoming mixed-format frames.
//!
//! Run with: `cargo run --example receive_reassemble`
//!
//! Then, in another terminal, run the `send_multicast` example.

use std::net::{Ipv4Addr, SocketAddr, UdpSocket};

use ponk_protocol::{DEFAULT_PORT, MULTICAST_ADDR, PonkAssembler};

const RECV_BUFFER_SIZE: usize = 65_536;

fn main() -> std::io::Result<()> {
    let group = Ipv4Addr::from(MULTICAST_ADDR);
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, DEFAULT_PORT))?;
    socket.join_multicast_v4(&group, &Ipv4Addr::UNSPECIFIED)?;
    println!("listening for PONK on {group}:{DEFAULT_PORT} — Ctrl-C to stop");

    // Strict mode is the default. Configure sender-specific canonical boundary
    // repair only when the application knows that a sender needs it.
    let mut assembler = PonkAssembler::with_max_assemblies(64);
    let mut buffer = [0u8; RECV_BUFFER_SIZE];

    loop {
        let (len, peer): (usize, SocketAddr) = socket.recv_from(&mut buffer)?;
        match assembler.push_wire_datagram(&buffer[..len], peer) {
            Ok(Some(completed)) => {
                let points: usize = completed
                    .frame
                    .paths
                    .iter()
                    .map(|path| path.points.len())
                    .sum();
                println!(
                    "frame from {:?} (id {}): {} path(s), {} point(s), {:?}",
                    completed.frame.sender_name,
                    completed.frame.sender_id,
                    completed.frame.paths.len(),
                    points,
                    completed.completion,
                );
            }
            Ok(None) => {}
            Err(error) => eprintln!("malformed datagram from {peer}: {error}"),
        }
    }
}
