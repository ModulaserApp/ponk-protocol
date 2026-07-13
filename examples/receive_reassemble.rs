//! Join the PONK multicast group and reassemble incoming mixed-format frames.
//!
//! Run with: `cargo run --example receive_reassemble [interface-ipv4]`
//!
//! Pass a local interface's own IPv4 address to join the group on that
//! interface; omit it to let the operating system choose.
//!
//! Then, in another terminal, run the `send_multicast` example.

use std::net::{Ipv4Addr, SocketAddr, UdpSocket};

use ponk_protocol::{DEFAULT_PORT, MULTICAST_ADDR, PonkAssembler};

const RECV_BUFFER_SIZE: usize = 65_536;

fn main() -> std::io::Result<()> {
    // Optionally join on a specific interface.
    //
    // With the default unspecified address the operating system picks the
    // interface from its routing metrics. On Windows a Wi-Fi interface often
    // loses that selection, so multicast is never received over WLAN. Passing
    // the interface's own IPv4 address joins the group on that interface,
    // mirroring the local-address field on TouchDesigner's UDP operators.
    let interface = interface_from_args();

    let group = Ipv4Addr::from(MULTICAST_ADDR);
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, DEFAULT_PORT))?;
    socket.join_multicast_v4(&group, &interface)?;
    println!(
        "listening for PONK on {group}:{DEFAULT_PORT} via interface {} — Ctrl-C to stop",
        describe_interface(interface),
    );

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

/// Read an optional interface IPv4 address from the first CLI argument.
///
/// Falls back to the unspecified address (all interfaces) when absent, and
/// warns but keeps the fallback when the argument does not parse.
fn interface_from_args() -> Ipv4Addr {
    match std::env::args().nth(1) {
        None => Ipv4Addr::UNSPECIFIED,
        Some(arg) => arg.parse().unwrap_or_else(|_| {
            eprintln!("invalid interface address {arg:?}; using all interfaces");
            Ipv4Addr::UNSPECIFIED
        }),
    }
}

fn describe_interface(interface: Ipv4Addr) -> String {
    if interface.is_unspecified() {
        "default".to_string()
    } else {
        interface.to_string()
    }
}
