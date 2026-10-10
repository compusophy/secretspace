//! The scene's shaders: the world (one shader, its material chosen by
//! the instance: plain, terrain, foliage, water, metal, rim, cloth,
//! skin, energy), the sky with its clouds and sun, sparks (glows,
//! streaks, flames, smoke, glints), and grass grown on the terrain about
//! the eye.

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
    // Where it is from the thing's own middle (world scale): energy's
    // noise rides with what it is on.
    @location(5) rel: vec3<f32>,
};

const PLAIN: i32 = 0;
const TERRAIN: i32 = 1;
const FOLIAGE: i32 = 2;
const WATER: i32 = 3;
const METAL: i32 = 4;
const RIM: i32 = 5;
const CLOTH: i32 = 6;
const SKIN: i32 = 7;
const ENERGY: i32 = 8;

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
    // The normal by the model's cofactor (its inverse-transpose, scaled),
    // so what is stretched or squashed still faces the way its surface
    // does; turned back if the model mirrors (`normal_of` in lib.rs).
    let a0 = v.m0.xyz;
    let a1 = v.m1.xyz;
    let a2 = v.m2.xyz;
    let cof = mat3x3<f32>(cross(a1, a2), cross(a2, a0), cross(a0, a1));
    o.nrm = cof * v.nrm * select(1.0, -1.0, dot(a0, cross(a1, a2)) < 0.0);
    o.col = v.col;
    o.tint = v.tint;
    o.extra = v.extra;
    o.rel = w.xyz - v.m3.xyz;
    return o;
}

/// How wide a pixel is on a surface at `pos` facing `n` (metres): wider
/// far off, and wider still seen edge on.
fn footprint(pos: vec3<f32>, n: vec3<f32>) -> f32 {
    let to = g.eye.xyz - pos;
    let d = length(to);
    return d / g.up.w / max(abs(dot(n, to / max(d, 1e-4))), 0.2);
}

/// How much of a detail `cell` metres across to keep where a pixel is
/// `foot` wide: all of it while four pixels or more cover a cell, none
/// once two do (finer, it would only shimmer).
fn detail(cell: f32, foot: f32) -> f32 {
    return 1.0 - smoothstep(cell * 0.25, cell * 0.5, foot);
}

/// The surface's own small bumps: the normal tipped by noise (`f` cells
/// a metre), smoothed away far off.
fn bump(pos: vec3<f32>, n: vec3<f32>, k: f32, f: f32) -> vec3<f32> {
    let keep = detail(1.0 / f, footprint(pos, n));
    if (keep <= 0.0) {
        return n;
    }
    let e = 0.12;
    let c = noise3(pos * f);
    let gx = noise3((pos + vec3<f32>(e, 0.0, 0.0)) * f) - c;
    let gy = noise3((pos + vec3<f32>(0.0, e, 0.0)) * f) - c;
    let gz = noise3((pos + vec3<f32>(0.0, 0.0, e)) * f) - c;
    let grad = vec3<f32>(gx, gy, gz) / e;
    return normalize(n - k * keep * (grad - dot(grad, n) * n));
}

/// The ground's grass where it is: lush or dry by the broad noise `n1`,
/// mottled by the finer `n2` and `n3` (what the blades grown on it are
/// coloured by too).
fn meadow(n1: f32, n2: f32, n3: f32) -> vec3<f32> {
    let lush = mix(vec3<f32>(0.045, 0.11, 0.022), vec3<f32>(0.09, 0.17, 0.035), n1 + n3 * 0.15);
    let dry = vec3<f32>(0.20, 0.18, 0.07);
    return mix(lush, dry, smoothstep(0.58, 0.78, n1 + n2 * 0.12));
}

/// A noise `cell` metres across at `p`, faded to its middle where a pixel
/// spans it (`foot` metres).
fn fine(p: vec3<f32>, cell: f32, foot: f32) -> f32 {
    let keep = detail(cell, foot);
    if (keep <= 0.0) {
        return 0.5;
    }
    return mix(0.5, noise3(p / cell), keep);
}

