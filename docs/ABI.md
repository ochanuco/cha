# cha WASM ABI — `cha-abi/1`

Status: PoC ABI, fixed for the Aracha vertical slice.

This document fills in the ABI that `WASM_CONTRACT.md` leaves open.
`cha-core` exposes the same functions natively (`cha_core::api`), string in / string out, and `cha-wasm` forwards to them unchanged.

## Exports

```ts
// wasm-bindgen --target web; initialize with initSync({ module: WebAssembly.Module })
export function abi_version(): string;                          // "cha-abi/1"
export function blob_id(bytes: Uint8Array): string;             // never throws
export function prepare_revision(input: string): string;
export function prepare_restore(input: string): string;
export function prepare_conflict_resolution(input: string): string;
export function semantic_diff(input: string): string;
export function prepare_operation(input: string): string;
```

Every `input: string` is a JSON document. Every returned string is canonical JSON (see below), so equal inputs give byte-equal outputs.

## Errors

A failing call throws a JavaScript `Error` with:

| Property | Value |
|---|---|
| `name` | `"ChaError"` |
| `code` | one of the codes below |
| `message` | human-readable, not stable, never match on it |
| `context` | JSON object, stable per code |

| Code | Raised when | `context` |
|---|---|---|
| `INVALID_JSON` | input is not JSON, or contains a number that is not an integer in `[-(2^53-1), 2^53-1]` | `{}` |
| `INVALID_INPUT` | missing field, unknown field, or wrong JSON type, including inside a typed-tree node | `{ "field": "<dotted path>" }`; `""` when the input root is not an object |
| `INVALID_ID` | malformed DocumentId / ChangeId / OperationId / BlobId / RevisionId / attachment_id / actor_id | `{ "field": "<dotted path>" }` |
| `INVALID_PARENT_SET` | duplicate parent, or parent count outside the function's allowed range | `{ "count": <n> }` |
| `CHANGE_CLOSED` | `change.state` is `"closed"` | `{ "change_id": "<id>" }` |
| `INVALID_REVISION` | the Revision being prepared breaks a Revision invariant (tombstone/content rule, duplicate attachment) | `{ "reason": "tombstone_has_content" \| "tombstone_has_attachments" \| "missing_content" \| "duplicate_attachment" }` |
| `INVALID_CANONICAL_STATE` | a stored revision passed in (`target`, `heads[i]`) is not canonical `cha.revision/1` JSON, or does not hash to its `revision_id` | `{ "field": "target.canonical" \| "heads.<i>.canonical", "reason": "not_canonical" \| "id_mismatch" }` |
| `INVALID_DOCUMENT` | a stored revision passed in belongs to a different `document_id` | `{ "field": "target" \| "heads.<i>" }` |
| `CONFLICT_STATE` | conflict resolution reuses the ChangeId of one of the heads | `{ "change_id": "<id>" }` |
| `INVALID_RESTORE_TARGET` | restore target is a tombstone | `{}` |
| `INVALID_TYPED_TREE` | a node has an empty `kind`, or sits deeper than 48 levels below the root (the root is depth 0) | `{ "field": "<dotted path of the node>" }` |
| `INVALID_OPERATION` | Operation lists no change, or repeats a DocumentId or ChangeId | `{ "reason": "empty" \| "duplicate_document" \| "duplicate_change" }` |
| `HASHING_ERROR` | reserved; not raised by `cha-abi/1` | `{}` |

Checks run in this order and the first failure wins: `INVALID_JSON`, `INVALID_INPUT`, `INVALID_ID`, `CHANGE_CLOSED`, `INVALID_PARENT_SET`, then the remaining codes in the order each function lists its rules.

Dotted paths use array indices, for example `attachments.1.blob_id`, `heads.0.canonical`, or `old.children.0.kind`.

`INVALID_PARENT_SET.count` is the length of the supplied `parents` or `heads` list.
Stored revisions are verified in index order, each followed by its document check, before `CONFLICT_STATE` is considered.
In `semantic_diff`, every `INVALID_INPUT` check on both trees precedes any `INVALID_TYPED_TREE` check, and `old` is checked before `new`.

## Canonical JSON

RFC 8785 (JCS), restricted to integers:

- object keys sorted by UTF-16 code units, no whitespace, UTF-8 output;
- strings escaped as JCS specifies (`\"`, `\\`, `\b`, `\f`, `\n`, `\r`, `\t`, other controls below U+0020 as lowercase `\u00xx`, everything else literal);
- numbers are integers in `[-(2^53-1), 2^53-1]` written in plain decimal. Floats, exponents, and fractions are rejected on input, which keeps the byte form independent of any float formatter;
- no Unicode normalization. Strings are hashed exactly as supplied.

