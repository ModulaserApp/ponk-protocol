## What changed

Describe the public behavior and why it is needed.

## Wire and safety impact

- Wire compatibility:
- Resource-limit impact:
- Laser-safety documentation impact:
- Dependency or MSRV impact:

## Test evidence

- [ ] A public-seam test was added before a behavioral implementation change.
- [ ] The failing test was observed, or no behavior changed.
- [ ] `cargo fmt --all --check`
- [ ] `cargo check --all-targets`
- [ ] `cargo test --all-targets`
- [ ] `cargo test --doc`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps`
- [ ] `cargo package`
- [ ] `cargo publish --dry-run`

## Documentation

- [ ] README/rustdoc and CHANGELOG reflect user-visible changes.
- [ ] No third-party source, copyrighted prose, credentials, private URLs, artifacts, or unrelated history is included.
