//! The match as someone watching it sees it. Fed what the room tells a
//! watcher, it keeps the island and the match (`State`), follows the
//! action over a fighter's shoulder (staying with one until it falls or
//! the fight moves elsewhere), and draws a frame into any target. The
//! hub's card is one.

use std::collections::HashMap;

use render::{Camera, Renderer, V3};
use wandfall::map::{Kind, Map};
use wandfall::proto::{self, flag, Seen};
use wandfall::trig;

use crate::fx::Draw;
use crate::look::Look;
use crate::rig::Anim;
use crate::scene::{self, Eyes, Show};
use crate::sky::{Hour, Sky, Weather};
use crate::state::State;

/// Ms to stay with one fighter at least, and at most while others fight.
const STAY: f64 = 6000.0;
const RESTLESS: f64 = 16000.0;
/// Metres behind and above it; ms the camera takes to settle; how near a
/// foe must be to be looked toward.
const BACK: f32 = 4.6;
const UP: f32 = 1.9;
const FOE: f32 = 45.0;
const EASE: f32 = 350.0;
const FOV: f32 = 1.05;

/// How much of the way from `a` to `b` (0 to 1) is clear to see along:
/// no trunk, stone or tower across it, no tree's crown about it.
fn clear(map: &Map, a: V3, b: V3) -> f32 {
    const STEPS: usize = 12;
    for k in 1..=STEPS {
        let t = k as f32 / STEPS as f32;
        let p = [
            a[0] + (b[0] - a[0]) * t,
            a[1] + (b[1] - a[1]) * t,
            a[2] + (b[2] - a[2]) * t,
        ];
        let crown = map.near(p[0], p[2], 2.0).any(|q| {
            matches!(q.kind, Kind::Tree | Kind::Shroom)
                && p[1] > q.y + q.h * 0.3
                && p[1] < q.y + q.h * 1.05
        });
        // Rocks look bigger than what blocks wizards; give them room.
        let solid = map.near(p[0], p[2], 0.8).any(|q| {
            let tall = if q.kind == Kind::Rock { q.h * 1.7 } else { q.h };
            p[1] > q.y - 0.2 && p[1] < q.y + tall + 0.2
        });
        if crown || solid {
            return (k - 1) as f32 / STEPS as f32;
        }
    }
    1.0
}

#[derive(Default)]
pub struct Spectator {
    pub st: State,
    anims: HashMap<u16, Anim>,
    island: Option<(u64, Map, Look)>,
    /// Who the camera follows, and since when.
    follow: (u16, f64),
    /// The camera, eased: where it is and which way it looks.
    eye: Option<(V3, f32, f32)>,
    last: f64,
    frame_at: f64,
    /// The sky, turning with the island's day.
    sky: Sky,
}

impl Spectator {
    pub fn new() -> Spectator {
        Spectator::default()
    }

    /// What the room said.
    pub fn message(&mut self, b: &[u8], now: f64) {
        if let Some((_, _, seed, _)) = proto::read_welcome(b) {
            self.st.seed = Some(seed);
        } else if let Some(list) = proto::read_roster(b) {
            self.st.names = list
                .into_iter()
                .map(|(id, bot, n)| (id, (n, bot)))
                .collect();
        } else if let Some(list) = proto::read_events(b) {
            self.st.events(list, now);
        } else if let Some(l) = proto::Loot::decode(b) {
            self.st.loot = l;
        } else if let Some(f) = proto::Frame::decode(b) {
            self.st.take(f, now);
            self.frame_at = now;
        }
    }

    /// Whether there is a match to show: frames coming.
    pub fn live(&self, now: f64) -> bool {
        self.st.seed.is_some() && now - self.frame_at < 3000.0
    }

    /// Who the camera follows now.
    pub fn following(&self) -> u16 {
        self.follow.0
    }

    /// Let the island's meshes go.
    pub fn free(&mut self, r: &mut Renderer) {
        if let Some((_, _, look)) = self.island.take() {
            look.free(r);
        }
    }

    /// A frame of the match into `target` (`size` pixels).
    pub fn draw(
        &mut self,
        r: &mut Renderer,
        encoder: &mut render::wgpu::CommandEncoder,
        target: &render::wgpu::TextureView,
        size: (u32, u32),
        now: f64,
    ) {
        let dt = (now - self.last).clamp(0.0, 250.0);
        self.last = now;
        // The island, built when its seed is new.
        let Some(seed) = self.st.seed else {
            return;
        };
        if self.island.as_ref().map(|i| i.0) != Some(seed) {
            self.free(r);
            let map = Map::new(seed);
            let look = Look::new(r, &map);
            self.island = Some((seed, map, look));
        }
        let others = self.st.others(now);
        let aspect = size.0 as f32 / size.1.max(1) as f32;
        let cam = self.camera(&others, now, dt as f32, aspect);
        let Some((_, _, look)) = &self.island else {
            return;
        };
        let mut d = Draw::default();
        let show = Show {
            hold: None,
            storm: true,
        };
        let eyes = Eyes::default();
        let in_storm = scene::draw(
            look,
            &mut d,
            &mut self.st,
            &mut self.anims,
            &others,
            &eyes,
            &cam,
            (now, dt),
            show,
        );
        let (hour, weather) = self
            .st
            .frame
            .as_ref()
            .map_or((Hour::Dusk, Weather::Clear), |f| {
                (Hour::from(f.hour), Weather::from(f.weather))
            });
        let wet = self
            .sky
            .weigh(now, |_, w| (w == Weather::Rain) as i32 as f32);
        let look = self.sky.look((hour, weather), in_storm, now);
        crate::fx::rain(&mut d, cam.eye, (now / 1000.0) as f32, wet, look.wind);
        let frame = render::Frame {
            cam,
            look,
            time: (now / 1000.0) as f32,
            items: &d.items,
            lights: &d.lights,
            sparks: &d.sparks,
            view_fov: 0.9,
        };
        r.draw(encoder, target, size, &frame);
    }

