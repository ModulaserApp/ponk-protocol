//! Encode a frame to datagrams and decode it back — no sockets involved.
//!
//! Run with: `cargo run --example roundtrip`

use ponk_protocol::{
    DataFormat, PonkFrame, PonkMetadata, PonkPath, PonkPoint, decode_datagram, encode_datagrams,
};

fn main() {
    let frame = PonkFrame {
        sender_id: 1,
        sender_name: "roundtrip-example".to_string(),
        frame_number: 0,
        paths: vec![PonkPath {
            metadata: vec![PonkMetadata {
                key: "PATHNUMB".to_string(),
                value: 0.0,
            }],
            points: vec![
                PonkPoint {
                    x: -0.5,
                    y: -0.5,
                    rgb: [255, 0, 0],
                },
                PonkPoint {
                    x: 0.5,
                    y: 0.5,
                    rgb: [0, 255, 0],
                },
            ],
        }],
    };

    let datagrams =
        encode_datagrams(&frame, DataFormat::XyF32RgbU8, 1200).expect("frame should encode");
    println!("encoded into {} datagram(s)", datagrams.len());

    let decoded = decode_datagram(&datagrams[0])
        .expect("valid datagram")
        .expect("single-chunk frame");

    println!(
        "decoded frame from '{}' with {} path(s), {} point(s)",
        decoded.sender_name,
        decoded.paths.len(),
        decoded.paths[0].points.len(),
    );
    assert_eq!(decoded.paths[0].points, frame.paths[0].points);
    println!("roundtrip ok");
}
