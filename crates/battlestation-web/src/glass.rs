//! The monitor's picture on its glass: the screen's pixels uploaded as a
//! texture and drawn, smoothly filtered and in perspective, on the four
//! corners of the monitor's face, after the engine has drawn the room
//! (the engine draws a lit pane there, for the light and the bloom).
//! Nothing stands between your eyes and the screen, so no depth is
//! needed.

use crate::gear::on_screen;
use battlestation::laws::{SCREEN_H, SCREEN_W};
use render::{wgpu, M4};

/// Corners in, perspective out; the picture with a faint sheen, and the
/// panel's black a little lit.
pub const WGSL: &str = r#"
struct U {
    vp: mat4x4<f32>,
    a: vec4<f32>,
    b: vec4<f32>,
    c: vec4<f32>,
    d: vec4<f32>,
    k: vec4<f32>,
};
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var pic: texture_2d<f32>;
@group(0) @binding(2) var smp: sampler;

struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> Out {
    var corner = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);
    let k = corner[i];
    var p = u.d.xyz;
    var uv = vec2<f32>(0.0, 0.0);
    if (k == 0u) {
        p = u.a.xyz;
        uv = vec2<f32>(0.0, 1.0);
    } else if (k == 1u) {
        p = u.b.xyz;
        uv = vec2<f32>(1.0, 1.0);
    } else if (k == 2u) {
        p = u.c.xyz;
        uv = vec2<f32>(1.0, 0.0);
    }
    var o: Out;
    o.pos = u.vp * vec4<f32>(p, 1.0);
    o.uv = uv;
    return o;
}

@fragment
fn fs(i: Out) -> @location(0) vec4<f32> {
    let c = textureSample(pic, smp, i.uv).rgb;
    let edge = min(min(i.uv.x, 1.0 - i.uv.x), min(i.uv.y, 1.0 - i.uv.y));
    let rim = 0.88 + 0.12 * smoothstep(0.0, 0.02, edge);
    let sheen = 0.03 * (1.0 - i.uv.y) * (1.0 - 0.7 * i.uv.x);
    let black = vec3<f32>(0.011, 0.012, 0.017);
    return vec4<f32>(black + c * u.k.x * rim + vec3<f32>(sheen), 1.0);
}
"#;

/// The picture's corners in the world: bottom left, bottom right, top
/// right, top left.
pub fn corners() -> [[f32; 3]; 4] {
    let (w, h, out) = (SCREEN_W / 2.0, SCREEN_H / 2.0, 0.0007);
    [
        on_screen([-w, -h, out]),
        on_screen([w, -h, out]),
        on_screen([w, h, out]),
        on_screen([-w, h, out]),
    ]
}

pub struct Glass {
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    texture: wgpu::Texture,
    group: wgpu::BindGroup,
    size: (u32, u32),
}

impl Glass {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, size: (u32, u32)) -> Glass {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("glass"),
            source: wgpu::ShaderSource::Wgsl(WGSL.into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("glass"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("glass"),
            size: 16 * 4 + 5 * 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glass"),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("glass"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("glass"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        Glass {
            pipeline,
            uniform,
            texture,
            group,
            size,
        }
    }

    /// Upload the picture (RGBA, the size it was made for) and draw it
    /// over `view` as the camera `vp` sees it, `bright` as bright.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        vp: &M4,
        picture: &[u8],
        bright: f32,
    ) {
        let (w, h) = self.size;
        if picture.len() == (w * h * 4) as usize {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                picture,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(w * 4),
                    rows_per_image: Some(h),
                },
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );
        }
        let mut u: Vec<f32> = vp.to_vec();
        for c in corners() {
            u.extend_from_slice(&[c[0], c[1], c[2], 1.0]);
        }
        u.extend_from_slice(&[bright, 0.0, 0.0, 0.0]);
        let bytes: Vec<u8> = u.iter().flat_map(|f| f.to_le_bytes()).collect();
        queue.write_buffer(&self.uniform, 0, &bytes);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("glass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.group, &[]);
        pass.draw(0..6, 0..1);
    }
}
