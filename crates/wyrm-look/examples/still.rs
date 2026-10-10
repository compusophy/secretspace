//! Draws one moment of a bot arena, as the page would, to a PPM image:
//! for looking at the look without a browser. One snake is shown as a
//! ghost, one boosting, and a burst plays by the one watched.
//! cargo run -p secretspace-wyrm-look --example still -- out.ppm [minutes] [zoom] [burst age ms]

use look::{Burst, Live, View};
use pixels::Canvas;
use wyrm::laws::{view_scale, TICK_HZ};
use wyrm::proto::{self, Down};
use wyrm::view::Viewer;
use wyrm::world::World;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap_or_else(|| "still.ppm".into());
    let minutes: u32 = args.next().and_then(|a| a.parse().ok()).unwrap_or(4);
    let zoom: f32 = args.next().and_then(|a| a.parse().ok()).unwrap_or(1.0);
    let age: f64 = args.next().and_then(|a| a.parse().ok()).unwrap_or(160.0);
    let (w, h) = (640, 400);
    let mut world = World::new(3);
    let mut viewer = Viewer::default();
    viewer.screen = (1280.0, 800.0);
    let mut live = Live::default();
    live.receive(0.0, Down::decode(&proto::hello(2400, 20)).unwrap());
    let mut now = 0.0;
    for _ in 0..minutes * 60 * TICK_HZ {
        world.step();
        now += 50.0;
        let f = Down::decode(&viewer.frame(&world)).unwrap();
        live.receive(now, f);
    }
    live.age(now);
    let centre = live.mirror.centre;
    // The snakes nearest the middle: the second a ghost, the third boosting.
    let mut ids: Vec<(f32, u16)> = live
        .mirror
        .snakes
        .values()
        .map(|s| {
            let (x, y) = s.body[0];
            ((x - centre.0).powi(2) + (y - centre.1).powi(2), s.id)
        })
        .collect();
    ids.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (k, (_, id)) in ids.iter().enumerate() {
        let s = live.mirror.snakes.get_mut(id).unwrap();
        s.ghost = k == 1;
        s.boosting = k == 2;
    }
    live.bursts.push(Burst {
        x: centre.0 + 120.0,
        y: centre.1 - 60.0,
        hue: 30,
        r: 20.0,
        at: now - age,
    });
    let mut c = Canvas::new(w, h);
    let at = now + 25.0;
    let v = View {
        w: w as f32,
        h: h as f32,
        k: zoom * view_scale(18.0, 1280.0, 800.0) / 2.0,
        cx: centre.0,
        cy: centre.1,
    };
    look::world(&mut c, &v, &live.scene(at, None, Some(1)), at);
    let mut out = format!("P6 {w} {h} 255\n").into_bytes();
    for px in c.data.chunks_exact(4) {
        out.extend_from_slice(&px[..3]);
    }
    std::fs::write(&path, out).expect("the image is written");
}
