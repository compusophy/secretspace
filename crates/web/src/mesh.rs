//! The transport between devices: WebRTC data channels, introduced by
//! trackers that speak the WebTorrent protocol. Public trackers do it with
//! no server of our own; this project's relay speaks the same protocol (and
//! also counts the world). Once a channel opens, envelopes go browser to
//! browser and no tracker hears them.
//!
//! An island announces itself in one swarm with a few offers, each a
//! complete WebRTC description (trackers carry no trickled candidates). A
//! tracker hands each offer to another island in the swarm; its answer
//! comes back by offer id. When two islands meet twice, the link offered by
//! the lower island id is kept.
//!
//! Every handler re-enters the page through `crate::with`, after the event
//! loop has let go of it: nothing is called back synchronously.

use std::collections::HashMap;

use js_sys::{Array, Object, Reflect, JSON};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{
    BinaryType, MessageEvent, RtcConfiguration, RtcDataChannel, RtcDataChannelEvent,
    RtcDataChannelState, RtcDataChannelType, RtcIceGatheringState, RtcIceServer, RtcPeerConnection,
    RtcPeerConnectionState, RtcSdpType, RtcSessionDescriptionInit, WebSocket,
};

/// The swarm every island joins: exactly 20 characters.
const INFO_HASH: &str = "secretspace-world-v1";
/// Channels this island keeps open at most, and seeks until it has.
const MAX_LINKS: usize = 8;
const WANT_LINKS: usize = 4;
const OFFERS_PER_ANNOUNCE: usize = 3;
/// Announce often while seeking, rarely once settled (staying in the swarm
/// so others' offers still reach us).
const ANNOUNCE_SEEKING_MS: f64 = 20_000.0;
const ANNOUNCE_SETTLED_MS: f64 = 90_000.0;
/// While seeking, offers go out as soon as they are ready, but no closer
/// together than this.
const ANNOUNCE_MIN_GAP_MS: f64 = 2_500.0;
const OFFER_TTL_MS: f64 = 60_000.0;
/// A channel that has not opened by then is given up.
const OPEN_WITHIN_MS: f64 = 20_000.0;
/// How long to gather candidates before an offer or answer goes out anyway.
const GATHER_MS: i32 = 3_000;
const STUN: &str = "stun:stun.l.google.com:19302";
/// Redialling an unreachable tracker backs off from this to RETRY_MAX_MS.
const RETRY_MIN_MS: f64 = 5_000.0;
const RETRY_MAX_MS: f64 = 300_000.0;

type Handler = Closure<dyn FnMut(JsValue)>;

fn handler(f: impl FnMut(JsValue) + 'static) -> Handler {
    Closure::<dyn FnMut(JsValue)>::new(f)
}

struct Tracker {
    url: String,
    /// This project's relay: it also takes the census.
    ours: bool,
    ws: Option<WebSocket>,
    retry_at: f64,
    backoff: f64,
    announced: f64,
    /// Islands in the swarm, by this tracker's count.
    swarm: Option<u32>,
    _handlers: Vec<Handler>,
}

impl Tracker {
    fn up(&self) -> bool {
        self.ws
            .as_ref()
            .is_some_and(|w| w.ready_state() == WebSocket::OPEN)
    }
}

/// An offer made, waiting to be sent and answered.
struct Offer {
    pc: RtcPeerConnection,
    dc: RtcDataChannel,
    sdp: Option<String>,
    made: f64,
    sent: bool,
    _handlers: Vec<Handler>,
}

struct Link {
    pc: RtcPeerConnection,
    dc: Option<RtcDataChannel>,
    since: f64,
    /// The island whose offer made this link.
    offerer: u64,
    /// Names this link's channel in its close handler.
    tag: String,
    _handlers: Vec<Handler>,
}

fn close_pc(pc: &RtcPeerConnection, dc: Option<&RtcDataChannel>) {
    pc.set_ondatachannel(None);
    pc.set_onconnectionstatechange(None);
    if let Some(dc) = dc {
        dc.set_onmessage(None);
        dc.set_onclose(None);
        dc.close();
    }
    pc.close();
}

impl Link {
    fn open(&self) -> bool {
        self.dc
            .as_ref()
            .is_some_and(|d| d.ready_state() == RtcDataChannelState::Open)
    }
}

pub struct Mesh {
    me: u64,
    peer_id: String,
    trackers: Vec<Tracker>,
    offers: HashMap<String, Offer>,
    links: HashMap<u64, Link>,
    making: usize,
    serial: u64,
}

