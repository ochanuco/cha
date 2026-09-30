# cha

Change History & Amendments for documents.

cha is a pure, deterministic Rust library for the history semantics of a document store: content-addressed blob and revision identities, a revision DAG with tombstones, restore and conflict resolution that move history forward, operations that group document-local changes, and a generic typed-tree semantic diff. It performs no I/O and knows nothing about the host's storage or document format.

The host-facing contract is [`docs/ABI.md`](docs/ABI.md). Design background is in [`docs/DESIGN.md`](docs/DESIGN.md) and [`docs/adr/`](docs/adr/).

## Layout

| Path | Content |
|---|---|
| `crates/cha-core` | the library; `cha_core::api` is the string-in / string-out surface |
| `crates/cha-wasm` | thin `wasm-bindgen` binding over `cha_core::api` |
| `tests/vectors` | contract test vectors, one file per exported function |
| `tests/wasm/run_vectors.mjs` | runs the vectors against the built wasm package |
| `scripts/build-wasm.sh` | builds `pkg/` |
| `scripts/check_vectors.py` | verifies the vectors' hashes independently of the Rust code |

## Test

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python3 scripts/check_vectors.py
```

## Build and test the wasm package

Requires the `wasm32-unknown-unknown` target and a `wasm-bindgen` CLI whose version equals the `wasm-bindgen` pin in `Cargo.toml`.

```sh
./scripts/build-wasm.sh
node tests/wasm/run_vectors.mjs
```
