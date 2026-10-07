//! The island in first person, drawn by the GPU: a mesh a chunk (built
//! once, again when a tile in it changes), the sky, every Lumen, mote and
//! glim in sight, the nearest sixteen lights, sparks, the build ghost and
//! the rings of nodes, and your own hand. World tile (x, y) at height h
//! is (x, h, y) here.

pub mod chunk;
pub mod shaders;
pub mod shapes;
pub mod things;

use std::collections::HashMap;

use kit::gl::{m4, Gl, Mesh, Program, M4};
use luciphon::proto::kind;
use luciphon::tiles::Tiles;
use web_sys::WebGl2RenderingContext as GL;

use crate::state::Thing;
use shapes::{mix, rgb, scale, LAYOUT, V3};
use things::{Light, GOLD};

pub const FOG: V3 = rgb(14, 16, 32);
const ZENITH: V3 = rgb(3, 4, 10);
const DEEP: V3 = rgb(2, 2, 6);
const SKY: V3 = rgb(50, 55, 86);
const LOW: V3 = rgb(16, 14, 20);
const MOON: V3 = rgb(84, 88, 116);

/// Where the camera is and where it looks.
#[derive(Clone, Copy, Debug, Default)]
pub struct Camera {
    pub eye: V3,
    pub yaw: f32,
    pub pitch: f32,
    pub fov: f32,
    pub aspect: f32,
    pub far: f32,
}

impl Camera {
    pub fn forward(&self) -> V3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        [cy * cp, sp, sy * cp]
    }

    /// The view-projection, and the camera's right and up.
    pub fn matrices(&self) -> (M4, V3, V3) {
        let (v, right, up) = m4::look(self.eye, self.forward(), [0.0, 1.0, 0.0]);
        let p = m4::perspective(self.fov, self.aspect, 0.05, self.far);
        (m4::mul(&p, &v), right, up)
    }
}

/// Your own hand, before you: its hue, how bright, and where in a swing.
#[derive(Clone, Copy, Debug, Default)]
pub struct Hand {
    pub hue: V3,
    pub flame: f32,
    /// -1 drawn back (a wind-up) through 1 (struck across).
    pub swing: f32,
    /// A charge, 0 to 1 (gold at the perfect window).
    pub charge: f32,
    pub perfect: bool,
}

/// Where your wand is held, and its tip, for the camera and where it is
/// in its kick (1 just fired, -1 drawn back).
pub fn wand(cam: &Camera, swing: f32) -> (V3, V3) {
    let fwd = cam.forward();
    let (_, right, up) = cam.matrices();
    let kick = swing.max(0.0);
    let at = |ahead: f32, side: f32, lift: f32| {
        shapes::add(
            cam.eye,
            shapes::add(
                scale(fwd, ahead),
                shapes::add(scale(right, side), scale(up, lift)),
            ),
        )
    };
    let back = 0.07 * kick + 0.05 * (-swing).max(0.0);
    let grip = at(0.36 - back, 0.2, -0.27);
    let tip = at(0.7 - back, 0.16, -0.21 + 0.06 * kick);
    (grip, tip)
}

/// Everything one frame draws.
pub struct Frame<'a> {
    pub cam: Camera,
    pub things: &'a [Thing],
    pub hand: Option<Hand>,
    /// The build ghost: its tile, whether it may go there, a hearth's.
    pub ghost: Option<(i32, i32, bool, bool)>,
    /// Rings on the ground: where, how wide, how bright.
    pub rings: Vec<(V3, f32, f32)>,
    /// A strike's reach before you: yaw, reach, how bright.
    /// Beams of light in the air: from, to, how thick, colour and alpha.
    pub beams: Vec<(V3, V3, f32, [f32; 4])>,
    pub sparks: &'a [f32],
    /// Your own light (hue, flame) where you are.
    pub you: Option<(V3, V3)>,
    pub under: bool,
    pub now: f64,
    pub fog: (f32, f32),
}

struct Chunk {
    mesh: Mesh,
    lights: Vec<Light>,
    centre: V3,
}

pub struct Scene {
    world: Program,
    sky: Program,
    points: Program,
    chunks: HashMap<(i32, i32), Chunk>,
    robe: Mesh,
    glow: Mesh,
    mote: Mesh,
    glim: Mesh,
    /// Every kind of monster: its body and its light.
    beasts: Vec<(Mesh, Mesh)>,
    hand: Mesh,
    cube: Mesh,
    ring: Mesh,
    rod: Mesh,
    screen: Mesh,
    sparks: Mesh,
    pub vertices: usize,
}