/// The ground's colour and roughness where it is: grass, dry grass,
/// rock on the slopes, sand by the sea, darker where wet; its fine
/// mottling smoothed where a pixel spans it (`foot` metres).
fn earth(pos: vec3<f32>, n: vec3<f32>, foot: f32) -> vec4<f32> {
    let slope = 1.0 - n.y;
    let h = pos.y - g.deep.w;
    let n1 = fbm3(pos * 0.06);
    let n2 = fine(pos, 1.25, foot);
    let n3 = fine(pos, 0.32, foot);
    var c = meadow(n1, n2, n3);
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
    // Each octave turned from the last, so no grid lines up.
    let a = mat2x2<f32>(0.8, 0.6, -0.6, 0.8);
    let b = mat2x2<f32>(0.28, 0.96, -0.96, 0.28);
    return noise2(q * 0.16 + vec2<f32>(t * 0.05, t * 0.03))
        + 0.5 * noise2(a * q * 0.5 - vec2<f32>(t * 0.09, -t * 0.07))
        + 0.22 * noise2(b * q * 1.5 + vec2<f32>(t * 0.2, t * 0.17));
}

/// The sea's surface at `pos`: waves, which far off are wider than a
/// pixel, so blurred (a wider step) and flattened, and the sun's glint
/// let spread, so the sea does not sparkle and crawl as you move.
fn sea_normal(pos: vec3<f32>) -> vec3<f32> {
    let t = g.eye.w;
    let p = pos.xz;
    let dist = length(g.eye.xyz - pos);
    let e = 0.1 + dist * 0.012;
    let h0 = wave(p, t);
    let s = g.water.w / (1.0 + dist * 0.02);
    var n = vec3<f32>(-(wave(p + vec2<f32>(e, 0.0), t) - h0) / e * s, 1.0, -(wave(p + vec2<f32>(0.0, e), t) - h0) / e * s);
    // In the rain, pocked with drops (near: far off they blur away).
    let wet = g.wind.z / (1.0 + dist * 0.05);
    if (wet > 0.01) {
        let q = p * 3.5 + vec2<f32>(t * 0.7, -t * 0.5);
        let r = vec2<f32>(noise2(q) - 0.5, noise2(q + vec2<f32>(17.3, 5.1)) - 0.5);
        let f = 0.5 + 0.5 * sin(t * 9.0 + noise2(p * 1.7) * 30.0);
        n = n + vec3<f32>(r.x, 0.0, r.y) * wet * 0.5 * f;
    }
    return normalize(n);
}

/// The sea: waves, the sky in it (and `mirror`: what of the scene it
/// reflects, how surely), the deep and the shallows, the sun's glint,
/// foam at the shore.
fn sea_shade(pos: vec3<f32>, alpha: f32, n: vec3<f32>, mirror: vec4<f32>) -> vec4<f32> {
    let t = g.eye.w;
    let p = pos.xz;
    let dist = length(g.eye.xyz - pos);
    let v = normalize(g.eye.xyz - pos);
    let nv = max(dot(n, v), 0.0);
    let fres = 0.02 + 0.98 * pow(1.0 - nv, 5.0);
    // The sky in it and its clouds (fewer octaves: only mirrored), or
    // what of the scene it mirrors.
    let rd = reflect(-v, n);
    let refl = mix(clouded(sky(rd), rd, cloud(rd, 2)), mirror.rgb, mirror.a);
    // Past the terrain there is no bed at all: as deep as can be.
    let depth = select(1e4, max(pos.y - ground(p), 0.0), on_terrain(p));
    let light = g.sky.rgb * 0.9 + g.sun.rgb * max(g.sun_dir.y, 0.0) * 0.3;
    let body = mix(vec3<f32>(0.03, 0.22, 0.20), g.water.rgb, smoothstep(0.0, SEA_DEEP, depth)) * light;
    let lit = sunlit(pos, vec3<f32>(0.0, 1.0, 0.0));
    let glint = ggx(n, v, g.sun_dir.xyz, clamp(0.08 + dist * 0.004, 0.08, 0.4), vec3<f32>(0.02)) * g.sun.rgb * max(dot(n, g.sun_dir.xyz), 0.0) * lit;
    var c = mix(body, refl, fres) + glint;
    let foam = (1.0 - smoothstep(0.0, 0.8, depth)) * smoothstep(0.4, 0.75, noise2(p * 1.2 + vec2<f32>(t * 0.25, -t * 0.2)));
    c = mix(c, vec3<f32>(0.9, 0.93, 0.95) * light * 1.4, foam * 0.75);
    // Where it mirrors the land, it shows less of its own body; deep, it
    // shows nothing of its bed (so where the bed ends does not show).
    let clear = mix(0.6, 1.0, max(fres, mirror.a * 0.5));
    let a = clamp(max(clear, smoothstep(SEA_DEEP * 0.4, SEA_DEEP, depth)) * smoothstep(0.0, 0.3, depth) + foam * 0.6, 0.0, 1.0);
    return vec4<f32>(air(c, pos), a * alpha);
}

