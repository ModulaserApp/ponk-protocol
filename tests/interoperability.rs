use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use ponk_protocol::*;

// Independent literal fixtures for madmappersoftware/Ponk@2c166392cb505bfd48440a2a51bfcfb15f3ccfec.
const CANONICAL_F32_DATAGRAM: &[u8] = &[
    0x50, 0x4f, 0x4e, 0x4b, 0x2d, 0x55, 0x44, 0x50, 0x00, 0x04, 0x03, 0x02, 0x01, 0x53, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05, 0x01, 0x00,
    0x3a, 0x05, 0x00, 0x00, 0x01, 0x01, 0x50, 0x41, 0x54, 0x48, 0x4e, 0x55, 0x4d, 0x42, 0x00, 0x00,
    0x80, 0x3f, 0x01, 0x00, 0x00, 0x00, 0x00, 0x3f, 0x00, 0x00, 0x80, 0xbe, 0x12, 0x34, 0x56,
];

const CANONICAL_U16_NON_REPEATED: &[u8] = &[
    0x50, 0x4f, 0x4e, 0x4b, 0x2d, 0x55, 0x44, 0x50, 0x00, 0x04, 0x03, 0x02, 0x01, 0x53, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05, 0x01, 0x00,
    0x61, 0x05, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0xff, 0x7f, 0xcd, 0xab, 0x34, 0x12, 0x78, 0x56,
    0xbc, 0x9a,
];

const CANONICAL_MIXED: &[u8] = &[
    0x50, 0x4f, 0x4e, 0x4b, 0x2d, 0x55, 0x44, 0x50, 0x00, 0x04, 0x03, 0x02, 0x01, 0x53, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05, 0x01, 0x00,
    0x30, 0x05, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x3f,
    0x01, 0x02, 0x03, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0xff, 0xff, 0x34, 0x12, 0x78, 0x56, 0xbc,
    0x9a,
];

fn peer(port: u16) -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, port))
}

fn sample_wire_frame(frame_number: u8, points: usize) -> PonkWireFrame {
    PonkWireFrame {
        sender_id: 42,
        sender_name: "test sender".into(),
        frame_number,
        paths: vec![PonkWirePath {
            metadata: vec![PonkMetadata {
                key: "PATHNUMB".into(),
                value: 3.0,
            }],
            points: PonkPathPoints::XyF32RgbU8(
                (0..points)
                    .map(|i| PonkPoint {
                        x: -1.0 + (i % 50) as f32 * 0.04,
                        y: 0.25,
                        rgb: [i as u8, 255, 128],
                    })
                    .collect(),
            ),
        }],
    }
}

fn options(max_datagram_len: usize) -> PonkEncodeOptions {
    PonkEncodeOptions {
        max_datagram_len,
        ..PonkEncodeOptions::default()
    }
}

fn canonical_boundary_frame(paths: usize) -> PonkWireFrame {
    let path = PonkWirePath {
        metadata: vec![
            PonkMetadata {
                key: "M".into(),
                value: 0.0,
            };
            8
        ],
        points: PonkPathPoints::XyF32RgbU8(vec![
            PonkPoint {
                x: 0.0,
                y: 0.0,
                rgb: [0, 0, 0],
            };
            120
        ]),
    };
    PonkWireFrame {
        sender_id: 99,
        sender_name: "canonical-bug".into(),
        frame_number: 7,
        paths: vec![path; paths],
    }
}

fn advertise_extra_trailing_chunk(datagrams: &mut [Vec<u8>]) {
    let advertised = datagrams.len() as u8 + 1;
    for datagram in datagrams {
        datagram[46] = advertised;
    }
}

#[test]
fn canonical_f32_fixture_is_exact() {
    let frame = decode_wire_datagram(CANONICAL_F32_DATAGRAM)
        .unwrap()
        .unwrap();
    assert_eq!(frame.paths[0].points.data_format(), DataFormat::XyF32RgbU8);
    assert_eq!(
        encode_wire_datagrams(&frame, &options(1_472)).unwrap(),
        [CANONICAL_F32_DATAGRAM]
    );
}

#[test]
fn canonical_u16_preserves_every_wire_bit() {
    let frame = decode_wire_datagram(CANONICAL_U16_NON_REPEATED)
        .unwrap()
        .unwrap();
    let PonkPathPoints::XyRgbU16(points) = &frame.paths[0].points else {
        panic!("wrong format")
    };
    assert_eq!(
        points[0],
        XyRgbU16Point {
            x: 0x7fff,
            y: 0xabcd,
            rgb: [0x1234, 0x5678, 0x9abc],
        }
    );
    assert_eq!(
        encode_wire_datagrams(&frame, &options(1_472)).unwrap(),
        [CANONICAL_U16_NON_REPEATED]
    );

    let legacy = frame.into_legacy(U16ColorReduction::MostSignificantByte);
    assert_eq!(legacy.paths[0].points[0].rgb, [0x12, 0x56, 0x9a]);
}

