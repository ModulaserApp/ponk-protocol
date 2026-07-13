//! Encode and decode a mixed-format frame without reducing its point fields.
//!
//! Run with: `cargo run --example roundtrip`

use ponk_protocol::{
    PonkEncodeOptions, PonkPathPoints, PonkPoint, PonkWireFrame, PonkWirePath, XyRgbU16Point,
    decode_wire_datagram, encode_wire_datagrams,
};

fn main() {
    let frame = PonkWireFrame {
        sender_id: 1,
        sender_name: "roundtrip-example".to_string(),
        frame_number: 0,
        paths: vec![
            PonkWirePath {
                metadata: vec![],
                points: PonkPathPoints::XyF32RgbU8(vec![PonkPoint {
                    x: -0.5,
                    y: 0.5,
                    rgb: [255, 0, 0],
                }]),
            },
            PonkWirePath {
                metadata: vec![],
                points: PonkPathPoints::XyRgbU16(vec![XyRgbU16Point {
                    x: 0x1234,
                    y: 0xabcd,
                    rgb: [0x00ff, 0x1234, 0xff00],
                }]),
            },
        ],
    };

    let datagrams =
        encode_wire_datagrams(&frame, &PonkEncodeOptions::default()).expect("frame should encode");
    println!("encoded into {} datagram(s)", datagrams.len());

    let decoded = decode_wire_datagram(&datagrams[0])
        .expect("valid datagram")
        .expect("single-chunk frame");

    assert_eq!(decoded, frame);
    println!(
        "round-tripped {} paths with formats {:?} and {:?}",
        decoded.paths.len(),
        decoded.paths[0].points.data_format(),
        decoded.paths[1].points.data_format(),
    );
}
