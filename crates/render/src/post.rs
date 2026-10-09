//! After the scene: its targets (an HDR picture, multisampled and
//! resolved, and its depth), ambient occlusion (`ao`), bloom (halve it,
//! again and again, then back up, each step a little wider, added
//! together), and the finish into the screen (exposure, tone mapping, a
//! vignette, sRGB, the grade).

use gpu::wgpu;

use crate::ao::Ao;
use crate::shafts::{Shafts, SHAFT};
use crate::{shaders, Camera, Look};

pub const HDR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
pub const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Where the scene draws: the colour (multisampled, or the picture
/// itself), what it resolves into, and its depth.
pub struct Targets {
    pub color: wgpu::TextureView,
    pub resolve: Option<wgpu::TextureView>,
    pub depth: wgpu::TextureView,
}

impl Targets {
    /// A pass drawing the scene: its colour cleared or kept, its depth
    /// cleared (to infinity) or kept, and kept after it or not, the
    /// picture resolved at its end or not.
    pub fn pass<'a>(
        &self,
        encoder: &'a mut wgpu::CommandEncoder,
        label: &str,
        clear: Option<[f32; 3]>,
        depth: (bool, bool),
        resolve: bool,
    ) -> wgpu::RenderPass<'a> {
        let load = match clear {
            Some(c) => wgpu::LoadOp::Clear(wgpu::Color {
                r: c[0] as f64,
                g: c[1] as f64,
                b: c[2] as f64,
                a: 1.0,
            }),
            None => wgpu::LoadOp::Load,
        };
        let (clear_depth, keep_depth) = depth;
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(label),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.color,
                depth_slice: None,
                resolve_target: if resolve { self.resolve.as_ref() } else { None },
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth,
                depth_ops: Some(wgpu::Operations {
                    load: if clear_depth {
                        wgpu::LoadOp::Clear(0.0)
                    } else {
                        wgpu::LoadOp::Load
                    },
                    store: if keep_depth {
                        wgpu::StoreOp::Store
                    } else {
                        wgpu::StoreOp::Discard
                    },
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        })
    }
}

struct Pass {
    group: wgpu::BindGroup,
    target: usize,
    up: bool,
}

pub struct Post {
    msaa: u32,
    levels: u32,
    layout: wgpu::BindGroupLayout,
    finish_layout: wgpu::BindGroupLayout,
    down: wgpu::RenderPipeline,
    up: wgpu::RenderPipeline,
    finish: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    size: (u32, u32),
    pub targets: Option<Targets>,
    chain: Vec<wgpu::TextureView>,
    passes: Vec<Pass>,
    finish_group: Option<wgpu::BindGroup>,
    finish_buf: wgpu::Buffer,
    grade_buf: wgpu::Buffer,
    ao: Option<Ao>,
    shafts: Option<Shafts>,
    /// No shafts: a texel of nothing for the finish.
    dark: wgpu::TextureView,
}

