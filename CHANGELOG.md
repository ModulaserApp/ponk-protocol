# Changelog

This project follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- The `send_multicast` and `receive_reassemble` examples accept an optional local interface IPv4 address, pinning multicast egress and group membership to a chosen interface. This works around default-interface selection failing over Wi-Fi on Windows. Interface selection stays an application-level policy in the examples; the codec is unchanged.

## [0.2.0] - 2026-07-13

### Added

- Point-field-preserving `PonkWireFrame`, `PonkWirePath`, and `PonkPathPoints` APIs for mixed-format frames.
- Lossless `XyRgbU16Point` storage plus explicit normalized-coordinate and 8-bit color conversion helpers.
- Separate `PonkDecoderLimits`, `PonkEncoderLimits`, `PonkReassemblyLimits`, `PonkEncodeOptions`, and `PonkAssemblerConfig` policies.
- Multiple bounded in-flight frame identities per peer and sender, including wrapped frame numbers and distinguishable same-number generations.
- Global and per-sender assembly limits, progress-based deterministic eviction, exact-duplicate handling, and conflicting-duplicate rejection.
- Sender-specific, opt-in repair for the canonical v0 exact-1,420-byte chunk-count bug, with completion provenance.
- Selectable header-only or explicit-zero-point-path empty-frame encoding.
- Canonical mixed-format, arbitrary-U16, overlap, resource-bound, compatibility, and malformed-input tests.

### Changed

- U16 coordinate encoding now uses the canonical sender's truncating conversion; normalized `0.0` encodes as `0x7fff`.
- Finite F32 coordinates now retain their encoded value instead of being clamped to `[-1.0, 1.0]`.
- `PonkAssembler::with_max_assemblies` now applies its argument as both the global and per-sender limit. Use `PonkAssembler::with_config` to set a lower per-sender limit.
- Encoder policy limits can be raised or disabled for trusted input without changing decoder safety defaults. Actual field widths, checked size arithmetic, the UDP maximum, and the 255-chunk maximum remain mandatory.
- `PonkAssembler` retains adjacent frame identities instead of replacing every incomplete frame from the same peer and sender.
- The multicast receiver and offline round-trip examples use mixed-format data and reassembly.

### Breaking changes

- This release increments the pre-1.0 minor version because `PonkError` adds `BufferedBytesLimit`, `ConflictingChunk`, and `InconsistentSenderName`. Downstream exhaustive matches must handle these variants.

### Compatibility

- `PonkFrame`, `PonkPath`, `PonkPoint`, `PonkLimits`, `encode_datagrams`, `decode_datagram`, and `PonkAssembler::push_datagram` remain available.
- The legacy U16 decoder still returns the most significant color byte. Use `decode_wire_datagram` to preserve all 16 bits.
- Canonical exact-boundary repair is disabled by default. Its additive checksum cannot distinguish a phantom final chunk from a legitimate missing all-zero final chunk, so repair requires explicit sender allowlisting.

[Unreleased]: https://github.com/ModulaserApp/ponk-protocol/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/ModulaserApp/ponk-protocol/compare/v0.1.0...v0.2.0