#[test]
fn mixed_format_fixture_roundtrips_exactly() {
    let frame = decode_wire_datagram(CANONICAL_MIXED).unwrap().unwrap();
    assert_eq!(frame.paths.len(), 2);
    assert_eq!(frame.paths[0].points.data_format(), DataFormat::XyF32RgbU8);
    assert_eq!(frame.paths[1].points.data_format(), DataFormat::XyRgbU16);
    assert_eq!(
        encode_wire_datagrams(&frame, &options(1_472)).unwrap(),
        [CANONICAL_MIXED]
    );
}

#[test]
fn coordinate_encoding_uses_canonical_truncation() {
    assert_eq!(normalized_coord_to_u16(0.0).unwrap(), 0x7fff);
    assert_eq!(normalized_coord_to_u16(-1.0).unwrap(), 0);
    assert_eq!(normalized_coord_to_u16(1.0).unwrap(), u16::MAX);
    assert_eq!(
        normalized_coord_to_u16(f32::NAN),
        Err(PonkError::InvalidCoordinate)
    );
}

#[test]
fn explicit_color_helpers_do_not_hide_reduction() {
    assert_eq!(expand_rgb8([0x12, 0x80, 0xff]), [0x1212, 0x8080, 0xffff]);
    assert_eq!(
        reduce_rgb16(
            [0x1234, 0x00ff, 0xff00],
            U16ColorReduction::MostSignificantByte
        ),
        [0x12, 0x00, 0xff]
    );
    assert_eq!(
        reduce_rgb16([0x1234, 0x00ff, 0xff00], U16ColorReduction::Rounded),
        [0x12, 0x01, 0xfe]
    );
}

#[test]
fn zero_point_paths_retain_individual_formats() {
    let frame = PonkWireFrame {
        sender_id: 1,
        sender_name: "empty paths".into(),
        frame_number: 0,
        paths: vec![
            PonkWirePath {
                metadata: vec![],
                points: PonkPathPoints::XyRgbU16(vec![]),
            },
            PonkWirePath {
                metadata: vec![],
                points: PonkPathPoints::XyF32RgbU8(vec![]),
            },
        ],
    };
    let datagram = encode_wire_datagrams(&frame, &options(1_472))
        .unwrap()
        .remove(0);
    let decoded = decode_wire_datagram(&datagram).unwrap().unwrap();
    assert_eq!(decoded, frame);
}

#[test]
fn header_only_and_explicit_empty_frame_are_selectable() {
    let frame = PonkWireFrame {
        sender_id: 1,
        sender_name: "empty".into(),
        frame_number: 0,
        paths: vec![],
    };
    let mut header_only = options(1_472);
    header_only.empty_frame = EmptyFrameEncoding::HeaderOnly;
    let datagram = encode_wire_datagrams(&frame, &header_only)
        .unwrap()
        .remove(0);
    assert_eq!(datagram.len(), HEADER_LEN);
    assert!(
        decode_wire_datagram(&datagram)
            .unwrap()
            .unwrap()
            .paths
            .is_empty()
    );

    let explicit = encode_wire_datagrams(&frame, &options(1_472))
        .unwrap()
        .remove(0);
    assert_eq!(&explicit[HEADER_LEN..], &[1, 0, 0, 0]);
}

#[test]
fn mixed_multichunk_roundtrip_in_reverse_order() {
    let mut frame = decode_wire_datagram(CANONICAL_MIXED).unwrap().unwrap();
    let PonkPathPoints::XyF32RgbU8(points) = &mut frame.paths[0].points else {
        unreachable!()
    };
    points.extend(vec![points[0].clone(); 50]);
    let datagrams = encode_wire_datagrams(&frame, &options(HEADER_LEN + 40)).unwrap();
    assert!(datagrams.len() > 1);

    let mut assembler = PonkAssembler::new();
    let mut output = None;
    for datagram in datagrams.iter().rev() {
        output = assembler
            .push_wire_datagram(datagram, peer(5583))
            .unwrap()
            .or(output);
    }
    assert_eq!(output.unwrap().frame, frame);
}

#[test]
fn encoder_policy_can_be_relaxed_but_wire_widths_cannot() {
    let many_paths = PonkWireFrame {
        sender_id: 1,
        sender_name: "many".into(),
        frame_number: 0,
        paths: (0..=DEFAULT_MAX_PATHS)
            .map(|_| PonkWirePath {
                metadata: vec![],
                points: PonkPathPoints::XyF32RgbU8(vec![]),
            })
            .collect(),
    };
    assert_eq!(
        encode_wire_datagrams(&many_paths, &options(MAX_DATAGRAM_LEN)),
        Err(PonkError::TooManyPaths {
            max: DEFAULT_MAX_PATHS
        })
    );
    let relaxed = PonkEncodeOptions {
        max_datagram_len: MAX_DATAGRAM_LEN,
        limits: PonkEncoderLimits::protocol_only(),
        ..PonkEncodeOptions::default()
    };
    assert!(encode_wire_datagrams(&many_paths, &relaxed).is_ok());

    let too_many_metadata = PonkWireFrame {
        paths: vec![PonkWirePath {
            metadata: vec![
                PonkMetadata {
                    key: "M".into(),
                    value: 0.0,
                };
                256
            ],
            points: PonkPathPoints::XyF32RgbU8(vec![]),
        }],
        ..many_paths
    };
    assert_eq!(
        encode_wire_datagrams(&too_many_metadata, &relaxed),
        Err(PonkError::TooManyMetadata(256))
    );
}

