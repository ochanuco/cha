# ADR-0006: WASM bindings are thin and explicit
Status: Accepted

cha-wasm exposes small explicit operations such as blob_id, prepare_revision, semantic_diff, prepare_restore, and conflict-resolution helpers.
No storage callback bridge to TypeScript is used.
