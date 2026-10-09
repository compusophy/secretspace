//! The engine: a retained 3D scene drawn on WebGPU. A game makes meshes
//! once (`geo::Geo` into `Renderer::mesh`), sets what never moves once
//! (`Renderer::statics`), and each frame hands over a `Frame`: the camera,
//! its `Look`, what moves (`Item`s), its `Light`s, `Spark`s and `Decal`s,
//! and the first-person viewmodel. Generic: no game knowledge.
//!
//! Space: x east, y up, z south. Depth is reversed (1 near, 0 at infinity)
//! for precision across a kilometre-wide map.

pub mod geo;
pub mod grid;
pub mod laws;
pub mod sculpt;
pub mod shaders;
pub mod shadow;
pub mod terrain;

mod ao;
mod buffers;
mod cull;
mod decals;
mod draw;
mod globals;
mod pipes;
mod post;
mod shafts;

pub use draw::{Renderer, Stats};
pub use geo::{rgb, V3};
pub use gpu::wgpu;
pub use kit::gl::{m4, M4};
pub use terrain::Terrain;

/// A mesh the renderer holds (from `Renderer::mesh`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Mesh(pub u32);

/// How a thing is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Pass {
    /// Solid, lit, writing depth.
    Opaque,
    /// See-through by its tint's alpha, behind what is solid.
    Faint,
    /// Added as light (beams, halos).
    Glow,
    /// The first-person viewmodel: over everything, with its own field of
    /// view.
    View,
}

/// What a surface is made of (the world shader's choice).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Material {
    Plain = 0,
    /// The ground: grass, dry grass, rock and sand by slope and height.
    Terrain = 1,
    /// Leaves: sway in the wind, let light through.
    Foliage = 2,
    /// The sea (drawn by the engine from `Look::sea`).
    Water = 3,
    Metal = 4,
    /// Energy (shields, shockwaves, beams): bright edge on, clear face
    /// on, unlit. Drawn in the `Glow` or `Faint` pass.
    Rim = 5,
    /// Cloth: soft-edged light, and a sheen where it turns away (light
    /// caught by its fibres), the colour of the cloth.
    Cloth = 6,
    /// Skin: light that wraps past the edge of the shadow, warmed.
    Skin = 7,
    /// Energy (fire, plasma, magic): noise flowing over the surface,
    /// bright where it is thick, unlit; `detail` its grain (noise cells a
    /// metre, 0 for 2.2); `rough` how much its rim glows (0 fire, ragged
    /// at its edges; 1 plasma, a bubble). Drawn in `Glow`.
    Energy = 8,
}

/// One drawn thing: a mesh, where, its tint (alpha for `Faint`), how much
/// it lights itself, how rough it is, its material, and how bumpy; and
/// (still things) a lighter mesh to draw instead beyond a distance.
#[derive(Clone, Copy, Debug)]
pub struct Item {
    pub mesh: Mesh,
    pub model: M4,
    pub tint: [f32; 4],
    pub glow: f32,
    pub pass: Pass,
    pub rough: f32,
    pub material: Material,
    pub detail: f32,
    pub far: Option<(Mesh, f32)>,
}

impl Item {
    pub fn new(mesh: Mesh, model: M4) -> Item {
        Item {
            mesh,
            model,
            tint: [1.0; 4],
            glow: 0.0,
            pass: Pass::Opaque,
            rough: 0.8,
            material: Material::Plain,
            detail: 0.0,
            far: None,
        }
    }

    /// Beyond `d` metres from the eye, `mesh` is drawn instead (for
    /// statics: chosen as the eye moves). The sun's shadow draws it for
    /// what moves, and for statics past its nearest cascade: a coarser
    /// shadow is the same shadow.
    pub fn far(self, mesh: Mesh, d: f32) -> Item {
        Item {
            far: Some((mesh, d)),
            ..self
        }
    }

    pub fn rough(self, rough: f32) -> Item {
        Item { rough, ..self }
    }

