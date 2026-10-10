//! The sun's shadow: cascades, each an orthographic view from the sun
//! over a stretch of what the eye sees (near ones sharp, far ones wide),
//! fitted to a sphere so they do not shimmer as you turn, and snapped to
//! their texels so they do not crawl as you walk; and the map they are
//! drawn into (`Shadows`), a layer a cascade, by a depth-only pipeline.

use crate::buffers::{make, runs_of, INST};
use crate::cull::Cull;
use crate::draw::Stats;
use crate::geo::{self, V3};
use crate::{m4, Camera, Quality, M4};
use gpu::wgpu;

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// The sun's shadow map: a layer a cascade (each also a view of its own
/// to draw into), the sampler that compares against it, the pipeline
/// that draws what casts, and each cascade's matrix as that reads it.
pub(crate) struct Shadows {
    layers: Vec<wgpu::TextureView>,
    pub array: wgpu::TextureView,
    pub cmp: wgpu::Sampler,
    pipe: wgpu::RenderPipeline,
    casters: Vec<(wgpu::Buffer, wgpu::BindGroup)>,
}

impl Shadows {
    /// As many layers as `quality` has cascades (at least one, so the
    /// scene always has a map to bind), each its shadow size square.
    pub fn new(device: &wgpu::Device, quality: Quality) -> Shadows {
        let cascades = quality.cascades.max(1);
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow"),
            size: wgpu::Extent3d {
                width: quality.shadow_size,
                height: quality.shadow_size,
                depth_or_array_layers: cascades,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let layers = (0..cascades)
            .map(|c| {
                tex.create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: c,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let array = tex.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let caster_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("caster"),
            entries: &[crate::pipes::buffer(
                0,
                wgpu::BufferBindingType::Uniform,
                wgpu::ShaderStages::VERTEX,
            )],
        });
        let casters = (0..cascades)
            .map(|_| {
                let buf = make(
                    device,
                    "caster",
                    64,
                    wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                );
                let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("caster"),
                    layout: &caster_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buf.as_entire_binding(),
                    }],
                });
                (buf, group)
            })
            .collect();
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shadow"),
            source: wgpu::ShaderSource::Wgsl(crate::shaders::shadow().into()),
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shadow"),
            bind_group_layouts: &[Some(&caster_layout)],
            immediate_size: 0,
        });
        let pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("shadow_vs"),
                compilation_options: Default::default(),
                buffers: &[
                    Some(wgpu::VertexBufferLayout {
                        array_stride: (geo::STRIDE * 4) as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x3],
                    }),
                    Some(wgpu::VertexBufferLayout {
                        array_stride: (INST * 4) as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4],
                    }),
                ],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 2.5,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: None,
            multiview_mask: None,
            cache: None,
        });
        Shadows {
            layers,
            array,
            cmp: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("shadow"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                compare: Some(wgpu::CompareFunction::LessEqual),
                ..Default::default()
            }),
            pipe,
            casters,
        }
    }

    /// Each cascade's matrix, for the pass that draws it.
    pub fn aim(&self, queue: &wgpu::Queue, cascades: &[M4]) {
        for (m, (buf, _)) in cascades.iter().zip(&self.casters) {
            queue.write_buffer(buf, 0, &crate::buffers::bytes(m));
        }
    }

    /// Draw what casts into each of the first `count` cascades: what
    /// stands still, then what moves (`cull`'s lists).
    pub fn record(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        cull: &Cull,
        meshes: &[Option<crate::buffers::MeshBuf>],
        count: usize,
        stats: &mut Stats,
    ) {
        for (c, layer) in self.layers.iter().enumerate().take(count) {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: layer,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipe);
            pass.set_bind_group(0, &self.casters[c].1, &[]);
            let mut s = Stats::default();
            runs_of(&mut pass, meshes, &cull.bufs[c].buf, &cull.runs[c], &mut s);
            runs_of(
                &mut pass,
                meshes,
                &cull.moving_bufs[c].buf,
                &cull.moving_runs[c],
                &mut s,
            );
            stats.draws += s.draws;
            stats.shadow[c.min(2)] = s.triangles;
        }
    }
}

/// An orthographic projection to WebGPU's 0..1 depth.
pub fn ortho(r: f32, near: f32, far: f32) -> M4 {
    let mut m = [0.0; 16];
    m[0] = 1.0 / r;
    m[5] = 1.0 / r;
    m[10] = -1.0 / (far - near);
    m[14] = -near / (far - near);
    m[15] = 1.0;
    m
}

/// The cascades' sun-space matrices, for a camera, the way to the sun,
/// where each cascade ends, and the shadow map's size.
pub fn fit(cam: &Camera, sun: V3, ends: &[f32], size: u32) -> Vec<M4> {
    let sun = geo::norm(sun);
    let fwd = cam.forward();
    let ty = (cam.fov / 2.0).tan();
    let tx = ty * cam.aspect;
    let up = if sun[1].abs() > 0.99 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let mut near = 0.05;
    let mut out = Vec::with_capacity(ends.len());
    for &far in ends {
        // A sphere about this stretch of the view.
        let mid = (near + far) / 2.0;
        let centre = geo::add(cam.eye, geo::scale(fwd, mid));
        let spread = tx * tx + ty * ty;
        let rf = ((far - mid).powi(2) + far * far * spread).sqrt();
        let rn = ((mid - near).powi(2) + near * near * spread).sqrt();
        let r = (rf.max(rn) * 1.02).ceil();
        // Snap the centre to the map's texels, in the sun's frame.
        let (_, s, u) = m4::look([0.0; 3], geo::scale(sun, -1.0), up);
        let texel = 2.0 * r / size as f32;
        let cx = (geo::dot(centre, s) / texel).floor() * texel;
        let cy = (geo::dot(centre, u) / texel).floor() * texel;
        let cz = geo::dot(centre, geo::scale(sun, -1.0));
        let snapped = geo::add(
            geo::add(geo::scale(s, cx), geo::scale(u, cy)),
            geo::scale(sun, -cz),
        );
        let back = 300.0 + r;
        let eye = geo::add(snapped, geo::scale(sun, back));
        let (view_at, _, _) = m4::look(eye, geo::scale(sun, -1.0), up);
        out.push(m4::mul(&ortho(r, 0.0, back + r * 2.0), &view_at));
        near = far;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_the_eye_sees_falls_in_the_first_cascade() {
        let cam = Camera {
            eye: [10.0, 2.0, 5.0],
            ..Camera::default()
        };
        let m = fit(&cam, [0.4, 0.8, 0.2], &[14.0, 48.0, 150.0], 2048);
        assert_eq!(m.len(), 3);
        let f = cam.forward();
        let p = geo::add(cam.eye, geo::scale(f, 6.0));
        let (x, y, w) = m4::project(&m[0], p);
        assert!(
            w > 0.0 && (x / w).abs() <= 1.0 && (y / w).abs() <= 1.0,
            "{x} {y}"
        );
        let z = (0..4)
            .map(|k| m[0][k * 4 + 2] * [p[0], p[1], p[2], 1.0][k])
            .sum::<f32>();
        assert!((0.0..=1.0).contains(&z), "depth in range: {z}");
    }
}
