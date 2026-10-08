//! The scene's shaders: the world (one shader, its material chosen by
//! the instance: plain, terrain, foliage, water, metal), the sky with its
//! clouds and sun, sparks, and grass grown on the terrain about the eye.

pub const WORLD: &str = r#"
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
    @location(4) extra: vec4<f32>,
};

const PLAIN: i32 = 0;
const TERRAIN: i32 = 1;
const FOLIAGE: i32 = 2;
const WATER: i32 = 3;
const METAL: i32 = 4;

@vertex
fn world_vs(v: WorldIn) -> WorldOut {
    let m = mat4x4<f32>(v.m0, v.m1, v.m2, v.m3);
    var w = m * vec4<f32>(v.pos, 1.0);
    // Leaves sway in the wind, more the higher they are.
    if (i32(v.extra.z + 0.5) == FOLIAGE) {
        let h = max(v.pos.y - 1.5, 0.0);
        let phase = g.eye.w * 1.3 + v.m3.x * 0.21 + v.m3.z * 0.17;
        let sway = (sin(phase) * 0.6 + sin(phase * 2.3 + 1.7) * 0.25) * h * 0.035;
        w = vec4<f32>(w.xyz + vec3<f32>(g.wind.x, 0.0, g.wind.y) * sway, 1.0);
    }
    var o: WorldOut;
    o.clip = g.vp * w;
    o.pos = w.xyz;
    o.nrm = (m * vec4<f32>(v.nrm, 0.0)).xyz;
    o.col = v.col;
    o.tint = v.tint;
    o.extra = v.extra;
    return o;
}

/// The surface's own small bumps: the normal tipped by noise.
fn bump(pos: vec3<f32>, n: vec3<f32>, k: f32, f: f32) -> vec3<f32> {
    let e = 0.12;
    let c = noise3(pos * f);
    let gx = noise3((pos + vec3<f32>(e, 0.0, 0.0)) * f) - c;
    let gy = noise3((pos + vec3<f32>(0.0, e, 0.0)) * f) - c;
    let gz = noise3((pos + vec3<f32>(0.0, 0.0, e)) * f) - c;
    let grad = vec3<f32>(gx, gy, gz) / e;
    return normalize(n - k * (grad - dot(grad, n) * n));
}

/// The ground's colour and roughness where it is: grass, dry grass,
/// rock on the slopes, sand by the sea, darker where wet.
fn earth(pos: vec3<f32>, n: vec3<f32>) -> vec4<f32> {
    let slope = 1.0 - n.y;
    let h = pos.y - g.deep.w;
    let n1 = fbm3(pos * 0.06);
    let n2 = noise3(pos * 0.8);
    let n3 = noise3(pos * 3.1);
    let lush = mix(vec3<f32>(0.045, 0.11, 0.022), vec3<f32>(0.09, 0.17, 0.035), n1 + n3 * 0.15);
    let dry = vec3<f32>(0.20, 0.18, 0.07);
    var c = mix(lush, dry, smoothstep(0.58, 0.78, n1 + n2 * 0.12));
    let rock = mix(vec3<f32>(0.11, 0.105, 0.10), vec3<f32>(0.22, 0.21, 0.19), n2 * 0.7 + n3 * 0.3);
    let r = smoothstep(0.26, 0.42, slope + (n2 - 0.5) * 0.15);
    c = mix(c, rock, r);
    let sand = mix(vec3<f32>(0.40, 0.33, 0.20), vec3<f32>(0.52, 0.45, 0.30), n3);
    let beach = 1.0 - smoothstep(0.2, 0.9, h + (n2 - 0.5) * 0.4);
    c = mix(c, sand, beach);
    let wet = 1.0 - smoothstep(-0.3, 0.35, h);
    c = c * (1.0 - 0.45 * wet);
    let rough = mix(mix(0.92, 0.78, r), 0.35, wet);
    return vec4<f32>(c, rough);
}

fn wave(q: vec2<f32>, t: f32) -> f32 {
    return noise2(q * 0.16 + vec2<f32>(t * 0.05, t * 0.03))
        + 0.5 * noise2(q * 0.5 - vec2<f32>(t * 0.09, -t * 0.07))
        + 0.22 * noise2(q * 1.5 + vec2<f32>(t * 0.2, t * 0.17));
}

