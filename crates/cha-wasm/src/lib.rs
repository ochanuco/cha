use cha_core::{api, ChaError};
use js_sys::{Error, Object, Reflect, JSON};
use wasm_bindgen::prelude::*;

fn to_js(err: ChaError) -> JsValue {
    let js = Error::new(&err.message());
    js.set_name("ChaError");
    let context = JSON::parse(&err.context().to_string()).unwrap_or_else(|_| Object::new().into());
    let _ = Reflect::set(&js, &"code".into(), &err.code().into());
    let _ = Reflect::set(&js, &"context".into(), &context);
    js.into()
}

fn forward(result: Result<String, ChaError>) -> Result<String, JsValue> {
    result.map_err(to_js)
}

#[wasm_bindgen]
pub fn abi_version() -> String {
    api::abi_version().to_string()
}

#[wasm_bindgen]
pub fn blob_id(bytes: &[u8]) -> String {
    api::blob_id(bytes)
}

#[wasm_bindgen]
pub fn prepare_revision(input: &str) -> Result<String, JsValue> {
    forward(api::prepare_revision(input))
}

#[wasm_bindgen]
pub fn prepare_restore(input: &str) -> Result<String, JsValue> {
    forward(api::prepare_restore(input))
}

#[wasm_bindgen]
pub fn prepare_conflict_resolution(input: &str) -> Result<String, JsValue> {
    forward(api::prepare_conflict_resolution(input))
}

#[wasm_bindgen]
pub fn semantic_diff(input: &str) -> Result<String, JsValue> {
    forward(api::semantic_diff(input))
}

#[wasm_bindgen]
pub fn prepare_operation(input: &str) -> Result<String, JsValue> {
    forward(api::prepare_operation(input))
}
