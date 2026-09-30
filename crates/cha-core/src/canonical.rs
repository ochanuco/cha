use serde_json::Value;
use std::cmp::Ordering;

pub const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

/// Orders strings by UTF-16 code units, as RFC 8785 requires. This differs from
/// UTF-8 byte order for supplementary-plane characters versus U+E000..U+FFFF.
pub fn cmp_utf16(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// Parses JSON and rejects any number that is not an integer in
/// `[-(2^53-1), 2^53-1]`, anywhere in the document.
pub fn parse_strict(text: &str) -> Option<Value> {
    let value: Value = serde_json::from_str(text).ok()?;
    numbers_are_safe(&value).then_some(value)
}

fn numbers_are_safe(value: &Value) -> bool {
    match value {
        Value::Number(n) => match (n.as_i64(), n.as_u64()) {
            (Some(i), _) => (-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&i),
            (None, Some(u)) => u <= MAX_SAFE_INTEGER as u64,
            (None, None) => false,
        },
        Value::Array(items) => items.iter().all(numbers_are_safe),
        Value::Object(map) => map.values().all(numbers_are_safe),
        _ => true,
    }
}

/// Serializes a value whose numbers were already validated by `parse_strict`
/// or built from integers.
pub fn to_canonical(value: &Value) -> String {
    let mut out = String::new();
    write_value(&mut out, value);
    out
}

fn write_value(out: &mut String, value: &Value) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => out.push_str(&n.to_string()),
        Value::String(s) => write_string(out, s),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(out, item);
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_by(|a, b| cmp_utf16(a.0, b.0));
            out.push('{');
            for (i, (key, item)) in entries.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(out, key);
                out.push(':');
                write_value(out, item);
            }
            out.push('}');
        }
    }
}

pub fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sorts_keys_and_drops_whitespace() {
        let v = parse_strict(r#"{ "b": [1, 2], "a": {"z": null, "y": true} }"#).unwrap();
        assert_eq!(to_canonical(&v), r#"{"a":{"y":true,"z":null},"b":[1,2]}"#);
    }

    #[test]
    fn keys_sort_by_utf16_not_utf8() {
        let v = json!({"\u{ff5e}": 1, "\u{1f600}": 2});
        assert_eq!(to_canonical(&v), "{\"\u{1f600}\":2,\"\u{ff5e}\":1}");
    }

    #[test]
    fn escapes_follow_jcs() {
        let v = json!("a\"b\\c\u{8}\u{c}\n\r\t\u{1}\u{1f}\u{7f}\u{2028}é");
        assert_eq!(
            to_canonical(&v),
            "\"a\\\"b\\\\c\\b\\f\\n\\r\\t\\u0001\\u001f\u{7f}\u{2028}é\""
        );
    }

    #[test]
    fn rejects_non_integer_and_out_of_range_numbers() {
        for bad in [
            "1.5",
            "1.0",
            "1e3",
            "1E2",
            "-0.0",
            "9007199254740992",
            "-9007199254740992",
            "18446744073709551615",
            "[1,[2,{\"a\":0.5}]]",
            "",
            "{",
            "[1,]",
            "1 2",
        ] {
            assert!(parse_strict(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn accepts_boundary_integers() {
        let v = parse_strict("[9007199254740991,-9007199254740991,0]").unwrap();
        assert_eq!(to_canonical(&v), "[9007199254740991,-9007199254740991,0]");
    }

    #[test]
    fn strings_are_not_normalized() {
        let v = json!(["e\u{301}", "\u{e9}"]);
        assert_eq!(to_canonical(&v), "[\"e\u{301}\",\"\u{e9}\"]");
    }
}
