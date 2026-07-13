# ponk-protocol

[![CI](https://github.com/ModulaserApp/ponk-protocol/actions/workflows/ci.yml/badge.svg)](https://github.com/ModulaserApp/ponk-protocol/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/ponk-protocol.svg)](https://crates.io/crates/ponk-protocol)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)
[![MSRV 1.88](https://img.shields.io/badge/MSRV-1.88-orange.svg)](#minimum-supported-rust-version)

`ponk-protocol` is a zero-dependency Rust codec and bounded fragment reassembler for PONK ("Pathes Over NetworK") UDP frames. Its mixed-format model retains each path's point format, every 16-bit coordinate and color field, and every finite F32 coordinate. Socket ownership and I/O remain with the application.

## Protocol scope and attribution

PONK carries ordered 2D paths of colored points over UDP, conventionally through multicast group `239.255.10.24` on port `5583`. It was originally developed to transfer laser paths between applications, but its wire format does not define laser hardware, scanning, or rendering behavior. The protocol was created and published by MadMapper / GarageCube. Its canonical public repository is [`madmappersoftware/Ponk`](https://github.com/madmappersoftware/Ponk).

This crate is an independent Rust implementation published by the Modulaser project for interoperability. Wire behavior and compatibility fixtures are checked against canonical commit [`2c166392`](https://github.com/madmappersoftware/Ponk/tree/2c166392cb505bfd48440a2a51bfcfb15f3ccfec). Modulaser and this crate are not affiliated with, sponsored by, or endorsed by MadMapper or GarageCube. The names “PONK” and “MadMapper” identify the protocol and products with which this crate interoperates. See [NOTICE](./NOTICE).

## Installation

```sh
cargo add ponk-protocol
```

The crate uses only the Rust standard library.

## Mixed-format frames

PONK stores `dataFormat` inside each path. Use `PonkWireFrame` when decoding, forwarding, or constructing mixed-format frames without reducing their point fields:

```rust
use ponk_protocol::{
    PonkPathPoints, PonkPoint, PonkWireFrame, PonkWirePath, XyRgbU16Point,
    decode_wire_datagram, encode_wire_datagrams, PonkEncodeOptions,
};

let frame = PonkWireFrame {
    sender_id: 7,
    sender_name: "example-sender".into(),
    frame_number: 42,
    paths: vec![
        PonkWirePath {
            metadata: vec![],
            points: PonkPathPoints::XyF32RgbU8(vec![PonkPoint {
                x: -0.5,
                y: 0.25,
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

let datagrams = encode_wire_datagrams(&frame, &PonkEncodeOptions::default())?;
let decoded = decode_wire_datagram(&datagrams[0])?.expect("complete frame");
assert_eq!(decoded, frame);
# Ok::<(), ponk_protocol::PonkError>(())
```

`decode_wire_datagram` returns `None` for multipart input. Use `PonkAssembler::push_wire_datagram` when a frame may span several datagrams.

### One format for a whole frame

`PonkFrame`, `PonkPath`, and `PonkPoint` remain available for callers that use 8-bit color and one selected format for every path:

```rust
use ponk_protocol::{
    DataFormat, PonkFrame, PonkPath, PonkPoint, encode_datagrams,
};

let frame = PonkFrame {
    sender_id: 7,
    sender_name: "uniform-format".into(),
    frame_number: 1,
    paths: vec![PonkPath {
        metadata: vec![],
        points: vec![PonkPoint { x: 0.0, y: 0.0, rgb: [255, 64, 0] }],
    }],
};
let datagrams = encode_datagrams(&frame, DataFormat::XyF32RgbU8, 1_472)?;
assert_eq!(datagrams.len(), 1);
# Ok::<(), ponk_protocol::PonkError>(())
```

The legacy decoder projects U16 color to the most significant byte for backward compatibility. That conversion is intentionally lossy. New code should decode to `PonkWireFrame` and call `XyRgbU16Point::to_8bit` or `reduce_rgb16` with an explicit `U16ColorReduction` policy only when an 8-bit result is required.

## Supported wire formats

| `DataFormat` | Coordinates | Color | Bytes per point | Typed storage |
| --- | --- | --- | ---: | --- |
| `XyF32RgbU8` | little-endian `f32` | 8-bit RGB | 11 | `PonkPathPoints::XyF32RgbU8(Vec<PonkPoint>)` |
| `XyRgbU16` | little-endian `u16` | full 16-bit RGB | 10 | `PonkPathPoints::XyRgbU16(Vec<XyRgbU16Point>)` |

F32 coordinates must be finite. Encoding and decoding preserve finite values outside the conventional `[-1.0, 1.0]` range; applications must apply any semantic bounds they require. `XyRgbU16Point` stores raw coordinate and color words. `normalized_coord_to_u16` uses the canonical C++ sender's clamping and truncating conversion, so `0.0` maps to `0x7fff`; `u16_to_normalized_coord` performs the inverse normalization.

Each datagram has a 52-byte header. Sender names occupy 32 bytes and metadata keys occupy 8 bytes. Encoding truncates both at a UTF-8 character boundary. Decoding replaces invalid UTF-8 with the Unicode replacement character.

## Encoder policy is separate from decoder safety

`PonkEncodeOptions` controls datagram size, empty-frame representation, and policy for trusted encoder input. Its default policy retains the crate's conservative path, aggregate-point, and payload caps. Raise individual limits or use `PonkEncoderLimits::protocol_only()` for trusted frames:

```rust
use ponk_protocol::{PonkEncodeOptions, PonkEncoderLimits, MAX_DATAGRAM_LEN};

let options = PonkEncodeOptions {
    max_datagram_len: MAX_DATAGRAM_LEN,
    limits: PonkEncoderLimits::protocol_only(),
    ..PonkEncodeOptions::default()
};
# let _ = options;
```

`protocol_only()` does not disable wire validation. Every path still has at most 255 metadata records and 65,535 points. Datagrams cannot exceed 65,507 bytes, frames cannot require more than 255 chunks, and all size arithmetic is checked. PONK has no aggregate point-count or path-count field, so those are configurable policy limits rather than immutable wire restrictions.

For a frame with no paths, choose `EmptyFrameEncoding::HeaderOnly` or `EmptyFrameEncoding::ZeroPointPath(format)`. The legacy `encode_datagrams` wrapper keeps its interoperable explicit-zero-point-path behavior.

## Bounded overlapping reassembly

The assembler can retain multiple adjacent frame identities from one sender. This allows reordered chunks from frames 255 and 0, or other neighboring frames, to complete independently:

```rust
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;
use ponk_protocol::{
    PonkAssembler, PonkAssemblerConfig, PonkDecoderLimits, PonkReassemblyLimits,
};

let config = PonkAssemblerConfig {
    decoder: PonkDecoderLimits {
        max_frame_payload_bytes: 512 * 1024,
        max_paths: 2_048,
        max_total_points: 100_000,
    },
    reassembly: PonkReassemblyLimits {
        max_assemblies: 32,
        max_assemblies_per_sender: 4,
        max_buffered_bytes: 2 * 1024 * 1024,
        assembly_timeout: Duration::from_millis(300),
    },
};
let mut assembler = PonkAssembler::with_config(config);
let peer = SocketAddr::from((Ipv4Addr::LOCALHOST, 5583));
# let datagrams: Vec<Vec<u8>> = Vec::new();
for datagram in &datagrams {
    if let Some(completed) = assembler.push_wire_datagram(datagram, peer)? {
        let _frame = completed.frame;
    }
}
assembler.prune_stale();
# Ok::<(), ponk_protocol::PonkError>(())
```

An in-flight identity contains the UDP peer, sender ID, frame number, advertised chunk count, and checksum. Frame numbers are treated as wrapping identifiers, not numeric age. Sender names must stay consistent within one identity.

Every incomplete identity counts toward the global and per-sender limits. New progress updates its eviction age; exact duplicate chunks are no-ops and do not refresh the timeout. A conflicting duplicate drops only that identity. On pressure, the assembler evicts the least recently progressing identity for that sender first, then the least recently progressing identity globally. Completion releases all retained bytes.

The protocol cannot distinguish two generations with the same peer, sender ID, frame number, chunk count, checksum, and sender name. The checksum is only a wrapping sum of payload bytes, not CRC-32, authentication, or protection against same-sum corruption.

`PonkLimits` and `PonkAssembler::with_limits` remain as compatibility APIs. `PonkAssembler::with_max_assemblies(n)` applies `n` as both the global and per-sender limit. Use `PonkAssemblerConfig` to set a lower per-sender limit or to configure parser safety separately from reassembly capacity.

## Opt-in canonical sender repair

The canonical C++, JavaScript, and TouchDesigner v0 senders advertise one extra trailing chunk when the payload is an exact positive multiple of 1,420 bytes. Strict mode remains the default and waits for every advertised chunk.

Repair can be enabled for one known `(peer, sender_id)`:

```rust
use std::net::{Ipv4Addr, SocketAddr};
use ponk_protocol::{PonkAssembler, PonkSenderCompatibility, PonkSenderKey};

let peer = SocketAddr::from((Ipv4Addr::LOCALHOST, 5583));
let sender = PonkSenderKey { peer, sender_id: 123 };
let mut assembler = PonkAssembler::new();
assembler.set_sender_compatibility(
    sender,
    PonkSenderCompatibility::CanonicalV0ExactBoundaryChunkCount,
);
```

Repair completes only when the sole missing slot is the advertised final slot, every received chunk is exactly 1,420 bytes, the checksum matches, and the concatenated payload parses completely under decoder limits. `push_wire_datagram` reports `PonkCompletion::CanonicalExactBoundaryRepair` so repaired frames remain visible to the caller. Changing a sender from strict mode does not itself return a frame; if the candidate is already buffered, retransmitting any identical received chunk triggers repair evaluation.

The additive checksum leaves a residual ambiguity: a missing legitimate all-zero final chunk has the same checksum as no final chunk. Sender-specific opt-in, the exact-size fingerprint, full parsing, and visible provenance reduce the risk but cannot remove that protocol-level ambiguity. Do not enable repair globally for unknown multicast senders.

## Error behavior

The decoder distinguishes ignored traffic from malformed PONK input:

- `Ok(None)`: wrong magic, invalid chunk numbering, checksum mismatch, multipart input passed to a stateless decoder, or an incomplete assembly.
- `Err(PonkError::DatagramTooSmall)`: input starts with the PONK magic but cannot contain a complete header.
- `Err(...)`: unsupported versions/formats, oversized data, malformed payloads, non-finite coordinates, conflicting chunks, or configured resource-limit violations.
- `Ok(Some(...))`: checksum and complete structural parsing succeeded under the configured limits.

Header-only payloads decode as zero paths. A trailing partial path rejects the entire frame.

## Application responsibilities

This crate validates the wire representation and bounds parser/reassembly resources. It does not assign application semantics to decoded paths or decide whether received geometry is safe to render or use for device control. Applications must treat every decoded frame as untrusted content and apply validation appropriate to their use case.

When PONK data controls laser hardware, do not send decoded points directly to the device. Pass every frame through a non-bypassable, fail-dark safety pipeline that enforces arming, output blanking, scanner velocity and acceleration limits, dwell and energy limits, projection zones, coordinate mapping, DAC timing, and hardware fault handling.

## Examples

- [`roundtrip`](./examples/roundtrip.rs): mixed-format point-preserving round trip without sockets.
- [`send_multicast`](./examples/send_multicast.rs): send one uniform-format frame through a `UdpSocket`.
- [`receive_reassemble`](./examples/receive_reassemble.rs): join the multicast group and use bounded mixed-format reassembly.

```sh
cargo run --example roundtrip
```

The two multicast examples accept an optional local interface IPv4 address:

```sh
cargo run --example receive_reassemble -- 192.168.1.20
cargo run --example send_multicast -- 192.168.1.20
```

Omit the address to let the operating system choose the interface. Pass an interface's own address to pin multicast to it — useful on Windows, where a Wi-Fi interface is often not selected by default and multicast otherwise never crosses the WLAN. Socket ownership stays with the application, so interface selection is an application-level policy shown in the examples rather than part of the codec.

## Minimum supported Rust version

The minimum supported Rust version (MSRV) is Rust **1.88**. CI checks both stable Rust and Rust 1.88. An MSRV increase requires a documented minor-version change while the crate is pre-1.0.

## Testing

```sh
cargo fmt --all --check
cargo check --all-targets
cargo test --all-targets
cargo test --doc
cargo clippy --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
cargo package
cargo publish --dry-run
```

The tests cover canonical literal vectors, mixed formats, non-repeated U16 color words, coordinate conversion, empty paths, policy and wire limits, overlapping and wrapped frame identities, duplicate conflicts, deterministic bounds, stale cleanup, exact-boundary compatibility, malformed input, and UTF-8 boundaries.

## Contributing and security

Read [CONTRIBUTING.md](./CONTRIBUTING.md) before submitting a change and [CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md) before participating. Report vulnerabilities privately as described in [SECURITY.md](./SECURITY.md).

## License

The implementation is available under the [MIT License](./LICENSE). Attribution and non-affiliation details are in [NOTICE](./NOTICE).
