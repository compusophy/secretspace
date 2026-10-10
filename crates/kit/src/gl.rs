//! A WebGL2 screen driven from Rust, for a game that draws in 3D: the
//! canvas at the device's pixels (up to a cap), programs from GLSL
//! strings, meshes of interleaved floats, a little matrix maths, and a
//! pixel layer (`hud`, a `pixels::Canvas` the size `Screen` would make)
//! shown sharp over the picture for text and the HUD. Still no script but
//! wasm-bindgen's own glue.

use std::cell::RefCell;
use std::collections::HashMap;

use pixels::Canvas;
use wasm_bindgen::JsCast;
use web_sys::{
    HtmlCanvasElement, HtmlElement, WebGl2RenderingContext as GL, WebGlBuffer, WebGlProgram,
    WebGlShader, WebGlTexture, WebGlUniformLocation, WebGlVertexArrayObject,
};

/// A linked program, its uniforms found once each.
pub struct Program {
    pub p: WebGlProgram,
    at: RefCell<HashMap<&'static str, Option<WebGlUniformLocation>>>,
}

fn shader(gl: &GL, kind: u32, src: &str) -> Result<WebGlShader, String> {
    let s = gl.create_shader(kind).ok_or("no shader")?;
    gl.shader_source(&s, src);
    gl.compile_shader(&s);
    if gl
        .get_shader_parameter(&s, GL::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(s)
    } else {
        Err(gl.get_shader_info_log(&s).unwrap_or_default())
    }
}

impl Program {
    pub fn new(gl: &GL, vs: &str, fs: &str) -> Result<Program, String> {
        let p = gl.create_program().ok_or("no program")?;
        gl.attach_shader(&p, &shader(gl, GL::VERTEX_SHADER, vs)?);
        gl.attach_shader(&p, &shader(gl, GL::FRAGMENT_SHADER, fs)?);
        gl.link_program(&p);
        if !gl
            .get_program_parameter(&p, GL::LINK_STATUS)
            .as_bool()
            .unwrap_or(false)
        {
            return Err(gl.get_program_info_log(&p).unwrap_or_default());
        }
        Ok(Program {
            p,
            at: RefCell::new(HashMap::new()),
        })
    }

    pub fn at(&self, gl: &GL, name: &'static str) -> Option<WebGlUniformLocation> {
        self.at
            .borrow_mut()
            .entry(name)
            .or_insert_with(|| gl.get_uniform_location(&self.p, name))
            .clone()
    }

    pub fn use_on(&self, gl: &GL) {
        gl.use_program(Some(&self.p));
    }

    pub fn f1(&self, gl: &GL, name: &'static str, v: f32) {
        gl.uniform1f(self.at(gl, name).as_ref(), v);
    }

    pub fn i1(&self, gl: &GL, name: &'static str, v: i32) {
        gl.uniform1i(self.at(gl, name).as_ref(), v);
    }

    pub fn f2(&self, gl: &GL, name: &'static str, v: [f32; 2]) {
        gl.uniform2f(self.at(gl, name).as_ref(), v[0], v[1]);
    }

    pub fn f3(&self, gl: &GL, name: &'static str, v: [f32; 3]) {
        gl.uniform3f(self.at(gl, name).as_ref(), v[0], v[1], v[2]);
    }

    pub fn f4(&self, gl: &GL, name: &'static str, v: [f32; 4]) {
        gl.uniform4f(self.at(gl, name).as_ref(), v[0], v[1], v[2], v[3]);
    }

    /// An array of vec3s or vec4s (`n` floats each).
    pub fn fv(&self, gl: &GL, name: &'static str, n: usize, v: &[f32]) {
        let at = self.at(gl, name);
        match n {
            3 => gl.uniform3fv_with_f32_array(at.as_ref(), v),
            _ => gl.uniform4fv_with_f32_array(at.as_ref(), v),
        }
    }

    pub fn mat4(&self, gl: &GL, name: &'static str, m: &M4) {
        gl.uniform_matrix4fv_with_f32_array(self.at(gl, name).as_ref(), false, m);
    }
}

