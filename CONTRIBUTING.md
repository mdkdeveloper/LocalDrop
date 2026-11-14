# Contributing

- Documentation, code comments and commit messages are in **English**.
- Before pushing:
  ```bash
  cargo fmt --all
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  ```
- `unsafe` is forbidden workspace-wide. If an FFI crate ever needs it, it gets a scoped
  exception with a comment explaining why.
- Anything that changes bytes on the wire (announcements, handshake, messages, key
  derivation) must update the matching document in `docs/` in the same change and,
  if incompatible, bump the relevant version constant.
- Never use `KdfParams::INSECURE_TEST` outside tests.
