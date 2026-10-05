//! The transport between devices: WebRTC data channels, introduced by the
//! relay. The relay hears only who wants to meet whom and the census; once
//! a channel opens, envelopes go browser to browser.
//!
//! Every handler here re-enters the page through `crate::with`, after the
//! event loop has let go of it: nothing is called back synchronously.

use std::collections::HashMap;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{
    BinaryType, MessageEvent, RtcConfiguration, RtcDataChannel, RtcDataChannelEvent,
    RtcDataChannelState, RtcDataChannelType, RtcIceCandidateInit, RtcIceServer, RtcPeerConnection,
    RtcPeerConnectionIceEvent, RtcPeerConnectionState, RtcSdpType, RtcSessionDescriptionInit,
    WebSocket,
};

/// Channels this island keeps open at most, and wants at least.
const MAX_LINKS: usize = 8;
const WANT_LINKS: usize = 3;
/// A channel that has not opened by then is given up.
const OPEN_WITHIN_MS: f64 = 15_000.0;
const MORE_EVERY_MS: f64 = 8_000.0;
const STUN: &str = "stun:stun.l.google.com:19302";
/// Redialling an unreachable relay backs off from this to RETRY_MAX_MS.
const RETRY_MIN_MS: f64 = 5_000.0;
const RETRY_MAX_MS: f64 = 300_000.0;

type Handler = Closure<dyn FnMut(JsValue)>;

struct Link {
    pc: RtcPeerConnection,
    dc: Option<RtcDataChannel>,
    since: f64,
    remote_set: bool,
    early_ice: Vec<RtcIceCandidateInit>,
    _handlers: Vec<Handler>,
}

impl Link {
    fn open(&self) -> bool {
        self.dc
            .as_ref()
            .is_some_and(|d| d.ready_state() == RtcDataChannelState::Open)
    }

    fn close(self) {
        self.pc.set_onicecandidate(None);
        self.pc.set_ondatachannel(None);
        self.pc.set_onconnectionstatechange(None);
        if let Some(dc) = &self.dc {
            dc.set_onmessage(None);
            dc.set_onclose(None);
            dc.close();
        }
        self.pc.close();
    }
}

pub struct Mesh {
    me: u64,
    url: String,
    ws: Option<WebSocket>,
    links: HashMap<u64, Link>,
    last_more: f64,
    retry_at: f64,
    backoff: f64,
    _ws_handlers: Vec<Handler>,
}