/// Vertices of interleaved floats: attribute `k` is `layout[k]` floats.
pub struct Mesh {
    vao: WebGlVertexArrayObject,
    buf: WebGlBuffer,
    stride: i32,
    pub count: i32,
    pub mode: u32,
}

impl Mesh {
    pub fn new(gl: &GL, layout: &[i32], mode: u32) -> Option<Mesh> {
        let vao = gl.create_vertex_array()?;
        let buf = gl.create_buffer()?;
        gl.bind_vertex_array(Some(&vao));
        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&buf));
        let stride: i32 = layout.iter().sum();
        let mut off = 0;
        for (k, &n) in layout.iter().enumerate() {
            gl.enable_vertex_attrib_array(k as u32);
            gl.vertex_attrib_pointer_with_i32(k as u32, n, GL::FLOAT, false, stride * 4, off * 4);
            off += n;
        }
        gl.bind_vertex_array(None);
        Some(Mesh {
            vao,
            buf,
            stride,
            count: 0,
            mode,
        })
    }

    /// A mesh holding `data` from the start.
    pub fn of(gl: &GL, layout: &[i32], mode: u32, data: &[f32]) -> Option<Mesh> {
        let mut m = Mesh::new(gl, layout, mode)?;
        m.set(gl, data, false);
        Some(m)
    }

    /// Replace the vertices (`stream`: it changes every frame).
    pub fn set(&mut self, gl: &GL, data: &[f32], stream: bool) {
        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&self.buf));
        let view = js_sys::Float32Array::from(data);
        let usage = if stream {
            GL::STREAM_DRAW
        } else {
            GL::STATIC_DRAW
        };
        gl.buffer_data_with_array_buffer_view(GL::ARRAY_BUFFER, &view, usage);
        self.count = data.len() as i32 / self.stride.max(1);
    }

    pub fn draw(&self, gl: &GL) {
        if self.count == 0 {
            return;
        }
        gl.bind_vertex_array(Some(&self.vao));
        gl.draw_arrays(self.mode, 0, self.count);
        gl.bind_vertex_array(None);
    }

    pub fn free(self, gl: &GL) {
        gl.delete_buffer(Some(&self.buf));
        gl.delete_vertex_array(Some(&self.vao));
    }
}

/// A 4x4 matrix, column-major (as GLSL takes it).
pub type M4 = [f32; 16];

pub mod m4 {
    use super::M4;

