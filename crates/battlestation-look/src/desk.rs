//! The desk as the engine draws it: every mesh made once (`Desk::new`),
//! then each frame what moves and glows (the keys, the light under them,
//! the mouse, the hands, the fans, the strip, the neon), the lights, the
//! steam, the room drawn by the engine, and the monitor's picture laid on
//! its glass. The page draws it from the chair; the front page's card,
//! the same desk typing by itself.

use battlestation::hands::Hands;
use battlestation::keys::{self, Side};
use battlestation::laws::{EYE, FOV, FOV_MOST, PITCH, SCREEN_AT, SCREEN_PX, WALL_N};
use render::geo::{self, Geo};
use render::{m4, wgpu, Camera, Frame, Item, Material, Mesh, Pass, Renderer, M4};

use crate::body::{self, HandMeshes};
use crate::glass::{Glass, Show};
use crate::monitor::Screen;
use crate::{gear, light, scene};

/// The meshes of what moves or changes colour.
pub struct Meshes {
    pub keys: Vec<Mesh>,
    pub under: Mesh,
    pub mouse: Mesh,
    pub hands: HandMeshes,
    pub pane: Mesh,
    pub fan: Mesh,
    pub strip: Mesh,
    pub neon: Mesh,
    pub tower: Mesh,
}

/// The light a computer's picture throws (its mean is not read back).
pub const OS_MEAN: [f32; 3] = [0.06, 0.065, 0.085];

pub struct Desk {
    pub m: Meshes,
    pub glass: Glass,
    /// The hands' skin (of `body::TONES`).
    pub tone: usize,
}

fn ident() -> M4 {
    m4::place([0.0; 3], 0.0, [1.0; 3])
}

/// What never moves: the room, the desk and what stands on it.
fn statics(r: &mut Renderer) {
    let id = ident();
    let mut items = Vec::new();
    let mut put = |r: &mut Renderer, g: &Geo, f: &dyn Fn(Item) -> Item| {
        let mesh = r.mesh(g);
        items.push(f(Item::new(mesh, id)));
    };
    put(r, &scene::room(), &|i| i.rough(0.92));
    put(r, &scene::desk(), &|i| i.rough(0.45).detail(0.04));
    put(r, &scene::mat(), &|i| {
        i.material(Material::Cloth).rough(0.95)
    });
    put(r, &scene::lamp(), &|i| i.rough(0.4));
    put(r, &scene::plant(), &|i| i.rough(0.7));
    put(r, &scene::mug(), &|i| i.rough(0.25));
    put(r, &scene::tower(), &|i| i.rough(0.35));
    put(r, &gear::case(), &|i| i.rough(0.35));
    put(r, &gear::stand(), &|i| i.rough(0.4));
    put(r, &scene::city(), &|i| i.rough(0.9));
    put(r, &scene::glass(), &|i| {
        i.pass(Pass::Faint)
            .tint([0.1, 0.12, 0.16], 0.12)
            .rough(0.05)
    });
    let [x, y, z] = gear::screen_axes();
    let monitor = r.mesh(&gear::monitor());
    items.push(Item::new(monitor, m4::basis(SCREEN_AT, x, y, z)).rough(0.3));
    r.statics(items);
}

fn hand_meshes(r: &mut Renderer, tone: usize) -> HandMeshes {
    let g = body::hand_geo(body::TONES[tone % body::TONES.len()]);
    let seg: Vec<Mesh> = g.seg.iter().map(|s| r.mesh(s)).collect();
    HandMeshes {
        palm: [r.mesh(&g.palm[0]), r.mesh(&g.palm[1])],
        seg: std::array::from_fn(|k| seg[k]),
        arm: r.mesh(&g.arm),
        sleeve: r.mesh(&g.sleeve),
    }
}

/// The view's height for a screen this wide (width over height): wider
/// on a narrow screen, so the desk stays in it.
pub fn fov(aspect: f32) -> f32 {
    (2.0 * (0.55f32.tan() / aspect.max(0.1)).atan()).clamp(FOV, FOV_MOST)
}

/// The view from the chair, sitting back.
pub fn seated(aspect: f32) -> Camera {
    Camera {
        eye: EYE,
        yaw: -std::f32::consts::FRAC_PI_2,
        pitch: PITCH,
        fov: fov(aspect),
        aspect,
    }
}

impl Desk {
    /// The desk on this renderer (its statics set), its picture's glass for
    /// targets of `format`.
    pub fn new(
        r: &mut Renderer,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        tone: usize,
    ) -> Desk {
        statics(r);
        let m = Meshes {
            keys: keys::layout()
                .iter()
                .map(|k| r.mesh(&gear::key_mesh(k)))
                .collect(),
            under: r.mesh(&gear::underglow()),
            mouse: r.mesh(&gear::mouse()),
            hands: hand_meshes(r, tone),
            pane: r.mesh(&gear::pane()),
            fan: r.mesh(&scene::fan_ring()),
            strip: r.mesh(&scene::strip_part()),
            neon: r.mesh(&scene::neon("secretspace")),
            tower: r.mesh(&scene::tower_lights()),
        };
        let glass = Glass::new(device, format, (SCREEN_PX.0 as u32, SCREEN_PX.1 as u32));
        Desk { m, glass, tone }
    }

