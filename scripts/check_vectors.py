#!/usr/bin/env python3
"""Independent check of tests/vectors: no Rust involved.

For every `ok` case of the prepare_* functions the canonical string must be
canonical JSON (RFC 8785, integers only), `sha256(canonical)` must equal
`revision_id`, and `revision` must be the parse of `canonical`. Stored
revisions in the inputs of `ok` cases are checked the same way. `blob_id`
vectors are recomputed with hashlib.
"""
import hashlib
import json
import sys
from pathlib import Path

VECTORS = Path(__file__).resolve().parent.parent / "tests" / "vectors"


def escape(s):
    out = ['"']
    for c in s:
        if c == '"':
            out.append('\\"')
        elif c == "\\":
            out.append("\\\\")
        elif c == "\b":
            out.append("\\b")
        elif c == "\f":
            out.append("\\f")
        elif c == "\n":
            out.append("\\n")
        elif c == "\r":
            out.append("\\r")
        elif c == "\t":
            out.append("\\t")
        elif ord(c) < 0x20:
            out.append("\\u%04x" % ord(c))
        else:
            out.append(c)
    out.append('"')
    return "".join(out)


def canon(v):
    if v is None:
        return "null"
    if v is True:
        return "true"
    if v is False:
        return "false"
    if isinstance(v, int):
        assert abs(v) <= 2**53 - 1
        return str(v)
    if isinstance(v, str):
        return escape(v)
    if isinstance(v, list):
        return "[" + ",".join(canon(x) for x in v) + "]"
    if isinstance(v, dict):
        keys = sorted(v, key=lambda k: k.encode("utf-16-be"))
        return "{" + ",".join(escape(k) + ":" + canon(v[k]) for k in keys) + "}"
    raise TypeError(type(v))


def sha256_id(text):
    return "sha256:" + hashlib.sha256(text.encode("utf-8")).hexdigest()


def check_prepared(where, prepared):
    canonical = prepared["canonical"]
    assert prepared["revision_id"] == sha256_id(canonical), where + ": revision_id"
    revision = json.loads(canonical)
    assert prepared["revision"] == revision, where + ": revision"
    assert canon(revision) == canonical, where + ": not canonical"


def main():
    checked = 0
    for path in sorted(VECTORS.glob("*.json")):
        doc = json.loads(path.read_text(encoding="utf-8"))
        function = doc["function"]
        assert path.stem == function
        for case in doc["cases"]:
            where = "%s/%s" % (function, case["name"])
            ok = case["expect"].get("ok")
            if function == "blob_id" and ok is not None:
                data = bytes.fromhex(case["input"]["bytes_hex"])
                assert ok == "sha256:" + hashlib.sha256(data).hexdigest(), where
                checked += 1
            elif function.startswith("prepare_r") or function == "prepare_conflict_resolution":
                if ok is None:
                    continue
                check_prepared(where, ok)
                stored = list(case["input"].get("heads", []))
                if "target" in case["input"]:
                    stored.append(case["input"]["target"])
                for s in stored:
                    check_prepared(where + " (stored)", dict(s, revision=json.loads(s["canonical"])))
                checked += 1
    print("verified %d cases" % checked)


if __name__ == "__main__":
    try:
        main()
    except AssertionError as e:
        print("FAIL:", e)
        sys.exit(1)