    pub const ID: M4 = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];

    pub fn mul(a: &M4, b: &M4) -> M4 {
        let mut o = [0.0; 16];
        for c in 0..4 {
            for r in 0..4 {
                o[c * 4 + r] = (0..4).map(|k| a[k * 4 + r] * b[c * 4 + k]).sum();
            }
        }
        o
    }

    /// Perspective: vertical field of view (radians), aspect, near, far.
    pub fn perspective(fov: f32, aspect: f32, near: f32, far: f32) -> M4 {
        let f = 1.0 / (fov / 2.0).tan();
        let nf = 1.0 / (near - far);
        [
            f / aspect,
            0.0,
            0.0,
            0.0,
            0.0,
            f,
            0.0,
            0.0,
            0.0,
            0.0,
            (far + near) * nf,
            -1.0,
            0.0,
            0.0,
            2.0 * far * near * nf,
            0.0,
        ]
    }

    /// A camera at `eye` looking along `f`, with `up` roughly up: the view
    /// matrix, and its right and true up.
    pub fn look(eye: [f32; 3], f: [f32; 3], up: [f32; 3]) -> (M4, [f32; 3], [f32; 3]) {
        let s = norm(cross(f, up));
        let u = cross(s, f);
        let m = [
            s[0],
            u[0],
            -f[0],
            0.0,
            s[1],
            u[1],
            -f[1],
            0.0,
            s[2],
            u[2],
            -f[2],
            0.0,
            -dot(s, eye),
            -dot(u, eye),
            dot(f, eye),
            1.0,
        ];
        (m, s, u)
    }

    /// Scale, then turn `yaw` radians about the vertical (toward +z from
    /// +x), then move to `at`.
    pub fn place(at: [f32; 3], yaw: f32, scale: [f32; 3]) -> M4 {
        let (s, c) = yaw.sin_cos();
        [
            c * scale[0],
            0.0,
            s * scale[0],
            0.0,
            0.0,
            scale[1],
            0.0,
            0.0,
            -s * scale[2],
            0.0,
            c * scale[2],
            0.0,
            at[0],
            at[1],
            at[2],
            1.0,
        ]
    }

    /// A model whose axes are these three vectors, at `at`.
    pub fn basis(at: [f32; 3], x: [f32; 3], y: [f32; 3], z: [f32; 3]) -> M4 {
        [
            x[0], x[1], x[2], 0.0, y[0], y[1], y[2], 0.0, z[0], z[1], z[2], 0.0, at[0], at[1],
            at[2], 1.0,
        ]
    }

    /// Where a world point lands: clip space x, y (−1..1) and w (> 0 in
    /// front of the camera).
    pub fn project(m: &M4, p: [f32; 3]) -> (f32, f32, f32) {
        let x = m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12];
        let y = m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13];
        let w = m[3] * p[0] + m[7] * p[1] + m[11] * p[2] + m[15];
        (x, y, w)
    }

    pub fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }

    pub fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }

    pub fn norm(a: [f32; 3]) -> [f32; 3] {
        let l = dot(a, a).sqrt().max(1e-9);
        [a[0] / l, a[1] / l, a[2] / l]
    }
}

const LAYER_VS: &str = "#version 300 es
layout(location=0) in vec2 a;
out vec2 uv;
void main() { uv = vec2(a.x * 0.5 + 0.5, 0.5 - a.y * 0.5); gl_Position = vec4(a, 0.0, 1.0); }";

const LAYER_FS: &str = "#version 300 es
precision mediump float;
in vec2 uv;
uniform sampler2D t;
out vec4 o;
void main() { o = texture(t, uv); }";

/// How a 3D screen fits the window: CSS pixels per layer pixel (about 960
/// layer pixels across at most, and at least `min_short` on the short side
/// whenever the window allows: a game that must show so much of its world),
/// device pixels per CSS pixel drawn (up to `max_dpr`), the window in CSS
/// pixels, the drawing buffer, and the pixel layer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fit {
    pub scale: f64,
    pub dpr: f64,
    pub css: (f64, f64),
    pub size: (u32, u32),
    pub hud: (i32, i32),
}

pub fn measure(min_short: f64, max_dpr: f64) -> Fit {
    measure_in(crate::window_css(), crate::dpr(), min_short, max_dpr)
}

/// `measure` for a window `css` CSS pixels at `dpr` (pure).
pub fn measure_in(css: (f64, f64), dpr: f64, min_short: f64, max_dpr: f64) -> Fit {
    let s = (css.0 / 960.0)
        .ceil()
        .min((css.0.min(css.1) / min_short).floor());
    let scale = s.clamp(1.0, 4.0);
    let dpr = dpr.clamp(1.0, max_dpr.max(1.0));
    Fit {
        scale,
        dpr,
        css,
        size: (
            (css.0 * dpr).round().max(1.0) as u32,
            (css.1 * dpr).round().max(1.0) as u32,
        ),
        hud: ((css.0 / scale).ceil() as i32, (css.1 / scale).ceil() as i32),
    }
}

/// The canvas as a WebGL2 screen, with its pixel layer.
pub struct Gl {
    canvas: HtmlCanvasElement,
    pub gl: GL,
    /// The pixel layer, wiped and drawn each frame, shown over the 3D.
    pub hud: Canvas,
    /// CSS pixels per layer pixel; device pixels per CSS pixel drawn.
    pub scale: f64,
    pub dpr: f64,
    /// The window in CSS pixels; the drawing buffer in device pixels.
    pub css: (f64, f64),
    pub size: (i32, i32),
    layer: Program,
    tex: WebGlTexture,
    quad: Mesh,
}

