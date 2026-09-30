use crate::blob::sha256_hex;
use crate::canonical::{cmp_utf16, to_canonical, write_string};
use crate::errors::ChaError;
use serde_json::{Map, Value};

/// A node's path length may not exceed this; the root has path length 0.
pub const MAX_DEPTH: usize = 48;

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub kind: String,
    /// Sorted by key in UTF-16 code unit order.
    pub attributes: Vec<(String, Value)>,
    pub text: Option<String>,
    pub children: Vec<Node>,
    /// SHA-256 hex of the canonical JSON of the normalized subtree.
    pub fingerprint: String,
}

impl Node {
    /// Shape check only (`INVALID_INPUT`). `field` is the dotted path of `value`.
    pub fn from_value(value: &Value, field: &str) -> Result<Self, ChaError> {
        Ok(Self::parse(value, field)?.0)
    }

    fn parse(value: &Value, field: &str) -> Result<(Self, String), ChaError> {
        let invalid = |name: &str| ChaError::InvalidInput {
            field: join(field, name),
        };
        let map = value.as_object().ok_or_else(|| ChaError::InvalidInput {
            field: field.to_string(),
        })?;
        if let Some(unknown) = map
            .keys()
            .find(|k| !matches!(k.as_str(), "kind" | "attributes" | "text" | "children"))
        {
            return Err(invalid(unknown));
        }
        let kind = map
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("kind"))?
            .to_string();
        let empty = Map::new();
        let attributes = match map.get("attributes") {
            None => &empty,
            Some(v) => v.as_object().ok_or_else(|| invalid("attributes"))?,
        };
        let text = match map.get("text") {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) => Some(s.clone()),
            Some(_) => return Err(invalid("text")),
        };
        let raw_children: &[Value] = match map.get("children") {
            None => &[],
            Some(v) => v.as_array().ok_or_else(|| invalid("children"))?,
        };

        let mut children = Vec::with_capacity(raw_children.len());
        let mut child_canonical = Vec::with_capacity(raw_children.len());
        for (i, child) in raw_children.iter().enumerate() {
            let (node, canonical) =
                Self::parse(child, &format!("{}.{i}", join(field, "children")))?;
            children.push(node);
            child_canonical.push(canonical);
        }

        let mut attributes: Vec<(String, Value)> = attributes
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        attributes.sort_by(|a, b| cmp_utf16(&a.0, &b.0));

        let canonical = normalized_canonical(&kind, &attributes, text.as_deref(), &child_canonical);
        let fingerprint = sha256_hex(canonical.as_bytes());
        Ok((
            Self {
                kind,
                attributes,
                text,
                children,
                fingerprint,
            },
            canonical,
        ))
    }

    /// Semantic checks (`INVALID_TYPED_TREE`), in pre-order.
    pub fn validate(&self, field: &str) -> Result<(), ChaError> {
        self.validate_at(field, 0)
    }

    fn validate_at(&self, field: &str, depth: usize) -> Result<(), ChaError> {
        if self.kind.is_empty() || depth > MAX_DEPTH {
            return Err(ChaError::InvalidTypedTree {
                field: field.to_string(),
            });
        }
        for (i, child) in self.children.iter().enumerate() {
            child.validate_at(&format!("{}.{i}", join(field, "children")), depth + 1)?;
        }
        Ok(())
    }

    /// Own text followed by descendants' text in pre-order.
    pub fn subtree_text(&self) -> String {
        let mut out = String::new();
        self.collect_text(&mut out);
        out
    }

    fn collect_text(&self, out: &mut String) {
        if let Some(text) = &self.text {
            out.push_str(text);
        }
        for child in &self.children {
            child.collect_text(out);
        }
    }
}

fn join(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_string()
    } else {
        format!("{path}.{key}")
    }
}

fn normalized_canonical(
    kind: &str,
    attributes: &[(String, Value)],
    text: Option<&str>,
    children: &[String],
) -> String {
    let mut out = String::from("{\"attributes\":{");
    for (i, (key, value)) in attributes.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write_string(&mut out, key);
        out.push(':');
        out.push_str(&to_canonical(value));
    }
    out.push_str("},\"children\":[");
    out.push_str(&children.join(","));
    out.push_str("],\"kind\":");
    write_string(&mut out, kind);
    out.push_str(",\"text\":");
    match text {
        Some(t) => write_string(&mut out, t),
        None => out.push_str("null"),
    }
    out.push('}');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn node(v: Value) -> Node {
        Node::from_value(&v, "old").unwrap()
    }

    #[test]
    fn defaults_and_key_order_do_not_change_the_fingerprint() {
        let a = node(json!({"kind": "p"}));
        let b = node(json!({"kind": "p", "attributes": {}, "text": null, "children": []}));
        assert_eq!(a.fingerprint, b.fingerprint);
        let c = node(json!({"kind": "p", "attributes": {"x": {"b": 1, "a": 2}, "y": 1}}));
        let d = node(json!({"attributes": {"y": 1, "x": {"a": 2, "b": 1}}, "kind": "p"}));
        assert_eq!(c.fingerprint, d.fingerprint);
    }

    #[test]
    fn empty_text_differs_from_null_text() {
        let a = node(json!({"kind": "p", "text": ""}));
        let b = node(json!({"kind": "p"}));
        assert_ne!(a.fingerprint, b.fingerprint);
    }

    #[test]
    fn fingerprint_is_sha256_of_the_normalized_subtree() {
        let n = node(json!({"kind": "p"}));
        let expected = "{\"attributes\":{},\"children\":[],\"kind\":\"p\",\"text\":null}";
        assert_eq!(n.fingerprint, sha256_hex(expected.as_bytes()));
    }

    #[test]
    fn shape_errors_name_the_field() {
        let bad = |v: Value| Node::from_value(&v, "new").unwrap_err().context()["field"].clone();
        assert_eq!(bad(json!({})), "new.kind");
        assert_eq!(bad(json!({"kind": "p", "x": 1})), "new.x");
        assert_eq!(
            bad(json!({"kind": "p", "children": [{"kind": 1}]})),
            "new.children.0.kind"
        );
        assert_eq!(bad(json!({"kind": "p", "text": 1})), "new.text");
        assert_eq!(bad(json!([])), "new");
    }

    #[test]
    fn validates_kind_and_depth() {
        let mut deep = json!({"kind": "p"});
        for _ in 0..MAX_DEPTH {
            deep = json!({"kind": "p", "children": [deep]});
        }
        assert!(node(deep.clone()).validate("old").is_ok());
        let too_deep = json!({"kind": "p", "children": [deep]});
        let err = node(too_deep).validate("old").unwrap_err();
        assert_eq!(err.code(), "INVALID_TYPED_TREE");
        let path = "old".to_string() + &".children.0".repeat(MAX_DEPTH + 1);
        assert_eq!(err.context()["field"], path);

        let empty = node(json!({"kind": "p", "children": [{"kind": ""}]}));
        assert_eq!(
            empty.validate("old").unwrap_err().context()["field"],
            "old.children.0"
        );
    }

    #[test]
    fn subtree_text_is_preorder() {
        let n = node(json!({"kind": "a", "text": "1", "children": [
            {"kind": "b", "text": "2", "children": [{"kind": "c", "text": "3"}]},
            {"kind": "d", "text": "4"}
        ]}));
        assert_eq!(n.subtree_text(), "1234");
    }
}
