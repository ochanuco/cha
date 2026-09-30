# ADR-0002: BlobId and RevisionId are content-addressed
Status: Accepted

BlobId hashes raw bytes exactly as stored and is self-describing; PoC uses SHA-256.
RevisionId hashes canonical JSON of immutable revision state.
ChangeId is included. Timestamp, ActorId, and OperationId are excluded.
