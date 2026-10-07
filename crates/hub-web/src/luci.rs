//! The live preview on Luciphon's card: the hub watches the Sanctum (as a
//! watcher, never one of the people there) and draws it with the game's
//! own look.

use lucilook::{Body, Look, View};
use luciphon::mirror::Mirror;
use luciphon::proto::Down;
use pixels::{Canvas, Rect};

pub struct LuciWatch {
    link: Option<kit::Link>,
    mirror: Mirror,
    look: Look,
    buf: Canvas,
}

impl LuciWatch {
    pub fn new() -> LuciWatch {
        LuciWatch {
            link: None,
            mirror: Mirror::default(),
            look: Look::default(),
            buf: Canvas::new(1, 1),
        }
    }

    fn poll(&mut self, now: f64) {
        let link = self
            .link
            .get_or_insert_with(|| kit::Link::open("luciphon", Vec::new(), true));
        for ev in link.poll(now) {
            let kit::Net::Message(b) = ev else { continue };
            let Some(d) = Down::decode(&b) else { continue };
            self.mirror.apply(&d);
            match d {
                Down::Chunk { cx, cy, .. } => {
                    self.look
                        .ground
                        .bake(&self.mirror.tiles, cx as i32, cy as i32)
                }
                Down::ChunkGone { cx, cy } => self.look.ground.forget(cx as i32, cy as i32),
                _ => {}
            }
        }
    }

    /// The Sanctum, live, in `b`.
    pub fn draw(&mut self, c: &mut Canvas, b: Rect, round: f32, u: i32, now: f64) {
        self.poll(now);
        self.buf.resize(b.w as i32, b.h as i32);
        let bodies: Vec<Body> = self
            .mirror
            .ents
            .values()
            .map(|e| Body {
                id: e.id,
                kind: e.kind,
                x: e.x as f32 / 256.0,
                y: e.y as f32 / 256.0,
                facing: e.facing,
                state: e.state,
                flame: e.flame,
                glim: e.glim,
                hue: e.hue,
                flow: e.flow,
                name: String::new(),
                moving: e.state & 7 != 0 || e.state >> 3 & 7 != 0,
                you: false,
            })
            .collect();
        let view = View {
            cx: 0.5,
            cy: -0.5,
            sight: 40.0,
            u,
        };
        lucilook::world(
            &mut self.buf,
            &mut self.look,
            &view,
            &self.mirror.tiles,
            &bodies,
            now,
        );
        c.blit(&self.buf, b.x as i32, b.y as i32, round);
    }
}