    /// The hands in the next skin tone.
    pub fn next_tone(&mut self, r: &mut Renderer) {
        self.tone = (self.tone + 1) % body::TONES.len();
        let old = self.m.hands;
        self.m.hands = hand_meshes(r, self.tone);
        for m in old
            .palm
            .iter()
            .chain(&old.seg)
            .chain([&old.arm, &old.sleeve])
        {
            r.free(*m);
        }
    }

    /// What moves and what glows: `heat` is each key's light after a
    /// press (1 just pressed), `mean` the screen's mean colour.
    pub fn items(&self, hands: &Hands, heat: &[f32], mean: [f32; 3], t: f32) -> Vec<Item> {
        let m = &self.m;
        let mut out = Vec::with_capacity(280);
        for (i, k) in keys::layout().iter().enumerate() {
            let at = gear::cap_at(k);
            let hue = light::rainbow(at[0], t);
            let heat = heat.get(i).copied().unwrap_or(0.0);
            let tint = geo::mix(geo::mix([1.0; 3], hue, 0.8), [1.0; 3], heat * 0.8);
            let down = [at[0], at[1] - hands.depth(i), at[2]];
            out.push(
                Item::new(m.keys[i], m4::place(down, 0.0, [1.0; 3]))
                    .tint(tint, 1.0)
                    .glow(heat * 0.25)
                    .rough(0.6),
            );
            out.push(
                Item::new(
                    m.under,
                    m4::place([at[0], at[1] - 0.004, at[2]], 0.0, [k.w, 1.0, 1.0]),
                )
                .pass(Pass::Glow)
                .tint(geo::mix(hue, [1.0; 3], heat * 0.5), 1.0)
                .glow(0.2 + heat * 2.2),
            );
        }
        let at = hands.mouse.at();
        out.push(
            Item::new(m.mouse, m4::place(at, 0.0, [1.0; 3]))
                .tint(geo::mix([1.0; 3], light::rainbow(at[0], t), 0.9), 1.0)
                .rough(0.45),
        );
        for side in [Side::Left, Side::Right] {
            body::items(&hands.pose(side), side, &m.hands, &mut out);
        }
        let [x, y, z] = gear::screen_axes();
        out.push(
            Item::new(m.pane, m4::basis(SCREEN_AT, x, y, z))
                .tint(geo::add(mean, [0.04, 0.05, 0.08]), 1.0)
                .glow(1.3),
        );
        for (k, c) in scene::FANS.iter().enumerate() {
            out.push(
                Item::new(m.fan, m4::place(*c, 0.0, [1.0; 3]))
                    .pass(Pass::Glow)
                    .tint(light::rainbow(0.5 + k as f32 * 0.12, t), 1.0)
                    .glow(2.2),
            );
        }
        let (x0, x1, sy) = scene::STRIP;
        let len = (x1 - x0) / scene::STRIP_PARTS as f32;
        for k in 0..scene::STRIP_PARTS {
            let sx = x0 + k as f32 * len;
            out.push(
                Item::new(
                    m.strip,
                    m4::place([sx, sy, WALL_N + 0.003], 0.0, [len, 1.0, 1.0]),
                )
                .pass(Pass::Glow)
                .tint(light::rainbow(sx + len / 2.0, t), 1.0)
                .glow(2.6),
            );
        }
        out.push(
            Item::new(m.tower, ident())
                .pass(Pass::Glow)
                .tint(light::rainbow(0.6, t), 1.0)
                .glow(2.4),
        );
        out.push(
            Item::new(m.neon, m4::place(scene::NEON_AT, 0.0, [1.0; 3]))
                .pass(Pass::Glow)
                .tint(light::NEON, 1.0)
                .glow(light::neon_glow(t)),
        );
        out
    }

    /// One frame of the desk into `view` (`size` pixels) as `cam` sees it:
    /// the room by the engine, then on the glass the desk's own `screen`,
    /// or (`os`: the arrow at that point of it) the computer's picture.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        r: &mut Renderer,
        queue: &wgpu::Queue,
        (encoder, view, size): (&mut wgpu::CommandEncoder, &wgpu::TextureView, (u32, u32)),
        cam: Camera,
        (hands, heat): (&Hands, &[f32]),
        (screen, os): (&Screen, Option<(f32, f32)>),
        t: f32,
    ) {
        let mean = if os.is_some() { OS_MEAN } else { screen.mean() };
        let items = self.items(hands, heat, mean, t);
        let lights = light::lights(t, mean);
        let sparks = light::steam(t);
        let frame = Frame {
            cam,
            look: light::look(),
            time: t,
            items: &items,
            lights: &lights,
            sparks: &sparks,
            decals: &[],
            view_fov: 0.9,
        };
        r.draw(encoder, view, size, &frame);
        let (vp, _, _) = cam.matrices(cam.fov);
        let show = match os {
            Some(cursor) => Show::Os { cursor },
            None => Show::Pixels(&screen.c.data),
        };
        self.glass.draw(queue, encoder, view, &vp, show, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_seated_view_widens_for_a_phone() {
        assert!((seated(16.0 / 9.0).fov - FOV).abs() < 1e-6);
        let phone = seated(390.0 / 844.0).fov;
        assert!(phone > FOV && phone <= FOV_MOST);
    }
}
