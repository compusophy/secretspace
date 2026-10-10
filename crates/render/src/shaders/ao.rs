//! Ambient occlusion from the depth alone, at half the screen's size
//! (after HBAO): each pixel's place and facing found from the depth about
//! it, then a few ways out across the screen (turned pixel by pixel), a
//! few steps each; whatever rises over the surface within reach shades
//! it, the more the steeper and the nearer. It is laid over the lit
//! picture, so it is eased by how much of a pixel's light is direct (the
//! sun's where it reaches, a light's, a glow's: the solid pass keeps that
//! in the picture's alpha), which only the sky's light is shut out of,
//! and where the air between hides the surface (fog is not occluded).
//! Then a blur that keeps to its own depth, and a pass that lays it over
//! the picture (multiplied), each pixel taking its nearer neighbours' so
//! edges stay sharp.

const AO: &str = r#"
struct Ao {
    proj: vec4<f32>,   // x f/aspect, y f, z near, w reach (metres)
    size: vec4<f32>,   // xy the screen, zw half of it (pixels)
    k: vec4<f32>,      // x power, y slack (a slope), z ways, w fade (metres)
    m: vec4<f32>,      // x strength, yzw how far up the camera's right, up and forward go
    air: vec4<f32>,    // x the eye's height, y fog a metre, z its falloff with height, w how much direct light eases it
};

@group(0) @binding(0) var depth: DEPTH_TYPE;
@group(0) @binding(1) var<uniform> ao: Ao;
@group(0) @binding(2) var src: texture_2d<f32>;
// The picture of what is solid: alpha how much of its light is direct
// (only the occlusion itself reads it, not the pass laying it over).
@group(1) @binding(0) var picture: PICTURE_TYPE;

struct Out {
    @builtin(position) clip: vec4<f32>,
};

@vertex
fn ao_vs(@builtin(vertex_index) i: u32) -> Out {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
    var o: Out;
    o.clip = vec4<f32>(p, 0.0, 1.0);
    return o;
}

fn depth_at(px: vec2<i32>) -> f32 {
    let hi = vec2<i32>(ao.size.xy) - vec2<i32>(1);
    return textureLoad(depth, clamp(px, vec2<i32>(0), hi), 0);
}

/// Where the scene is at a pixel, seen from the eye (looking down -z).
fn view_at(px: vec2<i32>) -> vec3<f32> {
    let z = ao.proj.z / max(depth_at(px), 1e-7);
    let uv = (vec2<f32>(px) + 0.5) / ao.size.xy;
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    return vec3<f32>(ndc.x * z / ao.proj.x, ndc.y * z / ao.proj.y, -z);
}

/// The pixel a point seen from the eye falls on.
fn to_px(v: vec3<f32>) -> vec2<f32> {
    let z = max(-v.z, 1e-4);
    let ndc = vec2<f32>(v.x * ao.proj.x / z, v.y * ao.proj.y / z);
    return vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5) * ao.size.xy;
}

/// How alike two distances are, as a weight (1 the same).
fn alike(z: f32, z0: f32) -> f32 {
    let rel = abs(z - z0) / max(-z0, 0.1);
    return 1.0 / (1.0 + rel * 60.0);
}

@fragment
fn ao_fs(i: Out) -> @location(0) vec4<f32> {
    let half = vec2<i32>(i.clip.xy);
    let px = half * 2;
    if (depth_at(px) <= 0.0) {
        return vec4<f32>(1.0);
    }
    let p = view_at(px);
    let far = -p.z;
    // How many pixels its reach spans here (a few: too far to matter).
    let span = min(ao.proj.w * ao.proj.y * ao.size.y * 0.5 / far, ao.size.y * 0.2);
    if (far > ao.k.w || span < 3.0) {
        return vec4<f32>(1.0);
    }
    // Its facing: across the nearer neighbour each way, so an edge does
    // not smear.
    let l = view_at(px - vec2<i32>(2, 0));
    let r = view_at(px + vec2<i32>(2, 0));
    let u = view_at(px - vec2<i32>(0, 2));
    let d = view_at(px + vec2<i32>(0, 2));
    let dx = select(p - l, r - p, abs(r.z - p.z) < abs(p.z - l.z));
    let dy = select(p - u, d - p, abs(d.z - p.z) < abs(p.z - u.z));
    var n = normalize(cross(dx, dy));
    if (dot(n, -p) < 0.0) {
        n = -n;
    }
    // Out across the screen each way, a few steps each: whatever rises
    // over the surface within reach shades it, the more the steeper and
    // the nearer.
    // Turned and stepped from pixel to pixel, so the blur evens it out.
    let turn = ign(vec2<f32>(half));
    let jitter = ign(vec2<f32>(half + vec2<i32>(7, 3)));
    let ways = max(i32(ao.k.z), 1);
    let reach2 = ao.proj.w * ao.proj.w;
    var shut = 0.0;
    for (var w = 0; w < ways; w = w + 1) {
        let a = (f32(w) + turn) / f32(ways) * 6.2831853;
        let way = vec2<f32>(cos(a), sin(a));
        for (var s = 0; s < 4; s = s + 1) {
            let along = 2.0 + (span - 2.0) * (f32(s) + jitter) / 4.0;
            let v = view_at(px + vec2<i32>(round(way * along))) - p;
            let d2 = max(dot(v, v), 1e-8);
            let rise = dot(n, v) * inverseSqrt(d2) - ao.k.y;
            shut = shut + max(rise, 0.0) * clamp(1.0 - d2 / reach2, 0.0, 1.0);
        }
    }
    let open = clamp(1.0 - shut / f32(ways * 4) * ao.m.x, 0.0, 1.0);
    let fade = smoothstep(ao.k.w, ao.k.w * 0.6, far);
    var a = mix(1.0, pow(open, ao.k.x), fade);
    // What of its light is direct (the sun's where it reaches it), the
    // occlusion does not shut out: only the sky's.
    a = mix(a, 1.0, ao.air.w * textureLoad(picture, px, 0).a);
    return vec4<f32>(mix(1.0, a, through_air(p)), 0.0, 0.0, 1.0);
}

