//! The engine: a retained 3D scene drawn on WebGPU. A game makes meshes
//! once (`geo::Geo` into `Renderer::mesh`), sets what never moves once
//! (`Renderer::statics`), and each frame hands over a `Frame`: the camera,
//! its `Look`, what moves (`Item`s), its `Light`s and `Spark`s, and the
//! first-person viewmodel. Generic: no game knowledge.
//!
//! Space: x east, y up, z south. Depth is reversed (1 near, 0 at infinity)
//! for precision across a kilometre-wide map.

pub mod geo;
pub mod grid;
pub mod laws;
pub mod shaders;

mod draw;

pub use draw::{Renderer, Stats};
pub use geo::{rgb, V3};
pub use kit::gl::{m4, M4};

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

/// One drawn thing: a mesh, where, its tint (alpha for `Faint`), and how
/// much it lights itself.
#[derive(Clone, Copy, Debug)]
pub struct Item {
    pub mesh: Mesh,
    pub model: M4,
    pub tint: [f32; 4],
    pub glow: f32,
    pub pass: Pass,
}

impl Item {
    pub fn new(mesh: Mesh, model: M4) -> Item {
        Item {
            mesh,
            model,
            tint: [1.0; 4],
            glow: 0.0,
            pass: Pass::Opaque,
        }
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

/// A spark: a round point of light, `size` metres across.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spark {
    pub p: V3,
    pub size: f32,
    pub c: [f32; 4],
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

/// How the world looks: the game's colours and air.
#[derive(Clone, Copy, Debug)]
pub struct Look {
    /// The air far off, and the horizon.
    pub fog: V3,
    /// Where fog starts and where it is whole (metres).
    pub fog_range: (f32, f32),
    /// The light from above and from below on every surface.
    pub sky: V3,
    pub low: V3,
    /// The sun (or moon): toward it, its light, and its disc's size
    /// (radians across).
    pub sun_dir: V3,
    pub sun: V3,
    pub sun_size: f32,
    /// The sky straight up, and straight down.
    pub zenith: V3,
    pub deep: V3,
    /// How bright the stars are (0: none, by day).
    pub stars: f32,
}

impl Default for Look {
    fn default() -> Look {
        Look {
            fog: rgb(170, 190, 215),
            fog_range: (60.0, 400.0),
            sky: rgb(150, 165, 190),
            low: rgb(70, 62, 55),
            sun_dir: geo::norm([0.4, 0.75, 0.3]),
            sun: rgb(255, 238, 210),
            sun_size: 0.04,
            zenith: rgb(70, 120, 200),
            deep: rgb(40, 45, 55),
            stars: 0.0,
        }
    }
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