#[test]
fn aggregate_points_above_u16_are_valid_when_policy_allows() {
    let points = vec![
        XyRgbU16Point {
            x: 0,
            y: 0,
            rgb: [0, 0, 0],
        };
        32_768
    ];
    let frame = PonkWireFrame {
        sender_id: 1,
        sender_name: "aggregate".into(),
        frame_number: 0,
        paths: vec![
            PonkWirePath {
                metadata: vec![],
                points: PonkPathPoints::XyRgbU16(points.clone()),
            },
            PonkWirePath {
                metadata: vec![],
                points: PonkPathPoints::XyRgbU16(points),
            },
        ],
    };
    let relaxed = PonkEncodeOptions {
        max_datagram_len: MAX_DATAGRAM_LEN,
        limits: PonkEncoderLimits::protocol_only(),
        ..PonkEncodeOptions::default()
    };
    let datagrams = encode_wire_datagrams(&frame, &relaxed).unwrap();
    assert_eq!(datagrams.len(), 11);
}

#[test]
fn hard_datagram_and_chunk_limits_remain() {
    assert_eq!(
        encode_wire_datagrams(&sample_wire_frame(0, 1), &options(HEADER_LEN)),
        Err(PonkError::MaxDatagramTooSmall {
            min: HEADER_LEN + 1,
            actual: HEADER_LEN,
        })
    );
    assert_eq!(
        encode_wire_datagrams(&sample_wire_frame(0, 1), &options(MAX_DATAGRAM_LEN + 1)),
        Err(PonkError::DatagramTooLarge {
            max: MAX_DATAGRAM_LEN,
            actual: MAX_DATAGRAM_LEN + 1,
        })
    );

    let one_byte_chunks = PonkEncodeOptions {
        max_datagram_len: HEADER_LEN + 1,
        limits: PonkEncoderLimits::protocol_only(),
        ..PonkEncodeOptions::default()
    };
    let frame = sample_wire_frame(0, 22);
    assert!(matches!(
        encode_wire_datagrams(&frame, &one_byte_chunks),
        Err(PonkError::TooManyChunks(count)) if count > 255
    ));
}

#[test]
fn same_sender_frames_complete_when_interleaved_and_across_wrap() {
    for (a_number, b_number) in [(10, 11), (255, 0)] {
        let a = sample_wire_frame(a_number, 20);
        let b = sample_wire_frame(b_number, 21);
        let a_chunks = encode_wire_datagrams(&a, &options(HEADER_LEN + 50)).unwrap();
        let b_chunks = encode_wire_datagrams(&b, &options(HEADER_LEN + 50)).unwrap();
        let mut assembler = PonkAssembler::new();
        let mut outputs = Vec::new();
        for datagram in a_chunks.iter().take(1).chain(b_chunks.iter().take(1)) {
            assert!(
                assembler
                    .push_wire_datagram(datagram, peer(5583))
                    .unwrap()
                    .is_none()
            );
        }
        for datagram in a_chunks.iter().skip(1).chain(b_chunks.iter().skip(1)) {
            if let Some(output) = assembler.push_wire_datagram(datagram, peer(5583)).unwrap() {
                outputs.push(output.frame.frame_number);
            }
        }
        outputs.sort_unstable();
        let mut expected = vec![a_number, b_number];
        expected.sort_unstable();
        assert_eq!(outputs, expected);
        assert_eq!(assembler.assembly_count(), 0);
        assert_eq!(assembler.buffered_bytes(), 0);
    }
}

#[test]
fn same_wrapped_number_with_different_checksum_coexists() {
    let a = sample_wire_frame(0, 20);
    let mut b = sample_wire_frame(0, 20);
    let PonkPathPoints::XyF32RgbU8(points) = &mut b.paths[0].points else {
        unreachable!()
    };
    points[0].rgb = [1, 2, 3];
    let a_chunks = encode_wire_datagrams(&a, &options(HEADER_LEN + 50)).unwrap();
    let b_chunks = encode_wire_datagrams(&b, &options(HEADER_LEN + 50)).unwrap();
    let mut assembler = PonkAssembler::new();
    assembler
        .push_wire_datagram(&a_chunks[0], peer(5583))
        .unwrap();
    assembler
        .push_wire_datagram(&b_chunks[0], peer(5583))
        .unwrap();
    assert_eq!(assembler.assembly_count(), 2);

    let mut completed = 0;
    for datagram in a_chunks.iter().skip(1).chain(b_chunks.iter().skip(1)) {
        completed += usize::from(
            assembler
                .push_wire_datagram(datagram, peer(5583))
                .unwrap()
                .is_some(),
        );
    }
    assert_eq!(completed, 2);
}