impl Gl {
    /// The canvas with this id, if the browser gives it WebGL2.
    pub fn new(id: &str) -> Option<Gl> {
        let canvas: HtmlCanvasElement = crate::document().get_element_by_id(id)?.dyn_into().ok()?;
        let gl: GL = canvas.get_context("webgl2").ok()??.dyn_into().ok()?;
        let layer = Program::new(&gl, LAYER_VS, LAYER_FS).ok()?;
        let quad = Mesh::of(
            &gl,
            &[2],
            GL::TRIANGLES,
            &[
                -1.0, -1.0, 1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, 1.0,
            ],
        )?;
        let tex = gl.create_texture()?;
        gl.bind_texture(GL::TEXTURE_2D, Some(&tex));
        for (k, v) in [
            (GL::TEXTURE_MIN_FILTER, GL::NEAREST),
            (GL::TEXTURE_MAG_FILTER, GL::NEAREST),
            (GL::TEXTURE_WRAP_S, GL::CLAMP_TO_EDGE),
            (GL::TEXTURE_WRAP_T, GL::CLAMP_TO_EDGE),
        ] {
            gl.tex_parameteri(GL::TEXTURE_2D, k, v as i32);
        }
        let mut g = Gl {
            canvas,
            gl,
            hud: Canvas::new(1, 1),
            scale: 1.0,
            dpr: 1.0,
            css: (1.0, 1.0),
            size: (1, 1),
            layer,
            tex,
            quad,
        };
        g.fit(352.0, 2.0);
        Some(g)
    }

    /// Match the window: the layer's pixels as `measure` picks them (at
    /// least `min_short` on the short side), the picture at the device's
    /// pixels up to `max_dpr` a CSS pixel.
    pub fn fit(&mut self, min_short: f64, max_dpr: f64) {
        let f = measure(min_short, max_dpr);
        (self.scale, self.dpr, self.css) = (f.scale, f.dpr, f.css);
        self.size = (f.size.0 as i32, f.size.1 as i32);
        self.canvas.set_width(f.size.0);
        self.canvas.set_height(f.size.1);
        crate::place(&self.canvas, f.css.0, f.css.1);
        self.hud.resize(f.hud.0, f.hud.1);
    }

    /// The text scale that reads the same size on any screen.
    pub fn ui(&self) -> i32 {
        if self.scale >= 2.0 {
            1
        } else {
            2
        }
    }

    /// A CSS point as a layer point.
    pub fn to_px(&self, x: f64, y: f64) -> (f32, f32) {
        ((x / self.scale) as f32, (y / self.scale) as f32)
    }

    /// A layer box as a CSS box.
    pub fn to_css(&self, x: f32, y: f32, w: f32, h: f32) -> (f64, f64, f64, f64) {
        let s = self.scale;
        (x as f64 * s, y as f64 * s, w as f64 * s, h as f64 * s)
    }

    pub fn cursor(&self, c: &str) {
        let _ = HtmlElement::style(&self.canvas).set_property("cursor", c);
    }

    pub fn canvas(&self) -> &HtmlCanvasElement {
        &self.canvas
    }

    /// Whether the browser took the context away.
    pub fn lost(&self) -> bool {
        self.gl.is_context_lost()
    }

    /// Start a frame: the whole buffer, cleared to `sky`.
    pub fn begin(&self, sky: [f32; 3]) {
        let gl = &self.gl;
        gl.viewport(0, 0, self.size.0, self.size.1);
        gl.clear_color(sky[0], sky[1], sky[2], 1.0);
        gl.clear(GL::COLOR_BUFFER_BIT | GL::DEPTH_BUFFER_BIT);
    }