fn sea(pos: vec3<f32>, alpha: f32) -> vec4<f32> {
    return sea_shade(pos, alpha, sea_normal(pos), vec4<f32>(0.0));
}

/// The picture so far, before the sea and what glows (the pass that
/// draws them reads it): what the sea can mirror.
@group(1) @binding(1) var scene_color: texture_2d<f32>;
@group(1) @binding(2) var scene_samp: sampler;

/// Where on the screen `p` falls (pixels), and its distance along the
/// view (as the depth stores it: NEAR_PLANE / depth).
fn on_screen(p: vec3<f32>) -> vec3<f32> {
    let c = g.vp * vec4<f32>(p, 1.0);
    let uv = c.xy / c.w * vec2<f32>(0.5, -0.5) + 0.5;
    return vec3<f32>(uv, c.w);
}

/// What the sea at `pos` mirrors along `r` of what is on the screen: the
/// colour where the ray meets it, and how surely (0: nothing found, the
/// sky it is). The ray marches out in growing steps to 120 m, and is
/// refined where it first passes behind what stands there.
fn mirror(pos: vec3<f32>, r: vec3<f32>) -> vec4<f32> {
    if (r.y <= 0.0) {
        return vec4<f32>(0.0);
    }
    let size = g.view.xy;
    var prev = 0.0;
    var t = 0.6;
    for (var k = 0; k < SSR_STEPS; k++) {
        let s = on_screen(pos + r * t);
        if (s.z <= NEAR_PLANE || any(s.xy < vec2<f32>(0.0)) || any(s.xy > vec2<f32>(1.0))) {
            break;
        }
        let d = textureLoad(scene_depth, vec2<i32>(s.xy * size), 0);
        let behind = NEAR_PLANE / max(d, 1e-7);
        if (d > 0.0 && s.z > behind && s.z - behind < 1.5 + 0.06 * s.z) {
            var lo = prev;
            var hi = t;
            for (var j = 0; j < 4; j++) {
                let mid = 0.5 * (lo + hi);
                let m = on_screen(pos + r * mid);
                let dm = textureLoad(scene_depth, vec2<i32>(clamp(m.xy, vec2<f32>(0.0), vec2<f32>(0.999)) * size), 0);
                if (dm > 0.0 && m.z > NEAR_PLANE / dm) {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            let h = on_screen(pos + r * hi);
            let edge = smoothstep(0.0, 0.1, min(min(h.x, 1.0 - h.x), min(h.y, 1.0 - h.y)));
            let far = 1.0 - smoothstep(0.6, 1.0, f32(k) / f32(SSR_STEPS));
            return vec4<f32>(textureSampleLevel(scene_color, scene_samp, h.xy, 0.0).rgb, edge * far);
        }
        prev = t;
        t = t * SSR_GROW;
    }
    return vec4<f32>(0.0);
}

/// Energy: noise flowing up and over it, thick here and torn there. Fire
/// (`rough` 0) is thickest face on, white-hot where thickest, red and
/// ragged at its edges; plasma (`rough` 1) glows at its rim like a
/// bubble.
fn energy(i: WorldOut, n: vec3<f32>) -> vec4<f32> {
    let v = normalize(g.eye.xyz - i.pos);
    let facing = abs(dot(n, v));
    let edge = 1.0 - facing;
    let rim = clamp(i.extra.y, 0.0, 1.0);
    let t = g.eye.w;
    let q = i.rel * select(2.2, i.extra.w, i.extra.w > 0.0);
    let n1 = fbm3(q + vec3<f32>(t * 0.3, -t * 1.9, t * 0.2));
    let n2 = noise3(q * 2.6 + vec3<f32>(t * 0.9, -t * 2.6, -t * 0.6));
    let fire = clamp(n1 * 1.3 + n2 * 0.55 - 0.45 + mix(facing * 0.45 - 0.1, edge * 0.3, rim), 0.0, 1.0);
    let body = smoothstep(0.18, 0.7, fire);
    let col = linear(i.col.rgb) * linear(i.tint.rgb);
    let peak = max(col.r, max(col.g, col.b));
    // Cooler and redder where thin, white-hot where thick.
    let cool = col * vec3<f32>(1.0, 0.55, 0.4);
    let hot = mix(mix(cool, col, smoothstep(0.2, 0.55, fire)), vec3<f32>(1.0, 0.95, 0.85) * peak, smoothstep(0.6, 1.0, fire) * 0.8);
    let lit = 0.1 + body * 2.2 + rim * edge * edge * 1.6;
    let c = hot * lit * (1.0 + (i.col.a + i.extra.x) * 2.0) * g.wind.w;
    let a = clamp(body * mix(sqrt(facing), 0.3, rim) + rim * edge * edge * 0.6, 0.0, 1.0);
    return vec4<f32>(air(c, i.pos), i.tint.a * a);
}

@fragment
fn world_fs(i: WorldOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    return world(i, front);
}

/// How much of what is drawn at `clip` shows in front of what stands
/// behind it: none where it meets it, all from `SOFT_FADE` metres before
/// it, so what glows or is see-through cuts no hard line into the
/// ground and the walls.
fn soft(clip: vec4<f32>) -> f32 {
    let d = textureLoad(scene_depth, vec2<i32>(clip.xy), 0);
    if (d <= 0.0) {
        return 1.0;
    }
    let me = NEAR_PLANE / max(clip.z, 1e-7);
    return clamp((NEAR_PLANE / d - me) / SOFT_FADE, 0.0, 1.0);
}

/// What is see-through, in the pass after the solid one: the sea mirrors
/// what stands about it (`mirror`, when the tier has reflections); the
/// rest fades into what is behind it.
@fragment
fn faint_fs(i: WorldOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    if (i32(i.extra.z + 0.5) == WATER) {
        let n = sea_normal(i.pos);
        if (!SSR_ON) {
            return sea_shade(i.pos, i.tint.a, n, vec4<f32>(0.0));
        }
        let v = normalize(g.eye.xyz - i.pos);
        // Off waves calmed a little, so the reflection does not shimmer.
        let r = reflect(-v, normalize(mix(n, vec3<f32>(0.0, 1.0, 0.0), 0.65)));
        return sea_shade(i.pos, i.tint.a, n, mirror(i.pos, r));
    }
    let c = world(i, front);
    return vec4<f32>(c.rgb, c.a * soft(i.clip));
}

/// What glows, in the pass after the solid one: fading into what is
/// behind it; a shield or a ring brighter just where it meets it, as
/// energy touching the ground.
@fragment
fn glow_soft_fs(i: WorldOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let c = world(i, front);
    let k = soft(i.clip);
    if (i32(i.extra.z + 0.5) == RIM) {
        return vec4<f32>(c.rgb, c.a * mix(k, 1.6, k * (1.0 - k) * 4.0));
    }
    return vec4<f32>(c.rgb, c.a * k);
}

fn world(i: WorldOut, front: bool) -> vec4<f32> {
    var n = normalize(i.nrm);
    if (!front) {
        n = -n;
    }
    let mat = i32(i.extra.z + 0.5);
    if (mat == WATER) {
        return sea(i.pos, i.tint.a);
    }
    if (mat == RIM) {
        // Energy: bright where it is seen edge on, clear face on.
        let v = normalize(g.eye.xyz - i.pos);
        let edge = pow(1.0 - abs(dot(n, v)), 2.5);
        let c = linear(i.col.rgb) * linear(i.tint.rgb) * (0.1 + edge * 2.5) * (1.0 + (i.col.a + i.extra.x) * 3.0) * g.wind.w;
        return vec4<f32>(c, i.tint.a);
    }
    if (mat == ENERGY) {
        return energy(i, n);
    }
    var base = linear(i.col.rgb) * linear(i.tint.rgb);
    var rough = i.extra.y;
    var metal = 0.0;
    var through = 0.0;
    var ao = 1.0;
    var glow = i.col.a + i.extra.x;
    if (i.extra.w > 0.0) {
        n = bump(i.pos, n, i.extra.w, 1.6);
    }
    if (mat == TERRAIN) {
        // The ground, and the game's paint over it (alpha is how much).
        let m = earth(i.pos, n, footprint(i.pos, n));
        let paint = linear(i.col.rgb) * (0.8 + 0.4 * noise3(i.pos * 0.9));
        base = mix(m.rgb, paint, clamp(i.col.a, 0.0, 1.0)) * linear(i.tint.rgb);
        rough = mix(m.a, 0.8, clamp(i.col.a, 0.0, 1.0));
        glow = i.extra.x + max(i.col.a - 1.0, 0.0) * (0.7 + 0.6 * noise3(i.pos * 1.3 + g.eye.w * 0.2));
        n = bump(i.pos, n, 0.35, 0.9);
    } else if (mat == FOLIAGE) {
        // Its edge broken into leaves where it turns away (not a smooth
        // blob against the sky), its face clumped.
        let v = normalize(g.eye.xyz - i.pos);
        let edge = 1.0 - abs(dot(n, v));
        if (noise3(i.pos * LEAF_GRAIN) < edge * 1.2 - 0.5) {
            discard;
        }
        n = bump(i.pos, n, 0.6, 3.0);
        through = 0.45;
        base = base * (0.75 + 0.5 * noise3(i.pos * 1.7));
        ao = 0.8;
    } else if (mat == METAL) {
        metal = 1.0;
    }
    // Wet: darker, and glossier (but the leaves).
    if (g.wind.z > 0.0 && mat != FOLIAGE && glow < 0.01) {
        base = base * (1.0 - 0.25 * g.wind.z);
        rough = mix(rough, rough * 0.45, g.wind.z);
    }
    let c = shade(i.pos, n, base, rough, metal, glow * g.wind.w, ao, through, mat);
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
    let bare = sky(d);
    // Clouds: a layer high above, drifting, lit by the sun; the sun's
    // disc and the stars only where they leave the sky clear.
    let cl = cloud(d, 4);
    var c = clouded(bare, d, cl);
    let clear = 1.0 - cl.x;
    let mu = dot(d, g.sun_dir.xyz);
    c = c + g.sun.rgb * 12.0 * smoothstep(g.sun_dir.w, g.sun_dir.w + 0.00012, mu) * clear;
    // Stars, at night: round points (a ball in each lit cell of space,
    // met by the sky's sphere), gone where the sky is bright.
    if (g.sun.w > 0.0) {
        let cell = d * 260.0;
        let h = hash3(floor(cell));
        if (h > 0.997) {
            let f = fract(cell) - 0.5;
            let tw = 0.65 + 0.35 * sin(g.eye.w * 2.0 + h * 500.0);
            let dark = 1.0 - smoothstep(0.08, 0.25, dot(bare, vec3<f32>(0.2126, 0.7152, 0.0722)));
            let star = (h - 0.997) / 0.003 * exp(-dot(f, f) * 8.0) * 1.6;
            c = c + vec3<f32>(1.0, 0.95, 0.85) * star * tw * g.sun.w * smoothstep(0.0, 0.15, d.y) * clear * dark;
        }
    }
    return vec4<f32>(c, 1.0);
}

struct SparkOut {
    @builtin(position) clip: vec4<f32>,
    // Where in it, in its radii: x along its streak, y across.
    @location(0) uv: vec2<f32>,
    @location(1) col: vec4<f32>,
    // x its shape, y half its streak's length (in radii), z its seed.
    @location(2) shape: vec3<f32>,
    // x how far it is from the eye, y how deep it fades into what is
    // behind it (metres).
    @location(3) far: vec2<f32>,
};

/// The scene's depth, for sparks to fade into what stands behind them
/// (soft); only the pass that reads it, not the one that writes it.
@group(1) @binding(0) var scene_depth: DEPTH_TYPE;

const GLOW_SPARK: i32 = 0;
const FLAME: i32 = 1;
const SMOKE: i32 = 2;
const STAR: i32 = 3;

@vertex
fn spark_vs(@builtin(vertex_index) i: u32, @location(0) ps: vec4<f32>, @location(1) col: vec4<f32>, @location(2) vk: vec4<f32>) -> SparkOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0));
    let c = corners[i];
    let head = g.vp * vec4<f32>(ps.xyz, 1.0);
    // Its radius on screen, in pixels: a point of light kept small, a
    // flame or a puff let grow to a share of the screen.
    let kind = i32(floor(vk.w) + 0.5);
    let most = select(SPARK_MAX_PX, PUFF_MAX * g.view.y, kind == FLAME || kind == SMOKE);
    let size = ps.w * g.up.w / max(head.w, 0.05) * 0.5;
    let r = clamp(size, SPARK_MIN_PX, most * 0.5);
    // Drawn larger than it is, it is dimmed to keep its light (so far off
    // it does not blaze and twinkle), never below a floor (a far fight
    // still shows).
    let dim = max(min(size / r, 1.0) * min(size / r, 1.0), SPARK_FAR_FLOOR);
    // A streak runs on screen from where it was (its tail) to its head.
    var mid = head;
    var dir = vec2<f32>(1.0, 0.0);
    var half = 0.0;
    if (dot(vk.xyz, vk.xyz) > 1e-8) {
        let tail = g.vp * vec4<f32>(ps.xyz - vk.xyz, 1.0);
        if (tail.w > 0.05 && head.w > 0.05) {
            let d = (head.xy / head.w - tail.xy / tail.w) * g.view.xy * 0.5;
            let len = length(d);
            if (len > 0.5) {
                dir = d / len;
                half = len * 0.5;
                mid = (head + tail) * 0.5;
            }
        }
    }
    let off = dir * c.x * (half + r) + vec2<f32>(-dir.y, dir.x) * c.y * r;
    var clip = mid;
    clip.x = clip.x + off.x * 2.0 / g.view.x * clip.w;
    clip.y = clip.y + off.y * 2.0 / g.view.y * clip.w;
    var o: SparkOut;
    o.clip = clip;
    o.uv = vec2<f32>(c.x * (half + r) / r, c.y);
    // Fading out right by the eye.
    o.col = vec4<f32>(col.rgb, col.a * dim * smoothstep(SPARK_NEAR * 0.5, SPARK_NEAR, head.w));
    o.shape = vec3<f32>(floor(vk.w), half / r, fract(vk.w));
    o.far = vec2<f32>(mid.w, max(ps.w * 0.5, 0.15));
    return o;
}

