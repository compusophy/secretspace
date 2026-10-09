//! What every scene shader shares: the globals, the lights' grid, the
//! sun's shadow, the terrain's heights, noise, the sky, the air between
//! (aerial perspective), and the light a surface gives back (GGX, and
//! cloth's sheen).

pub const COMMON: &str = r#"
struct Globals {
    vp: mat4x4<f32>,
    shadow: array<mat4x4<f32>, 3>,
    eye: vec4<f32>,      // xyz, w: seconds
    fwd: vec4<f32>,      // xyz, w: tan of half the field of view across
    right: vec4<f32>,    // xyz, w: tan of half the field of view up
    up: vec4<f32>,       // xyz, w: pixels a metre at a metre
    sun_dir: vec4<f32>,  // xyz toward the sun, w: cos of its disc's radius
    sun: vec4<f32>,      // rgb its light, w: stars
    sky: vec4<f32>,      // rgb light from above, w: fog density a metre
    low: vec4<f32>,      // rgb light from below, w: exposure
    zenith: vec4<f32>,   // rgb the sky straight up, w: cloud cover
    horizon: vec4<f32>,  // rgb the sky at the horizon, w: fog's falloff with height
    deep: vec4<f32>,     // rgb below the horizon, w: sea level
    grid: vec4<f32>,     // the lights' grid: origin x, z, cell, cells a side
    view: vec4<f32>,     // viewport width, height; shadow texel; cascades
    splits: vec4<f32>,   // where each cascade ends (metres ahead)
    terrain: vec4<f32>,  // heights: origin x, z, cell, samples a side
    wind: vec4<f32>,     // xy the wind (x, z), z unused, w how bright glows are
    water: vec4<f32>,    // rgb deep water, w waves
    grass: vec4<f32>,    // spacing, blades a side, reach, on
};

struct Light {
    p: vec4<f32>,
    c: vec4<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var<storage, read> lights: array<Light>;
@group(0) @binding(2) var<storage, read> cells: array<vec2<u32>>;
@group(0) @binding(3) var<storage, read> index: array<u32>;
@group(0) @binding(4) var shadow_map: texture_depth_2d_array;
@group(0) @binding(5) var shadow_cmp: sampler_comparison;
@group(0) @binding(6) var heights: texture_2d<f32>;

const PI: f32 = 3.14159265;

/// An integer hash (PCG), and 0..1 for a whole-numbered cell: the same
/// bits for the same cell on every GPU, however it was reached.
fn hashu(x: u32) -> u32 {
    let h = x * 747796405u + 2891336453u;
    let w = ((h >> ((h >> 28u) + 4u)) ^ h) * 277803737u;
    return (w >> 22u) ^ w;
}

fn cell_hash(c: vec2<i32>, k: u32) -> f32 {
    let h = hashu(bitcast<u32>(c.x) ^ hashu(bitcast<u32>(c.y) ^ hashu(k)));
    return f32(h >> 8u) / 16777216.0;
}

fn hash3(p: vec3<f32>) -> f32 {
    var q = fract(p * 0.3183099 + vec3<f32>(0.71, 0.113, 0.419));
    q = q * 17.0;
    return fract(q.x * q.y * q.z * (q.x + q.y + q.z));
}

fn noise3(x: vec3<f32>) -> f32 {
    let i = floor(x);
    let f = fract(x);
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(hash3(i), hash3(i + vec3<f32>(1.0, 0.0, 0.0)), u.x);
    let b = mix(hash3(i + vec3<f32>(0.0, 1.0, 0.0)), hash3(i + vec3<f32>(1.0, 1.0, 0.0)), u.x);
    let c = mix(hash3(i + vec3<f32>(0.0, 0.0, 1.0)), hash3(i + vec3<f32>(1.0, 0.0, 1.0)), u.x);
    let d = mix(hash3(i + vec3<f32>(0.0, 1.0, 1.0)), hash3(i + vec3<f32>(1.0, 1.0, 1.0)), u.x);
    return mix(mix(a, b, u.y), mix(c, d, u.y), u.z);
}

fn noise2(x: vec2<f32>) -> f32 {
    return noise3(vec3<f32>(x.x, 0.5, x.y));
}

fn fbm3(x: vec3<f32>) -> f32 {
    var a = 0.5;
    var s = 0.0;
    var p = x;
    for (var i = 0; i < 4; i = i + 1) {
        s = s + a * noise3(p);
        p = p * 2.03 + vec3<f32>(1.7, 9.2, 3.1);
        a = a * 0.5;
    }
    return s;
}

/// The terrain's height at (x, z), between its samples.
fn ground(xz: vec2<f32>) -> f32 {
    let n = i32(g.terrain.w);
    if (n < 2) {
        return -1000.0;
    }
    let f = clamp((xz - g.terrain.xy) / g.terrain.z, vec2<f32>(0.0), vec2<f32>(f32(n - 1) - 0.001));
    let i = vec2<i32>(floor(f));
    let t = f - floor(f);
    let a = textureLoad(heights, i, 0).r;
    let b = textureLoad(heights, i + vec2<i32>(1, 0), 0).r;
    let c = textureLoad(heights, i + vec2<i32>(0, 1), 0).r;
    let d = textureLoad(heights, i + vec2<i32>(1, 1), 0).r;
    return mix(mix(a, b, t.x), mix(c, d, t.x), t.y);
}

/// How much grass grows at `xz` (0 to 1), as the game painted it.
fn lush(xz: vec2<f32>) -> f32 {
    let n = i32(g.terrain.w);
    if (n < 2) {
        return 0.0;
    }
    let f = clamp((xz - g.terrain.xy) / g.terrain.z, vec2<f32>(0.0), vec2<f32>(f32(n - 1) - 0.001));
    let i = vec2<i32>(floor(f));
    let t = f - floor(f);
    let a = textureLoad(heights, i, 0).g;
    let b = textureLoad(heights, i + vec2<i32>(1, 0), 0).g;
    let c = textureLoad(heights, i + vec2<i32>(0, 1), 0).g;
    let d = textureLoad(heights, i + vec2<i32>(1, 1), 0).g;
    return mix(mix(a, b, t.x), mix(c, d, t.x), t.y);
}

/// How much of the sun reaches `pos` (0 in shadow, 1 lit), softened.
fn sunlit(pos: vec3<f32>, n: vec3<f32>) -> f32 {
    let count = i32(g.view.w);
    let ahead = dot(pos - g.eye.xyz, g.fwd.xyz);
    var c = 0;
    if (ahead > g.splits.x) {
        c = 1;
    }
    if (ahead > g.splits.y) {
        c = 2;
    }
    if (count == 0 || c >= count || ahead > g.splits.z) {
        return 1.0;
    }
    let off = n * (0.05 + 0.12 * f32(c));
    let p = g.shadow[c] * vec4<f32>(pos + off, 1.0);
    let uv = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    if (uv.x < 0.0 || uv.y < 0.0 || uv.x > 1.0 || uv.y > 1.0 || p.z > 1.0) {
        return 1.0;
    }
    let z = p.z - 0.0008;
    let texel = g.view.z;
    var sum = 0.0;
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            sum = sum + textureSampleCompareLevel(shadow_map, shadow_cmp, uv + vec2<f32>(f32(x), f32(y)) * texel * 1.25, c, z);
        }
    }
    return mix(1.0, sum / 9.0, g.splits.w);
}

