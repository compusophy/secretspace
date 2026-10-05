//! Where this island is: the person's time zone names the place, and a
//! neighbor's clock is computed from the offset it sent.

use wasm_bindgen::JsValue;

pub struct Place {
    pub name: String,
    /// Minutes east of UTC.
    pub tz_min: i16,
}

pub fn param(key: &str) -> Option<String> {
    let search = web_sys::window()?.location().search().ok()?;
    let q = search.trim_start_matches('?');
    q.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k == key)
            .then(|| js_sys::decode_uri_component(v).ok()?.as_string())
            .flatten()
    })
}

/// This tab's place. `?place=Lisbon&tz=60` stands in for another city.
pub fn here() -> Place {
    let opts = js_sys::Intl::DateTimeFormat::new(&js_sys::Array::new(), &js_sys::Object::new())
        .resolved_options();
    let tz = js_sys::Reflect::get(&opts, &JsValue::from_str("timeZone"))
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_default();
    let zone = tz.rsplit('/').next().unwrap_or("").replace('_', " ");
    let offset = -(js_sys::Date::new_0().get_timezone_offset() as i16);
    Place {
        name: param("place").unwrap_or(if zone.is_empty() {
            "somewhere".into()
        } else {
            zone
        }),
        tz_min: param("tz").and_then(|t| t.parse().ok()).unwrap_or(offset),
    }
}

/// HH:MM at a place `tz_min` minutes east of UTC.
pub fn clock(tz_min: i16) -> String {
    let utc_min = (js_sys::Date::now() / 60_000.0).floor() as i64;
    let m = (utc_min + tz_min as i64).rem_euclid(1440);
    format!("{:02}:{:02}", m / 60, m % 60)
}