/// How much of a surface at `p` (seen from the eye) shows through the
/// air between, as the scene's fog has it (`air` in the scene's shaders).
fn through_air(p: vec3<f32>) -> f32 {
    let dist = length(p);
    let rise = (p.x * ao.m.y + p.y * ao.m.z - p.z * ao.m.w) / max(dist, 1e-3);
    let falloff = ao.air.z;
    let base = ao.air.y * exp(-falloff * max(ao.air.x, 0.0));
    var optical = base * dist;
    let k = falloff * rise * dist;
    if (abs(k) > 0.001) {
        optical = base * (1.0 - exp(-k)) / (falloff * rise);
    }
    return exp(-max(optical, 0.0));
}

/// Four by four, each kept to its own depth.
@fragment
fn blur_fs(i: Out) -> @location(0) vec4<f32> {
    let half = vec2<i32>(i.clip.xy);
    let hi = vec2<i32>(ao.size.zw) - vec2<i32>(1);
    let z0 = view_at(half * 2).z;
    var sum = 0.0;
    var w = 0.0;
    for (var y = -2; y < 2; y = y + 1) {
        for (var x = -2; x < 2; x = x + 1) {
            let q = clamp(half + vec2<i32>(x, y), vec2<i32>(0), hi);
            let wt = alike(view_at(q * 2).z, z0);
            sum = sum + textureLoad(src, q, 0).r * wt;
            w = w + wt;
        }
    }
    return vec4<f32>(sum / max(w, 1e-4), 0.0, 0.0, 1.0);
}

/// Over the picture, at full size: the half-size texels about each pixel,
/// those at its own depth counting most. The pass multiplies.
@fragment
fn apply_fs(i: Out) -> @location(0) vec4<f32> {
    let px = vec2<i32>(i.clip.xy);
    if (depth_at(px) <= 0.0) {
        return vec4<f32>(1.0);
    }
    let z0 = view_at(px).z;
    let hi = vec2<i32>(ao.size.zw) - vec2<i32>(1);
    let h0 = px / 2;
    let odd = vec2<f32>(px & vec2<i32>(1)) * 0.5;
    var sum = 0.0;
    var w = 0.0;
    for (var y = 0; y < 2; y = y + 1) {
        for (var x = 0; x < 2; x = x + 1) {
            let q = clamp(h0 + vec2<i32>(x, y), vec2<i32>(0), hi);
            let bx = select(1.0 - odd.x, odd.x, x == 1);
            let by = select(1.0 - odd.y, odd.y, y == 1);
            let wt = (bx * by + 1e-3) * alike(view_at(q * 2).z, z0);
            sum = sum + textureLoad(src, q, 0).r * wt;
            w = w + wt;
        }
    }
    let a = sum / max(w, 1e-5);
    return vec4<f32>(a, a, a, 1.0);
}
"#;

/// The AO module, for a depth and a picture that are many-sampled
/// (`msaa`) or not.
pub fn ao(msaa: bool) -> String {
    let picture = if msaa {
        "texture_multisampled_2d<f32>"
    } else {
        "texture_2d<f32>"
    };
    format!("{}{}", AO, super::IGN)
        .replace("DEPTH_TYPE", super::depth_type(msaa))
        .replace("PICTURE_TYPE", picture)
}
