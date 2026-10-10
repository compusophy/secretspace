//! The scene's pipelines (`Pipes`, made for a tier: its samples, its
//! sea's reflections), what they share (how one is made), and the layouts
//! of their bind groups: the scene's own; and what the pass after the
//! solid one reads (its depth, and a copy of the picture so far).

use crate::buffers::{INST, SPARK};
use crate::decals::Decals;
use crate::post::{DEPTH, HDR};
use crate::{geo, shaders, Quality};
use gpu::wgpu;

/// Every pipeline the scene draws with: the world's by pass (solid,
/// see-through, glowing), the sky, sparks, grass; and those of the pass
/// that reads the depth (soft sparks, the sea mirroring, decals) and the
/// layout of the group they read it by.
pub(crate) struct Pipes {
    pub opaque: wgpu::RenderPipeline,
    pub faint: wgpu::RenderPipeline,
    pub glow: wgpu::RenderPipeline,
    pub sky: wgpu::RenderPipeline,
    pub sparks: wgpu::RenderPipeline,
    pub grass: wgpu::RenderPipeline,
    /// A blade's two triangles and its tip, over its five corners.
    pub blade: wgpu::Buffer,
    pub soft_sparks: wgpu::RenderPipeline,
    pub faint_ssr: wgpu::RenderPipeline,
    pub decals: Decals,
    pub soft_layout: wgpu::BindGroupLayout,
}

impl Pipes {
    /// The pipelines for `quality`, reading the scene's group (`layout`).
    pub fn new(device: &wgpu::Device, quality: Quality, layout: &wgpu::BindGroupLayout) -> Pipes {
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("engine"),
            bind_group_layouts: &[Some(layout)],
            immediate_size: 0,
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene"),
            source: wgpu::ShaderSource::Wgsl(shaders::scene(quality.msaa > 1, quality.ssr).into()),
        });
        let vertex = wgpu::VertexBufferLayout {
            array_stride: (geo::STRIDE * 4) as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4],
        };
        let instance = wgpu::VertexBufferLayout {
            array_stride: (INST * 4) as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![
                3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4,
                7 => Float32x4, 8 => Float32x4
            ],
        };
        let spark = wgpu::VertexBufferLayout {
            array_stride: (SPARK * 4) as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4],
        };
        use wgpu::CompareFunction::GreaterEqual;
        let world = [Some(vertex), Some(instance)];
        let add = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::OVER,
        };
        let shared = Shared {
            device,
            layout: &pl,
            module: &module,
            samples: quality.msaa,
        };
        let world_kind = |blend, write, cull| Kind {
            entry: ("world_vs", "world_fs"),
            buffers: &world,
            blend,
            depth: (write, GreaterEqual),
            cull,
        };
        // Sparks that fade into what stands behind them read the depth (a
        // second group), in the pass that only reads it.
        let soft_layout = soft_layout(device, quality.msaa > 1);
        let soft_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("soft"),
            bind_group_layouts: &[Some(layout), Some(&soft_layout)],
            immediate_size: 0,
        });
        let soft = Shared {
            layout: &soft_pl,
            ..shared
        };
        Pipes {
            opaque: pipeline(&shared, world_kind(None, true, Some(wgpu::Face::Back))),
            faint: pipeline(
                &shared,
                world_kind(Some(wgpu::BlendState::ALPHA_BLENDING), false, None),
            ),
            glow: pipeline(&shared, world_kind(Some(add), false, None)),
            // Drawn after what is solid, at the far end of the depth:
            // only where nothing stands (the depth still cleared).
            sky: pipeline(
                &shared,
                Kind {
                    entry: ("sky_vs", "sky_fs"),
                    buffers: &[],
                    blend: None,
                    depth: (false, GreaterEqual),
                    cull: None,
                },
            ),
            sparks: pipeline(
                &shared,
                Kind {
                    entry: ("spark_vs", "spark_fs"),
                    buffers: &[Some(spark.clone())],
                    // Premultiplied: light (alpha 0) adds, smoke lays over.
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    depth: (false, GreaterEqual),
                    cull: None,
                },
            ),
            grass: pipeline(
                &shared,
                Kind {
                    entry: ("grass_vs", "grass_fs"),
                    buffers: &[],
                    blend: None,
                    depth: (true, GreaterEqual),
                    cull: None,
                },
            ),
            blade: {
                let buf = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("blade"),
                    size: 32,
                    usage: wgpu::BufferUsages::INDEX,
                    mapped_at_creation: true,
                });
                let corners: [u16; 16] = [0, 1, 2, 2, 1, 3, 2, 3, 4, 0, 0, 0, 0, 0, 0, 0];
                let bytes: Vec<u8> = corners.iter().flat_map(|c| c.to_le_bytes()).collect();
                if let Ok(mut view) = buf.slice(..).get_mapped_range_mut() {
                    view.copy_from_slice(&bytes);
                }
                buf.unmap();
                buf
            },
            soft_sparks: pipeline(
                &soft,
                Kind {
                    entry: ("spark_vs", "spark_soft_fs"),
                    buffers: &[Some(spark)],
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    depth: (false, GreaterEqual),
                    cull: None,
                },
            ),
            faint_ssr: pipeline(
                &soft,
                Kind {
                    entry: ("world_vs", "faint_fs"),
                    ..world_kind(Some(wgpu::BlendState::ALPHA_BLENDING), false, None)
                },
            ),
            decals: Decals::new(&soft),
            soft_layout,
        }
    }
}