    /// Over the shoulder of whoever is in the thick of it; high over the
    /// Spire when no one stands.
    fn camera(&mut self, others: &[Seen], now: f64, dt: f32, aspect: f32) -> Camera {
        let fight = self.st.frame.as_ref().is_some_and(|f| f.phase == 1);
        let standing: Vec<&Seen> = others
            .iter()
            .filter(|s| s.flags & flag::ALIVE != 0 && (!fight || s.flags & flag::ENTRANT != 0))
            .collect();
        // When each last hurt someone or was hurt: the thick of it.
        let busy = |id: u16| {
            let hit = self
                .st
                .bursts
                .iter()
                .filter(|b| b.1 == id)
                .map(|b| b.0)
                .fold(-1e9, f64::max);
            let fired = self.st.fired.get(&id).copied().unwrap_or(-1e9);
            hit.max(fired)
        };
        let held = now - self.follow.1;
        let here = standing.iter().any(|s| s.id == self.follow.0);
        let stale = now - busy(self.follow.0) > 4000.0;
        if !here || held > RESTLESS || (held > STAY && stale) {
            let pick = standing
                .iter()
                .max_by(|a, b| busy(a.id).total_cmp(&busy(b.id)).then(b.id.cmp(&a.id)));
            if let Some(s) = pick {
                if s.id != self.follow.0 || !here {
                    self.follow = (s.id, now);
                }
            }
        }
        let (want, yaw, pitch) = match standing.iter().find(|s| s.id == self.follow.0) {
            Some(s) => {
                // Behind it, looking past it at its nearest foe (the duel
                // in view), else toward the storm's safe ground, else the
                // way it faces; from another side when a tree or a stone
                // stands in the way.
                let toward = |x: f32, z: f32| (z - s.p[2]).atan2(x - s.p[0]);
                let d2 = |o: &Seen| (o.p[0] - s.p[0]).powi(2) + (o.p[2] - s.p[2]).powi(2);
                let foe = standing
                    .iter()
                    .filter(|o| o.id != s.id)
                    .min_by(|a, b| d2(a).total_cmp(&d2(b)))
                    .filter(|o| d2(o) < FOE * FOE);
                let storm = self
                    .st
                    .frame
                    .as_ref()
                    .filter(|f| f.phase == 1)
                    .map(|f| f.storm.0);
                let a = match (foe, storm) {
                    (Some(o), _) => toward(o.p[0], o.p[2]),
                    (None, Some(c)) if (c[0] - s.p[0]).hypot(c[1] - s.p[2]) > 12.0 => {
                        toward(c[0], c[1])
                    }
                    _ => trig::radians(s.yaw),
                } + ((now / 7000.0) as f32).sin() * 0.25;
                let chest = [s.p[0], s.p[1] + 1.2, s.p[2]];
                let from = |a: f32| {
                    [
                        chest[0] - a.cos() * BACK,
                        chest[1] + UP,
                        chest[2] - a.sin() * BACK,
                    ]
                };
                let mut best = (a, -1.0);
                if let Some((_, map, _)) = &self.island {
                    for off in [0.0, 0.7, -0.7, 1.4, -1.4, 2.2, -2.2, std::f32::consts::PI] {
                        let clear = clear(map, chest, from(a + off));
                        if clear > best.1 + 0.05 {
                            best = (a + off, clear);
                        }
                        if clear >= 1.0 {
                            break;
                        }
                    }
                }
                let a = best.0;
                (from(a), a, -(UP / BACK).atan())
            }
            None => {
                let a = (now / 20000.0) as f32;
                let eye = [a.cos() * 70.0, 45.0, a.sin() * 70.0];
                (eye, a + std::f32::consts::PI, -0.5)
            }
        };
        // Never under the ground.
        let want = match &self.island {
            Some((_, map, _)) => {
                let floor = map.height(want[0], want[2]).max(wandfall::laws::SEA) + 0.8;
                [want[0], want[1].max(floor), want[2]]
            }
            None => want,
        };
        let k = 1.0 - (-dt / EASE).exp();
        let (eye, y, p) = match self.eye {
            Some((e, y0, p0)) => {
                let mut dy = yaw - y0;
                while dy > std::f32::consts::PI {
                    dy -= std::f32::consts::TAU;
                }
                while dy < -std::f32::consts::PI {
                    dy += std::f32::consts::TAU;
                }
                (
                    [
                        e[0] + (want[0] - e[0]) * k,
                        e[1] + (want[1] - e[1]) * k,
                        e[2] + (want[2] - e[2]) * k,
                    ],
                    y0 + dy * k,
                    p0 + (pitch - p0) * k,
                )
            }
            None => (want, yaw, pitch),
        };
        // A long way to go (a new match, far off): there at once.
        let jump = (eye[0] - want[0]).powi(2) + (eye[2] - want[2]).powi(2) > 80.0 * 80.0;
        let (eye, y, p) = if jump {
            (want, yaw, pitch)
        } else {
            (eye, y, p)
        };
        self.eye = Some((eye, y, p));
        Camera {
            eye,
            yaw: y,
            pitch: p,
            fov: FOV,
            aspect,
        }
    }
}
