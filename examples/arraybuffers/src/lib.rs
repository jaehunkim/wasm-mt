use js_sys::ArrayBuffer;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use wasm_mt::prelude::*;
use wasm_mt::utils::{console_ln, fetch_as_arraybuffer};

#[wasm_bindgen]
pub fn app() {
    spawn_local(async move {
        let ab_js = fetch_as_arraybuffer("./pkg/arraybuffers.js").await.unwrap();
        let ab_wasm = fetch_as_arraybuffer("./pkg/arraybuffers_bg.wasm")
            .await
            .unwrap();
        run(ab_js, ab_wasm).await.unwrap();
    });
}

pub async fn run(ab_js: ArrayBuffer, ab_wasm: ArrayBuffer) -> Result<(), JsValue> {
    let mt = WasmMt::new_with_arraybuffers(ab_js, ab_wasm)
        .and_init()
        .await?;
    let th = mt.thread().and_init().await?;

    let ans = exec!(th, async move || Ok(JsValue::from(42))).await;
    console_ln!("ans: {:?}", ans);
    assert_eq!(ans, Ok(JsValue::from(42)));

    Ok(())
}