#[test]
fn duplicate_chunks_are_idempotent_and_conflicts_drop_only_one_identity() {
    let a = sample_wire_frame(1, 20);
    let b = sample_wire_frame(2, 20);
    let a_chunks = encode_wire_datagrams(&a, &options(HEADER_LEN + 50)).unwrap();
    let b_chunks = encode_wire_datagrams(&b, &options(HEADER_LEN + 50)).unwrap();
    let mut assembler = PonkAssembler::new();
    assembler
        .push_wire_datagram(&a_chunks[0], peer(5583))
        .unwrap();
    let bytes = assembler.buffered_bytes();
    assembler
        .push_wire_datagram(&a_chunks[0], peer(5583))
        .unwrap();
    assert_eq!(assembler.buffered_bytes(), bytes);

    assembler
        .push_wire_datagram(&b_chunks[0], peer(5583))
        .unwrap();
    let mut conflict = a_chunks[0].clone();
    conflict[HEADER_LEN] ^= 1;
    assert_eq!(
        assembler.push_wire_datagram(&conflict, peer(5583)),
        Err(PonkError::ConflictingChunk)
    );
    assert_eq!(assembler.assembly_count(), 1);

    let mut completed = None;
    for datagram in b_chunks.iter().skip(1) {
        completed = assembler
            .push_wire_datagram(datagram, peer(5583))
            .unwrap()
            .or(completed);
    }
    assert_eq!(completed.unwrap().frame.frame_number, 2);
}

#[test]
fn per_sender_and_global_limits_evict_deterministically() {
    let config = PonkAssemblerConfig {
        reassembly: PonkReassemblyLimits {
            max_assemblies: 2,
            max_assemblies_per_sender: 1,
            max_buffered_bytes: 1_024,
            assembly_timeout: Duration::from_secs(1),
        },
        ..PonkAssemblerConfig::default()
    };
    let first =
        encode_wire_datagrams(&sample_wire_frame(1, 20), &options(HEADER_LEN + 50)).unwrap();
    let second =
        encode_wire_datagrams(&sample_wire_frame(2, 20), &options(HEADER_LEN + 50)).unwrap();
    let other_peer =
        encode_wire_datagrams(&sample_wire_frame(3, 20), &options(HEADER_LEN + 50)).unwrap();
    let mut assembler = PonkAssembler::with_config(config);
    assembler.push_wire_datagram(&first[0], peer(5583)).unwrap();
    assembler
        .push_wire_datagram(&second[0], peer(5583))
        .unwrap();
    assert_eq!(assembler.assembly_count(), 1);
    assembler
        .push_wire_datagram(&other_peer[0], peer(5584))
        .unwrap();
    assert_eq!(assembler.assembly_count(), 2);
    assert!(assembler.buffered_bytes() <= config.reassembly.max_buffered_bytes);
}

#[test]
fn oversized_incoming_assembly_retains_nothing() {
    let chunks =
        encode_wire_datagrams(&sample_wire_frame(1, 20), &options(HEADER_LEN + 50)).unwrap();
    let config = PonkAssemblerConfig {
        reassembly: PonkReassemblyLimits {
            max_buffered_bytes: 10,
            ..PonkReassemblyLimits::default()
        },
        ..PonkAssemblerConfig::default()
    };
    let mut assembler = PonkAssembler::with_config(config);
    assert_eq!(
        assembler.push_wire_datagram(&chunks[0], peer(5583)),
        Err(PonkError::BufferedBytesLimit { max: 10 })
    );
    assert_eq!(assembler.assembly_count(), 0);
    assert_eq!(assembler.buffered_bytes(), 0);
}

#[test]
fn rejected_new_identity_does_not_evict_an_existing_assembly() {
    let existing =
        encode_wire_datagrams(&sample_wire_frame(1, 1), &options(HEADER_LEN + 8)).unwrap();
    let existing_payload_len: usize = existing
        .iter()
        .map(|datagram| datagram.len() - HEADER_LEN)
        .sum();
    let oversized = encode_wire_datagrams(
        &sample_wire_frame(2, 20),
        &options(HEADER_LEN + existing_payload_len + 1),
    )
    .unwrap();
    assert!(oversized[0].len() - HEADER_LEN > existing_payload_len);

    let config = PonkAssemblerConfig {
        reassembly: PonkReassemblyLimits {
            max_assemblies: 1,
            max_assemblies_per_sender: 1,
            max_buffered_bytes: existing_payload_len,
            ..PonkReassemblyLimits::default()
        },
        ..PonkAssemblerConfig::default()
    };
    let mut assembler = PonkAssembler::with_config(config);
    assembler
        .push_wire_datagram(&existing[0], peer(5583))
        .unwrap();
    let retained_bytes = assembler.buffered_bytes();

    assert_eq!(
        assembler.push_wire_datagram(&oversized[0], peer(5583)),
        Err(PonkError::BufferedBytesLimit {
            max: existing_payload_len,
        })
    );
    assert_eq!(assembler.assembly_count(), 1);
    assert_eq!(assembler.buffered_bytes(), retained_bytes);

    let mut completed = None;
    for datagram in existing.iter().skip(1) {
        completed = assembler
            .push_wire_datagram(datagram, peer(5583))
            .unwrap()
            .or(completed);
    }
    assert_eq!(completed.unwrap().frame.frame_number, 1);
}

#[test]
fn compatibility_max_assemblies_does_not_add_a_hidden_sender_cap() {
    let mut assembler = PonkAssembler::with_max_assemblies(9);
    for frame_number in 0..9 {
        let chunks = encode_wire_datagrams(
            &sample_wire_frame(frame_number, 20),
            &options(HEADER_LEN + 8),
        )
        .unwrap();
        assembler
            .push_wire_datagram(&chunks[0], peer(5583))
            .unwrap();
    }
    assert_eq!(assembler.assembly_count(), 9);
}