/// The sky looking along `d`: blue overhead, pale at the horizon, the
/// sun's glow about it.
fn sky(d: vec3<f32>) -> vec3<f32> {
    var c: vec3<f32>;
    if (d.y >= 0.0) {
        c = mix(g.zenith.rgb, g.horizon.rgb, pow(1.0 - d.y, 5.0));
    } else {
        c = mix(g.horizon.rgb, g.deep.rgb, smoothstep(0.0, -0.25, d.y));
    }
    let mu = max(dot(d, g.sun_dir.xyz), 0.0);
    c = c + g.sun.rgb * (pow(mu, 6.0) * 0.06 + pow(mu, 48.0) * 0.12 + pow(mu, 400.0) * 0.5);
    return c;
}

/// The air between the eye and `pos`: thicker low and far, the colour of
/// the sky behind it.
fn air(c: vec3<f32>, pos: vec3<f32>) -> vec3<f32> {
    let v = pos - g.eye.xyz;
    let dist = length(v);
    let d = v / max(dist, 0.001);
    let falloff = g.horizon.w;
    let base = g.sky.w * exp(-falloff * max(g.eye.y, 0.0));
    var optical = base * dist;
    let k = falloff * d.y * dist;
    if (abs(k) > 0.001) {
        optical = base * (1.0 - exp(-k)) / (falloff * d.y);
    }
    let f = clamp(1.0 - exp(-max(optical, 0.0)), 0.0, 1.0);
    return mix(c, sky(vec3<f32>(d.x, max(d.y, 0.03), d.z)), f);
}

fn ggx(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, rough: f32, f0: vec3<f32>) -> vec3<f32> {
    let h = normalize(v + l);
    let nl = max(dot(n, l), 0.0);
    let nv = max(dot(n, v), 0.001);
    let nh = max(dot(n, h), 0.0);
    let vh = max(dot(v, h), 0.0);
    let a = max(rough * rough, 0.002);
    let a2 = a * a;
    let dd = nh * nh * (a2 - 1.0) + 1.0;
    let dist = a2 / (PI * dd * dd);
    let k = a * 0.5;
    let geo = (nl / (nl * (1.0 - k) + k)) * (nv / (nv * (1.0 - k) + k));
    let f = f0 + (vec3<f32>(1.0) - f0) * pow(1.0 - vh, 5.0);
    return dist * geo * f / (4.0 * nv * max(nl, 0.001));
}

