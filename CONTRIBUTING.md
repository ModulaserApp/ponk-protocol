# Contributing to ponk-protocol

Contributions should preserve wire compatibility, bounded processing of untrusted input, and the zero-dependency design.

## Before changing behavior

Open an issue that describes the observed wire behavior, the intended public API, and how the change will be verified. Do not copy source code or copyrighted prose from third-party implementations. Describe protocol behavior in your own words and use independently constructed fixtures or interoperability results.

Behavior changes follow a test-first loop:

1. Add one test at a public seam and run it to observe the expected failure.
2. Make the smallest implementation change that passes the test.
3. Run the complete validation suite.

Tests should assert externally visible results through `encode_datagrams`, `decode_datagram`, `decode_datagram_with_limits`, or `PonkAssembler`. Include malformed input and resource boundaries when they apply.

## Local validation

The MSRV is Rust 1.88. Run:

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

Keep the crate zero-dependency unless a dependency removes more risk than it adds. Explain any proposed dependency, including maintenance, license, MSRV, supply-chain, binary-size, and compile-time effects.

## Releases

Install [`cargo-release`](https://github.com/crate-ci/cargo-release), then prepare and publish a release from a clean `main` branch:

```sh
cargo release <version>
cargo release <version> --execute
```

The first command previews the release. The second updates the version and lockfile, commits the change, creates a `v<version>` tag, and pushes the commit and tag. The tag starts `.github/workflows/release.yml`, which runs CI, verifies that the tag matches `Cargo.toml`, publishes to crates.io, and creates the GitHub release.

The repository must have a `CARGO_REGISTRY_TOKEN` Actions secret with permission to publish `ponk-protocol`.

## Pull requests

- Keep each pull request focused on one change.
- Update README and rustdoc when the public API or error behavior changes.
- Update CHANGELOG under `[Unreleased]` for user-visible changes.
- Do not weaken default limits or laser-safety warnings without an explicit design discussion.
- Use clear commit messages, preferably Conventional Commits.

By contributing, you agree to license your contribution under the repository's [MIT License](./LICENSE). Participation is governed by the [Code of Conduct](./CODE_OF_CONDUCT.md).