@fragment
fn spark_fs(i: SparkOut) -> @location(0) vec4<f32> {
    return spark(i);
}

/// A spark fading as it nears what stands behind it, so a puff meeting
/// the ground does not cut a hard line into it.
@fragment
fn spark_soft_fs(i: SparkOut) -> @location(0) vec4<f32> {
    let d = textureLoad(scene_depth, vec2<i32>(i.clip.xy), 0);
    var k = 1.0;
    if (d > 0.0) {
        let behind = NEAR_PLANE / d;
        k = clamp((behind - i.far.x) / i.far.y, 0.0, 1.0);
    }
    return spark(i) * k;
}

fn spark(i: SparkOut) -> vec4<f32> {
    let long = i.shape.y;
    let d = length(vec2<f32>(max(abs(i.uv.x) - long, 0.0), i.uv.y));
    let shape = i32(i.shape.x + 0.5);
    let seed = i.shape.z * 97.0;
    let t = g.eye.w;
    let col = linear(i.col.rgb);
    // A streak is brightest at its head, fading to its tail.
    var along = 1.0;
    if (long > 0.0) {
        along = 0.25 + 0.75 * smoothstep(-long - 1.0, long + 1.0, i.uv.x);
    }
    if (shape == FLAME) {
        let n = noise3(vec3<f32>(i.uv * 1.6 + seed, t * 2.6 + seed)) * 0.65
            + noise3(vec3<f32>(i.uv * 3.7 - seed, t * 4.1)) * 0.35;
        let body = 1.0 - smoothstep(0.2, 1.0, d + (n - 0.5) * 0.75);
        let core = 1.0 - smoothstep(0.0, 0.6, d + (n - 0.5) * 0.5);
        let hot = mix(col, vec3<f32>(1.0, 0.9, 0.7), core * 0.55);
        return vec4<f32>(hot * body * (1.1 + core * 1.9) * i.col.a * along * g.wind.w, 0.0);
    }
    if (shape == SMOKE) {
        let n = noise3(vec3<f32>(i.uv * 1.4 + seed, t * 0.5 + seed)) * 0.7
            + noise3(vec3<f32>(i.uv * 3.1 - seed, t * 0.9)) * 0.3;
        let a = (1.0 - smoothstep(0.1, 1.0, d + (n - 0.5) * 0.9)) * i.col.a;
        let light = g.sky.rgb + g.sun.rgb * max(g.sun_dir.y, 0.0) * 0.35;
        return vec4<f32>(col * light * a, a);
    }
    if (shape == STAR) {
        let q = abs(i.uv);
        let fade = 1.0 - smoothstep(0.0, 1.0, max(q.x, q.y));
        let rays = max(1.0 - smoothstep(0.0, 0.07, q.y), 1.0 - smoothstep(0.0, 0.07, q.x)) * fade;
        let core = 1.0 - smoothstep(0.0, 0.32, d);
        let k = rays * 1.4 + core * core * 4.0;
        return vec4<f32>(mix(col, vec3<f32>(1.0), core * 0.6) * k * i.col.a * g.wind.w, 0.0);
    }
    let a = (1.0 - smoothstep(0.3, 1.0, d)) * i.col.a * along;
    let core = 1.0 - smoothstep(0.0, 0.45, d);
    return vec4<f32>(col * a * (2.5 + core * 4.0) * g.wind.w, 0.0);
}