    pub fn material(self, material: Material) -> Item {
        Item { material, ..self }
    }

    /// Small bumps on the surface (0 none, 0.5 a rock's).
    pub fn detail(self, detail: f32) -> Item {
        Item { detail, ..self }
    }

    pub fn tint(self, c: V3, a: f32) -> Item {
        Item {
            tint: [c[0], c[1], c[2], a],
            ..self
        }
    }

    pub fn glow(self, glow: f32) -> Item {
        Item { glow, ..self }
    }

    pub fn pass(self, pass: Pass) -> Item {
        Item { pass, ..self }
    }
}

/// A point light: where, how far it reaches, its colour (brighter than 1
/// is allowed).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Light {
    pub p: V3,
    pub r: f32,
    pub c: V3,
}

/// A mark laid on whatever lies under it (the ground, a rock, a step):
/// a disc `r` across about `p`, turned `yaw`, cast down and up `depth`
/// metres (fading toward both ends, and on what is steep). `c`: its
/// colour, and how much of it (fade it out with this). Drawn once the
/// scene's depth can be read (with ambient occlusion on).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Decal {
    pub p: V3,
    pub r: f32,
    pub depth: f32,
    pub yaw: f32,
    pub c: [f32; 4],
    pub mark: Mark,
    /// 0..1: its noise its own.
    pub seed: f32,
}

/// What a decal looks like.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mark {
    /// Burnt black, ragged at its edge; cracks glowing `c` in its middle
    /// (as bright as `c` is: give it black once it cools).
    #[default]
    Scorch = 0,
    /// Rime laid over, `c`, lit by the sky, glinting.
    Frost = 1,
    /// A circle of runes glowing `c` (added).
    Runes = 2,
    /// A soft ring of light at its edge (added).
    Ring = 3,
}

/// A spark: a point of light (or a puff) `size` metres across, facing
/// the eye; stretched along `v` (where it was a moment ago, behind it:
/// a streak, brighter at its head); in a `shape`. `seed` (0..1) keeps a
/// flame's or a puff's noise its own from frame to frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spark {
    pub p: V3,
    pub size: f32,
    pub c: [f32; 4],
    pub v: V3,
    pub shape: Shape,
    pub seed: f32,
}

/// What a spark looks like.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shape {
    /// A soft round light with a hot core (added).
    #[default]
    Glow = 0,
    /// A licking flame: a hot core in a ragged, flickering edge (added).
    Flame = 1,
    /// A puff of smoke or mist, lit by the sky and sun (laid over, not
    /// added: it can darken).
    Smoke = 2,
    /// A glint: a bright core and four thin rays (added).
    Star = 3,
}

/// Where the camera is and where it looks. Forward is (cos yaw cos pitch,
/// sin pitch, sin yaw cos pitch).
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub eye: V3,
    pub yaw: f32,
    pub pitch: f32,
    /// Vertical field of view, radians.
    pub fov: f32,
    pub aspect: f32,
}

impl Default for Camera {
    fn default() -> Camera {
        Camera {
            eye: [0.0, 1.6, 0.0],
            yaw: 0.0,
            pitch: 0.0,
            fov: 1.2,
            aspect: 16.0 / 9.0,
        }
    }
}

impl Camera {
    pub fn forward(&self) -> V3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        [cy * cp, sp, sy * cp]
    }

    /// The view-projection at this field of view, and the camera's right
    /// and up.
    pub fn matrices(&self, fov: f32) -> (M4, V3, V3) {
        let (v, right, up) = m4::look(self.eye, self.forward(), [0.0, 1.0, 0.0]);
        (m4::mul(&perspective(fov, self.aspect), &v), right, up)
    }
}

/// An infinite perspective with depth reversed: 1 at the near plane, 0 at
/// infinity (WebGPU's 0..1 depth).
pub fn perspective(fov: f32, aspect: f32) -> M4 {
    let f = 1.0 / (fov / 2.0).tan();
    let mut m = [0.0; 16];
    m[0] = f / aspect;
    m[5] = f;
    m[11] = -1.0;
    m[14] = laws::NEAR;
    m
}