pub(crate) fn texture(
    device: &wgpu::Device,
    label: &str,
    size: (u32, u32),
    format: wgpu::TextureFormat,
    samples: u32,
    sampled: bool,
) -> wgpu::TextureView {
    let mut usage = wgpu::TextureUsages::RENDER_ATTACHMENT;
    if sampled {
        usage |= wgpu::TextureUsages::TEXTURE_BINDING;
    }
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: size.0.max(1),
                height: size.1.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: samples,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

fn uniform(device: &wgpu::Device, queue: &wgpu::Queue, v: [f32; 8]) -> wgpu::Buffer {
    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("post"),
        size: 32,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bytes: Vec<u8> = v.iter().flat_map(|f| f.to_le_bytes()).collect();
    queue.write_buffer(&buf, 0, &bytes);
    buf
}

impl Post {
    /// Post-processing into targets of `out` format, with occlusion
    /// looking `ao` ways out from a pixel (none at 0).
    pub fn new(
        device: &wgpu::Device,
        out: wgpu::TextureFormat,
        msaa: u32,
        levels: u32,
        (ao, shafts): (u32, bool),
    ) -> Post {
        let tex = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let common = [
            tex(0),
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ];
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("post"),
            entries: &common,
        });
        let mut with_bloom = common.to_vec();
        with_bloom.push(tex(3));
        with_bloom.push(wgpu::BindGroupLayoutEntry {
            binding: 4,
            ..common[2]
        });
        with_bloom.push(tex(5));
        let finish_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("finish"),
            entries: &with_bloom,
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("post"),
            source: wgpu::ShaderSource::Wgsl(shaders::post().into()),
        });
        let pipe = |layout: &wgpu::BindGroupLayout, entry: &str, format, blend| {
            let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(entry),
                bind_group_layouts: &[Some(layout)],
                immediate_size: 0,
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(&pl),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("post_vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
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
        let add = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::OVER,
        };
        Post {
            msaa,
            levels: levels.max(1),
            down: pipe(&layout, "down_fs", HDR, None),
            up: pipe(&layout, "up_fs", HDR, Some(add)),
            finish: pipe(&finish_layout, "finish_fs", out, None),
            layout,
            finish_layout,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("post"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            size: (0, 0),
            targets: None,
            chain: Vec::new(),
            passes: Vec::new(),
            finish_group: None,
            finish_buf: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("finish"),
                size: 32,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            grade_buf: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("grade"),
                size: 48,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            ao: (ao > 0).then(|| Ao::new(device, msaa, ao)),
            shafts: shafts.then(|| Shafts::new(device, msaa)),
            dark: texture(device, "dark", (1, 1), SHAFT, 1, true),
        }
    }

    /// Whether the scene's depth is read (`occlude`) once what is solid
    /// is drawn, before what glows.
    pub fn occludes(&self) -> bool {
        self.ao.is_some() || self.shafts.is_some()
    }

    /// Ambient occlusion over the picture drawn so far, and the sun's
    /// shafts from its depth.
    pub fn occlude(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        cam: &Camera,
        look: &Look,
    ) {
        if let (Some(ao), Some(t)) = (&self.ao, &self.targets) {
            ao.run(encoder, queue, &t.color, cam);
        }
        if let Some(sh) = &self.shafts {
            sh.run(encoder, queue, cam, look);
        }
    }

    /// Targets the size of the screen (made again when it changes).
    pub fn fit(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, size: (u32, u32)) {
        if self.size == size && self.targets.is_some() {
            return;
        }
        self.size = size;
        let hdr = texture(device, "hdr", size, HDR, 1, true);
        let read = self.occludes();
        let depth = texture(device, "depth", size, DEPTH, self.msaa, read);
        if let Some(ao) = &mut self.ao {
            ao.fit(device, &depth, size);
        }
        if let Some(sh) = &mut self.shafts {
            sh.fit(device, &depth, size);
        }
        let targets = if self.msaa > 1 {
            Targets {
                color: texture(device, "hdr (msaa)", size, HDR, self.msaa, false),
                resolve: Some(hdr.clone()),
                depth,
            }
        } else {
            Targets {
                color: hdr.clone(),
                resolve: None,
                depth,
            }
        };
        let sizes: Vec<(u32, u32)> = (1..=self.levels)
            .map(|l| ((size.0 >> l).max(1), (size.1 >> l).max(1)))
            .collect();
        self.chain = sizes
            .iter()
            .map(|&s| texture(device, "bloom", s, HDR, 1, true))
            .collect();
        let group = |src: &wgpu::TextureView, buf: &wgpu::Buffer| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("post"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(src),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: buf.as_entire_binding(),
                    },
                ],
            })
        };
        let mut passes = Vec::new();
        for l in 0..self.levels as usize {
            let (src, s) = if l == 0 {
                (&hdr, size)
            } else {
                (&self.chain[l - 1], sizes[l - 1])
            };
            let buf = uniform(
                device,
                queue,
                [
                    1.0 / s.0 as f32,
                    1.0 / s.1 as f32,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    1.0,
                ],
            );
            passes.push(Pass {
                group: group(src, &buf),
                target: l,
                up: false,
            });
        }
        for l in (0..self.levels as usize - 1).rev() {
            let s = sizes[l + 1];
            let buf = uniform(
                device,
                queue,
                [
                    1.0 / s.0 as f32,
                    1.0 / s.1 as f32,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    1.0,
                ],
            );
            passes.push(Pass {
                group: group(&self.chain[l + 1], &buf),
                target: l,
                up: true,
            });
        }
        self.passes = passes;
        self.finish_group = Some(
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("finish"),
                layout: &self.finish_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&hdr),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.finish_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&self.chain[0]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: self.grade_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::TextureView(
                            self.shafts
                                .as_ref()
                                .and_then(|s| s.view.as_ref())
                                .unwrap_or(&self.dark),
                        ),
                    },
                ],
            }),
        );
        self.targets = Some(targets);
    }

    /// Bloom, then the finish into `out`.
    pub fn run(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        out: &wgpu::TextureView,
        look: &Look,
    ) {
        let sun = if self.shafts.is_some() {
            look.sun
        } else {
            [0.0; 3]
        };
        let k = [
            sun[0],
            sun[1],
            sun[2],
            0.0,
            look.bloom,
            look.exposure,
            look.vignette,
            1.0,
        ];
        let bytes: Vec<u8> = k.iter().flat_map(|f| f.to_le_bytes()).collect();
        queue.write_buffer(&self.finish_buf, 0, &bytes);
        let g = &look.grade;
        let grade = [
            g.lift[0],
            g.lift[1],
            g.lift[2],
            g.saturation,
            g.gamma[0],
            g.gamma[1],
            g.gamma[2],
            g.contrast,
            g.gain[0],
            g.gain[1],
            g.gain[2],
            0.0,
        ];
        let bytes: Vec<u8> = grade.iter().flat_map(|f| f.to_le_bytes()).collect();
        queue.write_buffer(&self.grade_buf, 0, &bytes);
        for p in &self.passes {
            let load = if p.up {
                wgpu::LoadOp::Load
            } else {
                wgpu::LoadOp::Clear(wgpu::Color::BLACK)
            };
            let mut rp = begin(encoder, &self.chain[p.target], load);
            rp.set_pipeline(if p.up { &self.up } else { &self.down });
            rp.set_bind_group(0, &p.group, &[]);
            rp.draw(0..3, 0..1);
        }
        if let Some(group) = &self.finish_group {
            let mut rp = begin(encoder, out, wgpu::LoadOp::Clear(wgpu::Color::BLACK));
            rp.set_pipeline(&self.finish);
            rp.set_bind_group(0, group, &[]);
            rp.draw(0..3, 0..1);
        }
    }
}

/// A pass drawing into one view.
fn begin<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    load: wgpu::LoadOp<wgpu::Color>,
) -> wgpu::RenderPass<'a> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("post"),
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
    })
}
