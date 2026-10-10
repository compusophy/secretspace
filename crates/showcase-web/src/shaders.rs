//! The showcase's own shaders: the turning triangle drawn where there is
//! no WebGPU (GLSL, for WebGL2; on WebGPU the engine draws its scene).
//! `u` is (angle, width / height, 0, 0).

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
