//! Encode a frame and multicast it as PONK datagrams.
//!
//! Run with: `cargo run --example send_multicast`
//!
//! Pair with the `receive_reassemble` example (run the receiver first).

use std::net::{Ipv4Addr, SocketAddr, UdpSocket};

use ponk_protocol::{
    DEFAULT_PORT, DataFormat, MULTICAST_ADDR, PonkFrame, PonkPath, PonkPoint, encode_datagrams,
};

// Keep each datagram within a typical Ethernet MTU.
const MAX_DATAGRAM_LEN: usize = 1_472;

fn main() -> std::io::Result<()> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
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
        "sent {} datagram(s) to {} (multicast group {:?})",
        datagrams.len(),
        target,
        MULTICAST_ADDR,
    );
    Ok(())
}
