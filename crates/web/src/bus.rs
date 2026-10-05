//! The transport between tabs of one browser: a BroadcastChannel. Every
//! envelope is heard by every other tab; the portal protocol ignores what
//! is not addressed to it. No server is involved.

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{BroadcastChannel, MessageEvent};

pub struct Bus {
    ch: BroadcastChannel,
    _on_message: Closure<dyn FnMut(MessageEvent)>,
}

impl Bus {
    pub fn new(name: &str, mut on_bytes: impl FnMut(Vec<u8>) + 'static) -> Result<Bus, JsValue> {
        let ch = BroadcastChannel::new(name)?;
        let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
            if let Ok(arr) = e.data().dyn_into::<js_sys::Uint8Array>() {
                on_bytes(arr.to_vec());
            }
        });
        ch.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        Ok(Bus {
            ch,
            _on_message: on_message,
        })
    }

    pub fn post(&self, bytes: &[u8]) {
        let _ = self.ch.post_message(&js_sys::Uint8Array::from(bytes));
    }
}
