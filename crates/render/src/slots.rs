//! What each bind group holds, binding by binding: one table a group's
//! layout is made from, and the tests hold every shader's bindings to
//! (`tests/wgsl.rs`), so a binding the WGSL and the layout disagree on is
//! a failed test, not a black page.

use gpu::wgpu;

/// What a binding holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Uniform,
    /// Read only.
    Storage,
    /// A depth texture: an array of layers or not, many-sampled or not.
    Depth {
        array: bool,
        msaa: bool,
    },
    /// A float texture, filterable or not.
    Float {
        filterable: bool,
    },
    /// The scene's picture, read a texel at a time: many-sampled or not.
    Picture {
        msaa: bool,
    },
    /// A sampler: one that compares (for a depth) or one that filters.
    Sampler {
        compare: bool,
    },
}

/// Which stages read a binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Seen {
    Vertex,
    Fragment,
    Both,
}

use Seen::{Both, Fragment, Vertex};
use Slot::{Depth, Float, Picture, Sampler, Storage, Uniform};

/// The scene's own group: the globals, the lights, their grid's cells
/// and index, the sun's shadow map and its sampler, the terrain.
pub const SCENE: [(Slot, Seen); 7] = [
    (Uniform, Both),
    (Storage, Fragment),
    (Storage, Fragment),
    (Storage, Fragment),
    (
        Depth {
            array: true,
            msaa: false,
        },
        Fragment,
    ),
    (Sampler { compare: true }, Fragment),
    (Float { filterable: false }, Both),
];

/// What the pass after the solid one reads: the scene's depth, the
/// picture so far and a sampler for it.
pub fn soft(msaa: bool) -> [(Slot, Seen); 3] {
    [
        (Depth { array: false, msaa }, Fragment),
        (Float { filterable: true }, Fragment),
        (Sampler { compare: false }, Fragment),
    ]
}

/// Ambient occlusion's: the scene's depth, its numbers, the occlusion
/// read back.
pub fn ao(msaa: bool) -> [(Slot, Seen); 3] {
    [
        (Depth { array: false, msaa }, Fragment),
        (Uniform, Fragment),
        (Float { filterable: false }, Fragment),
    ]
}

/// What ambient occlusion reads of the picture of what is solid (how much
/// of its light is direct, in its alpha): a group of its own, so the pass
/// that lays the occlusion over the picture binds none of it.
pub fn picture(msaa: bool) -> [(Slot, Seen); 1] {
    [(Picture { msaa }, Fragment)]
}

/// The sun's shafts': the scene's depth, their numbers.
pub fn shafts(msaa: bool) -> [(Slot, Seen); 2] {
    [
        (Depth { array: false, msaa }, Fragment),
        (Uniform, Fragment),
    ]
}

/// Bloom's: the picture it halves (or doubles), a sampler, its numbers.
pub const POST: [(Slot, Seen); 3] = [
    (Float { filterable: true }, Fragment),
    (Sampler { compare: false }, Fragment),
    (Uniform, Fragment),
];

/// The finish's: bloom's, then the bloom, the grade, the shafts.
pub const FINISH: [(Slot, Seen); 6] = [
    (Float { filterable: true }, Fragment),
    (Sampler { compare: false }, Fragment),
    (Uniform, Fragment),
    (Float { filterable: true }, Fragment),
    (Uniform, Fragment),
    (Float { filterable: true }, Fragment),
];

/// A shadow cascade's: its matrix.
pub const CASTER: [(Slot, Seen); 1] = [(Uniform, Vertex)];

/// A group's layout from its table (binding k the k-th slot).
pub(crate) fn layout(
    device: &wgpu::Device,
    label: &str,
    slots: &[(Slot, Seen)],
) -> wgpu::BindGroupLayout {
    let entries: Vec<wgpu::BindGroupLayoutEntry> = slots
        .iter()
        .enumerate()
        .map(|(k, &(slot, seen))| wgpu::BindGroupLayoutEntry {
            binding: k as u32,
            visibility: match seen {
                Vertex => wgpu::ShaderStages::VERTEX,
                Fragment => wgpu::ShaderStages::FRAGMENT,
                Both => wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            },
            ty: match slot {
                Uniform | Storage => wgpu::BindingType::Buffer {
                    ty: if slot == Uniform {
                        wgpu::BufferBindingType::Uniform
                    } else {
                        wgpu::BufferBindingType::Storage { read_only: true }
                    },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                Depth { array, msaa } => wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: if array {
                        wgpu::TextureViewDimension::D2Array
                    } else {
                        wgpu::TextureViewDimension::D2
                    },
                    multisampled: msaa,
                },
                Float { filterable } => wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                Picture { msaa } => wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: msaa,
                },
                Sampler { compare } => wgpu::BindingType::Sampler(if compare {
                    wgpu::SamplerBindingType::Comparison
                } else {
                    wgpu::SamplerBindingType::Filtering
                }),
            },
            count: None,
        })
        .collect();
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &entries,
    })
}
