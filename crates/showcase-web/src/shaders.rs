//! The showcase's shaders: the same turning triangle in WGSL (WebGPU) and
//! GLSL (the WebGL2 fallback). `u` is (angle, width / height, 0, 0).

pub const TRIANGLE_WGSL: &str = r#"
struct U { t: vec4<f32> };
@group(0) @binding(0) var<uniform> u: U;

struct V {
    @builtin(position) p: vec4<f32>,
    @location(0) c: vec3<f32>,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> V {
    var pos = array<vec2<f32>, 3>(vec2<f32>(0.0, 0.6), vec2<f32>(-0.52, -0.3), vec2<f32>(0.52, -0.3));
    var col = array<vec3<f32>, 3>(vec3<f32>(1.0, 0.82, 0.48), vec3<f32>(0.4, 0.6, 1.0), vec3<f32>(0.9, 0.3, 0.7));
    let a = u.t.x;
    let q = pos[i];
    let r = vec2<f32>(q.x * cos(a) - q.y * sin(a), q.x * sin(a) + q.y * cos(a));
    let k = min(1.0, u.t.y);
    var o: V;
    o.p = vec4<f32>(r.x * k / u.t.y, r.y * k, 0.0, 1.0);
    o.c = col[i];
    return o;
}

@fragment
fn fs(v: V) -> @location(0) vec4<f32> {
    return vec4<f32>(v.c, 1.0);
}
"#;

pub const TRIANGLE_VS: &str = r#"#version 300 es
uniform vec4 u;
out vec3 c;
void main() {
    vec2 pos[3] = vec2[3](vec2(0.0, 0.6), vec2(-0.52, -0.3), vec2(0.52, -0.3));
    vec3 col[3] = vec3[3](vec3(1.0, 0.82, 0.48), vec3(0.4, 0.6, 1.0), vec3(0.9, 0.3, 0.7));
    float a = u.x;
    vec2 q = pos[gl_VertexID];
    vec2 r = vec2(q.x * cos(a) - q.y * sin(a), q.x * sin(a) + q.y * cos(a));
    float k = min(1.0, u.y);
    gl_Position = vec4(r.x * k / u.y, r.y * k, 0.0, 1.0);
    c = col[gl_VertexID];
}
"#;

pub const TRIANGLE_FS: &str = r#"#version 300 es
precision mediump float;
in vec3 c;
out vec4 o;
void main() {
    o = vec4(c, 1.0);
}
"#;