#[test]
fn zero_aggregate_byte_limit_never_retains_even_empty_chunks() {
    let mut datagram = CANONICAL_F32_DATAGRAM[..HEADER_LEN].to_vec();
    datagram[46] = 2;
    datagram[47] = 0;
    datagram[48..52].fill(0);
    let config = PonkAssemblerConfig {
        reassembly: PonkReassemblyLimits {
            max_buffered_bytes: 0,
            ..PonkReassemblyLimits::default()
        },
        ..PonkAssemblerConfig::default()
    };
    let mut assembler = PonkAssembler::with_config(config);
    assert!(
        assembler
            .push_wire_datagram(&datagram, peer(5583))
            .unwrap()
            .is_none()
    );
    assert_eq!(assembler.assembly_count(), 0);
    assert_eq!(assembler.buffered_bytes(), 0);
}

#[test]
fn all_stale_overlapping_identities_are_pruned() {
    let config = PonkAssemblerConfig {
        reassembly: PonkReassemblyLimits {
            assembly_timeout: Duration::ZERO,
            ..PonkReassemblyLimits::default()
        },
        ..PonkAssemblerConfig::default()
    };
    let mut assembler = PonkAssembler::with_config(config);
    for number in 1..=3 {
        let chunks =
            encode_wire_datagrams(&sample_wire_frame(number, 20), &options(HEADER_LEN + 50))
                .unwrap();
        assembler
            .push_wire_datagram(&chunks[0], peer(5583))
            .unwrap();
    }
    assembler.prune_stale();
    assert_eq!(assembler.assembly_count(), 0);
    assert_eq!(assembler.buffered_bytes(), 0);
}

#[test]
fn canonical_exact_boundary_repair_is_opt_in_and_visible() {
    let frame = canonical_boundary_frame(1);
    let mut datagrams = encode_wire_datagrams(&frame, &options(1_472)).unwrap();
    assert_eq!(datagrams.len(), 1);
    assert_eq!(datagrams[0].len(), 1_472);
    advertise_extra_trailing_chunk(&mut datagrams);

    assert!(decode_wire_datagram(&datagrams[0]).unwrap().is_none());
    let mut strict = PonkAssembler::new();
    assert!(
        strict
            .push_wire_datagram(&datagrams[0], peer(5583))
            .unwrap()
            .is_none()
    );

    let sender = PonkSenderKey {
        peer: peer(5583),
        sender_id: frame.sender_id,
    };
    let mut compatible = PonkAssembler::new();
    compatible.set_sender_compatibility(
        sender,
        PonkSenderCompatibility::CanonicalV0ExactBoundaryChunkCount,
    );
    let completed = compatible
        .push_wire_datagram(&datagrams[0], peer(5583))
        .unwrap()
        .unwrap();
    assert_eq!(completed.frame, frame);
    assert_eq!(
        completed.completion,
        PonkCompletion::CanonicalExactBoundaryRepair {
            advertised_chunks: 2,
            received_chunks: 1,
        }
    );
    assert_eq!(compatible.assembly_count(), 0);
    assert_eq!(compatible.buffered_bytes(), 0);
}

#[test]
fn enabling_compatibility_recovers_a_buffered_frame_on_retransmission() {
    let frame = canonical_boundary_frame(1);
    let mut datagrams = encode_wire_datagrams(&frame, &options(1_472)).unwrap();
    advertise_extra_trailing_chunk(&mut datagrams);
    let sender = PonkSenderKey {
        peer: peer(5583),
        sender_id: frame.sender_id,
    };
    let mut assembler = PonkAssembler::new();

    assert!(
        assembler
            .push_wire_datagram(&datagrams[0], peer(5583))
            .unwrap()
            .is_none()
    );
    assembler.set_sender_compatibility(
        sender,
        PonkSenderCompatibility::CanonicalV0ExactBoundaryChunkCount,
    );
    let completed = assembler
        .push_wire_datagram(&datagrams[0], peer(5583))
        .unwrap()
        .unwrap();
    assert_eq!(completed.frame, frame);
    assert_eq!(
        completed.completion,
        PonkCompletion::CanonicalExactBoundaryRepair {
            advertised_chunks: 2,
            received_chunks: 1,
        }
    );
}

#[test]
fn canonical_two_chunk_boundary_repairs_out_of_order() {
    let frame = canonical_boundary_frame(2);
    let mut datagrams = encode_wire_datagrams(&frame, &options(1_472)).unwrap();
    assert_eq!(datagrams.len(), 2);
    assert!(datagrams.iter().all(|datagram| datagram.len() == 1_472));
    advertise_extra_trailing_chunk(&mut datagrams);

    let sender = PonkSenderKey {
        peer: peer(5583),
        sender_id: frame.sender_id,
    };
    let mut assembler = PonkAssembler::new();
    assembler.set_sender_compatibility(
        sender,
        PonkSenderCompatibility::CanonicalV0ExactBoundaryChunkCount,
    );
    assert!(
        assembler
            .push_wire_datagram(&datagrams[1], peer(5583))
            .unwrap()
            .is_none()
    );
    let completed = assembler
        .push_wire_datagram(&datagrams[0], peer(5583))
        .unwrap()
        .unwrap();
    assert_eq!(completed.frame, frame);
    assert_eq!(
        completed.completion,
        PonkCompletion::CanonicalExactBoundaryRepair {
            advertised_chunks: 3,
            received_chunks: 2,
        }
    );
}