    /// Show the pixel layer over what was drawn.
    pub fn present(&self) {
        let gl = &self.gl;
        gl.disable(GL::DEPTH_TEST);
        gl.disable(GL::CULL_FACE);
        gl.enable(GL::BLEND);
        // The layer holds premultiplied colour.
        gl.blend_func(GL::ONE, GL::ONE_MINUS_SRC_ALPHA);
        gl.active_texture(GL::TEXTURE0);
        gl.bind_texture(GL::TEXTURE_2D, Some(&self.tex));
        gl.pixel_storei(GL::UNPACK_ALIGNMENT, 1);
        let _ = gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
            GL::TEXTURE_2D,
            0,
            GL::RGBA as i32,
            self.hud.w,
            self.hud.h,
            0,
            GL::RGBA,
            GL::UNSIGNED_BYTE,
            Some(&self.hud.data),
        );
        self.layer.use_on(gl);
        self.layer.i1(gl, "t", 0);
        // The layer covers its last pixel a little past the window's edge.
        let k = (
            (self.hud.w as f64 * self.scale / self.css.0) as f32,
            (self.hud.h as f64 * self.scale / self.css.1) as f32,
        );
        let (w, h) = (
            (self.size.0 as f32 * k.0).round() as i32,
            (self.size.1 as f32 * k.1).round() as i32,
        );
        gl.viewport(0, self.size.1 - h, w, h);
        self.quad.draw(gl);
        gl.disable(GL::BLEND);
    }
}

#[cfg(test)]
mod tests {
    use super::m4::*;
    use super::{measure, measure_in};

    #[test]
    fn measure_in_uses_the_hosts_size() {
        // A laptop window at 125%, and a 4K one: as a page has always fit.
        let f = measure_in((1280.0, 720.0), 1.25, 300.0, 1.5);
        assert_eq!(
            (f.scale, f.dpr, f.size, f.hud),
            (2.0, 1.25, (1600, 900), (640, 360))
        );
        let f = measure_in((3840.0, 2160.0), 3.0, 300.0, 1.5);
        assert_eq!((f.scale, f.dpr, f.size), (4.0, 1.5, (5760, 3240)));
        // Hosted, the window is the host's surface (no window is asked).
        crate::host::end();
        crate::host::enter(crate::host::Hosted {
            css: (960.0, 540.0),
            dpr: 2.0,
            base: String::new(),
            ns: String::new(),
            opts: Default::default(),
        })
        .unwrap();
        let f = measure(300.0, 1.5);
        assert_eq!(f, measure_in((960.0, 540.0), 2.0, 300.0, 1.5));
        assert_eq!(
            (f.scale, f.dpr, f.css, f.size, f.hud),
            (1.0, 1.5, (960.0, 540.0), (1440, 810), (960, 540))
        );
        crate::host::resize((480.0, 270.0), 1.0);
        let f = measure(300.0, 1.5);
        assert_eq!((f.size, f.hud), ((480, 270), (480, 270)));
        crate::host::end();
    }

    #[test]
    fn a_camera_sees_what_is_in_front() {
        let (v, s, u) = look([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let p = perspective(1.2, 1.0, 0.1, 100.0);
        let vp = mul(&p, &v);
        let (x, y, w) = project(&vp, [5.0, 1.0, 0.0]);
        assert!(w > 0.0 && (x / w).abs() < 1e-5 && (y / w).abs() < 1e-5);
        assert!(project(&vp, [-5.0, 1.0, 0.0]).2 < 0.0, "behind");
        // +z is to the right when looking along +x.
        assert!(s[2] > 0.99 && u[1] > 0.99);
        let (x, _, w) = project(&vp, [5.0, 1.0, 1.0]);
        assert!(x / w > 0.0);
        let m = place(
            [1.0, 2.0, 3.0],
            std::f32::consts::FRAC_PI_2,
            [1.0, 1.0, 1.0],
        );
        // +x turned a quarter toward +z.
        let (x, _, _) = project(&m, [1.0, 0.0, 0.0]);
        assert!((x - 1.0).abs() < 1e-5);
        assert!((project(&m, [1.0, 0.0, 0.0]).1 - 2.0).abs() < 1e-5);
    }
}