fn hex(s: &str) -> String {
    s.bytes().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Option<String> {
    let bytes: Option<Vec<u8>> = (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect();
    String::from_utf8(bytes?).ok()
}

fn handler(f: impl FnMut(JsValue) + 'static) -> Handler {
    Closure::<dyn FnMut(JsValue)>::new(f)
}

fn bytes_of(data: JsValue) -> Option<Vec<u8>> {
    if let Some(buf) = data.dyn_ref::<js_sys::ArrayBuffer>() {
        return Some(js_sys::Uint8Array::new(buf).to_vec());
    }
    data.dyn_ref::<js_sys::Uint8Array>().map(|a| a.to_vec())
}

fn new_pc() -> Result<RtcPeerConnection, JsValue> {
    let server = RtcIceServer::new();
    server.set_urls(&JsValue::from_str(STUN));
    let config = RtcConfiguration::new();
    config.set_ice_servers(&js_sys::Array::of1(&server));
    RtcPeerConnection::new_with_configuration(&config)
}

impl Mesh {
    pub fn new(me: u64, url: String) -> Mesh {
        Mesh {
            me,
            url,
            ws: None,
            links: HashMap::new(),
            last_more: 0.0,
            retry_at: 0.0,
            backoff: RETRY_MIN_MS,
            _ws_handlers: Vec::new(),
        }
    }

    pub fn relay_up(&self) -> bool {
        self.ws
            .as_ref()
            .is_some_and(|w| w.ready_state() == WebSocket::OPEN)
    }

    pub fn open_links(&self) -> usize {
        self.links.values().filter(|l| l.open()).count()
    }

    fn dial(&mut self) {
        let Ok(ws) = WebSocket::new(&self.url) else {
            return;
        };
        ws.set_binary_type(BinaryType::Arraybuffer);
        let me = self.me;
        let on_open = handler(move |_| {
            crate::with(|p| {
                if let Some(m) = p.mesh.as_mut() {
                    m.backoff = RETRY_MIN_MS;
                    if let Some(ws) = &m.ws {
                        let _ = ws.send_with_str(&format!("hi {me:x}"));
                    }
                }
            });
        });
        let on_message = handler(|e: JsValue| {
            let Ok(e) = e.dyn_into::<MessageEvent>() else {
                return;
            };
            let data = e.data();
            if let Some(text) = data.as_string() {
                crate::with(|p| crate::mesh_text(p, &text));
            } else if let Some(bytes) = bytes_of(data) {
                crate::with(|p| crate::relay_bytes(p, &bytes));
            }
        });
        let on_close = handler(|_| {
            crate::with(|p| {
                if let Some(m) = p.mesh.as_mut() {
                    m.ws = None;
                    m.retry_at = crate::now() + m.backoff;
                    m.backoff = (m.backoff * 2.0).min(RETRY_MAX_MS);
                }
            });
        });
        ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));
        ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));
        self.ws = Some(ws);
        self._ws_handlers = vec![on_open, on_message, on_close];
    }

    /// Keep the relay dialled and enough channels open.
    pub fn maintain(&mut self, now: f64) {
        if self.ws.is_none() && now >= self.retry_at {
            self.retry_at = now + self.backoff;
            self.dial();
        }
        let stale: Vec<u64> = self
            .links
            .iter()
            .filter(|(_, l)| !l.open() && now - l.since > OPEN_WITHIN_MS)
            .map(|(&id, _)| id)
            .collect();
        for id in stale {
            self.drop_link(id);
        }
        if self.relay_up() && self.open_links() < WANT_LINKS && now - self.last_more > MORE_EVERY_MS
        {
            self.last_more = now;
            self.say("more");
        }
    }

    fn say(&self, line: &str) {
        if let Some(ws) = &self.ws {
            if ws.ready_state() == WebSocket::OPEN {
                let _ = ws.send_with_str(line);
            }
        }
    }

    fn signal(&self, to: u64, payload: &str) {
        self.say(&format!("to {to:x} {payload}"));
    }

    /// The census goes to the relay too, so it can count the world.
    pub fn census(&self, bytes: &[u8]) {
        if let Some(ws) = &self.ws {
            if ws.ready_state() == WebSocket::OPEN {
                let _ = ws.send_with_u8_array(bytes);
            }
        }
    }

    /// An envelope out: to one island's channel, or to every channel.
    pub fn send(&self, to: u64, bytes: &[u8]) {
        let targets: Box<dyn Iterator<Item = &Link>> = if to == 0 {
            Box::new(self.links.values())
        } else {
            Box::new(self.links.get(&to).into_iter())
        };
        for l in targets {
            if let Some(dc) = l.dc.as_ref().filter(|_| l.open()) {
                let _ = dc.send_with_u8_array(bytes);
            }
        }
    }

    pub fn drop_link(&mut self, id: u64) {
        if let Some(l) = self.links.remove(&id) {
            l.close();
        }
    }

    fn watch_channel(&self, id: u64, dc: &RtcDataChannel) -> Vec<Handler> {
        dc.set_binary_type(RtcDataChannelType::Arraybuffer);
        let on_message = handler(|e: JsValue| {
            let Ok(e) = e.dyn_into::<MessageEvent>() else {
                return;
            };
            if let Some(bytes) = bytes_of(e.data()) {
                crate::with(|p| crate::envelope_bytes(p, &bytes));
            }
        });
        let on_close = handler(move |_| {
            crate::with(|p| {
                if let Some(m) = p.mesh.as_mut() {
                    m.drop_link(id);
                }
            });
        });
        dc.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        dc.set_onclose(Some(on_close.as_ref().unchecked_ref()));
        vec![on_message, on_close]
    }

    fn watch_pc(&self, id: u64, pc: &RtcPeerConnection) -> Vec<Handler> {
        let on_ice = handler(move |e: JsValue| {
            let Ok(e) = e.dyn_into::<RtcPeerConnectionIceEvent>() else {
                return;
            };
            let Some(c) = e.candidate() else { return };
            let line = format!(
                "ice {} {} {}",
                hex(&c.candidate()),
                hex(&c.sdp_mid().unwrap_or_default()),
                c.sdp_m_line_index().unwrap_or(0)
            );
            crate::with(|p| {
                if let Some(m) = &p.mesh {
                    m.signal(id, &line);
                }
            });
        });
        let pc2 = pc.clone();
        let on_state = handler(move |_| {
            if matches!(
                pc2.connection_state(),
                RtcPeerConnectionState::Failed | RtcPeerConnectionState::Closed
            ) {
                crate::with(|p| {
                    if let Some(m) = p.mesh.as_mut() {
                        m.drop_link(id);
                    }
                });
            }
        });
        pc.set_onicecandidate(Some(on_ice.as_ref().unchecked_ref()));
        pc.set_onconnectionstatechange(Some(on_state.as_ref().unchecked_ref()));
        vec![on_ice, on_state]
    }

    /// The relay introduced some islands: offer each a channel.
    pub fn introduced(&mut self, ids: &[u64], now: f64) {
        for &id in ids {
            if id == self.me || self.links.contains_key(&id) || self.links.len() >= MAX_LINKS {
                continue;
            }
            let Ok(pc) = new_pc() else { continue };
            let dc = pc.create_data_channel("secretspace");
            let mut handlers = self.watch_pc(id, &pc);
            handlers.extend(self.watch_channel(id, &dc));
            self.links.insert(
                id,
                Link {
                    pc: pc.clone(),
                    dc: Some(dc),
                    since: now,
                    remote_set: false,
                    early_ice: Vec::new(),
                    _handlers: handlers,
                },
            );
            spawn_local(async move {
                let Ok(offer) = JsFuture::from(pc.create_offer()).await else {
                    return;
                };
                let sdp = js_sys::Reflect::get(&offer, &"sdp".into())
                    .ok()
                    .and_then(|s| s.as_string())
                    .unwrap_or_default();
                let init = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
                init.set_sdp(&sdp);
                if JsFuture::from(pc.set_local_description(&init))
                    .await
                    .is_ok()
                {
                    crate::with(|p| {
                        if let Some(m) = &p.mesh {
                            m.signal(id, &format!("offer {}", hex(&sdp)));
                        }
                    });
                }
            });
        }
    }

    /// A signaling line from another island, through the relay.
    pub fn signaled(&mut self, from: u64, payload: &str, now: f64) {
        let mut parts = payload.split(' ');
        match parts.next() {
            Some("offer") => {
                let Some(sdp) = parts.next().and_then(unhex) else {
                    return;
                };
                if self.links.contains_key(&from) {
                    // Both offered at once: the lower id's offer stands.
                    if self.me < from {
                        return;
                    }
                    self.drop_link(from);
                }
                if self.links.len() >= MAX_LINKS {
                    return;
                }
                let Ok(pc) = new_pc() else { return };
                let mut handlers = self.watch_pc(from, &pc);
                let on_channel = handler(move |e: JsValue| {
                    let Ok(e) = e.dyn_into::<RtcDataChannelEvent>() else {
                        return;
                    };
                    let dc = e.channel();
                    crate::with(|p| {
                        let Some(m) = p.mesh.as_mut() else { return };
                        let hs = m.watch_channel(from, &dc);
                        if let Some(l) = m.links.get_mut(&from) {
                            l.dc = Some(dc);
                            l._handlers.extend(hs);
                        }
                    });
                });
                pc.set_ondatachannel(Some(on_channel.as_ref().unchecked_ref()));
                handlers.push(on_channel);
                self.links.insert(
                    from,
                    Link {
                        pc: pc.clone(),
                        dc: None,
                        since: now,
                        remote_set: false,
                        early_ice: Vec::new(),
                        _handlers: handlers,
                    },
                );
                spawn_local(async move {
                    let init = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
                    init.set_sdp(&sdp);
                    if JsFuture::from(pc.set_remote_description(&init))
                        .await
                        .is_err()
                    {
                        return;
                    }
                    crate::with(|p| {
                        if let Some(m) = p.mesh.as_mut() {
                            m.remote_ready(from);
                        }
                    });
                    let Ok(answer) = JsFuture::from(pc.create_answer()).await else {
                        return;
                    };
                    let sdp = js_sys::Reflect::get(&answer, &"sdp".into())
                        .ok()
                        .and_then(|s| s.as_string())
                        .unwrap_or_default();
                    let init = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
                    init.set_sdp(&sdp);
                    if JsFuture::from(pc.set_local_description(&init))
                        .await
                        .is_ok()
                    {
                        crate::with(|p| {
                            if let Some(m) = &p.mesh {
                                m.signal(from, &format!("answer {}", hex(&sdp)));
                            }
                        });
                    }
                });
            }
            Some("answer") => {
                let Some(sdp) = parts.next().and_then(unhex) else {
                    return;
                };
                let Some(l) = self.links.get(&from) else {
                    return;
                };
                let pc = l.pc.clone();
                spawn_local(async move {
                    let init = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
                    init.set_sdp(&sdp);
                    if JsFuture::from(pc.set_remote_description(&init))
                        .await
                        .is_ok()
                    {
                        crate::with(|p| {
                            if let Some(m) = p.mesh.as_mut() {
                                m.remote_ready(from);
                            }
                        });
                    }
                });
            }
            Some("ice") => {
                let (Some(c), Some(mid), Some(idx)) = (
                    parts.next().and_then(unhex),
                    parts.next().and_then(unhex),
                    parts.next(),
                ) else {
                    return;
                };
                let init = RtcIceCandidateInit::new(&c);
                init.set_sdp_mid(Some(&mid));
                init.set_sdp_m_line_index(idx.parse().ok());
                let Some(l) = self.links.get_mut(&from) else {
                    return;
                };
                if l.remote_set {
                    let _ =
                        l.pc.add_ice_candidate_with_opt_rtc_ice_candidate_init(Some(&init));
                } else {
                    l.early_ice.push(init);
                }
            }
            _ => {}
        }
    }

    /// The far side's description is set: candidates that came early can
    /// be added now.
    fn remote_ready(&mut self, id: u64) {
        if let Some(l) = self.links.get_mut(&id) {
            l.remote_set = true;
            for c in l.early_ice.drain(..) {
                let _ =
                    l.pc.add_ice_candidate_with_opt_rtc_ice_candidate_init(Some(&c));
            }
        }
    }
}
