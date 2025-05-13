extern crate fib;

use js_sys::SharedArrayBuffer;
use serde::Deserialize;
use serde::Serialize;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;
use wasm_mt::prelude::*;
wasm_bindgen_test_configure!(run_in_browser);

use wasm_mt::console_ln;
use wasm_mt::utils::sleep;
use wasm_mt::Thread;
use wasm_mt::WasmMt;
use wasm_mt_pool::{pool_exec, ThreadPool};
use wasm_mt_test::get_arraybuffers;

use js_sys::{Map, Number, Reflect};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

// Helper: grab or create the global Map at `self.__SAB_MAP__`
fn global_sab_map() -> Map {
    // `js_sys::global()` gives you `self` (worker) or `window`
    let global = js_sys::global();
    let key = JsValue::from_str("__SAB_MAP__");
    // if it already exists, grab it
    if Reflect::has(&global, &key).unwrap() {
        Reflect::get(&global, &key).unwrap().unchecked_into::<Map>()
    } else {
        // otherwise create + store it
        let map = Map::new();
        Reflect::set(&global, &key, map.as_ref()).unwrap();
        map
    }
}

// Helper: grab or init a numeric counter at `self.__NEXT_SAB_ID__`
fn global_next_id() -> u32 {
    let global = js_sys::global();
    let key = JsValue::from_str("__NEXT_SAB_ID__");
    if Reflect::has(&global, &key).unwrap() {
        Reflect::get(&global, &key).unwrap().as_f64().unwrap() as u32
    } else {
        // set to 1 if missing
        Reflect::set(&global, &key, &JsValue::from_f64(1.0)).unwrap();
        1
    }
}

fn bump_next_id() -> u32 {
    let global = js_sys::global();
    let key = JsValue::from_str("__NEXT_SAB_ID__");
    let cur = global_next_id();
    let next = cur + 1;
    Reflect::set(&global, &key, &JsValue::from_f64(next as f64)).unwrap();
    cur
}

pub fn register_sab_in_global_map(sab: SharedArrayBuffer) -> u32 {
    let id = bump_next_id();
    let map = global_sab_map();
    map.set(&Number::from(id).into(), sab.as_ref());
    id
}

pub fn lookup_sab_from_global_map(id: u32) -> SharedArrayBuffer {
    let map = global_sab_map();
    let key = Number::from(id).into();

    // 1) guard with `has()`
    if !map.has(&key) {
        panic!("No SharedArrayBuffer registered under ID {}", id);
    }

    // 2) now we know `get()` won’t be undefined
    let val = map.get(&key);
    val.unchecked_into::<SharedArrayBuffer>()
}

#[derive(Serialize, Deserialize)]
struct SabHandle(u32);

impl From<SharedArrayBuffer> for SabHandle {
    fn from(sab: SharedArrayBuffer) -> SabHandle {
        // register it in a global map and hand out the index
        SabHandle(register_sab_in_global_map(sab))
    }
}

impl Into<SharedArrayBuffer> for SabHandle {
    fn into(self) -> SharedArrayBuffer {
        // look up the real buffer in the worker’s map
        lookup_sab_from_global_map(self.0)
    }
}

#[wasm_bindgen_test]
async fn thread_test() {
    let (ab_js, ab_wasm) = get_arraybuffers().await.unwrap();
    let th = WasmMt::new_with_arraybuffers(ab_js, ab_wasm)
        .and_init()
        .await
        .unwrap();

    let ans = exec!(th, move || {
        //sleep(1000).await;
        Ok(JsValue::from(42))
    });
}

#[wasm_bindgen_test]
async fn app() {
    let (ab_js, ab_wasm) = get_arraybuffers().await.unwrap();

    let threadpool = ThreadPool::new_with_arraybuffers(4, ab_js, ab_wasm)
        .and_init()
        .await
        .unwrap();

    let num = 8;
    let sender_sab = SharedArrayBuffer::new(2);
    //let receiver_sab = SharedArrayBuffer::new(2);

    console_ln!("b) 💦 pool_exec! {} async closures:", num);
    for i in 0..num {
        //let sab_handle = SabHandle::from(sender_sab.clone());
        pool_exec!(
            threadpool,
            async move || {
                if i == 0 {
                    //let sab: SharedArrayBuffer = sab_handle.into();
                    console_ln!("b) async closure {} : wait for request sab start.", i);
                    //let request_flag = js_sys::Int32Array::new(&sab);
                    //let outcome = js_sys::Atomics::wait(&request_flag, 0, 0).unwrap();
                    console_ln!("b) async closure {} : wait for request sab done.", i);
                }

                console_ln!("b) async closure: wait start.");
                sleep(1000).await;
                console_ln!("b) async closure: done.");
                Ok(JsValue::NULL)
            },
            move |result| {
                console_ln!("b) 💦 after pool exec! callback called.");
            }
        );
        console_ln!("b) pending jobs: {}", threadpool.count_pending_jobs());
    }

    // want to blocking wait here
    // console_ln!("b) 💦 pool_exec! notify request sab start.");
    // let request_flag = js_sys::Int32Array::new(&sender_sab);
    // js_sys::Atomics::store(&request_flag, 0, 1).unwrap();
    // js_sys::Atomics::notify(&request_flag, 0).unwrap();
    console_ln!("b) 💦 pool_exec! notify request sab done.");
    console_ln!("b) pending jobs: {}", threadpool.count_pending_jobs());
    console_ln!("b) 💦 pool_exec! waiting for all jobs to complete.");
    sleep(10_000).await; // Do sleep long enough to ensure all jobs are completed.
    assert_eq!(threadpool.count_pending_jobs(), 0);
    console_ln!("b) 💦 pool_exec! done.");
}
