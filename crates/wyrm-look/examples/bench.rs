//! How long `look::world` takes to draw one picture, on made-up arenas of
//! a few sizes of snake, as the page and the hub's preview draw them: for
//! seeing what a change to the look costs before a phone does.
//! cargo run -p secretspace-wyrm-look --release --example bench [pictures] [dir]

use std::time::Instant;

use look::{Scene, View};
use pixels::Canvas;
use wyrm::laws::{body_len, view_scale, STEP};
use wyrm::mirror::Mirror;
use wyrm::proto::{q, FoodInfo, Frame, SnakeUpdate};

/// `n` snakes of `mass` wandering about a box `w` x `h` world units round
/// the middle, and food strewn about it.
fn arena(n: u16, mass: u32, w: f32, h: f32) -> Mirror {
    let mut f = Frame::default();
    for id in 1..=n {
        let mut p = (
            (id as f32 * 0.37).sin() * w * 0.3,
            (id as f32 * 0.61).cos() * h * 0.3,
        );
        let mut a = id as f32 * 2.1;
        let mut points = Vec::new();
        for k in 0..body_len(mass as f32) {
            points.push((q(p.0), q(p.1)));
            // Wander, and turn back toward the middle near the edge.
            let home = (-p.1).atan2(-p.0);
            let out = (p.0 / (w * 0.42)).abs().max((p.1 / (h * 0.42)).abs());
            let turn = 0.05 * (k as f32 * 0.03 + id as f32).sin();
            let back = (home - a).sin() * 0.25 * (out - 0.6).max(0.0);
            a += turn + back;
            p = (p.0 + a.cos() * STEP, p.1 + a.sin() * STEP);
        }
        f.snakes.push(SnakeUpdate {
            id,
            boosting: id == 2,
            ghost: false,
            mass,
            len: points.len() as u16,
            angle: 0,
            new: Some((format!("snake {id}"), (id * 37) as u8)),
            points,
        });
    }
    for id in 0..200u32 {
        let (x, y) = ((id as f32 * 1.3).sin(), (id as f32 * 0.7).cos());
        f.food.push(FoodInfo {
            id,
            x: q(x * w * 0.5),
            y: q(y * h * 0.5),
            value: 1 + (id % 3) as u8,
            hue: (id * 53) as u8,
        });
    }
    let mut m = Mirror::default();
    m.apply(&f);
    m
}

/// Milliseconds a picture of `w` x `h` pixels at `k` pixels a world unit
/// takes, and the last picture.
fn time(m: &Mirror, w: i32, h: i32, k: f32, pictures: u32) -> (f64, Canvas) {
    let mut c = Canvas::new(w, h);
    let v = View {
        w: w as f32,
        h: h as f32,
        k,
        cx: 0.0,
        cy: 0.0,
    };
    let scene = Scene {
        mirror: m,
        alpha: 0.5,
        arena: 2400.0,
        gulps: &[],
        bursts: &[],
        steer: None,
        names: Some(1),
    };
    // One picture first, so nothing is timed cold; then the middle time of
    // many, so a busy machine's slow moments do not count.
    look::world(&mut c, &v, &scene, 0.0);
    let mut ms: Vec<f64> = (0..pictures.max(1))
        .map(|i| {
            let t = Instant::now();
            look::world(&mut c, &v, &scene, i as f64 * 16.7);
            t.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    ms.sort_by(f64::total_cmp);
    (ms[ms.len() / 2], c)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let pictures: u32 = args.next().and_then(|a| a.parse().ok()).unwrap_or(60);
    // Where to write each page's picture (PPM), to see what was timed.
    let out = args.next();
    // The page: a 960 x 600 buffer, as a small snake on a desktop sees it.
    // The hub: wyrm's card, a smaller window on the same arena.
    let page = (960, 600, view_scale(18.0, 1280.0, 800.0) * 0.75);
    let hub = (182, 104, 0.8 * view_scale(18.0, 1280.0, 800.0));
    let (w, h) = (1.5 * page.0 as f32 / page.2, 1.5 * page.1 as f32 / page.2);
    for (n, mass) in [(8, 30), (8, 300), (4, 1000), (2, 3000)] {
        let m = arena(n, mass, w, h);
        for (what, (w, h, k)) in [("page", page), ("hub", hub)] {
            let (ms, c) = time(&m, w, h, k, pictures);
            println!("{what}: {n} snakes of mass {mass}: {ms:.2} ms a picture");
            if let Some(dir) = out.as_ref().filter(|_| what == "page") {
                let mut ppm = format!("P6 {w} {h} 255\n").into_bytes();
                for px in c.data.chunks_exact(4) {
                    ppm.extend_from_slice(&px[..3]);
                }
                let path = format!("{dir}/bench-{n}x{mass}.ppm");
                std::fs::write(path, ppm).expect("the picture is written");
            }
        }
    }
}
