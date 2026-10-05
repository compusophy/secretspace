//! What this browser keeps between visits: the person's name as an author,
//! the lineages they released, and the fossils of their last island.

use space::island::Traveler;
use space::wire::{decode, encode, Envelope, Msg};

const FOSSILS: &str = "secretspace/fossils";
const AUTHOR: &str = "secretspace/author";
const MINE: &str = "secretspace/mine";

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

pub fn get(key: &str) -> Option<String> {
    storage()?.get_item(key).ok().flatten()
}

pub fn set(key: &str, value: &str) {
    if let Some(s) = storage() {
        let _ = s.set_item(key, value);
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

pub fn save_fossils(fossils: &[Traveler]) {
    let packed: Vec<String> = fossils
        .iter()
        .map(|t| {
            hex(&encode(&Envelope {
                from: 0,
                to: 0,
                msg: Msg::Mote {
                    seq: 0,
                    side: 0,
                    traveler: t.clone(),
                },
            }))
        })
        .collect();
    set(FOSSILS, &packed.join(","));
}

/// The fossils left last time, once: waking them consumes them.
pub fn take_fossils() -> Vec<Traveler> {
    let Some(s) = get(FOSSILS) else {
        return Vec::new();
    };
    if let Some(st) = storage() {
        let _ = st.remove_item(FOSSILS);
    }
    s.split(',')
        .filter_map(|h| match decode(&unhex(h)?).ok()?.msg {
            Msg::Mote { traveler, .. } => Some(traveler),
            _ => None,
        })
        .collect()
}

pub fn author() -> String {
    if let Some(a) = get(AUTHOR).filter(|a| !a.trim().is_empty()) {
        return a;
    }
    let a = format!("anon-{:04x}", (js_sys::Math::random() * 65536.0) as u32);
    set(AUTHOR, &a);
    a
}

pub fn set_author(a: &str) {
    set(AUTHOR, a.trim());
}

pub fn mine() -> Vec<u64> {
    get(MINE)
        .map(|s| {
            s.split(',')
                .filter_map(|x| u64::from_str_radix(x, 16).ok())
                .collect()
        })
        .unwrap_or_default()
}

pub fn add_mine(lineage: u64) {
    let mut m = mine();
    if !m.contains(&lineage) {
        m.push(lineage);
        let s: Vec<String> = m.iter().rev().take(32).map(|x| format!("{x:x}")).collect();
        set(MINE, &s.join(","));
    }
}
