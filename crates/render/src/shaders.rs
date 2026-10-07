//! The engine's WGSL, one module: the world (lit by the sky, the sun and
//! the lights in the grid, fogged), the sky (a gradient, the sun's disc,
//! stars), and sparks (round billboards, added as light). Depth is
//! reversed: 1 near, 0 far.

/// Bytes of `Globals` (a mat4 and 13 vec4s).
pub const GLOBALS: u64 = 64 + 13 * 16;

pub const WGSL: &str = r#"
struct Globals {
    vp: mat4x4<f32>,
    eye: vec4<f32>,      // xyz, w: seconds
    fwd: vec4<f32>,      // xyz, w: tan of half the field of view, across
    right: vec4<f32>,    // xyz, w: tan of half the field of view, up
    up: vec4<f32>,       // xyz, w: pixels a metre at a metre (sparks)
    fog: vec4<f32>,      // rgb, w: where fog starts
    sky: vec4<f32>,      // rgb, w: where fog is whole
    low: vec4<f32>,      // rgb, w: unused
    sun_dir: vec4<f32>,  // xyz, w: cos of the disc's radius
    sun: vec4<f32>,      // rgb, w: stars
    zenith: vec4<f32>,
    deep: vec4<f32>,
    grid: vec4<f32>,     // origin x, origin z, cell, cells a side
    view: vec4<f32>,     // viewport width, height (pixels)
};

struct Light {
    p: vec4<f32>,        // xyz, w: reach
    c: vec4<f32>,        // rgb
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var<storage, read> lights: array<Light>;
@group(0) @binding(2) var<storage, read> cells: array<vec2<u32>>;
@group(0) @binding(3) var<storage, read> index: array<u32>;

fn shade(pos: vec3<f32>, n: vec3<f32>, base: vec3<f32>, glow: f32) -> vec3<f32> {
    var light = mix(g.low.rgb, g.sky.rgb, n.y * 0.5 + 0.5)
        + g.sun.rgb * max(dot(n, g.sun_dir.xyz), 0.0);
    let side = i32(g.grid.w);
    let gc = vec2<i32>(floor((pos.xz - g.grid.xy) / g.grid.z));
    if (gc.x >= 0 && gc.y >= 0 && gc.x < side && gc.y < side) {
        let cell = cells[gc.y * side + gc.x];
        for (var i = 0u; i < cell.y; i = i + 1u) {
            let l = lights[index[cell.x + i]];
            let d = l.p.xyz - pos;
            let dist = length(d);
            let k = clamp(1.0 - dist / l.p.w, 0.0, 1.0);
            light += l.c.rgb * (k * k) * (0.3 + 0.7 * max(dot(n, d / max(dist, 0.001)), 0.0));
        }
    }
    var c = mix(base * light, base * 1.3, glow);
    let f = smoothstep(g.fog.w, g.sky.w, length(pos - g.eye.xyz));
    return mix(c, g.fog.rgb, f * (1.0 - glow * 0.5));
}

struct WorldIn {
    @location(0) pos: vec3<f32>,
    @location(1) nrm: vec3<f32>,
    @location(2) col: vec4<f32>,
    @location(3) m0: vec4<f32>,
    @location(4) m1: vec4<f32>,
    @location(5) m2: vec4<f32>,
    @location(6) m3: vec4<f32>,
    @location(7) tint: vec4<f32>,
    @location(8) extra: vec4<f32>,
};

struct WorldOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) pos: vec3<f32>,
    @location(1) nrm: vec3<f32>,
    @location(2) col: vec4<f32>,
    @location(3) tint: vec4<f32>,
    @location(4) glow: f32,
};

@vertex
fn world_vs(v: WorldIn) -> WorldOut {
    let m = mat4x4<f32>(v.m0, v.m1, v.m2, v.m3);
    let w = m * vec4<f32>(v.pos, 1.0);
    var o: WorldOut;
    o.clip = g.vp * w;
    o.pos = w.xyz;
    o.nrm = (m * vec4<f32>(v.nrm, 0.0)).xyz;
    o.col = v.col;
    o.tint = v.tint;
    o.glow = v.extra.x;
    return o;
}

@fragment
fn world_fs(i: WorldOut) -> @location(0) vec4<f32> {
    let n = normalize(i.nrm);
    let base = i.col.rgb * i.tint.rgb;
    let glow = clamp(i.col.a + i.glow, 0.0, 1.0);
    return vec4<f32>(shade(i.pos, n, base, glow), i.tint.a);
}

struct SkyOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn sky_vs(@builtin(vertex_index) i: u32) -> SkyOut {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
    var o: SkyOut;
    o.clip = vec4<f32>(p, 0.0, 1.0);
    o.ndc = p;
    return o;
}

fn hash(q: vec3<f32>) -> f32 {
    return fract(sin(dot(q, vec3<f32>(12.9898, 78.233, 37.719))) * 43758.5453);
}

@fragment
fn sky_fs(i: SkyOut) -> @location(0) vec4<f32> {
    let d = normalize(g.fwd.xyz + i.ndc.x * g.fwd.w * g.right.xyz + i.ndc.y * g.right.w * g.up.xyz);
    var c: vec3<f32>;
    if (d.y >= 0.0) {
        c = mix(g.fog.rgb, g.zenith.rgb, smoothstep(0.0, 0.7, d.y));
    } else {
        c = mix(g.fog.rgb, g.deep.rgb, smoothstep(0.0, -0.45, d.y));
    }
    let s = dot(d, g.sun_dir.xyz);
    c += g.sun.rgb * (smoothstep(g.sun_dir.w, g.sun_dir.w + 0.0004, s) * 3.0
        + pow(max(s, 0.0), 48.0) * 0.3);
    if (g.sun.w > 0.0) {
        let h = hash(floor(d * 220.0));
        if (h > 0.9965) {
            let tw = 0.65 + 0.35 * sin(g.eye.w * 2.0 + h * 500.0);
            let k = (h - 0.9965) / 0.0035 * tw * select(0.35, 1.0, d.y > 0.0);
            c += vec3<f32>(0.95, 0.9, 0.8) * k * g.sun.w * smoothstep(-0.02, 0.15, abs(d.y));
        }
    }
    return vec4<f32>(c, 1.0);
}

struct SparkOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) col: vec4<f32>,
};

@vertex
fn spark_vs(@builtin(vertex_index) i: u32, @location(0) ps: vec4<f32>, @location(1) col: vec4<f32>) -> SparkOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0));
    let c = corners[i];
    var clip = g.vp * vec4<f32>(ps.xyz, 1.0);
    // Its size in pixels, across.
    let s = clamp(ps.w * g.up.w / max(clip.w, 0.05), 1.0, SPARK_MAX_PX);
    clip.x += c.x * s / g.view.x * clip.w;
    clip.y += c.y * s / g.view.y * clip.w;
    var o: SparkOut;
    o.clip = clip;
    o.uv = c;
    o.col = col;
    return o;
}

@fragment
fn spark_fs(i: SparkOut) -> @location(0) vec4<f32> {
    let d = length(i.uv) * 0.5;
    let a = (1.0 - smoothstep(0.25, 0.5, d)) * i.col.a;
    return vec4<f32>(i.col.rgb * a, 0.0);
}
"#;

/// The module as compiled: the engine's numbers put in.
pub fn wgsl() -> String {
    WGSL.replace("SPARK_MAX_PX", &format!("{:.1}", crate::laws::SPARK_MAX_PX))
}