/// The sea: waves, the sky in it, the deep and the shallows, the sun's
/// glint, foam at the shore.
fn sea(pos: vec3<f32>, alpha: f32) -> vec4<f32> {
    let t = g.eye.w;
    let p = pos.xz;
    let e = 0.1;
    let h0 = wave(p, t);
    let s = g.water.w;
    let n = normalize(vec3<f32>(-(wave(p + vec2<f32>(e, 0.0), t) - h0) / e * s, 1.0, -(wave(p + vec2<f32>(0.0, e), t) - h0) / e * s));
    let v = normalize(g.eye.xyz - pos);
    let nv = max(dot(n, v), 0.0);
    let fres = 0.02 + 0.98 * pow(1.0 - nv, 5.0);
    let refl = sky(reflect(-v, n));
    let depth = max(pos.y - ground(p), 0.0);
    let light = g.sky.rgb * 0.9 + g.sun.rgb * max(g.sun_dir.y, 0.0) * 0.3;
    let body = mix(vec3<f32>(0.03, 0.22, 0.20), g.water.rgb, smoothstep(0.0, 6.0, depth)) * light;
    let lit = sunlit(pos, vec3<f32>(0.0, 1.0, 0.0));
    let glint = ggx(n, v, g.sun_dir.xyz, 0.05, vec3<f32>(0.02)) * g.sun.rgb * max(dot(n, g.sun_dir.xyz), 0.0) * lit;
    var c = mix(body, refl, fres) + glint;
    let foam = (1.0 - smoothstep(0.0, 0.8, depth)) * smoothstep(0.4, 0.75, noise2(p * 1.2 + vec2<f32>(t * 0.25, -t * 0.2)));
    c = mix(c, vec3<f32>(0.9, 0.93, 0.95) * light * 1.4, foam * 0.75);
    let a = clamp(mix(0.6, 1.0, fres) * smoothstep(0.0, 0.3, depth) + foam * 0.6, 0.0, 1.0);
    return vec4<f32>(air(c, pos), a * alpha);
}