#[test]
fn compatibility_requires_checksum_full_chunks_and_structural_parse() {
    let frame = canonical_boundary_frame(1);
    let mut datagram = encode_wire_datagrams(&frame, &options(1_472))
        .unwrap()
        .remove(0);
    datagram[46] = 2;
    let sender = PonkSenderKey {
        peer: peer(5583),
        sender_id: frame.sender_id,
    };

    let mut bad_checksum = datagram.clone();
    bad_checksum[48] ^= 1;
    let mut assembler = PonkAssembler::new();
    assembler.set_sender_compatibility(
        sender,
        PonkSenderCompatibility::CanonicalV0ExactBoundaryChunkCount,
    );
    assert!(
        assembler
            .push_wire_datagram(&bad_checksum, peer(5583))
            .unwrap()
            .is_none()
    );

    let mut short = datagram.clone();
    short.pop();
    let crc = checksum(&short[HEADER_LEN..]);
    short[48..52].copy_from_slice(&crc.to_le_bytes());
    let mut assembler = PonkAssembler::new();
    assembler.set_sender_compatibility(
        sender,
        PonkSenderCompatibility::CanonicalV0ExactBoundaryChunkCount,
    );
    assert!(
        assembler
            .push_wire_datagram(&short, peer(5583))
            .unwrap()
            .is_none()
    );

    let mut malformed = datagram;
    malformed[HEADER_LEN] = 99;
    let crc = checksum(&malformed[HEADER_LEN..]);
    malformed[48..52].copy_from_slice(&crc.to_le_bytes());
    let mut assembler = PonkAssembler::new();
    assembler.set_sender_compatibility(
        sender,
        PonkSenderCompatibility::CanonicalV0ExactBoundaryChunkCount,
    );
    assert_eq!(
        assembler.push_wire_datagram(&malformed, peer(5583)),
        Err(PonkError::UnsupportedDataFormat(99))
    );
}

#[test]
fn malformed_prefixes_and_boundaries_do_not_panic() {
    for fixture in [
        CANONICAL_F32_DATAGRAM,
        CANONICAL_U16_NON_REPEATED,
        CANONICAL_MIXED,
    ] {
        for len in 0..fixture.len() {
            let _ = decode_wire_datagram(&fixture[..len]);
        }
    }

    let mut invalid = CANONICAL_F32_DATAGRAM.to_vec();
    invalid[46] = 0;
    assert_eq!(decode_wire_datagram(&invalid), Ok(None));
    invalid[46] = 1;
    invalid[47] = 1;
    assert_eq!(decode_wire_datagram(&invalid), Ok(None));
}

#[test]
fn checksum_mismatch_is_ignored_and_checksum_arithmetic_wraps() {
    let mut bad = CANONICAL_F32_DATAGRAM.to_vec();
    bad[48] = bad[48].wrapping_add(1);
    assert!(decode_wire_datagram(&bad).unwrap().is_none());

    let data = vec![u8::MAX; (u32::MAX as usize / u8::MAX as usize) + 2];
    let expected = data
        .iter()
        .fold(0u32, |sum, byte| sum.wrapping_add(u32::from(*byte)));
    assert_eq!(checksum(&data), expected);
}

#[test]
fn float_coordinates_reject_nonfinite_and_preserve_finite_values() {
    let mut frame = sample_wire_frame(0, 1);
    let PonkPathPoints::XyF32RgbU8(points) = &mut frame.paths[0].points else {
        unreachable!()
    };
    points[0].x = f32::NAN;
    assert_eq!(
        encode_wire_datagrams(&frame, &options(1_472)),
        Err(PonkError::InvalidCoordinate)
    );
    let PonkPathPoints::XyF32RgbU8(points) = &mut frame.paths[0].points else {
        unreachable!()
    };
    points[0].x = 2.0;
    points[0].y = -3.5;
    let datagram = encode_wire_datagrams(&frame, &options(1_472))
        .unwrap()
        .remove(0);
    let point_offset = HEADER_LEN + 16;
    assert_eq!(
        &datagram[point_offset..point_offset + 4],
        &2.0f32.to_le_bytes()
    );
    assert_eq!(
        &datagram[point_offset + 4..point_offset + 8],
        &(-3.5f32).to_le_bytes()
    );

    let decoded = decode_wire_datagram(&datagram).unwrap().unwrap();
    let PonkPathPoints::XyF32RgbU8(points) = &decoded.paths[0].points else {
        unreachable!()
    };
    assert_eq!(points[0].x, 2.0);
    assert_eq!(points[0].y, -3.5);
    assert_eq!(
        encode_wire_datagrams(&decoded, &options(1_472)).unwrap(),
        [datagram]
    );
}

