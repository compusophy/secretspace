//! The sun's shadow map (depth only, from the sun), and what follows the
//! scene: bloom (a chain of halvings, then back up, each a little wider)
//! and the finish (the sun's shafts added, exposure, ACES tone mapping,
//! a vignette, sRGB, the grade, and a dither so the sky's gradients do
//! not band).

pub const SHADOW: &str = r#"
struct Caster {
    m: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> caster: Caster;

struct CastIn {
    @location(0) pos: vec3<f32>,
    @location(3) m0: vec4<f32>,
    @location(4) m1: vec4<f32>,
    @location(5) m2: vec4<f32>,
    @location(6) m3: vec4<f32>,
};

@vertex
fn shadow_vs(v: CastIn) -> @builtin(position) vec4<f32> {
    let m = mat4x4<f32>(v.m0, v.m1, v.m2, v.m3);
    return caster.m * (m * vec4<f32>(v.pos, 1.0));
}
"#;

pub const POST: &str = r#"
struct Post {
    texel: vec4<f32>,   // xy one texel of the source (the finish: rgb the shafts' light)
    k: vec4<f32>,       // x bloom, y exposure, z vignette, w spread
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var lin: sampler;
@group(0) @binding(2) var<uniform> pp: Post;
@group(0) @binding(3) var bloom: texture_2d<f32>;

struct Grade {
    lift: vec4<f32>,    // rgb, w saturation
    gamma: vec4<f32>,   // rgb, w contrast
    gain: vec4<f32>,    // rgb, w unused
};

@group(0) @binding(4) var<uniform> grade: Grade;
@group(0) @binding(5) var shafts: texture_2d<f32>;

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn post_vs(@builtin(vertex_index) i: u32) -> Out {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
    var o: Out;
    o.clip = vec4<f32>(p, 0.0, 1.0);
    o.uv = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    return o;
}

fn tap(uv: vec2<f32>, dx: f32, dy: f32) -> vec3<f32> {
    return textureSampleLevel(src, lin, uv + vec2<f32>(dx, dy) * pp.texel.xy, 0.0).rgb;
}

/// Half the size: thirteen taps, weighted so bright specks do not flicker.
@fragment
fn down_fs(i: Out) -> @location(0) vec4<f32> {
    let uv = i.uv;
    let a = tap(uv, -2.0, -2.0);
    let b = tap(uv, 0.0, -2.0);
    let c = tap(uv, 2.0, -2.0);
    let d = tap(uv, -1.0, -1.0);
    let e = tap(uv, 1.0, -1.0);
    let f = tap(uv, -2.0, 0.0);
    let m = tap(uv, 0.0, 0.0);
    let h = tap(uv, 2.0, 0.0);
    let j = tap(uv, -1.0, 1.0);
    let k = tap(uv, 1.0, 1.0);
    let l = tap(uv, -2.0, 2.0);
    let n = tap(uv, 0.0, 2.0);
    let o = tap(uv, 2.0, 2.0);
    var c4 = (d + e + j + k) * 0.125;
    c4 = c4 + (a + b + f + m) * 0.03125 + (b + c + m + h) * 0.03125;
    c4 = c4 + (f + m + l + n) * 0.03125 + (m + h + n + o) * 0.03125;
    return vec4<f32>(min(c4, vec3<f32>(64.0)), 1.0);
}

/// Back up: a tent of nine taps, added to what is there.
@fragment
fn up_fs(i: Out) -> @location(0) vec4<f32> {
    let uv = i.uv;
    let r = pp.k.w;
    var c = tap(uv, 0.0, 0.0) * 4.0;
    c = c + (tap(uv, -r, 0.0) + tap(uv, r, 0.0) + tap(uv, 0.0, -r) + tap(uv, 0.0, r)) * 2.0;
    c = c + tap(uv, -r, -r) + tap(uv, r, -r) + tap(uv, -r, r) + tap(uv, r, r);
    return vec4<f32>(c / 16.0, 1.0);
}

/// ACES, fitted (Narkowicz): film's response to light.
fn aces(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

/// The grade, on the picture as it is shown (0..1): lift, gamma and gain
/// per channel, then saturation, then contrast (an S-curve).
fn graded(x: vec3<f32>) -> vec3<f32> {
    var c = grade.gain.rgb * (x + grade.lift.rgb * (vec3<f32>(1.0) - x));
    c = pow(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(1.0) / max(grade.gamma.rgb, vec3<f32>(0.05)));
    let luma = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    c = clamp(mix(vec3<f32>(luma), c, grade.lift.w), vec3<f32>(0.0), vec3<f32>(1.0));
    let s = c * c * (3.0 - 2.0 * c);
    return clamp(mix(c, s, grade.gamma.w - 1.0), vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn finish_fs(i: Out) -> @location(0) vec4<f32> {
    let hdr = textureSampleLevel(src, lin, i.uv, 0.0).rgb;
    let glow = textureSampleLevel(bloom, lin, i.uv, 0.0).rgb;
    let shaft = textureSampleLevel(shafts, lin, i.uv, 0.0).r;
    var c = (mix(hdr, glow, pp.k.x) + pp.texel.rgb * shaft) * pp.k.y;
    c = aces(c);
    let q = i.uv - 0.5;
    c = c * (1.0 - pp.k.z * dot(q, q) * 1.6);
    c = graded(srgb(c));
    // Under a step of the screen's, from pixel to pixel, so gradients
    // become a fine grain instead of bands.
    let px = i.clip.xy;
    let n = fract(52.9829189 * fract(0.06711056 * px.x + 0.00583715 * px.y));
    return vec4<f32>(c + (n - 0.5) / 255.0, 1.0);
}
"#;
