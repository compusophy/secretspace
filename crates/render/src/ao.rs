//! Ambient occlusion (`shaders::ao`): from the scene's depth once what is
//! solid is drawn, at half the screen's size, blurred, then multiplied
//! into the picture before what is see-through, what glows and the
//! viewmodel are drawn over it.

use gpu::wgpu;

use crate::post::{texture, HDR};
use crate::{laws, shaders, Camera};

const AO: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;

pub struct Ao {
    ways: u32,
    layout: wgpu::BindGroupLayout,
    ao: wgpu::RenderPipeline,
    blur: wgpu::RenderPipeline,
    apply: wgpu::RenderPipeline,
    buf: wgpu::Buffer,
    size: (u32, u32),
    /// The raw occlusion, the blurred, and the groups reading each.
    views: Option<[wgpu::TextureView; 2]>,
    groups: Option<[wgpu::BindGroup; 2]>,
}

impl Ao {
    /// Occlusion looking `ways` ways out from each pixel, over a picture
    /// of `msaa` samples.
    pub fn new(device: &wgpu::Device, msaa: u32, ways: u32) -> Ao {
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty,
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ao"),
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
                entry(
                    2,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                ),
            ],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ao"),
            source: wgpu::ShaderSource::Wgsl(shaders::ao::ao(msaa > 1).into()),
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ao"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipe = |entry: &str, format, count, blend| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(&pl),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("ao_vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count,
                    ..Default::default()
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        // The picture times the occlusion; its alpha kept.
        let times = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Zero,
                dst_factor: wgpu::BlendFactor::Src,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Zero,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
        };
        Ao {
            ways,
            ao: pipe("ao_fs", AO, 1, None),
            blur: pipe("blur_fs", AO, 1, None),
            apply: pipe("apply_fs", HDR, msaa, Some(times)),
            layout,
            buf: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("ao"),
                size: 64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            size: (0, 0),
            views: None,
            groups: None,
        }
    }

    fn half(&self) -> (u32, u32) {
        (
            self.size.0.div_ceil(2).max(1),
            self.size.1.div_ceil(2).max(1),
        )
    }

    /// Made again with the scene's depth (`size` pixels).
    pub fn fit(&mut self, device: &wgpu::Device, depth: &wgpu::TextureView, size: (u32, u32)) {
        self.size = size;
        let half = self.half();
        let views = [
            texture(device, "ao", half, AO, 1, true),
            texture(device, "ao (soft)", half, AO, 1, true),
        ];
        let group = |src: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ao"),
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
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(src),
                    },
                ],
            })
        };
        // The raw is made reading the soft (unused), the soft from the
        // raw, and the picture from the soft.
        self.groups = Some([group(&views[1]), group(&views[0])]);
        self.views = Some(views);
    }

    /// Occlude the picture (`color`) as `cam` sees it.
    pub fn run(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        color: &wgpu::TextureView,
        cam: &Camera,
    ) {
        let (Some(views), Some(groups)) = (&self.views, &self.groups) else {
            return;
        };
        let f = 1.0 / (cam.fov / 2.0).tan();
        let half = self.half();
        let k = [
            f / cam.aspect,
            f,
            laws::NEAR,
            laws::AO_REACH,
            self.size.0 as f32,
            self.size.1 as f32,
            half.0 as f32,
            half.1 as f32,
            laws::AO_POWER,
            laws::AO_SLACK,
            self.ways as f32,
            laws::AO_FADE,
            laws::AO_STRENGTH,
            0.0,
            0.0,
            0.0,
        ];
        let bytes: Vec<u8> = k.iter().flat_map(|f| f.to_le_bytes()).collect();
        queue.write_buffer(&self.buf, 0, &bytes);
        let steps = [
            (
                &views[0],
                &self.ao,
                &groups[0],
                wgpu::LoadOp::Clear(wgpu::Color::WHITE),
            ),
            (
                &views[1],
                &self.blur,
                &groups[1],
                wgpu::LoadOp::Clear(wgpu::Color::WHITE),
            ),
            (color, &self.apply, &groups[0], wgpu::LoadOp::Load),
        ];
        for (view, pipe, group, load) in steps {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ao"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_pipeline(pipe);
            rp.set_bind_group(0, group, &[]);
            rp.draw(0..3, 0..1);
        }
    }
}