#[test]
fn per_frame_payload_limit_drops_partial_identity() {
    let chunks =
        encode_wire_datagrams(&sample_wire_frame(1, 20), &options(HEADER_LEN + 50)).unwrap();
    let config = PonkAssemblerConfig {
        decoder: PonkDecoderLimits {
            max_frame_payload_bytes: 60,
            ..PonkDecoderLimits::default()
        },
        ..PonkAssemblerConfig::default()
    };
    let mut assembler = PonkAssembler::with_config(config);
    assembler
        .push_wire_datagram(&chunks[0], peer(5583))
        .unwrap();
    assert_eq!(
        assembler.push_wire_datagram(&chunks[1], peer(5583)),
        Err(PonkError::FramePayloadTooLarge { max: 60 })
    );
    assert_eq!(assembler.assembly_count(), 0);
    assert_eq!(assembler.buffered_bytes(), 0);
}

#[test]
fn legacy_uniform_format_wrapper_matches_wire_adapter() {
    let legacy = PonkFrame {
        sender_id: 1,
        sender_name: "legacy".into(),
        frame_number: 0,
        paths: vec![PonkPath {
            metadata: vec![],
            points: vec![PonkPoint {
                x: 0.0,
                y: -0.5,
                rgb: [0x12, 0x34, 0x56],
            }],
        }],
    };
    for format in [DataFormat::XyF32RgbU8, DataFormat::XyRgbU16] {
        let wire = PonkWireFrame::from_legacy(&legacy, format).unwrap();
        let mut wire_options = options(1_472);
        wire_options.empty_frame = EmptyFrameEncoding::ZeroPointPath(format);
        assert_eq!(
            encode_datagrams(&legacy, format, 1_472).unwrap(),
            encode_wire_datagrams(&wire, &wire_options).unwrap()
        );
    }
}

#[test]
fn decoder_limits_remain_independent_from_encoder_policy() {
    let frame = PonkWireFrame {
        sender_id: 1,
        sender_name: "paths".into(),
        frame_number: 0,
        paths: vec![
            PonkWirePath {
                metadata: vec![],
                points: PonkPathPoints::XyF32RgbU8(vec![]),
            },
            PonkWirePath {
                metadata: vec![],
                points: PonkPathPoints::XyRgbU16(vec![]),
            },
        ],
    };
    let datagram = encode_wire_datagrams(&frame, &options(1_472))
        .unwrap()
        .remove(0);
    let limits = PonkDecoderLimits {
        max_paths: 1,
        ..PonkDecoderLimits::default()
    };
    assert_eq!(
        decode_wire_datagram_with_limits(&datagram, &limits),
        Err(PonkError::TooManyPaths { max: 1 })
    );
}

#[test]
fn protocol_accepts_exactly_255_chunks_and_rejects_256() {
    let point = PonkPoint {
        x: 0.0,
        y: 0.0,
        rgb: [0, 0, 0],
    };
    let metadata = PonkMetadata {
        key: "PATHNUMB".into(),
        value: 0.0,
    };
    let frame_255 = PonkWireFrame {
        sender_id: 1,
        sender_name: "chunks".into(),
        frame_number: 0,
        paths: vec![PonkWirePath {
            metadata: vec![metadata.clone(); 9],
            points: PonkPathPoints::XyF32RgbU8(vec![point.clone(); 13]),
        }],
    };
    let one_byte = PonkEncodeOptions {
        max_datagram_len: HEADER_LEN + 1,
        limits: PonkEncoderLimits::protocol_only(),
        ..PonkEncodeOptions::default()
    };
    assert_eq!(
        encode_wire_datagrams(&frame_255, &one_byte).unwrap().len(),
        255
    );

    let frame_256 = PonkWireFrame {
        paths: vec![PonkWirePath {
            metadata: vec![metadata; 10],
            points: PonkPathPoints::XyF32RgbU8(vec![point; 12]),
        }],
        ..frame_255
    };
    assert_eq!(
        encode_wire_datagrams(&frame_256, &one_byte),
        Err(PonkError::TooManyChunks(256))
    );
}

#[test]
fn malformed_point_counts_trailing_paths_and_nonfinite_values_are_rejected() {
    let mut too_many_claimed = CANONICAL_F32_DATAGRAM.to_vec();
    let point_count_offset = HEADER_LEN + 14;
    too_many_claimed[point_count_offset..point_count_offset + 2]
        .copy_from_slice(&u16::MAX.to_le_bytes());
    let crc = checksum(&too_many_claimed[HEADER_LEN..]);
    too_many_claimed[48..52].copy_from_slice(&crc.to_le_bytes());
    assert_eq!(
        decode_wire_datagram(&too_many_claimed),
        Err(PonkError::MalformedPayload)
    );

    let mut trailing_partial = CANONICAL_F32_DATAGRAM.to_vec();
    trailing_partial.push(0);
    let crc = checksum(&trailing_partial[HEADER_LEN..]);
    trailing_partial[48..52].copy_from_slice(&crc.to_le_bytes());
    assert_eq!(
        decode_wire_datagram(&trailing_partial),
        Err(PonkError::MalformedPayload)
    );

    let mut nan = CANONICAL_F32_DATAGRAM.to_vec();
    let x_offset = HEADER_LEN + 16;
    nan[x_offset..x_offset + 4].copy_from_slice(&f32::NAN.to_le_bytes());
    let crc = checksum(&nan[HEADER_LEN..]);
    nan[48..52].copy_from_slice(&crc.to_le_bytes());
    assert_eq!(
        decode_wire_datagram(&nan),
        Err(PonkError::InvalidCoordinate)
    );
}

