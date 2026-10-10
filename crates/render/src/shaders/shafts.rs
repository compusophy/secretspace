//! Shafts of sunlight (god rays), at a quarter of the screen's size:
//! from each pixel toward the sun on screen, a march over the sky near the
//! sun (where nothing stands: the depth is clear), each step a little
//! dimmer; what stands between breaks the light into shafts.

const SHAFTS: &str = r#"
struct Shafts {
    sun: vec4<f32>,    // xy the sun on screen (0..1), z how strong, w its glow's size
    size: vec4<f32>,   // xy the screen, zw this target (pixels)
};

@group(0) @binding(0) var depth: DEPTH_TYPE;
@group(0) @binding(1) var<uniform> sh: Shafts;

struct Out {
    @builtin(position) clip: vec4<f32>,
};

@vertex
fn shafts_vs(@builtin(vertex_index) i: u32) -> Out {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
    var o: Out;
    o.clip = vec4<f32>(p, 0.0, 1.0);
    return o;
}

/// How much sun shines from the sky at `uv`: open sky, near the sun.
fn shine(uv: vec2<f32>) -> f32 {
    if (uv.x < 0.0 || uv.y < 0.0 || uv.x >= 1.0 || uv.y >= 1.0) {
        return 0.0;
    }
    let px = vec2<i32>(uv * sh.size.xy);
    if (textureLoad(depth, px, 0) > 0.0) {
        return 0.0;
    }
    let aspect = sh.size.x / sh.size.y;
    let off = (uv - sh.sun.xy) * vec2<f32>(aspect, 1.0);
    let k = max(1.0 - length(off) / sh.sun.w, 0.0);
    return k * k * k;
}

@fragment
fn shafts_fs(i: Out) -> @location(0) vec4<f32> {
    let uv = i.clip.xy / sh.size.zw;
    let steps = SHAFT_STEPS;
    let toward = (sh.sun.xy - uv) / f32(steps);
    // A turn of the starting point, pixel by pixel, so the steps do not
    // band (the finish blurs it as it scales it up).
    let jitter = ign(i.clip.xy);
    var p = uv + toward * jitter;
    var fade = 1.0;
    var sum = 0.0;
    for (var k = 0; k < steps; k = k + 1) {
        sum = sum + shine(p) * fade;
        fade = fade * SHAFT_DECAY;
        p = p + toward;
    }
    return vec4<f32>(sum / f32(steps) * sh.sun.z, 0.0, 0.0, 1.0);
}
"#;

/// The shafts' module, for a depth that is many-sampled (`msaa`) or not.
pub fn shafts(msaa: bool) -> String {
    format!("{SHAFTS}{}", super::IGN)
        .replace("DEPTH_TYPE", super::depth_type(msaa))
        .replace("SHAFT_STEPS", &format!("{}", crate::laws::SHAFT_STEPS))
        .replace("SHAFT_DECAY", &format!("{:.4}", crate::laws::SHAFT_DECAY))
}