/// How the world looks: the sun, the sky, the air, the sea. Colours are
/// as people pick them (sRGB); the sun's and the sky's light are linear
/// and may be brighter than 1 (the picture is HDR, tone mapped at the end).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// Toward the sun (or moon), its light, its disc's size (radians).
    pub sun_dir: V3,
    pub sun: V3,
    pub sun_size: f32,
    /// The light from the sky above and from the ground below.
    pub sky: V3,
    pub low: V3,
    /// The sky straight up, at the horizon, and below it.
    pub zenith: V3,
    pub horizon: V3,
    pub deep: V3,
    /// The air: density a metre at sea level, and how fast it thins
    /// with height.
    pub fog: f32,
    pub fog_falloff: f32,
    /// Cloud cover 0 (clear) to 1 (overcast); stars (0 by day).
    pub clouds: f32,
    pub stars: f32,
    /// Exposure, bloom (0..0.2), the vignette (0..1).
    pub exposure: f32,
    pub bloom: f32,
    pub vignette: f32,
    /// The sea's level, if there is one, and its deep colour; how high
    /// its waves read.
    pub sea: Option<f32>,
    pub water: V3,
    pub waves: f32,
    /// The wind over the ground (direction and strength).
    pub wind: [f32; 2],
    /// The picture's grade, after tone mapping.
    pub grade: Grade,
    /// How bright all that glows is (glowing things, sparks, lights): 1
    /// as made; less where the exposure is high (a night), so a spell
    /// keeps its colour at every hour instead of burning white.
    pub glow: f32,
    /// How wet it all is (rain), 0 to 1: the ground darker and glossier,
    /// the sea pocked with drops.
    pub wet: f32,
}

/// A colour grade, applied to the finished picture (as it is shown):
/// lift raises the shadows, gamma bends the middle, gain scales the
/// highlights, each per channel (so a tint toward a colour in shadow and
/// another in light); then saturation and contrast (an S-curve; 1 for
/// neither). Neutral by default.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grade {
    pub lift: V3,
    pub gamma: V3,
    pub gain: V3,
    pub saturation: f32,
    pub contrast: f32,
}

impl Default for Grade {
    fn default() -> Grade {
        Grade {
            lift: [0.0; 3],
            gamma: [1.0; 3],
            gain: [1.0; 3],
            saturation: 1.0,
            contrast: 1.0,
        }
    }
}

impl Default for Look {
    fn default() -> Look {
        Look {
            sun_dir: geo::norm([0.45, 0.55, 0.3]),
            sun: [3.4, 3.1, 2.7],
            sun_size: 0.03,
            sky: [0.42, 0.52, 0.72],
            low: [0.16, 0.14, 0.11],
            zenith: [0.10, 0.24, 0.62],
            horizon: [0.62, 0.72, 0.86],
            deep: [0.10, 0.14, 0.20],
            fog: 0.0012,
            fog_falloff: 0.012,
            clouds: 0.45,
            stars: 0.0,
            exposure: 1.0,
            bloom: 0.05,
            vignette: 0.25,
            sea: None,
            water: [0.02, 0.10, 0.16],
            waves: 0.35,
            wind: [0.8, 0.4],
            grade: Grade::default(),
            glow: 1.0,
            wet: 0.0,
        }
    }
}

/// How much the renderer does: anti-aliasing samples, the sun's shadow
/// (cascades and their size), grass (spacing and reach), bloom's depth,
/// how many ways ambient occlusion looks out from a pixel (0 none),
/// shafts of sunlight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quality {
    pub msaa: u32,
    pub cascades: u32,
    pub shadow_size: u32,
    pub grass_spacing: f32,
    pub grass_reach: f32,
    pub bloom_levels: u32,
    pub ao: u32,
    pub shafts: bool,
    /// Steps the sea's reflections march across the screen (0: the sky
    /// alone).
    pub ssr: u32,
}

