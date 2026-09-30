# cha PoC design

Status: Accepted for PoC implementation

> **cha — Change History & Amendments for documents**

cha is a standalone Rust project.

The PoC produces:
- `cha-core`
- `cha-wasm`

No standalone CLI is required yet.

## Hard non-dependencies

`cha-core` must not depend on:
- any host application
- Cloudflare
- Durable Objects
- D1
- R2
- OKF
- Markdown
- HTTP
- authentication
- TypeScript

It must be deterministic and I/O-free.

## IDs

- DocumentId: UUIDv7
- ChangeId: UUIDv7
- OperationId: UUIDv7
- ActorId: opaque string
- BlobId: self-describing hash
- RevisionId: hash of canonical revision JSON

## BlobId

Input: raw bytes exactly as stored.

PoC:
```text
sha256:<hex-digest>
```

No normalization.

## Revision

Immutable logical snapshot:

```text
Revision
├─ document_id
├─ change_id
├─ parents[]
├─ content_blob_id
├─ attachment manifest
└─ semantic revision state
```

RevisionId hashes canonical JSON.

Include:
- document_id,
- change_id,
- sorted parent IDs,
- content_blob_id,
- deterministic attachment manifest,
- semantic revision state.

Exclude:
- timestamps,
- ActorId,
- OperationId,
- restore reason,
- UI metadata.

Canonicalize parent and attachment ordering.

Provide deterministic test vectors.

## Change

One ChangeId is one semantic editing session for one document.

One Change contains multiple immutable Revisions.

State:
```text
open
closed
```

Host decides when to close it.

Closed Change must not be appended to.

Change supports optional description.

## Operation

Operation is repository mutation/action history.

ActorId belongs on Operation only.

Single- vs multi-document persistence is host-specific.

A multi-document Operation groups separate document-local ChangeIds.

A Change never spans multiple documents.

## Restore

Restore creates/prepares a new forward-moving Revision using historical content/attachment references.

It does not rewind a head.

## Delete

Deletion is tombstone revision state.

Restore creates a new non-tombstone Revision.

## Conflict semantics

cha supports multiple parents.

Manual resolution:
- creates new ChangeId,
- creates new Revision,
- all conflicting heads become parents.

No automatic merge in PoC.

## Generic typed-tree IR

Example:

```json
{
  "kind": "heading",
  "attributes": {
    "level": 2
  },
  "text": "Storage",
  "children": []
}
```

Do not introduce Markdown-specific node types into cha-core.

## Semantic diff

No stable persisted block IDs.

Compute ephemeral deterministic fingerprints.

Matching:
1. exact kind + fingerprint,
2. structural/path/nearby position,
3. text similarity fallback.

Return generic events:
- NodeAdded
- NodeRemoved
- NodeModified
- AttributeChanged
- TextChanged
- ChildAdded
- ChildRemoved

No global optimization or move detection in PoC.

Diff output is computed on demand and is not canonical repository state.

## WASM crate

`cha-wasm` is a thin binding layer.

Prefer small explicit exports:
- blob_id
- prepare_revision
- semantic_diff
- prepare_restore
- prepare_conflict_resolution

No storage callbacks into TypeScript.

## Errors

Use typed domain errors.

Examples:
- InvalidDocument
- InvalidRevision
- InvalidCanonicalState
- InvalidTypedTree
- InvalidParentSet
- ConflictStateError
- HashingError

WASM serializes structured errors:
```json
{
  "code": "INVALID_REVISION",
  "message": "...",
  "context": {}
}
```

## Suggested layout

```text
cha/
├─ crates/
│  ├─ cha-core/
│  │  ├─ ids
│  │  ├─ blob
│  │  ├─ revision
│  │  ├─ change
│  │  ├─ operation
│  │  ├─ typed_tree
│  │  ├─ diff
│  │  └─ errors
│  └─ cha-wasm/
└─ tests/
   ├─ blob_id_vectors
   ├─ revision_id_vectors
   ├─ semantic_diff_vectors
   └─ revision_graph_cases
```

## Required tests

- stable BlobId from raw bytes,
- canonical RevisionId vectors,
- ActorId does not affect RevisionId,
- OperationId does not affect RevisionId,
- timestamp does not affect RevisionId,
- ChangeId does affect RevisionId,
- parent input ordering does not affect RevisionId,
- attachment input ordering does not affect RevisionId,
- semantic diff deterministic,
- restore produces forward revision model,
- conflict resolution supports multiple parents,
- WASM result matches native core result.

## Deferred

- standalone CLI
- SQLite adapter
- native FFI
- Git interoperability
- branches/bookmarks
- automatic merge
- stable block IDs
- format-specific parsers
- Cloudflare adapters
