pub mod builder;

pub mod util;
pub mod solver;
pub mod test;

use crate::util::DFA;
use crate::util::*;

pub fn wbf_fix<S : 'static, F: std::future::Future<Output = S> + 'static>(f: F) -> S {
    
    let ret_channel = std::sync::mpsc::channel();
    let retless_func = async move {
        //web_sys::console::log_1(&"Async function with return value started...".into());
        let result = f.await;
        //web_sys::console::log_1(&"Async function with return value finished.".into());
        ret_channel.0.send(result).unwrap();
        //web_sys::console::log_1(&"Async return value sent to sync thread".into());
    };
    execute(retless_func);
    //web_sys::console::log_1(&"Sync thread waiting for async...".into());
    let result = ret_channel.1.recv();
    //web_sys::console::log_1(&"Sync thread received return value".into());
    result.unwrap()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn execute<F: std::future::Future<Output = ()> + 'static>(f: F) {
    futures::executor::block_on(f);
}

#[cfg(target_arch = "wasm32")]
pub fn execute<F: std::future::Future<Output = ()> + 'static>(f: F) {
    wasm_bindgen_futures::spawn_local(f);
}