fn new_pc() -> Result<RtcPeerConnection, JsValue> {
    let server = RtcIceServer::new();
    server.set_urls(&JsValue::from_str(STUN));
    let config = RtcConfiguration::new();
    config.set_ice_servers(&Array::of1(&server));
    RtcPeerConnection::new_with_configuration(&config)
}

async fn sleep(ms: i32) {
    let p = js_sys::Promise::new(&mut |resolve, _| {
        if let Some(w) = web_sys::window() {
            let _ = w.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
        }
    });
    let _ = JsFuture::from(p).await;
}

/// Wait for candidates (or GATHER_MS), then the description with them in it.
async fn gathered_sdp(pc: &RtcPeerConnection) -> Option<String> {
    for _ in 0..GATHER_MS / 100 {
        if pc.ice_gathering_state() == RtcIceGatheringState::Complete {
            break;
        }
        sleep(100).await;
    }
    pc.local_description().map(|d| d.sdp())
}

fn bytes_of(data: JsValue) -> Option<Vec<u8>> {
    if let Some(buf) = data.dyn_ref::<js_sys::ArrayBuffer>() {
        return Some(js_sys::Uint8Array::new(buf).to_vec());
    }
    data.dyn_ref::<js_sys::Uint8Array>().map(|a| a.to_vec())
}

fn set(o: &Object, key: &str, v: impl Into<JsValue>) {
    let _ = Reflect::set(o, &JsValue::from_str(key), &v.into());
}

fn get_str(o: &JsValue, key: &str) -> Option<String> {
    Reflect::get(o, &JsValue::from_str(key)).ok()?.as_string()
}

fn get_num(o: &JsValue, key: &str) -> Option<f64> {
    Reflect::get(o, &JsValue::from_str(key)).ok()?.as_f64()
}

fn description(kind: &str, sdp: &str) -> Object {
    let d = Object::new();
    set(&d, "type", kind);
    set(&d, "sdp", sdp);
    d
}

/// The island behind a tracker peer id: ours are its id in hex, then "ssp1".
fn island_of(peer_id: &str) -> Option<u64> {
    (peer_id.len() == 20 && peer_id.ends_with("ssp1"))
        .then(|| u64::from_str_radix(&peer_id[..16], 16).ok())
        .flatten()
}

impl Mesh {
    /// `trackers`: (url, is this project's relay).
    pub fn new(me: u64, trackers: Vec<(String, bool)>) -> Mesh {
        Mesh {
            me,
            peer_id: format!("{me:016x}ssp1"),
            trackers: trackers
                .into_iter()
                .map(|(url, ours)| Tracker {
                    url,
                    ours,
                    ws: None,
                    retry_at: 0.0,
                    backoff: RETRY_MIN_MS,
                    announced: 0.0,
                    swarm: None,
                    _handlers: Vec::new(),
                })
                .collect(),
            offers: HashMap::new(),
            links: HashMap::new(),
            making: 0,
            serial: 0,
        }
    }

    pub fn trackers_up(&self) -> usize {
        self.trackers.iter().filter(|t| t.up()).count()
    }

    pub fn open_links(&self) -> usize {
        self.links.values().filter(|l| l.open()).count()
    }

    /// Islands in the swarm, by the largest count any tracker gave.
    pub fn swarm(&self) -> Option<u32> {
        self.trackers
            .iter()
            .filter(|t| t.up())
            .filter_map(|t| t.swarm)
            .max()
    }