#[test]
fn oversized_datagram_is_rejected_without_retention() {
    let mut datagram = CANONICAL_F32_DATAGRAM.to_vec();
    datagram.resize(MAX_DATAGRAM_LEN + 1, 0);
    assert_eq!(
        decode_wire_datagram(&datagram),
        Err(PonkError::DatagramTooLarge {
            max: MAX_DATAGRAM_LEN,
            actual: MAX_DATAGRAM_LEN + 1,
        })
    );
    let mut assembler = PonkAssembler::new();
    assert_eq!(
        assembler.push_wire_datagram(&datagram, peer(5583)),
        Err(PonkError::DatagramTooLarge {
            max: MAX_DATAGRAM_LEN,
            actual: MAX_DATAGRAM_LEN + 1,
        })
    );
    assert_eq!(assembler.buffered_bytes(), 0);
}

#[test]
fn sender_name_mismatch_drops_only_the_matching_identity() {
    let chunks =
        encode_wire_datagrams(&sample_wire_frame(1, 20), &options(HEADER_LEN + 50)).unwrap();
    let sibling =
        encode_wire_datagrams(&sample_wire_frame(2, 20), &options(HEADER_LEN + 50)).unwrap();
    let mut assembler = PonkAssembler::new();
    assembler
        .push_wire_datagram(&chunks[0], peer(5583))
        .unwrap();
    assembler
        .push_wire_datagram(&sibling[0], peer(5583))
        .unwrap();

    let mut renamed = chunks[1].clone();
    renamed[13] = b'X';
    assert_eq!(
        assembler.push_wire_datagram(&renamed, peer(5583)),
        Err(PonkError::InconsistentSenderName)
    );
    assert_eq!(assembler.assembly_count(), 1);
}

#[test]
fn aggregate_byte_eviction_releases_victim_and_survivor_completes() {
    let first =
        encode_wire_datagrams(&sample_wire_frame(1, 20), &options(HEADER_LEN + 50)).unwrap();
    let second =
        encode_wire_datagrams(&sample_wire_frame(2, 10), &options(HEADER_LEN + 50)).unwrap();
    let config = PonkAssemblerConfig {
        reassembly: PonkReassemblyLimits {
            max_buffered_bytes: 220,
            max_assemblies_per_sender: 4,
            ..PonkReassemblyLimits::default()
        },
        ..PonkAssemblerConfig::default()
    };
    let mut assembler = PonkAssembler::with_config(config);
    for chunk in first.iter().take(4) {
        assembler.push_wire_datagram(chunk, peer(5583)).unwrap();
    }
    assembler
        .push_wire_datagram(&second[0], peer(5583))
        .unwrap();
    assert_eq!(assembler.assembly_count(), 1);

    let mut output = None;
    for chunk in second.iter().skip(1) {
        output = assembler
            .push_wire_datagram(chunk, peer(5583))
            .unwrap()
            .or(output);
    }
    assert_eq!(output.unwrap().frame.frame_number, 2);
    assert_eq!(assembler.buffered_bytes(), 0);
}

#[test]
fn canonical_repair_never_skips_an_interior_chunk() {
    let frame = canonical_boundary_frame(3);
    let mut datagrams = encode_wire_datagrams(&frame, &options(1_472)).unwrap();
    assert_eq!(datagrams.len(), 3);
    advertise_extra_trailing_chunk(&mut datagrams);
    let sender = PonkSenderKey {
        peer: peer(5583),
        sender_id: frame.sender_id,
    };
    let mut assembler = PonkAssembler::new();
    assembler.set_sender_compatibility(
        sender,
        PonkSenderCompatibility::CanonicalV0ExactBoundaryChunkCount,
    );
    assert!(
        assembler
            .push_wire_datagram(&datagrams[0], peer(5583))
            .unwrap()
            .is_none()
    );
    assert!(
        assembler
            .push_wire_datagram(&datagrams[2], peer(5583))
            .unwrap()
            .is_none()
    );
    let completed = assembler
        .push_wire_datagram(&datagrams[1], peer(5583))
        .unwrap()
        .unwrap();
    assert_eq!(completed.frame, frame);
}

#[test]
fn fixed_utf8_fields_do_not_split_code_points() {
    let mut frame = sample_wire_frame(0, 1);
    frame.sender_name = format!("{}é", "a".repeat(SENDER_NAME_LEN - 1));
    frame.paths[0].metadata[0].key = "abcdefgé".into();
    let datagram = encode_wire_datagrams(&frame, &options(1_472))
        .unwrap()
        .remove(0);
    let decoded = decode_wire_datagram(&datagram).unwrap().unwrap();
    assert_eq!(decoded.sender_name, "a".repeat(SENDER_NAME_LEN - 1));
    assert_eq!(decoded.paths[0].metadata[0].key, "abcdefg");
}
