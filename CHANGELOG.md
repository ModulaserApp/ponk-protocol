# Changelog

This project follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-07-13

### Added

- Zero-dependency encoding and decoding for PONK UDP datagrams.
- `XyF32RgbU8` and `XyRgbU16` wire formats.
- Automatic fragmentation into at most 255 datagrams, with an explicit zero-point path for empty frames.
- Out-of-order multipart reassembly keyed by peer address and sender ID.
- Configurable limits for assemblies, per-frame and aggregate buffered bytes, paths, points, and assembly lifetime.
- Canonical wire-vector, malformed-input, boundary, round-trip, and resource-bound tests.
- Socket-free round-trip plus multicast sender and receiver examples.

[Unreleased]: https://github.com/ModulaserApp/ponk-protocol/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/ModulaserApp/ponk-protocol/releases/tag/v0.1.0
