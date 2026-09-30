use cha_core::api;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

const FUNCTIONS: [&str; 6] = [
    "blob_id",
    "prepare_revision",
    "prepare_restore",
    "prepare_conflict_resolution",
    "semantic_diff",
    "prepare_operation",
];

fn vector_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/vectors")
}

fn decode_hex(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}

fn call(function: &str, input: &Value) -> Result<Value, cha_core::ChaError> {
    if function == "blob_id" {
        let hex = input["bytes_hex"].as_str().unwrap();
        return Ok(Value::String(api::blob_id(&decode_hex(hex))));
    }
    let text = serde_json::to_string(input).unwrap();
    let out = match function {
        "prepare_revision" => api::prepare_revision(&text),
        "prepare_restore" => api::prepare_restore(&text),
        "prepare_conflict_resolution" => api::prepare_conflict_resolution(&text),
        "semantic_diff" => api::semantic_diff(&text),
        "prepare_operation" => api::prepare_operation(&text),
        other => panic!("unknown function {other}"),
    }?;
    Ok(serde_json::from_str(&out).unwrap())
}

#[test]
fn every_vector_file_passes() {
    let mut seen = Vec::new();
    let mut cases_run = 0;
    for function in FUNCTIONS {
        let path = vector_dir().join(format!("{function}.json"));
        let doc: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(doc["abi"], "cha-abi/1");
        assert_eq!(doc["function"], function);
        seen.push(function);
        for case in doc["cases"].as_array().unwrap() {
            let name = format!("{function}: {}", case["name"].as_str().unwrap());
            let result = call(function, &case["input"]);
            let expect = &case["expect"];
            match (result, expect.get("ok"), expect.get("error")) {
                (Ok(actual), Some(ok), None) => assert_eq!(&actual, ok, "{name}"),
                (Err(actual), None, Some(error)) => {
                    assert_eq!(actual.code(), error["code"], "{name}");
                    assert_eq!(actual.context(), error["context"], "{name}");
                }
                (result, _, _) => panic!("{name}: unexpected result {result:?}"),
            }
            cases_run += 1;
        }
    }
    let mut files: Vec<String> = fs::read_dir(vector_dir())
        .unwrap()
        .map(|e| {
            e.unwrap()
                .path()
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    files.sort();
    let mut expected: Vec<&str> = seen.clone();
    expected.sort();
    assert_eq!(files, expected, "unexpected files in tests/vectors");
    assert!(cases_run > 200);
}

#[test]
fn every_error_code_is_covered_by_a_vector() {
    let mut codes = std::collections::BTreeSet::new();
    for function in FUNCTIONS {
        let path = vector_dir().join(format!("{function}.json"));
        let doc: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        for case in doc["cases"].as_array().unwrap() {
            if let Some(code) = case["expect"]["error"]["code"].as_str() {
                codes.insert(code.to_string());
            }
        }
    }
    let expected = [
        "CHANGE_CLOSED",
        "CONFLICT_STATE",
        "INVALID_CANONICAL_STATE",
        "INVALID_DOCUMENT",
        "INVALID_ID",
        "INVALID_INPUT",
        "INVALID_JSON",
        "INVALID_OPERATION",
        "INVALID_PARENT_SET",
        "INVALID_RESTORE_TARGET",
        "INVALID_REVISION",
        "INVALID_TYPED_TREE",
    ];
    assert_eq!(codes.into_iter().collect::<Vec<_>>(), expected);
}