fn mesh(gl: &GL, g: &shapes::Geo) -> Result<Mesh, String> {
    Mesh::of(gl, &LAYOUT, GL::TRIANGLES, &g.v).ok_or_else(|| "no mesh".to_string())
}

/// A rarity's colour (0 common glim gold, then fine, rare, radiant), for
/// gear lying on the ground.
pub fn rarity(item: u8) -> V3 {
    match luciphon::gear::get(item).map(|g| g.rarity) {
        Some(1) => rgb(127, 224, 255),
        Some(2) => rgb(200, 150, 255),
        Some(3) => rgb(255, 236, 160),
        Some(_) => rgb(244, 238, 222),
        None => GOLD,
    }
}

/// A flame's colour: embers when low, gold, then white-hot when full.
pub fn flame(f: f32) -> V3 {
    let f = f.clamp(0.0, 1.0);
    if f < 0.5 {
        mix(rgb(255, 90, 50), GOLD, f * 2.0)
    } else {
        mix(GOLD, rgb(255, 246, 225), (f - 0.5) * 2.0)
    }
}

impl Scene {
    pub fn new(g: &Gl) -> Result<Scene, String> {
        let gl = &g.gl;
        let (robe, glow) = things::lumen();
        Ok(Scene {
            world: Program::new(gl, shaders::WORLD_VS, shaders::WORLD_FS)?,
            sky: Program::new(gl, shaders::SKY_VS, shaders::SKY_FS)?,
            points: Program::new(gl, shaders::POINTS_VS, shaders::POINTS_FS)?,
            chunks: HashMap::new(),
            robe: mesh(gl, &robe)?,
            glow: mesh(gl, &glow)?,
            mote: mesh(gl, &things::mote())?,
            glim: mesh(gl, &things::glim())?,
            beasts: (0..3u8)
                .map(|k| {
                    let (body, glow) = things::beast(k);
                    Ok((mesh(gl, &body)?, mesh(gl, &glow)?))
                })
                .collect::<Result<Vec<_>, String>>()?,
            hand: mesh(gl, &things::hand())?,
            cube: mesh(gl, &things::cube())?,
            ring: mesh(gl, &things::ring())?,
            rod: mesh(gl, &things::rod())?,
            screen: Mesh::of(gl, &[2], GL::TRIANGLES, &[-1.0, -1.0, 3.0, -1.0, -1.0, 3.0])
                .ok_or("no mesh")?,
            sparks: Mesh::new(gl, &[3, 4, 1], GL::POINTS).ok_or("no mesh")?,
            vertices: 0,
        })
    }

    /// (Re)build a chunk from the tiles the page knows.
    pub fn build(&mut self, gl: &GL, tiles: &Tiles, at: (i32, i32), hues: &HashMap<u16, u8>) {
        let b = chunk::build(tiles, at.0, at.1, hues);
        self.forget(gl, at);
        if b.geo.is_empty() {
            return;
        }
        self.vertices += b.geo.len();
        if let Ok(mesh) = mesh(gl, &b.geo) {
            let c = luciphon::tiles::CHUNK as f32;
            let centre = [
                at.0 as f32 * c - 64.0 + c / 2.0,
                0.0,
                at.1 as f32 * c - 64.0 + c / 2.0,
            ];
            self.chunks.insert(
                at,
                Chunk {
                    mesh,
                    lights: b.lights,
                    centre,
                },
            );
        }
    }

    pub fn forget(&mut self, gl: &GL, at: (i32, i32)) {
        if let Some(c) = self.chunks.remove(&at) {
            self.vertices = self.vertices.saturating_sub(c.mesh.count as usize);
            c.mesh.free(gl);
        }
    }

