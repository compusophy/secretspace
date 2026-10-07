//! gestures_read_as_recorded: every trace in `fixtures/traces.txt` reads
//! as the gestures written above it, and nothing else.

use kit::pointer::{Kind, Press};
use luciphon_web::gesture::{Gesture, Recogniser};
use luciphon_web::laws::FEEL;

fn word(g: &Gesture) -> String {
    match g {
        Gesture::Tap => "tap".into(),
        Gesture::Flick { heading } => format!(
            "flick {}",
            (*heading as f64 * 360.0 / 65536.0).round() as i32 % 360
        ),
        Gesture::Lift => "lift".into(),
        Gesture::Hold { .. } => "hold".into(),
        Gesture::Release { len, .. } => {
            if *len < FEEL.throw_px {
                "release 0".into()
            } else {
                format!("release {}", len.round())
            }
        }
        Gesture::Cancel => "cancel".into(),
        Gesture::Heart => "heart".into(),
        Gesture::Wheel(s) => format!("wheel {s}"),
    }
}

/// Whether two readings agree: headings within 6 degrees, lengths within 3 px.
fn agree(got: &str, want: &str) -> bool {
    let (g, w): (Vec<&str>, Vec<&str>) = (got.split(' ').collect(), want.split(' ').collect());
    if g.len() != w.len() || g[0] != w[0] {
        return false;
    }
    if g.len() == 1 {
        return true;
    }
    let (a, b): (f64, f64) = (g[1].parse().unwrap(), w[1].parse().unwrap());
    let d = (a - b).abs();
    match g[0] {
        "flick" => d.min(360.0 - d) <= 6.0,
        _ => d <= 3.0,
    }
}

#[test]
fn gestures_read_as_recorded() {
    let text = include_str!("fixtures/traces.txt");
    let heart =
        |x: f64, y: f64| ((x - 195.0).powi(2) + (y - 700.0).powi(2)).sqrt() < FEEL.heart_hit;
    let mut traces = 0;
    for block in text.split("\n\n") {
        let mut lines = block.lines().filter(|l| !l.trim().is_empty());
        let name = lines
            .clone()
            .find(|l| l.starts_with("# ") && !l.contains("Pointer traces"));
        let Some(want) = lines.clone().find_map(|l| l.strip_prefix("= ")) else {
            continue;
        };
        let mut r = Recogniser::default();
        let mut got = Vec::new();
        let mut last_t = 0.0;
        for l in lines
            .by_ref()
            .filter(|l| !l.starts_with('#') && !l.starts_with('='))
        {
            let f: Vec<&str> = l.split_whitespace().collect();
            let kind = match f[0] {
                "d" => Kind::Down,
                "m" => Kind::Move,
                "u" => Kind::Up,
                "h" => Kind::Hover,
                _ => Kind::Cancel,
            };
            let p = Press {
                kind,
                x: f[1].parse().unwrap(),
                y: f[2].parse().unwrap(),
                t: f[3].parse().unwrap(),
                touch: f[4] == "1",
                second: false,
            };
            last_t = p.t;
            r.feed(&p, &FEEL, &heart, &mut got);
        }
        r.tick(last_t + 1000.0, &FEEL, &mut got);
        let got: Vec<String> = got.iter().map(word).collect();
        let want: Vec<&str> = want.split(", ").collect();
        let ok = got.len() == want.len() && got.iter().zip(&want).all(|(g, w)| agree(g, w));
        assert!(
            ok,
            "{}: read {got:?}, should be {want:?}",
            name.unwrap_or("?")
        );
        traces += 1;
    }
    assert!(traces >= 40, "{traces} traces");
}