Input nesting deeper than 128 JSON levels is `INVALID_JSON`.
An input object must not repeat a key: the repeat is not detected and only the last value is used. A repeated key inside a stored `canonical` string is detected, as `not_canonical`.

## Identifier formats

| Type | Format |
|---|---|
| DocumentId, ChangeId, OperationId | UUIDv7, lowercase hyphenated `8-4-4-4-12`, version nibble `7`, variant bits `10`. The host generates them; cha validates the format only. Uppercase is rejected rather than folded, because the id is hashed as written. |
| BlobId | `sha256:` + 64 lowercase hex digits, SHA-256 of the raw bytes, no normalization |
| RevisionId | `sha256:` + 64 lowercase hex digits, SHA-256 of the UTF-8 bytes of the canonical revision |
| attachment_id, ActorId | opaque non-empty string |

## Canonical revision — `cha.revision/1`

```json
{"attachments":[{"attachment_id":"fig-1","blob_id":"sha256:…"}],"change_id":"…","content_blob_id":"sha256:…","document_id":"…","parents":["sha256:…"],"schema":"cha.revision/1","state":{"host":{"path":"concepts/rust"},"tombstone":false}}
```

| Field | Rule |
|---|---|
| `schema` | always `"cha.revision/1"` |
| `document_id`, `change_id` | as supplied |
| `parents` | RevisionIds sorted ascending |
| `content_blob_id` | BlobId, or `null` exactly when `state.tombstone` is `true` |
| `attachments` | sorted by `attachment_id` in UTF-16 code unit order; empty when `state.tombstone` is `true` |
| `state.tombstone` | the only part of `state` cha interprets |
| `state.host` | opaque JSON object owned by the host, hashed as part of the revision |

Timestamps, ActorId, OperationId, restore reason, Change state, and Change description are not part of the revision and cannot be supplied.

## Shared input shapes

```ts
type Change      = { id: string; state: "open" | "closed" };
type Attachment  = { attachment_id: string; blob_id: string };
type State       = { tombstone: boolean; host: object };
type StoredRevision = { revision_id: string; canonical: string };  // exactly as returned by a prepare_* call

type PreparedRevision = {
  revision_id: string;
  canonical: string;   // the hashed bytes; persist this verbatim
  revision: object;    // JSON.parse(canonical), for convenience
};
```

A `StoredRevision` is verified before use: `canonical` must be a valid `cha.revision/1` revision that re-serializes to itself (`not_canonical` otherwise) and hash to `revision_id` (`id_mismatch` otherwise).

## `prepare_revision`

```ts
input:  { document_id: string; change: Change; parents: string[];
          content_blob_id: string | null; attachments: Attachment[]; state: State }
output: PreparedRevision
```

Rules:

1. `parents` holds 0 entries (document creation) or 1 entry. More than one parent is a conflict resolution and must go through `prepare_conflict_resolution`.
2. `state.tombstone: true` requires `content_blob_id: null` and `attachments: []`. `state.tombstone: false` requires a `content_blob_id`.
3. `attachment_id` values are unique.

## `prepare_restore`

```ts
input:  { document_id: string; change: Change; parents: string[];
          target: StoredRevision; host?: object }
output: PreparedRevision
```

The new revision takes `content_blob_id` and `attachments` from `target`, sets `tombstone` to `false`, and uses `parents` as supplied, so history moves forward and no head is rewound.
`state.host` is `host` when supplied and `target`'s host otherwise. Supplying it lets a document restored after a rename keep its current path.

Rules:

1. `parents` holds 1 or more entries: the current heads.
2. `target` verifies as a `StoredRevision` and carries the same `document_id`.
3. `target` is not a tombstone. To undelete, restore the last non-tombstone revision.

## `prepare_conflict_resolution`

```ts
input:  { document_id: string; change: Change; heads: StoredRevision[];
          content_blob_id: string | null; attachments: Attachment[]; state: State }
output: PreparedRevision
```

The new revision's `parents` are the `revision_id`s of all `heads`. Passing the stored heads instead of bare ids lets cha check the two conditions the resolution depends on.

Rules:

1. `heads` holds 2 or more entries with distinct `revision_id`s.
2. Each head verifies as a `StoredRevision` and carries the same `document_id`.
3. `change.id` differs from the `change_id` of every head.
4. Revision rules 2 and 3 of `prepare_revision` apply. A tombstone state resolves the conflict as a deletion.

## `semantic_diff`

```ts
type Node = { kind: string; attributes?: object; text?: string | null; children?: Node[] };

input:  { old: Node; new: Node }
output: { events: Event[] }
```

`kind` is a non-empty opaque string. A missing `attributes` equals `{}`, a missing `text` equals `null`, a missing `children` equals `[]`. `""` and `null` are different texts. Unknown node fields are rejected.

A path is the array of child indices from the root; the root is `[]`.