    /// The lights nearest the eye that reach it, at most sixteen.
    fn lights(&self, f: &Frame) -> Vec<Light> {
        let mut all: Vec<Light> = self
            .chunks
            .values()
            .flat_map(|c| c.lights.iter().copied())
            .collect();
        for t in f.things {
            let at = [t.x, t.z, t.y];
            match t.kind {
                kind::LUMEN if !t.you => all.push(Light {
                    p: [at[0], at[1] + 0.9, at[2]],
                    r: 3.0 + 2.5 * t.flame as f32 / 100.0,
                    c: scale(
                        mix(chunk::hue(t.hue), flame(t.flame as f32 / 100.0), 0.5),
                        0.6,
                    ),
                }),
                kind::MOTE => all.push(Light {
                    p: [at[0], 0.9, at[2]],
                    r: 3.5,
                    c: scale(chunk::hue(t.hue), 0.9),
                }),
                kind::PICKUP => all.push(Light {
                    p: [at[0], 0.4, at[2]],
                    r: if t.hue != 0 { 2.8 } else { 1.8 },
                    c: scale(rarity(t.hue), 0.6),
                }),
                kind::BEAST if t.state & 7 == 2 => all.push(Light {
                    p: [at[0], 1.5, at[2]],
                    r: 4.0,
                    c: scale(rgb(127, 224, 255), 0.5),
                }),
                _ => {}
            }
        }
        if let Some((hue, fl)) = f.you {
            all.push(Light {
                p: [f.cam.eye[0], f.cam.eye[1] - 0.4, f.cam.eye[2]],
                r: 4.5,
                c: scale(mix(hue, fl, 0.6), 0.45),
            });
        }
        let e = f.cam.eye;
        let reach = |l: &Light| {
            let d = ((l.p[0] - e[0]).powi(2) + (l.p[1] - e[1]).powi(2) + (l.p[2] - e[2]).powi(2))
                .sqrt();
            d - l.r * 1.5
        };
        all.sort_by(|a, b| reach(a).total_cmp(&reach(b)));
        all.truncate(16);
        all
    }

    fn put(&self, gl: &GL, mesh: &Mesh, model: &M4, tint: [f32; 4], glow: f32) {
        let p = &self.world;
        p.mat4(gl, "u_model", model);
        p.f4(gl, "u_tint", tint);
        p.f1(gl, "u_glow", glow);
        mesh.draw(gl);
    }

