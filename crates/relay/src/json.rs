//! Just enough JSON for the tracker protocol: parse a message, read a few
//! fields, write a message. Bounded in depth; every input is a stranger's.

#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

const MAX_DEPTH: usize = 16;

impl Json {
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn str(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    #[cfg(test)]
    pub fn num(&self, key: &str) -> Option<f64> {
        match self.get(key)? {
            Json::Num(n) => Some(*n),
            _ => None,
        }
    }

    pub fn arr(&self, key: &str) -> Option<&[Json]> {
        match self.get(key)? {
            Json::Arr(a) => Some(a),
            _ => None,
        }
    }
}

/// An object from its fields, in order.
pub fn obj(fields: Vec<(&str, Json)>) -> Json {
    Json::Obj(
        fields
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    )
}

pub fn s(v: &str) -> Json {
    Json::Str(v.to_string())
}

struct P<'a> {
    b: &'a [u8],
    i: usize,
}

impl P<'_> {
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }

    fn eat(&mut self, c: u8) -> Option<()> {
        self.ws();
        (self.b.get(self.i) == Some(&c)).then(|| self.i += 1)
    }

    fn lit(&mut self, word: &str, v: Json) -> Option<Json> {
        self.b[self.i..].starts_with(word.as_bytes()).then(|| {
            self.i += word.len();
            v
        })
    }

    fn value(&mut self, depth: usize) -> Option<Json> {
        if depth > MAX_DEPTH {
            return None;
        }
        self.ws();
        match *self.b.get(self.i)? {
            b'{' => {
                self.i += 1;
                let mut fields = Vec::new();
                if self.eat(b'}').is_some() {
                    return Some(Json::Obj(fields));
                }
                loop {
                    self.ws();
                    let k = self.string()?;
                    self.eat(b':')?;
                    fields.push((k, self.value(depth + 1)?));
                    if self.eat(b',').is_none() {
                        self.eat(b'}')?;
                        return Some(Json::Obj(fields));
                    }
                }
            }
            b'[' => {
                self.i += 1;
                let mut items = Vec::new();
                if self.eat(b']').is_some() {
                    return Some(Json::Arr(items));
                }
                loop {
                    items.push(self.value(depth + 1)?);
                    if self.eat(b',').is_none() {
                        self.eat(b']')?;
                        return Some(Json::Arr(items));
                    }
                }
            }
            b'"' => self.string().map(Json::Str),
            b't' => self.lit("true", Json::Bool(true)),
            b'f' => self.lit("false", Json::Bool(false)),
            b'n' => self.lit("null", Json::Null),
            _ => {
                let start = self.i;
                while self.i < self.b.len()
                    && matches!(
                        self.b[self.i],
                        b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'
                    )
                {
                    self.i += 1;
                }
                std::str::from_utf8(&self.b[start..self.i])
                    .ok()?
                    .parse()
                    .ok()
                    .map(Json::Num)
            }
        }
    }

    fn string(&mut self) -> Option<String> {
        if self.b.get(self.i) != Some(&b'"') {
            return None;
        }
        self.i += 1;
        let mut out = String::new();
        loop {
            let c = *self.b.get(self.i)?;
            self.i += 1;
            match c {
                b'"' => return Some(out),
                b'\\' => {
                    let e = *self.b.get(self.i)?;
                    self.i += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hex = std::str::from_utf8(self.b.get(self.i..self.i + 4)?).ok()?;
                            self.i += 4;
                            let mut code = u32::from_str_radix(hex, 16).ok()?;
                            // A surrogate pair spells one character.
                            if (0xD800..0xDC00).contains(&code)
                                && self.b[self.i..].starts_with(b"\\u")
                            {
                                let lo = std::str::from_utf8(self.b.get(self.i + 2..self.i + 6)?)
                                    .ok()?;
                                let lo = u32::from_str_radix(lo, 16).ok()?;
                                if (0xDC00..0xE000).contains(&lo) {
                                    self.i += 6;
                                    code = 0x10000 + ((code - 0xD800) << 10) + (lo - 0xDC00);
                                }
                            }
                            out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                        }
                        _ => return None,
                    }
                }
                _ => {
                    // Copy the rest of a UTF-8 sequence whole.
                    let start = self.i - 1;
                    let len = match c {
                        0x00..=0x7f => 1,
                        0xc0..=0xdf => 2,
                        0xe0..=0xef => 3,
                        _ => 4,
                    };
                    let end = (start + len).min(self.b.len());
                    out.push_str(std::str::from_utf8(&self.b[start..end]).ok()?);
                    self.i = end;
                }
            }
        }
    }
}

pub fn parse(text: &str) -> Option<Json> {
    let mut p = P {
        b: text.as_bytes(),
        i: 0,
    };
    let v = p.value(0)?;
    p.ws();
    (p.i == p.b.len()).then_some(v)
}

fn write_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

pub fn write(v: &Json) -> String {
    let mut out = String::new();
    write_into(&mut out, v);
    out
}

fn write_into(out: &mut String, v: &Json) {
    match v {
        Json::Null => out.push_str("null"),
        Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Json::Num(n) if n.is_finite() && n.fract() == 0.0 && n.abs() < 9.0e15 => {
            out.push_str(&format!("{}", *n as i64))
        }
        Json::Num(n) if n.is_finite() => out.push_str(&format!("{n}")),
        Json::Num(_) => out.push_str("null"),
        Json::Str(s) => write_str(out, s),
        Json::Arr(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_into(out, item);
            }
            out.push(']');
        }
        Json::Obj(fields) => {
            out.push('{');
            for (i, (k, val)) in fields.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_str(out, k);
                out.push(':');
                write_into(out, val);
            }
            out.push('}');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_an_announce_with_an_sdp() {
        let text = r#"{"action":"announce","info_hash":"secretspace-world-v1","numwant":2,"offers":[{"offer_id":"0123456789abcdef0123","offer":{"type":"offer","sdp":"v=0\r\no=- 1 2 IN IP4 127.0.0.1\r\n"}}],"ok":true,"none":null,"x":-1.5e2,"u":"\u00e9\ud83d\ude00"}"#;
        let v = parse(text).expect("parses");
        assert_eq!(v.str("action"), Some("announce"));
        assert_eq!(v.num("numwant"), Some(2.0));
        assert_eq!(v.num("x"), Some(-150.0));
        assert_eq!(v.str("u"), Some("é😀"));
        let offer = &v.arr("offers").unwrap()[0];
        assert!(offer
            .get("offer")
            .unwrap()
            .str("sdp")
            .unwrap()
            .contains("\r\n"));
        assert_eq!(parse(&write(&v)), Some(v));
    }

    #[test]
    fn junk_and_depth_bombs_are_refused() {
        for bad in [
            "",
            "{",
            "[1,]",
            "{\"a\" 1}",
            "\"\\x\"",
            "tru",
            "{}x",
            "\"\\u12\"",
        ] {
            assert_eq!(parse(bad), None, "{bad}");
        }
        let bomb = format!("{}{}", "[".repeat(100), "]".repeat(100));
        assert_eq!(parse(&bomb), None);
    }
}
