extern crate arraybuffers;

use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;
wasm_bindgen_test_configure!(run_in_browser);
use wasm_mt::prelude::FnOnce;
use wasm_mt::{console_ln, exec, WasmMt};
use wasm_mt_test::get_arraybuffers;
use web_sys::console;

#[wasm_bindgen_test]
async fn handle_test() {
    let (ab_js, ab_wasm) = get_arraybuffers().await.unwrap();
    let wasmmt = WasmMt::new_with_arraybuffers(ab_js, ab_wasm)
        .and_init()
        .await
        .unwrap();
    let th = wasmmt.thread().and_init().await.unwrap();

    let ans = exec!(th, async move || Ok(JsValue::from(42)))
        .await
        .unwrap();
    console_ln!("ans: {:?}", ans);
    assert_eq!(ans, JsValue::from(42));
}