    /// Draw a frame; the view-projection it used.
    pub fn draw(&mut self, g: &Gl, f: &Frame) -> M4 {
        let gl = &g.gl;
        let (vp, right, up) = f.cam.matrices();
        let fwd = f.cam.forward();
        g.begin(FOG);
        let under = if f.under { 1.0 } else { 0.0 };

        // The sky.
        gl.disable(GL::DEPTH_TEST);
        let s = &self.sky;
        s.use_on(gl);
        s.f3(gl, "u_fwd", fwd);
        s.f3(gl, "u_right", right);
        s.f3(gl, "u_up", up);
        let t = (f.cam.fov / 2.0).tan();
        s.f2(gl, "u_tan", [t * f.cam.aspect, t]);
        s.f3(gl, "u_fog", FOG);
        s.f3(gl, "u_zenith", ZENITH);
        s.f3(gl, "u_deep", DEEP);
        s.f1(gl, "u_time", f.now as f32 % 100_000.0);
        s.f1(gl, "u_under", under);
        self.screen.draw(gl);

        // The world.
        gl.enable(GL::DEPTH_TEST);
        gl.depth_func(GL::LEQUAL);
        gl.enable(GL::CULL_FACE);
        let w = &self.world;
        w.use_on(gl);
        w.mat4(gl, "u_vp", &vp);
        w.f3(gl, "u_eye", f.cam.eye);
        w.f3(gl, "u_fog", FOG);
        w.f2(gl, "u_fogr", [f.fog.0, f.fog.1]);
        w.f3(gl, "u_sky", SKY);
        w.f3(gl, "u_low", LOW);
        w.f3(gl, "u_moon_dir", shapes::norm([0.35, 0.85, 0.4]));
        w.f3(gl, "u_moon", MOON);
        w.f1(gl, "u_under", under);
        let lights = self.lights(f);
        let mut lp = Vec::with_capacity(64);
        let mut lc = Vec::with_capacity(48);
        for l in &lights {
            lp.extend_from_slice(&[l.p[0], l.p[1], l.p[2], l.r]);
            lc.extend_from_slice(&l.c);
        }
        w.i1(gl, "u_n", lights.len() as i32);
        if !lights.is_empty() {
            w.fv(gl, "u_lp", 4, &lp);
            w.fv(gl, "u_lc", 3, &lc);
        }
        let reach = f.cam.far + 24.0;
        for c in self.chunks.values() {
            let d = shapes::sub(c.centre, f.cam.eye);
            let flat = (d[0] * d[0] + d[2] * d[2]).sqrt();
            // Behind you, or too far to see.
            if flat > reach || (flat > 24.0 && shapes::dot(d, [fwd[0], 0.0, fwd[2]]) < -23.0) {
                continue;
            }
            self.put(gl, &c.mesh, &m4::ID, [1.0; 4], 0.0);
        }
        let mut faint = Vec::new();
        for t in f.things {
            self.thing(gl, t, f.now, &mut faint);
        }

        // See-through things, without writing depth.
        gl.enable(GL::BLEND);
        gl.blend_func(GL::SRC_ALPHA, GL::ONE_MINUS_SRC_ALPHA);
        gl.depth_mask(false);
        for (mesh, model, tint, glow) in faint {
            let m = match mesh {
                0 => &self.robe,
                _ => &self.glow,
            };
            self.put(gl, m, &model, tint, glow);
        }
        gl.disable(GL::CULL_FACE);
        if let Some((x, y, ok, hearth)) = f.ghost {
            let col = if ok { GOLD } else { rgb(255, 110, 90) };
            let pulse = 0.25 + 0.1 * ((f.now / 200.0).sin() as f32);
            let at = [x as f32 + 0.5, 0.0, y as f32 + 0.5];
            self.put(
                gl,
                &self.cube,
                &m4::place(at, 0.0, [1.0, 1.0, 1.0]),
                [col[0], col[1], col[2], pulse],
                1.0,
            );
            if hearth {
                let m = m4::place(at, 0.0, [5.0, 0.05, 5.0]);
                self.put(gl, &self.cube, &m, [col[0], col[1], col[2], 0.15], 1.0);
            }
        }
        for &(at, r, a) in &f.rings {
            let m = m4::place([at[0], at[1] + 0.03, at[2]], 0.0, [r, 1.0, r]);
            self.put(gl, &self.ring, &m, [0.5, 0.88, 1.0, a], 1.0);
        }
        // Beams, added as light.
        gl.blend_func(GL::SRC_ALPHA, GL::ONE);
        for &(from, to, w, c) in &f.beams {
            let along = shapes::sub(to, from);
            let side = shapes::norm(shapes::cross(along, [0.0, 1.0, 0.0]));
            let m = m4::basis(from, along, [0.0, w, 0.0], scale(side, w));
            self.put(gl, &self.rod, &m, c, 1.0);
        }

        // Sparks, added as light.
        gl.blend_func(GL::ONE, GL::ONE);
        if !f.sparks.is_empty() {
            let p = &self.points;
            p.use_on(gl);
            p.mat4(gl, "u_vp", &vp);
            p.f1(gl, "u_px", g.size.1 as f32 / 2.0 / t);
            self.sparks.set(gl, f.sparks, true);
            self.sparks.draw(gl);
        }
        gl.depth_mask(true);
        gl.disable(GL::BLEND);

        // Your hand, over everything.
        if let Some(h) = f.hand {
            gl.clear(GL::DEPTH_BUFFER_BIT);
            gl.enable(GL::CULL_FACE);
            let w = &self.world;
            w.use_on(gl);
            let (grip, tip) = wand(&f.cam, h.swing);
            // The shaft, lit like the world; the light at its tip.
            let along = shapes::sub(tip, grip);
            let m = m4::basis(grip, along, scale(up, 0.014), scale(right, 0.014));
            self.put(gl, &self.rod, &m, [0.5, 0.36, 0.24, 1.0], 0.12);
            let size = 0.7 + 1.6 * h.charge + 0.5 * h.swing.max(0.0);
            let m = m4::basis(tip, scale(fwd, size), scale(up, size), scale(right, size));
            let col = if h.perfect {
                GOLD
            } else {
                mix(h.hue, flame(h.flame), 0.45)
            };
            self.put(gl, &self.hand, &m, [col[0], col[1], col[2], 1.0], 1.0);
        }
        gl.disable(GL::CULL_FACE);
        gl.disable(GL::DEPTH_TEST);
        vp
    }