/// Cloth's sheen (Charlie, after Estevez and Kulla, with Neubelt's
/// visibility): light caught by fibres, strongest where the cloth turns
/// away from the eye or the light.
fn sheen(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, rough: f32) -> f32 {
    let h = normalize(v + l);
    let nh = dot(n, h);
    let sin2 = max(1.0 - nh * nh, 0.0001);
    let inv = 1.0 / rough;
    let d = (2.0 + inv) * pow(sin2, inv * 0.5) / (2.0 * PI);
    let nl = max(dot(n, l), 0.0);
    let nv = max(dot(n, v), 0.001);
    return d / (4.0 * (nl + nv - nl * nv));
}

/// The light a surface gives back: the sun (shadowed), the sky and the
/// ground's light, the sky in its sheen, the lights near it, its glow.
/// Cloth takes the light softly and shines at its edges; skin lets it
/// wrap past the shadow's edge, reddened (`kind`: the world's materials).
fn shade(pos: vec3<f32>, n: vec3<f32>, base: vec3<f32>, rough: f32, metal: f32, glow: f32, ao: f32, through: f32, kind: i32) -> vec3<f32> {
    let v = normalize(g.eye.xyz - pos);
    let l = g.sun_dir.xyz;
    let cloth = kind == CLOTH;
    let skin = kind == SKIN;
    let f0 = mix(vec3<f32>(select(0.04, 0.02, cloth)), base, metal);
    let diffuse = base * (1.0 - metal);
    var c = vec3<f32>(0.0);
    let nl = dot(n, l);
    let lit = sunlit(pos, n);
    // How the sun falls on it: the cosine, softened for cloth, and for
    // skin wrapped further in red than in green and blue.
    var wrap = vec3<f32>(0.0);
    if (cloth) {
        wrap = vec3<f32>(0.3);
    } else if (skin) {
        wrap = vec3<f32>(0.55, 0.3, 0.25);
    }
    let fall = max((vec3<f32>(nl) + wrap) / (vec3<f32>(1.0) + wrap), vec3<f32>(0.0));
    c = c + diffuse * g.sun.rgb * fall * lit;
    if (nl > 0.0) {
        c = c + ggx(n, v, l, rough, f0) * PI * g.sun.rgb * nl * lit;
    }
    // Light through leaves and blades, from behind.
    c = c + diffuse * g.sun.rgb * max(-nl, 0.0) * through * lit;
    let amb = mix(g.low.rgb, g.sky.rgb, n.y * 0.5 + 0.5);
    c = c + diffuse * amb * ao;
    let nv = max(dot(n, v), 0.0);
    if (cloth) {
        // The sheen's colour: the cloth's own, lighter.
        let tone = min(sqrt(base) * 0.5 + base, vec3<f32>(1.0));
        if (nl > 0.0) {
            c = c + tone * sheen(n, v, l, 0.45) * PI * g.sun.rgb * nl * lit;
        }
        c = c + tone * amb * pow(1.0 - nv, 3.0) * ao * 2.5;
    } else {
        let fr = f0 + (max(vec3<f32>(1.0 - rough), f0) - f0) * pow(1.0 - nv, 5.0);
        c = c + fr * sky(reflect(-v, n)) * (1.0 - rough * 0.8) * ao * 0.5;
    }
    let side = i32(g.grid.w);
    let gc = vec2<i32>(floor((pos.xz - g.grid.xy) / g.grid.z));
    if (gc.x >= 0 && gc.y >= 0 && gc.x < side && gc.y < side) {
        let cell = cells[gc.y * side + gc.x];
        for (var i = 0u; i < cell.y; i = i + 1u) {
            let q = lights[index[cell.x + i]];
            let d = q.p.xyz - pos;
            let dist = length(d);
            let k = clamp(1.0 - dist / q.p.w, 0.0, 1.0);
            let dl = d / max(dist, 0.001);
            let ndl = max(dot(n, dl), 0.0);
            c = c + (diffuse * (0.25 + 0.75 * ndl) + ggx(n, v, dl, max(rough, 0.3), f0) * ndl) * q.c.rgb * (k * k);
        }
    }
    return c + base * glow * 4.0;
}

/// sRGB colours (as people pick them) to the linear light shaders sum.
fn linear(c: vec3<f32>) -> vec3<f32> {
    return pow(max(c, vec3<f32>(0.0)), vec3<f32>(2.2));
}
"#;
