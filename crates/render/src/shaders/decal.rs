//! Decals: a box drawn about each (its inside faces, so the eye may stand
//! in it), each pixel finding where the scene stands there from its depth
//! and, if that lies in the decal, marking it: burnt, rimed, runed or
//! ringed. Laid over the lit picture (premultiplied: black lays over,
//! light adds).

pub const DECAL: &str = r#"
struct DecalOut {
    @builtin(position) clip: vec4<f32>,
    // Its middle and radius; (cos, sin) of its turn, its depth, its mark
    // and seed; its colour and how much.
    @location(0) @interpolate(flat) at: vec4<f32>,
    @location(1) @interpolate(flat) shape: vec4<f32>,
    @location(2) @interpolate(flat) col: vec4<f32>,
};

const SCORCH: i32 = 0;
const FROST: i32 = 1;
const RUNES: i32 = 2;

@vertex
fn decal_vs(@builtin(vertex_index) i: u32, @location(0) at: vec4<f32>, @location(1) shape: vec4<f32>, @location(2) col: vec4<f32>) -> DecalOut {
    // A box's twelve triangles, each wound to face out.
    var corners = array<u32, 36>(
        0u, 6u, 2u, 0u, 4u, 6u, 1u, 3u, 7u, 1u, 7u, 5u, 0u, 1u, 5u, 0u, 5u, 4u,
        2u, 7u, 3u, 2u, 6u, 7u, 0u, 3u, 1u, 0u, 2u, 3u, 4u, 5u, 7u, 4u, 7u, 6u);
    let k = corners[i];
    let c = vec3<f32>(
        select(-1.0, 1.0, (k & 1u) != 0u),
        select(-1.0, 1.0, (k & 2u) != 0u),
        select(-1.0, 1.0, (k & 4u) != 0u));
    let p = at.xyz + c * vec3<f32>(at.w, shape.z, at.w);
    var o: DecalOut;
    o.clip = g.vp * vec4<f32>(p, 1.0);
    o.at = at;
    o.shape = shape;
    o.col = col;
    return o;
}

@fragment
fn decal_fs(i: DecalOut) -> @location(0) vec4<f32> {
    let d = textureLoad(scene_depth, vec2<i32>(i.clip.xy), 0);
    // Where the scene stands at this pixel: along its ray, as far ahead
    // as the depth says.
    let ndc = vec2<f32>(i.clip.x / g.view.x * 2.0 - 1.0, 1.0 - i.clip.y / g.view.y * 2.0);
    let ray = g.fwd.xyz + g.right.xyz * (ndc.x * g.fwd.w) + g.up.xyz * (ndc.y * g.right.w);
    let pos = g.eye.xyz + ray * (NEAR_PLANE / max(d, 1e-7));
    // How level it is there (worked out before any pixel is cut away).
    let level = abs(normalize(cross(dpdx(pos), dpdy(pos))).y);
    let o = pos - i.at.xyz;
    let q = vec2<f32>(o.x * i.shape.x + o.z * i.shape.y, o.z * i.shape.x - o.x * i.shape.y) / i.at.w;
    let h = abs(o.y) / i.shape.z;
    let r = length(q);
    if (d <= 0.0 || r >= 1.0 || h >= 1.0) {
        discard;
    }
    // Burns and rime take to anything (grass, a trunk's foot); light
    // lies only on the ground (and the grass on it) or what is level.
    let k = i.col.a * (1.0 - smoothstep(0.5, 1.0, h));
    let lie = max(1.0 - smoothstep(0.4, 1.0, pos.y - ground(pos.xz)), smoothstep(0.75, 0.95, level));
    let mark = i32(floor(i.shape.w) + 0.5);
    let seed = fract(i.shape.w) * 61.0;
    let c = linear(i.col.rgb);
    if (mark == SCORCH) {
        // Black, ragged at its edge; cracks glowing in its middle.
        let n = fbm3(vec3<f32>(q * 3.0, seed));
        let burn = 1.0 - smoothstep(0.5, 1.0, r + (n - 0.45) * 0.5);
        // Cracks: where a warped noise crosses its middle, thin, running
        // out from the heart of it.
        let warp = vec2<f32>(noise3(vec3<f32>(q * 4.0, seed + 7.0)), noise3(vec3<f32>(q * 4.0, seed + 9.0)));
        let cn = noise3(vec3<f32>(q * 11.0 + warp * 2.5, seed + 3.0));
        let crack = (1.0 - smoothstep(0.0, 0.04, abs(cn - 0.5))) * (1.0 - smoothstep(0.1, 0.65, r));
        let ember = c * (crack * 3.0 + (1.0 - smoothstep(0.0, 0.35, r)) * 0.5) * g.wind.w * lie;
        // Soot, a little brown, over all of it.
        return vec4<f32>(ember * k + vec3<f32>(0.004, 0.003, 0.002) * burn * k, burn * 0.95 * k);
    }
    if (mark == FROST) {
        // Rime: feathers of it running out from the middle, glinting.
        let n = fbm3(vec3<f32>(q * 4.0, seed));
        let cover = 1.0 - smoothstep(0.4, 1.0, r + (n - 0.45) * 0.7);
        let a = atan2(q.y, q.x);
        let feather = noise3(vec3<f32>(a * 5.0, r * 3.0, seed));
        let light = g.sky.rgb + g.sun.rgb * max(g.sun_dir.y, 0.0) * 0.6;
        let glint = pow(hash3(floor(pos * 24.0) + seed), 40.0) * 6.0;
        let alpha = cover * (0.5 + 0.4 * feather) * k;
        return vec4<f32>((c * (0.7 + 0.3 * feather) + vec3<f32>(glint)) * light * alpha, alpha);
    }
    if (mark == RUNES) {
        // Two rings, and between them a band of glyphs: a stroke, a bar.
        let a = atan2(q.y, q.x);
        let ring = max(
            1.0 - smoothstep(0.0, 0.025, abs(r - 0.93)),
            1.0 - smoothstep(0.0, 0.02, abs(r - 0.7)));
        let u = (a / 6.2831853 + 0.5) * 18.0;
        let cell = floor(u);
        let fu = fract(u);
        let v = (r - 0.74) / 0.15;
        let hh = hash3(vec3<f32>(cell, seed, 1.0));
        let band = step(0.0, v) * step(v, 1.0);
        let stroke = (1.0 - smoothstep(0.05, 0.09, abs(fu - 0.5 + (hh - 0.5) * 0.4))) * step(0.25, fract(hh * 13.0));
        let bar = (1.0 - smoothstep(0.05, 0.1, abs(v - 0.15 - fract(hh * 7.0) * 0.7))) * step(0.2, fu) * step(fu, 0.8);
        let lit = max(ring, band * max(stroke, bar));
        return vec4<f32>(c * lit * 2.5 * g.wind.w * k * lie, 0.0);
    }
    // A ring of light at its edge.
    let e = (r - 0.88) / 0.08;
    return vec4<f32>(c * exp(-e * e) * 2.0 * g.wind.w * k * lie, 0.0);
}
"#;