    /// One thing; what must be drawn see-through goes in `faint`.
    fn thing(&self, gl: &GL, t: &Thing, now: f64, faint: &mut Vec<(u8, M4, [f32; 4], f32)>) {
        let yaw = t.facing as f32 / 65536.0 * std::f32::consts::TAU;
        let at = [t.x, t.z, t.y];
        match t.kind {
            kind::LUMEN if !t.you => {
                let hue = chunk::hue(t.hue);
                let fl = flame(t.flame as f32 / 100.0);
                let bob = if t.moving && t.z == 0.0 {
                    ((now / 85.0).sin().abs() * 0.05) as f32
                } else {
                    0.0
                };
                let model = if t.down() {
                    let (s, c) = yaw.sin_cos();
                    m4::basis(
                        [at[0], at[1] + 0.22, at[2]],
                        [0.0, 1.0, 0.0],
                        [-c, 0.0, -s],
                        [-s, 0.0, c],
                    )
                } else {
                    m4::place([at[0], at[1] + bob, at[2]], yaw, [1.0, 1.0, 1.0])
                };
                let stunned = t.mv() == 3 && (now / 70.0) as i64 % 2 == 0;
                let tint = if stunned { [1.0, 1.0, 1.0] } else { hue };
                let charging = t.act() == 4;
                let glow = if charging {
                    0.25 + 0.2 * ((now / 90.0).sin() as f32).abs()
                } else {
                    0.0
                };
                if t.ghost() {
                    faint.push((0, model, [tint[0], tint[1], tint[2], 0.45], glow));
                    faint.push((1, model, [fl[0], fl[1], fl[2], 0.6], 0.0));
                } else {
                    self.put(
                        gl,
                        &self.robe,
                        &model,
                        [tint[0], tint[1], tint[2], 1.0],
                        glow,
                    );
                    self.put(gl, &self.glow, &model, [fl[0], fl[1], fl[2], 1.0], 0.0);
                }
                // A dash leaves two after-images behind it.
                if t.mv() == 2 {
                    let (s, c) = yaw.sin_cos();
                    for k in 1..=2 {
                        let back = 0.45 * k as f32;
                        let m =
                            m4::place([at[0] - c * back, at[1], at[2] - s * back], yaw, [1.0; 3]);
                        faint.push((0, m, [hue[0], hue[1], hue[2], 0.3 / k as f32], 0.6));
                    }
                }
            }
            kind::MOTE => {
                let hue = chunk::hue(t.hue);
                let m = m4::place([at[0], 0.9, at[2]], yaw, [1.0; 3]);
                self.put(gl, &self.mote, &m, [hue[0], hue[1], hue[2], 1.0], 1.0);
            }
            kind::PICKUP => {
                let spin = (now / 500.0) as f32;
                let bob = 0.25 + 0.06 * ((now / 300.0 + t.id as f64).sin() as f32);
                // A piece of gear floats higher and bigger, in its rarity's
                // colour.
                let (k, lift) = if t.hue != 0 {
                    (1.8, 0.35)
                } else {
                    (0.8 + 0.1 * (t.glim as f32).min(10.0), 0.0)
                };
                let c = rarity(t.hue);
                let m = m4::place([at[0], bob + lift, at[2]], spin, [k, k, k]);
                self.put(gl, &self.glim, &m, [c[0], c[1], c[2], 1.0], 0.9);
            }
            kind::BEAST => {
                let k = (t.state & 7).min(2) as usize;
                let winding = t.state & 8 != 0;
                let staggered = t.state & 16 != 0 && (now / 60.0) as i64 % 2 == 0;
                let float = if k == 2 {
                    0.15 + 0.1 * ((now / 400.0 + t.id as f64).sin() as f32)
                } else {
                    0.0
                };
                let pulse = if winding {
                    1.0 + 0.08 * ((now / 50.0).sin() as f32).abs()
                } else {
                    1.0
                };
                let m = m4::place([at[0], float, at[2]], yaw, [pulse; 3]);
                let tint = if staggered { [2.0, 2.0, 2.0] } else { [1.0; 3] };
                let (body, glow) = &self.beasts[k];
                self.put(gl, body, &m, [tint[0], tint[1], tint[2], 1.0], 0.0);
                let eye = if winding {
                    rgb(255, 70, 40)
                } else {
                    [rgb(200, 170, 255), rgb(255, 140, 60), rgb(127, 224, 255)][k]
                };
                self.put(gl, glow, &m, [eye[0], eye[1], eye[2], 1.0], 0.0);
            }
            _ => {}
        }
    }

    pub fn has(&self, at: (i32, i32)) -> bool {
        self.chunks.contains_key(&at)
    }
}
