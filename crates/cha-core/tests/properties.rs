//! Behaviours the vector format cannot express: raw input text and repeated calls.

use cha_core::api;
use serde_json::{json, Value};

const DOC: &str = "01890a5d-ac96-7001-8000-000000000001";
const CHANGE: &str = "01890a5d-ac96-7001-8000-000000000011";
const BLOB: &str = "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

fn revision_input(host: &str) -> String {
    format!(
        r#"{{"document_id":"{DOC}","change":{{"id":"{CHANGE}","state":"open"}},"parents":[],"content_blob_id":"{BLOB}","attachments":[],"state":{{"tombstone":false,"host":{host}}}}}"#
    )
}

fn code(result: Result<String, cha_core::ChaError>) -> &'static str {
    result.unwrap_err().code()
}

#[test]
fn abi_version_is_fixed() {
    assert_eq!(api::abi_version(), "cha-abi/1");
}

#[test]
fn raw_text_that_is_not_json_or_not_integer_json() {
    for bad in [
        "",
        "{",
        "nope",
        "{} trailing",
        "\u{feff}{}",
        &revision_input("{\"x\":1.0}"),
        &revision_input("{\"x\":1e2}"),
        &revision_input("{\"x\":1E+2}"),
        &revision_input("{\"x\":-0.0}"),
        &revision_input("{\"x\":9007199254740993}"),
        &revision_input("{\"x\":18446744073709551616}"),
        &revision_input("{\"x\":[[{\"y\":0.5}]]}"),
    ] {
        assert_eq!(code(api::prepare_revision(bad)), "INVALID_JSON", "{bad}");
    }
}

#[test]
fn integer_boundaries_are_accepted() {
    let out = api::prepare_revision(&revision_input(
        "{\"hi\":9007199254740991,\"lo\":-9007199254740991}",
    ))
    .unwrap();
    assert!(out.contains("\"hi\":9007199254740991,\"lo\":-9007199254740991"));
}

#[test]
fn whitespace_and_key_order_of_the_input_do_not_change_the_output() {
    let compact = api::prepare_revision(&revision_input("{\"a\":1,\"b\":2}")).unwrap();
    let spaced = revision_input("{ \"b\" : 2 , \"a\" : 1 }").replace(',', " , ");
    assert_eq!(api::prepare_revision(&spaced).unwrap(), compact);
}

#[test]
fn outputs_are_canonical_and_repeatable() {
    let input = revision_input("{\"path\":\"x\"}");
    let first = api::prepare_revision(&input).unwrap();
    assert_eq!(first, api::prepare_revision(&input).unwrap());
    let parsed: Value = serde_json::from_str(&first).unwrap();
    assert_eq!(cha_core::canonical::to_canonical(&parsed), first);
    assert_eq!(
        cha_core::canonical::to_canonical(&parsed["revision"]),
        parsed["canonical"].as_str().unwrap()
    );
}

#[test]
fn diff_is_deterministic() {
    let input = json!({
        "old": {"kind": "doc", "children": [
            {"kind": "p", "text": "alpha"}, {"kind": "p", "text": "beta"}, {"kind": "p", "text": "gamma"}]},
        "new": {"kind": "doc", "children": [
            {"kind": "p", "text": "beta"}, {"kind": "p", "text": "gamma!"}, {"kind": "h", "text": "delta"}]},
    })
    .to_string();
    let first = api::semantic_diff(&input).unwrap();
    for _ in 0..5 {
        assert_eq!(api::semantic_diff(&input).unwrap(), first);
    }
}

#[test]
fn restore_output_is_a_forward_revision() {
    let root = api::prepare_revision(&revision_input("{\"path\":\"a\"}")).unwrap();
    let root: Value = serde_json::from_str(&root).unwrap();
    let head = "sha256:".to_string() + &"1".repeat(64);
    let input = json!({
        "document_id": DOC,
        "change": {"id": "01890a5d-ac96-7001-8000-000000000012", "state": "open"},
        "parents": [head],
        "target": {"revision_id": root["revision_id"], "canonical": root["canonical"]},
    })
    .to_string();
    let out: Value = serde_json::from_str(&api::prepare_restore(&input).unwrap()).unwrap();
    assert_eq!(out["revision"]["parents"], json!([head]));
    assert_eq!(out["revision"]["state"]["tombstone"], false);
    assert_eq!(out["revision"]["content_blob_id"], BLOB);
    assert_ne!(out["revision_id"], root["revision_id"]);
}
