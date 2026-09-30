# ADR-0003: Separate Change, Revision, and Operation
Status: Accepted

Change is a document-local semantic editing unit with stable UUIDv7 identity.
Revision is an immutable snapshot node.
Operation records repository mutation/action and owns ActorId.
A multi-document Operation groups separate document-local Changes.
