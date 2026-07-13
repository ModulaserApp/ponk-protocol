//! Encode a frame and multicast it as PONK datagrams.
//!
//! Run with: `cargo run --example send_multicast [interface-ipv4]`
//!
//! Pass a local interface's own IPv4 address to pin which interface the
//! multicast leaves through; omit it to let the operating system choose.
//!
//! Pair with the `receive_reassemble` example (run the receiver first).

use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};

use ponk_protocol::{
    DEFAULT_PORT, DataFormat, MULTICAST_ADDR, PonkFrame, PonkPath, PonkPoint, encode_datagrams,
};

// Keep each datagram within a typical Ethernet MTU.
const MAX_DATAGRAM_LEN: usize = 1_472;

fn main() -> std::io::Result<()> {
    // Optionally pin the outgoing interface by binding to its local address.
    //
    // With the default unspecified address the operating system picks the
    // egress interface from its routing metrics. On Windows a Wi-Fi interface
    // frequently loses that selection, so multicast never leaves the machine
    // over WLAN. Binding to the interface's own IPv4 address reliably steers
    // egress there on Windows, mirroring the local-address field on
    // TouchDesigner's UDP operators.
    //
    // This is not a general guarantee: binding a local unicast source address
    // is not the same as `IP_MULTICAST_IF`. On Linux (and other platforms) the
    // routing table and `IP_MULTICAST_IF` govern multicast egress, so the bound
    // address is only a hint there. Socket ownership stays with the application,
    // so this policy lives in the example rather than the codec. An application
    // needing the `IP_MULTICAST_IF` socket option directly can reach for
    // `socket2` or the platform APIs.
    let interface = interface_from_args()?;

    let socket = UdpSocket::bind((interface, 0))?;
    socket.set_multicast_ttl_v4(1)?;
    socket.set_multicast_loop_v4(true)?;

    let target = SocketAddr::from((MULTICAST_ADDR, DEFAULT_PORT));

    // A square drawn as one path.
    let frame = PonkFrame {
        sender_id: 0xC0FFEE,
        sender_name: "send-multicast-example".to_string(),
        frame_number: 0,
        paths: vec![PonkPath {
            metadata: vec![],
            points: [
                (-0.5, -0.5),
                (0.5, -0.5),
                (0.5, 0.5),
                (-0.5, 0.5),
                (-0.5, -0.5),
            ]
            .into_iter()
            .map(|(x, y)| PonkPoint {
                x,
                y,
                rgb: [0, 255, 255],
            })
            .collect(),
        }],
    };

    let datagrams = encode_datagrams(&frame, DataFormat::XyF32RgbU8, MAX_DATAGRAM_LEN)
        .expect("frame should encode");

    for datagram in &datagrams {
        socket.send_to(datagram, target)?;
    }
    println!(
        "sent {} datagram(s) to {} via interface {} (multicast group {:?})",
        datagrams.len(),
        target,
        describe_interface(interface),
        MULTICAST_ADDR,
    );
    Ok(())
}

/// Read an optional interface IPv4 address from the first CLI argument.
///
/// Falls back to the unspecified address (all interfaces) when absent. A
/// present-but-unparsable argument is a hard error, since silently reverting
/// to the default would defeat the point of selecting an interface.
fn interface_from_args() -> std::io::Result<Ipv4Addr> {
    match std::env::args().nth(1) {
        None => Ok(Ipv4Addr::UNSPECIFIED),
        Some(arg) => arg.parse().map_err(|_| {
            std::io::Error::new(
                ErrorKind::InvalidInput,
                format!("invalid interface address {arg:?}"),
            )
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
