//! Shape validation helpers shared by the `api` functions. Every failure here
//! is an `INVALID_INPUT` (or `INVALID_JSON`) carrying the dotted field path.

use crate::canonical::parse_strict;
use crate::errors::ChaError;
use serde_json::{Map, Value};

pub type Object = Map<String, Value>;

/// A string taken from the input together with its dotted path, so a later
/// identifier check can report where it came from.
pub struct Tagged<'a> {
    pub value: &'a str,
    pub field: String,
}

pub fn parse(text: &str) -> Result<Value, ChaError> {
    parse_strict(text).ok_or(ChaError::InvalidJson)
}

pub fn join(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_string()
    } else {
        format!("{path}.{key}")
    }
}

fn invalid(field: String) -> ChaError {
    ChaError::InvalidInput { field }
}

/// `value` must be an object whose keys are all in `allowed`.
pub fn object<'a>(value: &'a Value, path: &str, allowed: &[&str]) -> Result<&'a Object, ChaError> {
    let map = value.as_object().ok_or_else(|| invalid(path.to_string()))?;
    match map.keys().find(|k| !allowed.contains(&k.as_str())) {
        Some(unknown) => Err(invalid(join(path, unknown))),
        None => Ok(map),
    }
}

pub fn field<'a>(map: &'a Object, path: &str, key: &str) -> Result<&'a Value, ChaError> {
    map.get(key).ok_or_else(|| invalid(join(path, key)))
}

pub fn string<'a>(map: &'a Object, path: &str, key: &str) -> Result<Tagged<'a>, ChaError> {
    let value = field(map, path, key)?
        .as_str()
        .ok_or_else(|| invalid(join(path, key)))?;
    Ok(Tagged {
        value,
        field: join(path, key),
    })
}

pub fn boolean(map: &Object, path: &str, key: &str) -> Result<bool, ChaError> {
    field(map, path, key)?
        .as_bool()
        .ok_or_else(|| invalid(join(path, key)))
}

pub fn array<'a>(map: &'a Object, path: &str, key: &str) -> Result<&'a [Value], ChaError> {
    field(map, path, key)?
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| invalid(join(path, key)))
}

pub fn object_field<'a>(map: &'a Object, path: &str, key: &str) -> Result<&'a Object, ChaError> {
    field(map, path, key)?
        .as_object()
        .ok_or_else(|| invalid(join(path, key)))
}

pub fn strings<'a>(map: &'a Object, path: &str, key: &str) -> Result<Vec<Tagged<'a>>, ChaError> {
    array(map, path, key)?
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let field = join(&join(path, key), &i.to_string());
            item.as_str()
                .map(|value| Tagged {
                    value,
                    field: field.clone(),
                })
                .ok_or_else(|| invalid(field))
        })
        .collect()
}
