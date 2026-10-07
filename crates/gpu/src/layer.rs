//! The pixel layer composited over the picture: the `pixels::Canvas`
//! uploaded as a texture and drawn by one fullscreen triangle, each layer
//! pixel a sharp square of `k` device pixels, premultiplied over what is
//! there.

/// The composite's WGSL: a fullscreen triangle; each fragment loads the
/// layer texel it falls in (no filtering, so pixels stay sharp).
pub const WGSL: &str = r#"
struct U { k: vec4<f32> };
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var layer: texture_2d<f32>;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    let d = vec2<i32>(textureDimensions(layer));
    let t = min(vec2<i32>(floor(p.xy / u.k.x)), d - vec2<i32>(1));
    return textureLoad(layer, t, 0);
}
"#;

pub struct Layer {
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    texture: Option<(wgpu::Texture, wgpu::BindGroup, (u32, u32))>,
}

impl Layer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Layer {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("layer"),
            source: wgpu::ShaderSource::Wgsl(WGSL.into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("layer"),
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
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("layer"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Layer {
            pipeline,
            uniform,
            texture: None,
        }
    }

    /// Upload the layer and draw it over `view`, `k` device pixels a texel.
    pub fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        hud: &pixels::Canvas,
        k: f32,
    ) {
        let size = (hud.w as u32, hud.h as u32);
        if self.texture.as_ref().map(|t| t.2) != Some(size) {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("layer"),
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
            let tv = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("layer"),
                layout: &self.pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.uniform.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&tv),
                    },
                ],
            });
            self.texture = Some((texture, group, size));
        }
        let Some((texture, group, _)) = &self.texture else {
            return;
        };
        queue.write_buffer(&self.uniform, 0, &bytes([k.max(1e-3), 0.0, 0.0, 0.0]));
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &hud.data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size.0 * 4),
                rows_per_image: Some(size.1),
            },
            wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
        );
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("layer"),
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
        pass.set_bind_group(0, group, &[]);
        pass.draw(0..3, 0..1);
    }
}

/// Floats as the bytes a uniform buffer wants (little-endian, as WebGPU is).
pub fn bytes<const N: usize>(v: [f32; N]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}
