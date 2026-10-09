//! Shafts of sunlight (`shaders::shafts`): from the scene's depth once
//! what is solid is drawn, at a quarter of the screen's size; the finish
//! adds them, in the sun's colour, before tone mapping.

use gpu::wgpu;

use crate::post::texture;
use crate::{geo, laws, shaders, Camera, Look};

pub const SHAFT: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;

pub struct Shafts {
    layout: wgpu::BindGroupLayout,
    pipe: wgpu::RenderPipeline,
    buf: wgpu::Buffer,
    size: (u32, u32),
    pub view: Option<wgpu::TextureView>,
    group: Option<wgpu::BindGroup>,
}

impl Shafts {
    pub fn new(device: &wgpu::Device, msaa: u32) -> Shafts {
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty,
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shafts"),
            entries: &[
                entry(
                    0,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: msaa > 1,
                    },
                ),
                entry(
                    1,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                ),
            ],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shafts"),
            source: wgpu::ShaderSource::Wgsl(shaders::shafts::shafts(msaa > 1).into()),
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shafts"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shafts"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("shafts_vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("shafts_fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: SHAFT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Shafts {
            layout,
            pipe,
            buf: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("shafts"),
                size: 32,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            size: (0, 0),
            view: None,
            group: None,
        }
    }

    fn quarter(&self) -> (u32, u32) {
        (
            self.size.0.div_ceil(4).max(1),
            self.size.1.div_ceil(4).max(1),
        )
    }

    /// Made again with the scene's depth (`size` pixels).
    pub fn fit(&mut self, device: &wgpu::Device, depth: &wgpu::TextureView, size: (u32, u32)) {
        self.size = size;
        let view = texture(device, "shafts", self.quarter(), SHAFT, 1, true);
        self.group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shafts"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(depth),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.buf.as_entire_binding(),
                },
            ],
        }));
        self.view = Some(view);
    }

    /// The sun's shafts as `cam` sees them under `look`'s sun: how strong
    /// (0 when it is behind the eye, down or off far from the screen).
    pub fn run(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        cam: &Camera,
        look: &Look,
    ) {
        let (Some(view), Some(group)) = (&self.view, &self.group) else {
            return;
        };
        let sun = geo::norm(look.sun_dir);
        let (vp, _, _) = cam.matrices(cam.fov);
        let p = geo::add(cam.eye, sun);
        let clip = [
            vp[0] * p[0] + vp[4] * p[1] + vp[8] * p[2] + vp[12],
            vp[1] * p[0] + vp[5] * p[1] + vp[9] * p[2] + vp[13],
            vp[3] * p[0] + vp[7] * p[1] + vp[11] * p[2] + vp[15],
        ];
        let (uv, k) = if clip[2] > 0.01 && sun[1] > 0.0 {
            let ndc = [clip[0] / clip[2], clip[1] / clip[2]];
            let uv = [ndc[0] * 0.5 + 0.5, 0.5 - ndc[1] * 0.5];
            // Fading as the sun leaves the screen, and as it sets.
            let off = (uv[0] - 0.5).abs().max((uv[1] - 0.5).abs());
            let k = (1.0 - (off - 0.5) / 0.6).clamp(0.0, 1.0) * (sun[1] / 0.08).min(1.0);
            (uv, k * laws::SHAFTS)
        } else {
            ([0.5, 0.5], 0.0)
        };
        let q = self.quarter();
        let u = [
            uv[0],
            uv[1],
            k,
            laws::SHAFT_GLOW,
            self.size.0 as f32,
            self.size.1 as f32,
            q.0 as f32,
            q.1 as f32,
        ];
        let bytes: Vec<u8> = u.iter().flat_map(|f| f.to_le_bytes()).collect();
        queue.write_buffer(&self.buf, 0, &bytes);
        let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("shafts"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        rp.set_pipeline(&self.pipe);
        rp.set_bind_group(0, group, &[]);
        rp.draw(0..3, 0..1);
    }
}
