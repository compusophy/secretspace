//! A socket to a room that stays up: it says the platform Hello first on
//! every open, and when it drops (or the server says `Still`, a deploy) it
//! tries again after 250 ms, then 1.6 times longer each time, jittered,
//! never more than 4 s apart. The page polls it once a frame.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use wasm_bindgen::JsCast;
use web_sys::{BinaryType, MessageEvent, WebSocket};

const FIRST: f64 = 250.0;
const GROW: f64 = 1.6;
const MOST: f64 = 4000.0;

/// What happened since the last poll.
pub enum Net {
    /// Connected, and Hello said: say again whatever the room needs.
    Up,
    /// Lost, or told to hold still: keep the picture, it is coming back.
    Holding,
    /// A message: the platform's `Seen`, or the room's own.
    Message(Vec<u8>),
}

struct Inner {
    room: String,
    counted: bool,
    watch: bool,
    hello: Vec<u8>,
    ws: Option<WebSocket>,
    /// Which socket is current; events from older ones are ignored.
    gen: u32,
    events: VecDeque<Net>,
    delay: f64,
    retry_at: f64,
    up: bool,
}

pub struct Link(Rc<RefCell<Inner>>);

impl Link {
    /// A link to `room`; its first connection counts a visit unless it only
    /// watches (`watch`, which never says Hello).
    pub fn open(room: &str, hello: Vec<u8>, watch: bool) -> Link {
        Link(Rc::new(RefCell::new(Inner {
            room: room.to_string(),
            counted: watch,
            watch,
            hello,
            ws: None,
            gen: 0,
            events: VecDeque::new(),
            delay: FIRST,
            retry_at: 0.0,
            up: false,
        })))
    }

    /// The Hello said on every open from now on.
    pub fn set_hello(&self, hello: Vec<u8>) {
        self.0.borrow_mut().hello = hello;
    }

    pub fn up(&self) -> bool {
        self.0.borrow().up
    }

    /// Close for good: events from the socket are ignored from now on
    /// (and nothing reconnects once the page stops polling).
    pub fn close(&self) {
        let mut i = self.0.borrow_mut();
        i.gen += 1;
        i.up = false;
        if let Some(ws) = i.ws.take() {
            let _ = ws.close();
        }
    }

    pub fn send(&self, bytes: &[u8]) {
        let inner = self.0.borrow();
        if let Some(ws) = inner.ws.as_ref().filter(|_| inner.up) {
            let _ = ws.send_with_u8_array(bytes);
        }
    }

    /// Connect when it is time; what happened since the last poll.
    pub fn poll(&self, now: f64) -> Vec<Net> {
        let due = {
            let i = self.0.borrow();
            i.ws.is_none() && now >= i.retry_at
        };
        if due {
            self.connect();
        }
        self.0.borrow_mut().events.drain(..).collect()
    }

    fn connect(&self) {
        let (url, gen) = {
            let mut i = self.0.borrow_mut();
            let q = match (i.watch, i.counted) {
                (true, _) => "watch=1",
                (false, false) => "v=1",
                _ => "",
            };
            i.counted = true;
            i.gen += 1;
            (crate::room_url(&i.room, q), i.gen)
        };
        let Ok(ws) = WebSocket::new(&url) else {
            self.lost(gen);
            return;
        };
        ws.set_binary_type(BinaryType::Arraybuffer);
        let me = self.0.clone();
        crate::on(&ws, "open", move |_| {
            let mut i = me.borrow_mut();
            if i.gen != gen {
                return;
            }
            i.up = true;
            if !i.watch {
                if let Some(ws) = &i.ws {
                    let _ = ws.send_with_u8_array(&i.hello);
                }
            }
            i.events.push_back(Net::Up);
        });
        let me = self.0.clone();
        crate::on(&ws, "message", move |e| {
            let Ok(e) = e.dyn_into::<MessageEvent>() else {
                return;
            };
            let bytes = js_sys::Uint8Array::new(&e.data()).to_vec();
            let mut i = me.borrow_mut();
            if i.gen != gen {
                return;
            }
            // A server that talks to us is a server that works.
            i.delay = FIRST;
            if engine::who::is_still(&bytes) {
                if let Some(ws) = i.ws.take() {
                    let _ = ws.close();
                }
                drop(i);
                Link(me.clone()).lost(gen);
                return;
            }
            i.events.push_back(Net::Message(bytes));
        });
        for ev in ["close", "error"] {
            let me = self.0.clone();
            crate::on(&ws, ev, move |_| Link(me.clone()).lost(gen));
        }
        self.0.borrow_mut().ws = Some(ws);
    }

    /// The socket of this generation is gone: hold, and try again later.
    fn lost(&self, gen: u32) {
        let mut i = self.0.borrow_mut();
        if i.gen != gen {
            return;
        }
        if let Some(ws) = i.ws.take() {
            let _ = ws.close();
        }
        i.up = false;
        let jitter = 0.8 + js_sys::Math::random() * 0.4;
        i.retry_at = crate::now() + i.delay * jitter;
        i.delay = (i.delay * GROW).min(MOST);
        i.events.push_back(Net::Holding);
        // Events from this socket no longer count.
        i.gen += 1;
    }
}