struct GrassOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) pos: vec3<f32>,
    @location(1) col: vec3<f32>,
    @location(2) tip: f32,
    @location(3) side: vec3<f32>,
};

/// Whether a blade rising from `a` to `b` (in clip space) lies wholly
/// past one edge of the screen, or behind the eye, with `m` to spare
/// for its sway.
fn off_screen(a: vec4<f32>, b: vec4<f32>, m: f32) -> bool {
    return (a.x < -a.w - m && b.x < -b.w - m) || (a.x > a.w + m && b.x > b.w + m)
        || (a.y < -a.w - m && b.y < -b.w - m) || (a.y > a.w + m && b.y > b.w + m)
        || (a.w < -m && b.w < -m);
}

@vertex
fn grass_vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> GrassOut {
    // A blade's five corners (its two triangles and its tip share them).
    var shape = array<vec2<f32>, 5>(
        vec2<f32>(-1.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(-0.7, 0.5),
        vec2<f32>(0.7, 0.5), vec2<f32>(0.0, 1.0));
    let k = shape[vi];
    var o: GrassOut;
    o.clip = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    let sp = g.grass.x;
    let side = u32(g.grass.y);
    // Each blade belongs to a whole-numbered cell of the ground and is
    // hashed from it, so it is the same blade wherever the eye stands.
    let corner = vec2<i32>(floor(g.eye.xz / sp)) - vec2<i32>(i32(side / 2u));
    let cell = corner + vec2<i32>(i32(ii % side), i32(ii / side));
    let r1 = cell_hash(cell, 1u);
    let r2 = cell_hash(cell, 2u);
    let xz = vec2<f32>(cell) * sp + (vec2<f32>(r1, r2) - 0.5) * sp * 1.8;
    // Out of reach, or thinned away far off (the rest made wider to
    // cover as much): none, before the ground is read.
    let dist = length(xz - g.eye.xz);
    let reach = g.grass.z;
    let keep = 1.0 - GRASS_THIN * smoothstep(reach * 0.45, reach, dist);
    if (g.grass.w < 0.5 || dist > reach || cell_hash(cell, 4u) > keep) {
        return o;
    }
    let here = terrain_at(xz);
    let gh = here.h;
    // Off the screen, root and tallest tip: none.
    let foot = g.vp * vec4<f32>(xz.x, gh, xz.y, 1.0);
    let top = g.vp * vec4<f32>(xz.x, gh + 0.75, xz.y, 1.0);
    if (off_screen(foot, top, 1.2)) {
        return o;
    }
    // Where little grows, fewer blades (not a carpet of stubs): how much
    // is how many, and a little how tall.
    if (cell_hash(cell, 5u) > here.lush * 1.25) {
        return o;
    }
    let r3 = cell_hash(cell, 3u);
    let height = (0.28 + 0.42 * r3)
        * (1.0 - smoothstep(reach * 0.7, reach, dist))
        * (1.0 - smoothstep(0.35, 0.6, length(here.slope)))
        * smoothstep(g.deep.w + 0.7, g.deep.w + 1.4, gh)
        * smoothstep(0.2, 0.45, noise2(xz * 0.09))
        * mix(0.6, 1.0, here.lush);
    if (height < 0.04) {
        return o;
    }
    let a = r1 * 6.2832;
    let across = vec2<f32>(cos(a), sin(a));
    let width = 0.035 * (1.0 - k.y * 0.6) / keep;
    let phase = g.eye.w * 1.8 + xz.x * 0.35 + xz.y * 0.27;
    let bend = (sin(phase) * 0.5 + 0.7 + sin(phase * 2.7) * 0.2) * k.y * k.y * height * 0.45;
    let lean = vec2<f32>(g.wind.x, g.wind.y) * bend + (vec2<f32>(r2, r3) - 0.5) * k.y * height * 0.3;
    let p = vec3<f32>(xz.x + across.x * k.x * width + lean.x, gh + k.y * height, xz.y + across.y * k.x * width + lean.y);
    o.clip = g.vp * vec4<f32>(p, 1.0);
    o.pos = p;
    // The colour of the ground it grows from at its root, so it melts
    // into it; lighter and a little yellower toward its tip.
    let soil = meadow(fbm(vec3<f32>(xz.x, gh, xz.y) * 0.06, 3), 0.5, 0.5);
    let tone = noise2(xz * 0.05);
    let root = soil * 0.85;
    let tipc = soil * (1.35 + 0.35 * tone) + vec3<f32>(0.025, 0.02, 0.0) * tone;
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
    let c = shade(i.pos, n, i.col, 0.85, 0.0, 0.0, mix(GRASS_ROOT, 1.0, i.tip), 0.6, PLAIN);
    return vec4<f32>(air(c, i.pos), 1.0);
}
"#;