```ts
type Event =
  | { type: "NodeAdded";        path: number[]; kind: string }                  // path in new
  | { type: "NodeRemoved";      path: number[]; kind: string }                  // path in old
  | { type: "NodeModified";     old_path: number[]; new_path: number[]; kind: string }
  | { type: "AttributeChanged"; old_path: number[]; new_path: number[]; name: string;
      old_value?: unknown; new_value?: unknown }                                // key absent = attribute absent
  | { type: "TextChanged";      old_path: number[]; new_path: number[];
      old_text: string | null; new_text: string | null }
  | { type: "ChildAdded";       parent_old_path: number[]; parent_new_path: number[];
      index: number; kind: string }                                             // index in new parent
  | { type: "ChildRemoved";     parent_old_path: number[]; parent_new_path: number[];
      index: number; kind: string };                                            // index in old parent
```

### Matching

The fingerprint of a node is the SHA-256 of the canonical JSON of its normalized subtree. Fingerprints are recomputed on every call and never persisted.

The roots match when their kinds are equal. For each matched pair, the two child lists are aligned in three stages, each of which preserves sibling order:

1. **Exact.** A longest common subsequence over the fingerprint sequences. With `L[i][j]` the LCS length of `old[i..]` and `new[j..]`, walk from `(0,0)`: equal fingerprints match and advance both; otherwise advance `old` when `L[i+1][j] >= L[i][j+1]`, else advance `new`. Exactly matched subtrees are identical and produce no events.
2. **Structural.** The exact matches split the lists into gaps. A gap whose old run and new run have the same length and the same kind at every position is paired position by position.
3. **Text similarity.** In any other gap, each unmatched old node, in order, takes the most similar unmatched new node of the same kind at or after the gap cursor, provided similarity is at least 1/2; ties go to the lowest index, and the cursor moves past the chosen node. Similarity is the Dice coefficient over multisets of character bigrams of the subtree text (own text followed by descendants' text in pre-order; a one-character text is a single gram; an empty text has similarity 0). It is compared in integer arithmetic.

Nodes left unmatched are removed or added. A node that moves between parents, or past an exact match, is reported as a removal plus an addition.

### Event order

For a matched pair that is not identical:

1. if own attributes or own text differ: `NodeModified`, then one `AttributeChanged` per differing attribute in key order, then `TextChanged`;
2. then the children, walking both lists from the left: an unmatched old child first (`ChildRemoved`, then `NodeRemoved` for every node of its subtree in pre-order), otherwise an unmatched new child (`ChildAdded`, then `NodeAdded` for every node of its subtree in pre-order), otherwise recurse into the matched pair.

When the root kinds differ, the output is `NodeRemoved` for every old node in pre-order followed by `NodeAdded` for every new node in pre-order.

## `prepare_operation`

```ts
input:  { operation_id: string; actor_id: string;
          changes: { document_id: string; change_id: string }[] }
output: { operation: { operation_id: string; actor_id: string;
                       changes: { document_id: string; change_id: string }[] };  // sorted by document_id
          multi_document: boolean }
```

An Operation groups document-local Changes and is the only place ActorId appears. Its output is a normalized record for the host to persist; it is not hashed and does not feed any RevisionId.

Rules:

1. `changes` holds at least one entry.
2. Each `document_id` appears once, so a Change never spans documents and per-document application stays idempotent by `(operation_id, document_id)`.
3. Each `change_id` appears once.

## Build and artifacts

```sh
mise install                                 # wasm-bindgen CLI, pinned in mise.toml to the crate version
rustup target add wasm32-unknown-unknown
./scripts/build-wasm.sh                      # writes pkg/
node tests/wasm/run_vectors.mjs              # runs every vector against pkg/
```

| Path | Content |
|---|---|
| `pkg/cha_wasm.js`, `pkg/cha_wasm_bg.wasm`, `pkg/cha_wasm.d.ts` | wasm-bindgen `--target web` output |
| `tests/vectors/*.json` | contract test vectors |

## Test vectors

One file per function: `blob_id.json`, `prepare_revision.json`, `prepare_restore.json`, `prepare_conflict_resolution.json`, `semantic_diff.json`, `prepare_operation.json`.

```json
{
  "abi": "cha-abi/1",
  "function": "prepare_revision",
  "cases": [
    { "name": "root revision", "input": { }, "expect": { "ok": { } } },
    { "name": "closed change", "input": { }, "expect": { "error": { "code": "CHANGE_CLOSED", "context": { } } } }
  ]
}
```

To run a case, call the function with `JSON.stringify(input)` and deep-compare `JSON.parse(result)` with `expect.ok`, or compare the thrown error's `code` and `context` with `expect.error`.
`blob_id.json` differs only in its input and output: `input` is `{ "bytes_hex": "…" }`, to be decoded into a `Uint8Array`, and `expect.ok` is the BlobId string.
