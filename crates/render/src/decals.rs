//! Decals (`Decal`): a box each, drawn over the scene once its depth can
//! be read; the shader (`shaders::decal`) marks what stands inside it.

use crate::buffers::{put_f32s, Grow};
use crate::draw::Stats;
use crate::pipes::{pipeline, Kind, Shared};
use crate::Decal;
use gpu::wgpu;

/// Floats a decal: its middle and radius; its turn, depth, mark and
/// seed; its colour.
const FLOATS: usize = 12;
/// The most drawn a frame (the first so many).
const MOST: usize = 256;

pub(crate) struct Decals {
    pipe: wgpu::RenderPipeline,
    buf: Grow,
    n: u32,
}

impl Decals {
    /// `s` with the layout that reads the scene's depth.
    pub(crate) fn new(s: &Shared) -> Decals {
        let layout = wgpu::VertexBufferLayout {
            array_stride: (FLOATS * 4) as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4],
        };
        let pipe = pipeline(
            s,
            Kind {
                entry: ("decal_vs", "decal_fs"),
                buffers: &[Some(layout)],
                // Premultiplied: a burn lays black over, light adds.
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                depth: (false, wgpu::CompareFunction::Always),
                // Its inside faces: each pixel once, the eye in it or not.
                cull: Some(wgpu::Face::Front),
            },
        );
        Decals {
            pipe,
            buf: Grow::new(s.device, "decals", wgpu::BufferUsages::VERTEX),
            n: 0,
        }
    }

    /// This frame's decals.
    pub(crate) fn put(
        &mut self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        list: &[Decal],
        b: &mut Vec<u8>,
    ) {
        b.clear();
        for d in list.iter().take(MOST) {
            let (s, c) = d.yaw.sin_cos();
            let mark = d.mark as u32 as f32 + d.seed.clamp(0.0, 0.999);
            put_f32s(b, &[d.p[0], d.p[1], d.p[2], d.r.max(0.01)]);
            put_f32s(b, &[c, s, d.depth.max(0.01), mark]);
            put_f32s(b, &d.c);
        }
        self.n = list.len().min(MOST) as u32;
        if self.n > 0 {
            self.buf.put(device, queue, b);
        }
    }

    /// Drawn in a pass that reads the depth (`group`, the second group).
    pub(crate) fn draw(
        &self,
        pass: &mut wgpu::RenderPass,
        group: &wgpu::BindGroup,
        stats: &mut Stats,
    ) {
        if self.n == 0 {
            return;
        }
        pass.set_pipeline(&self.pipe);
        pass.set_bind_group(1, group, &[]);
        pass.set_vertex_buffer(0, self.buf.buf.slice(..));
        pass.draw(0..36, 0..self.n);
        stats.draws += 1;
    }
}
