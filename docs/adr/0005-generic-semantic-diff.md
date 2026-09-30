# ADR-0005: Semantic diff operates on generic typed trees
Status: Accepted

cha receives generic typed-tree IR and does not parse Markdown or OKF.
PoC matching uses deterministic staged matching with ephemeral fingerprints.
Diff output consists of generic change events.
