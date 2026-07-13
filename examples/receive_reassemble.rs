//! Join the PONK multicast group and reassemble incoming frames.
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

    // Cap in-progress assemblies so untrusted senders cannot exhaust memory.
    let mut assembler = PonkAssembler::with_max_assemblies(64);
    let mut buffer = [0u8; RECV_BUFFER_SIZE];

    loop {
        let (len, peer): (usize, SocketAddr) = socket.recv_from(&mut buffer)?;
        match assembler.push_datagram(&buffer[..len], peer) {
            Ok(Some(frame)) => {
                let points: usize = frame.paths.iter().map(|p| p.points.len()).sum();
                println!(
                    "frame from {:?} (id {}): {} path(s), {} point(s)",
                    frame.sender_name,
                    frame.sender_id,
                    frame.paths.len(),
                    points,
                );
            }
            Ok(None) => {}
            Err(error) => eprintln!("malformed datagram from {peer}: {error}"),
        }
    }
}