    fn dial(&mut self, i: usize) {
        let Ok(ws) = WebSocket::new(&self.trackers[i].url) else {
            return;
        };
        ws.set_binary_type(BinaryType::Arraybuffer);
        let on_open = handler(move |_| {
            crate::with(|p| {
                if let Some(t) = p.mesh.as_mut().and_then(|m| m.trackers.get_mut(i)) {
                    t.backoff = RETRY_MIN_MS;
                    t.announced = 0.0;
                }
            });
        });
        let on_message = handler(move |e: JsValue| {
            let Ok(e) = e.dyn_into::<MessageEvent>() else {
                return;
            };
            let data = e.data();
            if let Some(text) = data.as_string() {
                crate::with(|p| {
                    if let Some(m) = p.mesh.as_mut() {
                        m.tracker_text(i, &text, crate::now());
                    }
                });
            } else if let Some(bytes) = bytes_of(data) {
                crate::with(|p| crate::relay_bytes(p, &bytes));
            }
        });
        let on_close = handler(move |_| {
            crate::with(|p| {
                if let Some(t) = p.mesh.as_mut().and_then(|m| m.trackers.get_mut(i)) {
                    t.ws = None;
                    t.swarm = None;
                    t.retry_at = crate::now() + t.backoff;
                    t.backoff = (t.backoff * 2.0).min(RETRY_MAX_MS);
                }
            });
        });
        ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));
        ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));
        let t = &mut self.trackers[i];
        t.ws = Some(ws);
        t._handlers = vec![on_open, on_message, on_close];
    }

    /// Keep trackers dialled, offers ready, and the swarm told we are here.
    pub fn maintain(&mut self, now: f64) {
        for i in 0..self.trackers.len() {
            if self.trackers[i].ws.is_none() && now >= self.trackers[i].retry_at {
                let t = &mut self.trackers[i];
                t.retry_at = now + t.backoff;
                self.dial(i);
            }
        }
        // Offers no one answered, and links that never opened, are let go.
        let stale: Vec<String> = self
            .offers
            .iter()
            .filter(|(_, o)| now - o.made > OFFER_TTL_MS)
            .map(|(k, _)| k.clone())
            .collect();
        for id in stale {
            if let Some(o) = self.offers.remove(&id) {
                close_pc(&o.pc, Some(&o.dc));
            }
        }
        let dead: Vec<u64> = self
            .links
            .iter()
            .filter(|(_, l)| !l.open() && now - l.since > OPEN_WITHIN_MS)
            .map(|(&k, _)| k)
            .collect();
        for id in dead {
            self.drop_link(id);
        }
        let up = self.trackers_up();
        if up == 0 {
            return;
        }
        // Enough offers ready for one announce on every tracker that is up.
        if self.links.len() < MAX_LINKS {
            let ready = self.offers.values().filter(|o| !o.sent).count() + self.making;
            for _ in ready..(OFFERS_PER_ANNOUNCE * up).min(6) {
                self.make_offer(now);
            }
        }
        let seeking = self.open_links() < WANT_LINKS;
        let every = if seeking {
            ANNOUNCE_SEEKING_MS
        } else {
            ANNOUNCE_SETTLED_MS
        };
        for i in 0..self.trackers.len() {
            let ready = self.offers.values().any(|o| !o.sent && o.sdp.is_some());
            let t = &self.trackers[i];
            let since = now - t.announced;
            let due = t.announced == 0.0
                || since > every
                || (seeking
                    && ready
                    && self.links.len() < MAX_LINKS
                    && since > ANNOUNCE_MIN_GAP_MS);
            if t.up() && due {
                self.announce(i, now, seeking);
            }
        }
    }

    fn make_offer(&mut self, now: f64) {
        let Ok(pc) = new_pc() else { return };
        let dc = pc.create_data_channel("secretspace");
        self.serial += 1;
        let id = format!(
            "{:016x}{:04x}",
            js_sys::Math::random().to_bits() ^ self.me,
            self.serial & 0xffff
        );
        let handlers = self.watch_channel(&id, &dc);
        self.offers.insert(
            id.clone(),
            Offer {
                pc: pc.clone(),
                dc,
                sdp: None,
                made: now,
                sent: false,
                _handlers: handlers,
            },
        );
        self.making += 1;
        spawn_local(async move {
            let mut sdp = None;
            if let Ok(offer) = JsFuture::from(pc.create_offer()).await {
                let init = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
                init.set_sdp(&get_str(&offer, "sdp").unwrap_or_default());
                if JsFuture::from(pc.set_local_description(&init))
                    .await
                    .is_ok()
                {
                    sdp = gathered_sdp(&pc).await;
                }
            }
            crate::with(|p| {
                if let Some(m) = p.mesh.as_mut() {
                    m.making = m.making.saturating_sub(1);
                    match (m.offers.get_mut(&id), sdp) {
                        (Some(o), Some(sdp)) => o.sdp = Some(sdp),
                        (Some(_), None) => {
                            if let Some(o) = m.offers.remove(&id) {
                                close_pc(&o.pc, Some(&o.dc));
                            }
                        }
                        _ => {}
                    }
                }
            });
        });
    }

    fn send_json(&self, i: usize, msg: &Object) {
        if let Some(ws) = self.trackers.get(i).and_then(|t| t.ws.as_ref()) {
            if ws.ready_state() == WebSocket::OPEN {
                if let Ok(text) = JSON::stringify(msg) {
                    let _ = ws.send_with_str(&String::from(text));
                }
            }
        }
    }

    fn announce(&mut self, i: usize, now: f64, with_offers: bool) {
        let msg = Object::new();
        set(&msg, "action", "announce");
        set(&msg, "info_hash", INFO_HASH);
        set(&msg, "peer_id", self.peer_id.as_str());
        set(&msg, "uploaded", 0);
        set(&msg, "downloaded", 0);
        set(&msg, "left", 0);
        if self.trackers[i].announced == 0.0 {
            set(&msg, "event", "started");
        }
        let offers = Array::new();
        if with_offers && self.links.len() < MAX_LINKS {
            let ready: Vec<String> = self
                .offers
                .iter()
                .filter(|(_, o)| !o.sent && o.sdp.is_some())
                .map(|(k, _)| k.clone())
                .take(OFFERS_PER_ANNOUNCE)
                .collect();
            for id in ready {
                let o = self.offers.get_mut(&id).expect("ready offer");
                o.sent = true;
                let entry = Object::new();
                set(&entry, "offer_id", id.as_str());
                set(
                    &entry,
                    "offer",
                    description("offer", o.sdp.as_deref().unwrap_or_default()),
                );
                offers.push(&entry);
            }
        }
        set(&msg, "numwant", offers.length());
        set(&msg, "offers", offers);
        self.send_json(i, &msg);
        self.trackers[i].announced = now;
    }

    /// A message from tracker `i`.
    fn tracker_text(&mut self, i: usize, text: &str, now: f64) {
        let Ok(v) = JSON::parse(text) else { return };
        if get_str(&v, "info_hash").as_deref() != Some(INFO_HASH) {
            return;
        }
        if let Some(c) = get_num(&v, "complete") {
            let swarm = c + get_num(&v, "incomplete").unwrap_or(0.0);
            self.trackers[i].swarm = Some(swarm.clamp(0.0, 1e9) as u32);
        }
        let (Some(peer), Some(offer_id)) = (get_str(&v, "peer_id"), get_str(&v, "offer_id")) else {
            return;
        };
        let sdp_of = |key: &str| {
            Reflect::get(&v, &JsValue::from_str(key))
                .ok()
                .and_then(|d| get_str(&d, "sdp"))
        };
        if let Some(sdp) = sdp_of("offer") {
            self.answer_offer(i, peer, offer_id, sdp, now);
        } else if let Some(sdp) = sdp_of("answer") {
            self.accept_answer(peer, offer_id, sdp, now);
        }
    }

    /// Two links to one island: keep the one the lower id offered.
    fn keeps(&self, island: u64, offerer: u64) -> bool {
        match self.links.get(&island) {
            None => true,
            Some(l) => l.offerer != self.me.min(island) && offerer == self.me.min(island),
        }
    }

    fn answer_offer(&mut self, i: usize, peer: String, offer_id: String, sdp: String, now: f64) {
        let Some(island) = island_of(&peer) else {
            return;
        };
        if island == self.me
            || (!self.links.contains_key(&island) && self.links.len() >= MAX_LINKS)
            || !self.keeps(island, island)
        {
            return;
        }
        self.drop_link(island);
        let Ok(pc) = new_pc() else { return };
        let tag = format!("a{offer_id}");
        let tag2 = tag.clone();
        let on_channel = handler(move |e: JsValue| {
            let Ok(e) = e.dyn_into::<RtcDataChannelEvent>() else {
                return;
            };
            let dc = e.channel();
            crate::with(|p| {
                let Some(m) = p.mesh.as_mut() else { return };
                let hs = m.watch_channel(&tag2, &dc);
                if let Some(l) = m.links.get_mut(&island).filter(|l| l.tag == tag2) {
                    l.dc = Some(dc);
                    l._handlers.extend(hs);
                }
            });
        });
        pc.set_ondatachannel(Some(on_channel.as_ref().unchecked_ref()));
        let mut handlers = vec![on_channel];
        handlers.push(self.watch_state(island, &tag, &pc));
        self.links.insert(
            island,
            Link {
                pc: pc.clone(),
                dc: None,
                since: now,
                offerer: island,
                tag,
                _handlers: handlers,
            },
        );
        let me = self.peer_id.clone();
        spawn_local(async move {
            let init = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
            init.set_sdp(&sdp);
            if JsFuture::from(pc.set_remote_description(&init))
                .await
                .is_err()
            {
                return;
            }
            let Ok(answer) = JsFuture::from(pc.create_answer()).await else {
                return;
            };
            let init = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
            init.set_sdp(&get_str(&answer, "sdp").unwrap_or_default());
            if JsFuture::from(pc.set_local_description(&init))
                .await
                .is_err()
            {
                return;
            }
            let Some(sdp) = gathered_sdp(&pc).await else {
                return;
            };
            let msg = Object::new();
            set(&msg, "action", "announce");
            set(&msg, "info_hash", INFO_HASH);
            set(&msg, "peer_id", me.as_str());
            set(&msg, "to_peer_id", peer.as_str());
            set(&msg, "offer_id", offer_id.as_str());
            set(&msg, "answer", description("answer", &sdp));
            crate::with(|p| {
                if let Some(m) = &p.mesh {
                    m.send_json(i, &msg);
                }
            });
        });
    }

    fn accept_answer(&mut self, peer: String, offer_id: String, sdp: String, now: f64) {
        let Some(offer) = self.offers.remove(&offer_id) else {
            return;
        };
        let island = match island_of(&peer) {
            Some(id) if id != self.me && self.keeps(id, self.me) => id,
            _ => {
                close_pc(&offer.pc, Some(&offer.dc));
                return;
            }
        };
        self.drop_link(island);
        let pc = offer.pc.clone();
        let mut handlers = offer._handlers;
        handlers.push(self.watch_state(island, &offer_id, &pc));
        self.links.insert(
            island,
            Link {
                pc: offer.pc,
                dc: Some(offer.dc),
                since: now,
                offerer: self.me,
                tag: offer_id,
                _handlers: handlers,
            },
        );
        spawn_local(async move {
            let init = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
            init.set_sdp(&sdp);
            let _ = JsFuture::from(pc.set_remote_description(&init)).await;
        });
    }

    fn watch_channel(&self, tag: &str, dc: &RtcDataChannel) -> Vec<Handler> {
        dc.set_binary_type(RtcDataChannelType::Arraybuffer);
        let on_message = handler(|e: JsValue| {
            let Ok(e) = e.dyn_into::<MessageEvent>() else {
                return;
            };
            if let Some(bytes) = bytes_of(e.data()) {
                crate::with(|p| crate::envelope_bytes(p, &bytes));
            }
        });
        let tag = tag.to_string();
        let on_close = handler(move |_| {
            crate::with(|p| {
                if let Some(m) = p.mesh.as_mut() {
                    m.channel_closed(&tag);
                }
            });
        });
        dc.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        dc.set_onclose(Some(on_close.as_ref().unchecked_ref()));
        vec![on_message, on_close]
    }

    fn watch_state(&self, island: u64, tag: &str, pc: &RtcPeerConnection) -> Handler {
        let pc2 = pc.clone();
        let tag = tag.to_string();
        let h = handler(move |_| {
            if matches!(
                pc2.connection_state(),
                RtcPeerConnectionState::Failed | RtcPeerConnectionState::Closed
            ) {
                crate::with(|p| {
                    if let Some(m) = p.mesh.as_mut() {
                        if m.links.get(&island).is_some_and(|l| l.tag == tag) {
                            m.drop_link(island);
                        }
                    }
                });
            }
        });
        pc.set_onconnectionstatechange(Some(h.as_ref().unchecked_ref()));
        h
    }

    fn channel_closed(&mut self, tag: &str) {
        if let Some(o) = self.offers.remove(tag) {
            close_pc(&o.pc, Some(&o.dc));
        }
        if let Some(id) = self
            .links
            .iter()
            .find(|(_, l)| l.tag == tag)
            .map(|(&id, _)| id)
        {
            self.drop_link(id);
        }
    }

    pub fn drop_link(&mut self, id: u64) {
        if let Some(l) = self.links.remove(&id) {
            close_pc(&l.pc, l.dc.as_ref());
        }
    }

    /// The census goes to this project's relay, so it can count the world.
    pub fn census(&self, bytes: &[u8]) {
        for t in self.trackers.iter().filter(|t| t.ours) {
            if let Some(ws) = t.ws.as_ref().filter(|w| w.ready_state() == WebSocket::OPEN) {
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
}
