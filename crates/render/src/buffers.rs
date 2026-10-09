//! The renderer's buffers: ones that grow to fit, meshes on the GPU, runs
//! of instances drawn a mesh at a time, and the texture that stands in
//! for heights when there is no terrain.

use crate::draw::Stats;
use crate::Mesh;
use gpu::wgpu;
use std::ops::Range;

/// A buffer that grows to fit what is put in it.
pub struct Grow {
    pub buf: wgpu::Buffer,
    cap: u64,
    usage: wgpu::BufferUsages,
    label: &'static str,
}

impl Grow {
    pub fn new(device: &wgpu::Device, label: &'static str, usage: wgpu::BufferUsages) -> Grow {
        let usage = usage | wgpu::BufferUsages::COPY_DST;
        Grow {
            buf: make(device, label, 256, usage),
            cap: 256,
            usage,
            label,
        }
    }

    /// Put these bytes in (at least 16); whether the buffer was replaced.
    pub fn put(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[u8]) -> bool {
        let need = (data.len() as u64).max(16).next_multiple_of(4);
        let grew = need > self.cap;
        if grew {
            self.cap = need.next_power_of_two();
            self.buf = make(device, self.label, self.cap, self.usage);
        }
        if !data.is_empty() {
            queue.write_buffer(&self.buf, 0, data);
        }
        grew
    }
}

pub fn make(
    device: &wgpu::Device,
    label: &str,
    size: u64,
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage,
        mapped_at_creation: false,
    })
}

pub fn put_f32s(out: &mut Vec<u8>, v: &[f32]) {
    for f in v {
        out.extend_from_slice(&f.to_le_bytes());
    }
}

pub struct MeshBuf {
    pub v: wgpu::Buffer,
    pub i: wgpu::Buffer,
    pub count: u32,
    /// How far its farthest point is from its origin.
    pub r: f32,
}

/// Instances of one mesh in a row of the instance buffer.
pub struct Run {
    pub mesh: Mesh,
    pub at: Range<u32>,
}

/// Draw runs of instances, each of one mesh.
pub fn runs_of(
    pass: &mut wgpu::RenderPass,
    meshes: &[Option<MeshBuf>],
    inst: &wgpu::Buffer,
    runs: &[Run],
    stats: &mut Stats,
) {
    pass.set_vertex_buffer(1, inst.slice(..));
    for r in runs {
        let Some(Some(m)) = meshes.get(r.mesh.0 as usize) else {
            continue;
        };
        pass.set_vertex_buffer(0, m.v.slice(..));
        pass.set_index_buffer(m.i.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..m.count, 0, r.at.clone());
        stats.draws += 1;
        stats.instances += r.at.len() as u32;
        stats.triangles += m.count as u64 / 3 * r.at.len() as u64;
    }
}

pub fn tiny_texture(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
    let t = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("heights (none)"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rg32Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &t,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &[0; 8],
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(8),
            rows_per_image: Some(1),
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    t.create_view(&Default::default())
}
