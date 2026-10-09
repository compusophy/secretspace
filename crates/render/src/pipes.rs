//! What the scene's pipelines share: how one is made, and the layouts of
//! their bind groups (the scene's own; and what the pass after the solid
//! one reads: its depth, and a copy of the picture so far).

use crate::post::{DEPTH, HDR};
use gpu::wgpu;

/// What every scene pipeline shares.
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

/// What the pass after the solid one reads (a second group): the depth
/// (soft sparks fade into it), a copy of the picture so far and a sampler
/// for it (the sea's reflections).
pub(crate) fn soft_layout(device: &wgpu::Device, msaa: bool) -> wgpu::BindGroupLayout {
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