@fragment
fn world_fs(i: WorldOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    var n = normalize(i.nrm);
    if (!front) {
        n = -n;
    }
    let mat = i32(i.extra.z + 0.5);
    if (mat == WATER) {
        return sea(i.pos, i.tint.a);
    }
    var base = linear(i.col.rgb) * linear(i.tint.rgb);
    var rough = i.extra.y;
    var metal = 0.0;
    var through = 0.0;
    var ao = 1.0;
    let glow = i.col.a + i.extra.x;
    if (i.extra.w > 0.0) {
        n = bump(i.pos, n, i.extra.w, 1.6);
    }
    if (mat == TERRAIN) {
        let m = earth(i.pos, n);
        base = m.rgb * linear(i.tint.rgb);
        rough = m.a;
        n = bump(i.pos, n, 0.35, 0.9);
    } else if (mat == FOLIAGE) {
        through = 0.45;
        base = base * (0.75 + 0.5 * noise3(i.pos * 1.7));
        ao = 0.8;
    } else if (mat == METAL) {
        metal = 1.0;
    }
    let c = shade(i.pos, n, base, rough, metal, glow, ao, through);
    return vec4<f32>(air(c, i.pos), i.tint.a);
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

@fragment
fn sky_fs(i: SkyOut) -> @location(0) vec4<f32> {
    let d = normalize(g.fwd.xyz + i.ndc.x * g.fwd.w * g.right.xyz + i.ndc.y * g.right.w * g.up.xyz);
    var c = sky(d);
    // Clouds: a layer high above, drifting, lit by the sun.
    let cover = g.zenith.w;
    if (d.y > 0.0 && cover > 0.0) {
        let t = (1800.0 - g.eye.y) / max(d.y, 0.02);
        let p = g.eye.xz + d.xz * t;
        let q = p * 0.0006 + vec2<f32>(g.eye.w * 0.004, g.eye.w * 0.0015);
        let dens = fbm3(vec3<f32>(q.x * 2.5, g.eye.w * 0.002, q.y * 2.5));
        let k = smoothstep(1.0 - cover, 1.0 - cover + 0.3, dens);
        let mu = max(dot(d, g.sun_dir.xyz), 0.0);
        let lit = g.sun.rgb * (0.16 + 0.25 * pow(mu, 6.0)) + g.sky.rgb * 0.75;
        let shade = mix(1.0, 0.65, smoothstep(0.6, 1.0, dens));
        c = mix(c, lit * shade, k * smoothstep(0.0, 0.2, d.y) * 0.92);
    }
    // The sun's disc.
    let mu = dot(d, g.sun_dir.xyz);
    c = c + g.sun.rgb * 12.0 * smoothstep(g.sun_dir.w, g.sun_dir.w + 0.00012, mu);
    // Stars, at night.
    if (g.sun.w > 0.0) {
        let h = hash3(floor(d * 260.0));
        if (h > 0.997) {
            let tw = 0.65 + 0.35 * sin(g.eye.w * 2.0 + h * 500.0);
            c = c + vec3<f32>(1.0, 0.95, 0.85) * (h - 0.997) / 0.003 * tw * g.sun.w * smoothstep(0.0, 0.15, d.y);
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
    let s = clamp(ps.w * g.up.w / max(clip.w, 0.05), 1.0, SPARK_MAX_PX);
    clip.x = clip.x + c.x * s / g.view.x * clip.w;
    clip.y = clip.y + c.y * s / g.view.y * clip.w;
    var o: SparkOut;
    o.clip = clip;
    o.uv = c;
    o.col = col;
    return o;
}

@fragment
fn spark_fs(i: SparkOut) -> @location(0) vec4<f32> {
    let d = length(i.uv);
    let a = (1.0 - smoothstep(0.3, 1.0, d)) * i.col.a;
    let core = 1.0 - smoothstep(0.0, 0.45, d);
    return vec4<f32>(linear(i.col.rgb) * a * (2.5 + core * 4.0), 0.0);
}

struct GrassOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) pos: vec3<f32>,
    @location(1) col: vec3<f32>,
    @location(2) tip: f32,
    @location(3) side: vec3<f32>,
};

@vertex
fn grass_vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> GrassOut {
    var shape = array<vec2<f32>, 9>(
        vec2<f32>(-1.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(-0.7, 0.5),
        vec2<f32>(-0.7, 0.5), vec2<f32>(1.0, 0.0), vec2<f32>(0.7, 0.5),
        vec2<f32>(-0.7, 0.5), vec2<f32>(0.7, 0.5), vec2<f32>(0.0, 1.0));
    let k = shape[vi];
    let sp = g.grass.x;
    let side = u32(g.grass.y);
    let half = f32(side) * sp * 0.5;
    let base = floor((g.eye.xz - half) / sp) * sp;
    var xz = base + vec2<f32>(f32(ii % side), f32(ii / side)) * sp;
    let r1 = hash3(vec3<f32>(xz.x, 1.3, xz.y));
    let r2 = hash3(vec3<f32>(xz.x, 7.7, xz.y));
    let r3 = hash3(vec3<f32>(xz.x, 3.1, xz.y));
    xz = xz + (vec2<f32>(r1, r2) - 0.5) * sp * 1.8;
    let gh = ground(xz);
    let dx = ground(xz + vec2<f32>(0.6, 0.0)) - ground(xz - vec2<f32>(0.6, 0.0));
    let dz = ground(xz + vec2<f32>(0.0, 0.6)) - ground(xz - vec2<f32>(0.0, 0.6));
    let slope = length(vec2<f32>(dx, dz)) / 1.2;
    let dist = length(xz - g.eye.xz);
    let reach = g.grass.z;
    var height = (0.28 + 0.42 * r3)
        * (1.0 - smoothstep(reach * 0.7, reach, dist))
        * (1.0 - smoothstep(0.35, 0.6, slope))
        * smoothstep(g.deep.w + 0.7, g.deep.w + 1.4, gh)
        * smoothstep(0.2, 0.45, noise2(xz * 0.09));
    var o: GrassOut;
    if (height < 0.04 || g.grass.w < 0.5) {
        o.clip = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        return o;
    }
    let a = r1 * 6.2832;
    let across = vec2<f32>(cos(a), sin(a));
    let width = 0.035 * (1.0 - k.y * 0.6);
    let phase = g.eye.w * 1.8 + xz.x * 0.35 + xz.y * 0.27;
    let bend = (sin(phase) * 0.5 + 0.7 + sin(phase * 2.7) * 0.2) * k.y * k.y * height * 0.45;
    let lean = vec2<f32>(g.wind.x, g.wind.y) * bend + (vec2<f32>(r2, r3) - 0.5) * k.y * height * 0.3;
    let p = vec3<f32>(xz.x + across.x * k.x * width + lean.x, gh + k.y * height, xz.y + across.y * k.x * width + lean.y);
    o.clip = g.vp * vec4<f32>(p, 1.0);
    o.pos = p;
    let tone = noise2(xz * 0.05);
    let root = mix(vec3<f32>(0.03, 0.08, 0.015), vec3<f32>(0.06, 0.10, 0.02), tone);
    let tipc = mix(vec3<f32>(0.10, 0.20, 0.035), vec3<f32>(0.24, 0.24, 0.07), tone * tone);
    o.col = mix(root, tipc, k.y);
    o.tip = k.y;
    o.side = vec3<f32>(-across.y, 0.0, across.x);
    return o;
}

@fragment
fn grass_fs(i: GrassOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    // Mostly up, a little to the side: lit like the ground it grows on.
    var s = i.side;
    if (!front) {
        s = -s;
    }
    let n = normalize(vec3<f32>(0.0, 1.0, 0.0) + s * 0.35);
    let c = shade(i.pos, n, i.col, 0.85, 0.0, 0.0, mix(0.45, 1.0, i.tip), 0.6);
    return vec4<f32>(air(c, i.pos), 1.0);
}
"#;