impl Quality {
    /// The tier a page asked for (`?q=low|medium|high`), else one for
    /// this device: software adapters Low, touch screens Medium, else High;
    /// `ao=0` turns occlusion off, `shafts=0` the sun's shafts.
    pub fn pick(query: &str, software: bool, touch: bool) -> Quality {
        let q = match query {
            q if q.contains("q=low") => Quality::LOW,
            q if q.contains("q=medium") => Quality::MEDIUM,
            q if q.contains("q=high") => Quality::HIGH,
            _ if software => Quality::LOW,
            _ if touch => Quality::MEDIUM,
            _ => Quality::HIGH,
        };
        Quality {
            ao: if query.contains("ao=0") { 0 } else { q.ao },
            shafts: q.shafts && !query.contains("shafts=0"),
            ssr: if query.contains("ssr=0") { 0 } else { q.ssr },
            ..q
        }
    }

    /// One tier down (None at the bottom); what was turned off stays off.
    pub fn lower(self) -> Option<Quality> {
        let tiers = [Quality::HIGH, Quality::MEDIUM, Quality::LOW];
        let at = tiers.iter().position(|t| {
            Quality {
                ao: self.ao,
                shafts: self.shafts,
                ssr: self.ssr,
                ..*t
            } == self
        })?;
        let next = *tiers.get(at + 1)?;
        Some(Quality {
            ao: if self.ao == 0 { 0 } else { next.ao },
            shafts: self.shafts && next.shafts,
            ssr: if self.ssr == 0 { 0 } else { next.ssr },
            ..next
        })
    }

    pub const HIGH: Quality = Quality {
        msaa: 4,
        cascades: 3,
        shadow_size: 2048,
        grass_spacing: 0.24,
        grass_reach: 42.0,
        bloom_levels: 6,
        ao: 6,
        shafts: true,
        ssr: 16,
    };
    pub const MEDIUM: Quality = Quality {
        msaa: 4,
        cascades: 2,
        shadow_size: 1536,
        grass_spacing: 0.34,
        grass_reach: 28.0,
        bloom_levels: 5,
        ao: 4,
        shafts: true,
        ssr: 10,
    };
    pub const LOW: Quality = Quality {
        msaa: 1,
        cascades: 1,
        shadow_size: 1024,
        grass_spacing: 0.0,
        grass_reach: 0.0,
        bloom_levels: 4,
        ao: 0,
        shafts: false,
        ssr: 0,
    };
}

/// Everything one frame draws besides the statics.
pub struct Frame<'a> {
    pub cam: Camera,
    pub look: Look,
    /// Seconds, for twinkles.
    pub time: f32,
    pub items: &'a [Item],
    pub lights: &'a [Light],
    pub sparks: &'a [Spark],
    pub decals: &'a [Decal],
    /// The viewmodel's field of view (its `Pass::View` items).
    pub view_fov: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_is_reversed_and_infinite() {
        let cam = Camera {
            eye: [0.0, 0.0, 0.0],
            ..Camera::default()
        };
        let (vp, _, _) = cam.matrices(1.2);
        let depth = |x: f32| {
            let p = [x, 0.0, 0.0, 1.0];
            let z = (0..4).map(|k| vp[k * 4 + 2] * p[k]).sum::<f32>();
            let w = (0..4).map(|k| vp[k * 4 + 3] * p[k]).sum::<f32>();
            (z / w, w)
        };
        let (near, w) = depth(laws::NEAR);
        assert!((near - 1.0).abs() < 1e-4 && w > 0.0, "the near plane is 1");
        let (a, _) = depth(10.0);
        let (b, _) = depth(1000.0);
        assert!(a > b && b > 0.0, "farther is smaller, never 0: {a} {b}");
        assert!(depth(-5.0).1 < 0.0, "behind the camera");
    }
}