/// What every scene pipeline shares.
#[derive(Clone, Copy)]
pub(crate) struct Shared<'a> {
    pub device: &'a wgpu::Device,
    pub layout: &'a wgpu::PipelineLayout,
    pub module: &'a wgpu::ShaderModule,
    pub samples: u32,
}

pub(crate) struct Kind<'a> {
    pub entry: (&'a str, &'a str),
    pub buffers: &'a [Option<wgpu::VertexBufferLayout<'a>>],
    pub blend: Option<wgpu::BlendState>,
    pub depth: (bool, wgpu::CompareFunction),
    pub cull: Option<wgpu::Face>,
}

pub(crate) fn pipeline(s: &Shared, k: Kind) -> wgpu::RenderPipeline {
    s.device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(k.entry.0),
            layout: Some(s.layout),
            vertex: wgpu::VertexState {
                module: s.module,
                entry_point: Some(k.entry.0),
                compilation_options: Default::default(),
                buffers: k.buffers,
            },
            primitive: wgpu::PrimitiveState {
                cull_mode: k.cull,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH,
                depth_write_enabled: Some(k.depth.0),
                depth_compare: Some(k.depth.1),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: s.samples,
                ..Default::default()
            },
            fragment: Some(wgpu::FragmentState {
                module: s.module,
                entry_point: Some(k.entry.1),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR,
                    blend: k.blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        })
}

/// A buffer's entry in a layout.
pub(crate) fn buffer(
    binding: u32,
    ty: wgpu::BufferBindingType,
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

/// The scene's own group: the globals, the lights and their grid, the
/// sun's shadow and its sampler, the ground's heights.
pub(crate) fn scene_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let frag = wgpu::ShaderStages::FRAGMENT;
    let both = wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT;
    let storage = wgpu::BufferBindingType::Storage { read_only: true };
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("engine"),
        entries: &[
            buffer(0, wgpu::BufferBindingType::Uniform, both),
            buffer(1, storage, frag),
            buffer(2, storage, frag),
            buffer(3, storage, frag),
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: frag,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2Array,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 5,
                visibility: frag,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 6,
                visibility: both,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
        ],
    })
}

/// The scene's own group (`scene_layout`): the `globals`, the lights'
/// `lists`, the sun's `shadows`, the ground's `heights`.
pub(crate) fn scene_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    globals: &wgpu::Buffer,
    lists: &crate::grid::Lists,
    shadows: &crate::shadow::Shadows,
    heights: &wgpu::TextureView,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("engine"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: lists.lights.buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: lists.cells.buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: lists.index.buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(&shadows.array),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::Sampler(&shadows.cmp),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::TextureView(heights),
            },
        ],
    })
}

/// The soft group (`soft_layout`): the scene's `depth`, the picture so far
/// (`scene`) and the sampler it is read by.
pub(crate) fn soft_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    depth: &wgpu::TextureView,
    scene: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("soft"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(depth),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(scene),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

/// What the pass after the solid one reads (a second group): the depth
/// (soft sparks fade into it), a copy of the picture so far and a sampler
/// for it (the sea's reflections).
fn soft_layout(device: &wgpu::Device, msaa: bool) -> wgpu::BindGroupLayout {
    let frag = wgpu::ShaderStages::FRAGMENT;
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("soft"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: frag,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: msaa,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: frag,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: frag,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}
