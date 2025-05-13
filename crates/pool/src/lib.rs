#![feature(trait_alias)]

use js_sys::ArrayBuffer;
pub use wasm_mt;
use wasm_mt::{debug_ln, MtAsyncClosure, MtClosure, Thread, WasmMt};

pub mod prelude;
mod resolver;
use resolver::Resolver;

use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

type ResultJJ = Result<JsValue, JsValue>;

pub trait PoolCallback = FnOnce(ResultJJ) -> () + 'static;

struct ThreadPoolInner {
    size: usize,
    mt: WasmMt,
    threads: RefCell<Vec<Thread>>,
    resolver: Resolver,
}

impl ThreadPoolInner {
    fn new(size: usize, pkg_js_uri: &str) -> Self {
        assert!(size > 0);
        Self {
            size,
            mt: WasmMt::new(pkg_js_uri),
            threads: RefCell::new(Vec::with_capacity(size)),
            resolver: Resolver::new(),
        }
    }

    fn new_with_arraybuffers(size: usize, ab_js: ArrayBuffer, ab_wasm: ArrayBuffer) -> Self {
        assert!(size > 0);
        Self {
            size,
            mt: WasmMt::new_with_arraybuffers(ab_js, ab_wasm),
            threads: RefCell::new(Vec::with_capacity(size)),
            resolver: Resolver::new(),
        }
    }

    async fn init(&self) -> Result<(), JsValue> {
        let mut threads = self.threads.borrow_mut();

        self.mt.init().await?;
        for id in 0..self.size {
            let pth = self.mt.thread();
            pth.set_id(&id.to_string());
            threads.push(pth);
        }

        for pth in threads.iter() {
            pth.init().await?;
        }

        Ok(())
    }

    async fn execute<F>(&self, clos: F) -> ResultJJ
    where
        F: MtClosure,
    {
        let threads = self.threads.borrow();
        let pth = self.resolver.resolve_runnable(&threads).await?;

        let result = pth.exec(clos).await;
        debug_ln!(
            "pth {} done with result: {:?}",
            pth.get_id().unwrap(),
            result
        );
        self.resolver.notify_job_complete(pth);
        result
    }

    async fn execute_async<F, T>(&self, aclos: F) -> ResultJJ
    where
        F: MtAsyncClosure<T>,
    {
        let threads = self.threads.borrow();
        let pth = self.resolver.resolve_runnable(&threads).await?;

        let result = pth.exec_async(aclos).await;
        debug_ln!(
            "pth {} done with result: {:?}",
            pth.get_id().unwrap(),
            result
        );
        self.resolver.notify_job_complete(pth);
        result
    }

    async fn execute_js(&self, js: &str, is_async: bool) -> ResultJJ {
        let threads = self.threads.borrow();
        let pth = self.resolver.resolve_runnable(&threads).await?;

        let result = if is_async {
            pth.exec_js_async(js).await
        } else {
            pth.exec_js(js).await
        };
        debug_ln!(
            "pth {} done with result: {:?}",
            pth.get_id().unwrap(),
            result
        );
        self.resolver.notify_job_complete(pth);
        result
    }

    fn drop_inner(&self) {
        debug_ln!("[drop] drop_inner(): terminating {} workers ...", self.size);
        self.resolver.cancel_pending_jobs();
        self.threads.borrow().iter().for_each(|pth| pth.terminate());
    }
}

#[macro_export]
macro_rules! pool_exec {
    ($pool:expr, async $clos:expr) => (($pool).exec_async(FnOnce!(async $clos)));
    ($pool:expr, $clos:expr) => (($pool).exec(FnOnce!($clos)));
    ($pool:expr, async $clos:expr, $cb:expr) => (($pool).exec_async_with_cb(FnOnce!(async $clos), $cb));
    ($pool:expr, $clos:expr, $cb:expr) => (($pool).exec_with_cb(FnOnce!($clos), $cb));
}

#[macro_export]
macro_rules! pool_exec_js {
    ($pool:expr, $str:expr) => {
        ($pool).exec_js($str)
    };
    ($pool:expr, $str:expr, $cb:expr) => {
        ($pool).exec_js_with_cb($str, $cb)
    };
}

#[macro_export]
macro_rules! pool_exec_js_async {
    ($pool:expr, $str:expr) => {
        ($pool).exec_js_async($str)
    };
    ($pool:expr, $str:expr, $cb:expr) => {
        ($pool).exec_js_async_with_cb($str, $cb)
    };
}

pub struct ThreadPool(Rc<ThreadPoolInner>);

impl Drop for ThreadPool {
    fn drop(&mut self) {
        debug_ln!(
            "[drop] ThreadPool::drop(): sc: {}",
            Rc::strong_count(&self.0)
        );
        self.0.drop_inner();
    }
}

impl ThreadPool {
    pub fn new(size: usize, pkg_js_uri: &str) -> Self {
        Self(Rc::new(ThreadPoolInner::new(size, pkg_js_uri)))
    }

    pub fn new_with_arraybuffers(size: usize, ab_js: ArrayBuffer, ab_wasm: ArrayBuffer) -> Self {
        Self(Rc::new(ThreadPoolInner::new_with_arraybuffers(
            size, ab_js, ab_wasm,
        )))
    }

    pub fn set_ab_init(&self, ab: ArrayBuffer) {
        self.0.mt.set_ab_init(ab);
    }

    pub async fn init(&self) -> Result<&Self, JsValue> {
        self.0.init().await?;
        Ok(self)
    }

    pub async fn and_init(self) -> Result<Self, JsValue> {
        self.init().await?;
        Ok(self)
    }

    pub fn count_pending_jobs(&self) -> usize {
        self.0.resolver.count_pending_jobs()
    }

    fn drop_cb_result(_: ResultJJ) {}

    pub fn exec<F>(&self, job: F)
    where
        F: MtClosure,
    {
        self.exec_with_cb(job, Self::drop_cb_result);
    }
    pub fn exec_async<F, T>(&self, job: F)
    where
        F: MtAsyncClosure<T>,
    {
        self.exec_async_with_cb(job, Self::drop_cb_result);
    }
    pub fn exec_with_cb<F, G>(&self, job: F, cb: G)
    where
        F: MtClosure,
        G: PoolCallback,
    {
        let pool_inner = self.0.clone();
        spawn_local(async move {
            cb(pool_inner.execute(job).await);
        });
    }
    pub fn exec_async_with_cb<F, T, G>(&self, job: F, cb: G)
    where
        F: MtAsyncClosure<T>,
        G: PoolCallback,
    {
        let pool_inner = self.0.clone();
        spawn_local(async move {
            cb(pool_inner.execute_async(job).await);
        });
    }

    pub fn exec_js(&self, js: &str) {
        self.exec_js_inner(js, false, Self::drop_cb_result);
    }
    pub fn exec_js_async(&self, js: &str) {
        self.exec_js_inner(js, true, Self::drop_cb_result);
    }
    pub fn exec_js_with_cb<G>(&self, js: &str, cb: G)
    where
        G: PoolCallback,
    {
        self.exec_js_inner(js, false, cb);
    }
    pub fn exec_js_async_with_cb<G>(&self, js: &str, cb: G)
    where
        G: PoolCallback,
    {
        self.exec_js_inner(js, true, cb);
    }
    fn exec_js_inner<G>(&self, js: &str, is_async: bool, cb: G)
    where
        G: PoolCallback,
    {
        let pool_inner = self.0.clone();
        let js = js.to_string();
        spawn_local(async move {
            cb(pool_inner.execute_js(js.as_str(), is_async).await);
        });
    }
}
