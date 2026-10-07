//! The renderer's GLSL: the world (lit by the moon, the sky, and the
//! nearest sixteen lights, fogged into the Dark; negative in the
//! Underlight), the sky (a gradient with stars, the Dark below), and
//! sparks (round points, added as light).

pub const WORLD_VS: &str = "#version 300 es
layout(location=0) in vec3 a_pos;
layout(location=1) in vec3 a_nrm;
layout(location=2) in vec4 a_col;
uniform mat4 u_vp;
uniform mat4 u_model;
out vec3 v_pos;
out vec3 v_nrm;
out vec4 v_col;
void main() {
  vec4 w = u_model * vec4(a_pos, 1.0);
  v_pos = w.xyz;
  v_nrm = mat3(u_model) * a_nrm;
  v_col = a_col;
  gl_Position = u_vp * w;
}";

pub const WORLD_FS: &str = "#version 300 es
precision highp float;
in vec3 v_pos;
in vec3 v_nrm;
in vec4 v_col;
uniform vec3 u_eye;
uniform vec4 u_tint;
uniform float u_glow;
uniform vec3 u_fog;
uniform vec2 u_fogr;
uniform vec3 u_sky;
uniform vec3 u_low;
uniform vec3 u_moon_dir;
uniform vec3 u_moon;
uniform int u_n;
uniform vec4 u_lp[16];
uniform vec3 u_lc[16];
uniform float u_under;
out vec4 o;
void main() {
  vec3 n = normalize(v_nrm);
  vec3 base = v_col.rgb * u_tint.rgb;
  float glow = clamp(v_col.a + u_glow, 0.0, 1.0);
  vec3 light = mix(u_low, u_sky, n.y * 0.5 + 0.5) + u_moon * max(dot(n, u_moon_dir), 0.0);
  for (int i = 0; i < 16; i++) {
    if (i >= u_n) break;
    vec3 d = u_lp[i].xyz - v_pos;
    float dist = length(d);
    float k = clamp(1.0 - dist / u_lp[i].w, 0.0, 1.0);
    light += u_lc[i] * (k * k) * (0.3 + 0.7 * max(dot(n, d / max(dist, 0.001)), 0.0));
  }
  vec3 c = mix(base * light, base * 1.3, glow);
  float f = smoothstep(u_fogr.x, u_fogr.y, length(v_pos - u_eye));
  c = mix(c, u_fog, f * (1.0 - glow * 0.5));
  if (u_under > 0.5) {
    c = vec3(1.0) - c;
    c = mix(c, vec3(dot(c, vec3(0.3, 0.5, 0.2))), 0.5) * vec3(0.8, 0.92, 1.05);
  }
  o = vec4(c, u_tint.a);
}";

pub const SKY_VS: &str = "#version 300 es
layout(location=0) in vec2 a_pos;
out vec2 v_ndc;
void main() { v_ndc = a_pos; gl_Position = vec4(a_pos, 0.999, 1.0); }";

pub const SKY_FS: &str = "#version 300 es
precision highp float;
in vec2 v_ndc;
uniform vec3 u_fwd;
uniform vec3 u_right;
uniform vec3 u_up;
uniform vec2 u_tan;
uniform vec3 u_fog;
uniform vec3 u_zenith;
uniform vec3 u_deep;
uniform float u_time;
uniform float u_under;
out vec4 o;
float hash(vec3 q) { return fract(sin(dot(q, vec3(12.9898, 78.233, 37.719))) * 43758.5453); }
void main() {
  vec3 d = normalize(u_fwd + v_ndc.x * u_tan.x * u_right + v_ndc.y * u_tan.y * u_up);
  vec3 c = d.y >= 0.0
    ? mix(u_fog, u_zenith, smoothstep(0.0, 0.7, d.y))
    : mix(u_fog, u_deep, smoothstep(0.0, -0.45, d.y));
  // Far below, the Underlight's faint violet.
  c += vec3(0.07, 0.03, 0.13) * smoothstep(-0.35, -1.0, d.y);
  vec3 q = floor(d * 220.0);
  float s = hash(q);
  if (s > 0.9965) {
    float tw = 0.65 + 0.35 * sin(u_time * 0.002 + s * 500.0);
    float k = (s - 0.9965) / 0.0035 * tw * (d.y > 0.0 ? 1.0 : 0.35);
    c += vec3(0.95, 0.9, 0.8) * k * smoothstep(-0.02, 0.15, abs(d.y));
  }
  if (u_under > 0.5) { c = vec3(1.0) - c; c *= vec3(0.8, 0.92, 1.05); }
  o = vec4(c, 1.0);
}";

pub const POINTS_VS: &str = "#version 300 es
layout(location=0) in vec3 a_pos;
layout(location=1) in vec4 a_col;
layout(location=2) in float a_size;
uniform mat4 u_vp;
uniform float u_px;
out vec4 v_col;
void main() {
  gl_Position = u_vp * vec4(a_pos, 1.0);
  gl_PointSize = clamp(a_size * u_px / max(gl_Position.w, 0.05), 1.0, 48.0);
  v_col = a_col;
}";

pub const POINTS_FS: &str = "#version 300 es
precision mediump float;
in vec4 v_col;
out vec4 o;
void main() {
  float d = length(gl_PointCoord - vec2(0.5));
  float a = (1.0 - smoothstep(0.25, 0.5, d)) * v_col.a;
  o = vec4(v_col.rgb * a, 0.0);
}";
