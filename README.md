# ponk-protocol

[![CI](https://github.com/ModulaserApp/ponk-protocol/actions/workflows/ci.yml/badge.svg)](https://github.com/ModulaserApp/ponk-protocol/actions/workflows/ci.yml)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)
[![MSRV 1.88](https://img.shields.io/badge/MSRV-1.88-orange.svg)](#minimum-supported-rust-version)

`ponk-protocol` is a zero-dependency Rust codec and bounded fragment reassembler for PONK ("Pathes Over NetworK") UDP laser-path frames. It converts typed frames to datagrams, decodes complete single-datagram frames, and reassembles multipart frames from untrusted network traffic. Socket ownership and I/O remain with the application.

## Protocol scope and attribution

PONK carries ordered paths of colored points over UDP, conventionally through multicast group `239.255.10.24` on port `5583`. The protocol was created and published by MadMapper / GarageCube. Its canonical public repository is [`madmappersoftware/Ponk`](https://github.com/madmappersoftware/Ponk).

This crate is an independent Rust implementation published by the Modulaser project for interoperability. Modulaser and this crate are not affiliated with, sponsored by, or endorsed by MadMapper or GarageCube. The names “PONK” and “MadMapper” identify the protocol and products with which this crate interoperates. See [NOTICE](./NOTICE).

## Installation

The crate is currently distributed from GitHub, not crates.io:

```toml
[dependencies]
ponk-protocol = { git = "https://github.com/ModulaserApp/ponk-protocol", branch = "main" }
```

For an application or library, pin an immutable commit with `rev = "…"` or commit the resolved revision in `Cargo.lock`.

The crate uses only the Rust standard library.

## Encode and decode

```rust
use ponk_protocol::{
    DataFormat, PonkFrame, PonkMetadata, PonkPath, PonkPoint,
    decode_datagram, encode_datagrams,
};

let frame = PonkFrame {
    sender_id: 7,
    sender_name: "example-sender".into(),
    frame_number: 42,
    paths: vec![PonkPath {
        metadata: vec![PonkMetadata {
            key: "PATHNUMB".into(),
            value: 0.0,
        }],
        points: vec![
            PonkPoint { x: -0.5, y: -0.25, rgb: [255, 0, 0] },
            PonkPoint { x: 0.5, y: 0.25, rgb: [0, 255, 0] },
        ],
    }],
};

let datagrams = encode_datagrams(&frame, DataFormat::XyF32RgbU8, 1_472)?;
assert_eq!(datagrams.len(), 1);

let decoded = decode_datagram(&datagrams[0])?.expect("complete PONK frame");
assert_eq!(decoded.paths[0].points, frame.paths[0].points);
# Ok::<(), ponk_protocol::PonkError>(())
```

`decode_datagram` intentionally returns `None` for multipart input. Use `PonkAssembler` when a frame may span several datagrams. Encoding a frame with no paths emits one explicit zero-point path because interoperable receivers may ignore a header-only datagram.

## Bounded multipart reassembly

PONK traffic is usually untrusted multicast input. Configure finite limits and identify fragments by their actual UDP peer address:

```rust
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;
use ponk_protocol::{PonkAssembler, PonkLimits};

let limits = PonkLimits {
    max_assemblies: 32,
    max_frame_payload_bytes: 512 * 1024,
    max_buffered_bytes: 2 * 1024 * 1024,
    max_paths: 2_048,
    max_points: 65_535,
    assembly_timeout: Duration::from_millis(300),
};
let mut assembler = PonkAssembler::with_limits(limits);
let peer = SocketAddr::from((Ipv4Addr::LOCALHOST, 5583));
# let datagrams: Vec<Vec<u8>> = Vec::new();

for datagram in &datagrams {
    if let Some(frame) = assembler.push_datagram(datagram, peer)? {
        // Apply application validation and hand the frame to a safe renderer.
        let _ = frame;
    }
}
assembler.prune_stale();
# Ok::<(), ponk_protocol::PonkError>(())
```

The default `PonkAssembler::new()` limits concurrent assemblies, per-frame bytes, aggregate buffered bytes, path and point counts, and incomplete-frame lifetime. `with_max_assemblies` changes only the assembly-count limit. A zero assembly limit ignores multipart frames without retaining their payloads.

## Supported wire formats

| `DataFormat` | Coordinates | Color | Bytes per point | Notes |
| --- | --- | --- | ---: | --- |
| `XyF32RgbU8` | little-endian `f32` | 8-bit RGB | 11 | Rejects non-finite values and clamps coordinates to `[-1.0, 1.0]` at the wire boundary. |
| `XyRgbU16` | normalized unsigned 16-bit | 16-bit channels on wire, exposed as 8-bit RGB | 10 | Quantizes coordinates and expands encoded 8-bit color to 16-bit. |

Each datagram has a 52-byte header. Encoded frames may use at most 255 chunks. The header field commonly called `data_crc` is the protocol's wrapping byte-sum checksum, not CRC-32.

Sender names occupy 32 bytes and metadata keys occupy 8 bytes. Encoding truncates both at a UTF-8 character boundary. Decoding replaces invalid UTF-8 in received fixed-width fields with the Unicode replacement character.

## Error behavior

The decoder distinguishes traffic that should be ignored from malformed PONK input:

- `Ok(None)`: wrong magic, invalid chunk numbering, checksum mismatch, a multipart datagram passed to the stateless decoder, or an assembly that is not complete.
- `Err(PonkError::DatagramTooSmall)`: a datagram begins with the PONK magic but cannot contain the complete header.
- `Err(...)`: unsupported protocol/data versions, oversized datagrams or frames, malformed payloads, non-finite float coordinates, and configured resource-limit violations.
- `Ok(Some(frame))`: a complete frame passed checksum, structural validation, coordinate handling, and the configured path/point limits.

`PonkAssembler` accepts out-of-order chunks. A new frame identity for the same `(peer address, sender ID)` replaces that sender's incomplete assembly. When configured capacity is reached, the oldest incomplete assembly is evicted.

## Laser-safety scope

This crate validates the wire representation and bounds parser/reassembly resources. It does **not** implement laser arming, output blanking, scanner velocity or acceleration limits, dwell/energy limits, projection zones, coordinate mapping, DAC timing, or hardware fault handling.

Do not send decoded points directly to laser hardware. A laser application must treat every decoded frame as untrusted content and pass it through a non-bypassable, fail-dark hardware safety pipeline.

## Examples

- [`roundtrip`](./examples/roundtrip.rs): encode and decode without sockets.
- [`send_multicast`](./examples/send_multicast.rs): send one frame through a `UdpSocket`.
- [`receive_reassemble`](./examples/receive_reassemble.rs): join the multicast group and use bounded reassembly.

```sh
cargo run --example roundtrip
```

## Minimum supported Rust version

The minimum supported Rust version (MSRV) is Rust **1.88**. CI checks both stable Rust and Rust 1.88. An MSRV increase requires a documented minor-version change while the crate is pre-1.0.

## Testing

Run the same local checks as CI:

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

The test suite includes canonical wire vectors, both data formats, single- and multi-datagram round trips, malformed and oversized input, UTF-8 boundaries, 255/256-chunk boundaries, non-finite coordinates, stale-assembly cleanup, and byte/geometry resource limits.

## Contributing and security

Read [CONTRIBUTING.md](./CONTRIBUTING.md) before submitting a change and [CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md) before participating. Report vulnerabilities privately as described in [SECURITY.md](./SECURITY.md).

## License

The implementation is available under the [MIT License](./LICENSE). Attribution and non-affiliation details are in [NOTICE](./NOTICE).
