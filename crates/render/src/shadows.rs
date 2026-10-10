//! The sun's shadow map (`shadow` fits its layers): a layer a cascade,
//! then one over the whole of what stands still (the island), the
//! sampler that compares against it, and the depth-only pipeline that
//! draws what casts. The cascades are drawn every frame; the island's
//! layer only when what stands still changes or the sun has turned, so
//! the cascades past the second need draw only what moves.

use crate::buffers::{bytes, lay, make, runs_of, Grow, MeshBuf, Run, INST};
use crate::cull::Cull;
use crate::draw::Stats;
use crate::geo::{self, V3};
use crate::shadow::Layer;
use crate::{laws, Item, Quality};
use gpu::wgpu;

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// What stands still, as its layer draws it: all of it (the coarser
/// meshes), its bounding spheres, and the sun it was last drawn for.
struct Island {
    buf: Grow,
    runs: Vec<Run>,
    spheres: Vec<(V3, f32)>,
    layer: Option<Layer>,
    /// The sun its layer holds (None: draw it again).
    drawn: Option<V3>,
    /// Whether it is to be drawn this frame.
    due: bool,
}

/// The sun's shadow map: its layers (each also a view of its own to draw
/// into), the sampler that compares against it, the pipeline that draws
/// what casts, each layer's matrix as that reads it, and the island.
pub(crate) struct Shadows {
    layers: Vec<wgpu::TextureView>,
    pub array: wgpu::TextureView,
    pub cmp: wgpu::Sampler,
    pipe: wgpu::RenderPipeline,
    casters: Vec<(wgpu::Buffer, wgpu::BindGroup)>,
    size: u32,
    island: Island,
}

impl Shadows {
    /// As many layers as `quality` has cascades, and the island's, each
    /// its shadow size square.
    pub fn new(device: &wgpu::Device, quality: Quality) -> Shadows {
        let count = quality.cascades + 1;
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow"),
            size: wgpu::Extent3d {
                width: quality.shadow_size,
                height: quality.shadow_size,
                depth_or_array_layers: count,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let layers = (0..count)
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
        let caster_layout = crate::slots::layout(device, "caster", &crate::slots::CASTER);
        let casters = (0..count)
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
        let (constant, slope_scale) = laws::SHADOW_RASTER;
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
                    constant,
                    slope_scale,
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
            size: quality.shadow_size,
            island: Island {
                buf: Grow::new(device, "island casters", wgpu::BufferUsages::VERTEX),
                runs: Vec::new(),
                spheres: Vec::new(),
                layer: None,
                drawn: None,
                due: false,
            },
        }
    }

    /// What stands still, for the island's layer: each at its coarser
    /// mesh (drawn again next frame).
    pub fn statics(
        &mut self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        statics: &[Item],
        meshes: &[Option<MeshBuf>],
        b: &mut Vec<u8>,
    ) {
        let mut all: Vec<Item> = statics
            .iter()
            .map(|i| Item {
                mesh: i.far.map_or(i.mesh, |f| f.0),
                ..*i
            })
            .collect();
        all.sort_by_key(|i| i.mesh);
        let island = &mut self.island;
        island.spheres = all
            .iter()
            .map(|i| {
                (
                    [i.model[12], i.model[13], i.model[14]],
                    crate::cull::radius(i, meshes),
                )
            })
            .collect();
        let mut refs: Vec<&Item> = all.iter().collect();
        b.clear();
        island.runs = lay(&mut refs, b, 0);
        island.buf.put(device, queue, b);
        island.drawn = None;
    }

    /// The island's layer under the sun at `sun` (None while it is down,
    /// or nothing stands still): fitted again, and to be drawn, once the
    /// sun has turned far enough from the one it holds.
    pub fn island(&mut self, queue: &wgpu::Queue, sun: Option<V3>) -> Option<Layer> {
        let island = &mut self.island;
        island.due = false;
        let sun = geo::norm(sun?);
        let turned = island
            .drawn
            .is_none_or(|was| geo::dot(was, sun) < laws::SHADOW_TURN.cos());
        if turned {
            island.layer = crate::shadow::whole(&island.spheres, sun, self.size);
            island.drawn = Some(sun);
            island.due = island.layer.is_some();
            if let Some(l) = &island.layer {
                let last = self.casters.len() - 1;
                queue.write_buffer(&self.casters[last].0, 0, &bytes(&l.m));
            }
        }
        island.layer
    }

    /// Each cascade's matrix, for the pass that draws it.
    pub fn aim(&self, queue: &wgpu::Queue, cascades: &[Layer]) {
        for (l, (buf, _)) in cascades.iter().zip(&self.casters) {
            queue.write_buffer(buf, 0, &bytes(&l.m));
        }
    }

    /// The island's layer as the scene reads it (its index in the map).
    pub fn island_layer(&self) -> u32 {
        self.layers.len() as u32 - 1
    }

    /// Draw the island's layer if it is due, then what casts into each of
    /// the first `count` cascades: what stands still, then what moves
    /// (`cull`'s lists).
    pub fn record(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        cull: &Cull,
        meshes: &[Option<MeshBuf>],
        count: usize,
        stats: &mut Stats,
    ) {
        let island = &self.island;
        let last = self.layers.len() - 1;
        let lists = (0..count).map(|c| {
            let lists = [
                (&cull.bufs[c].buf, &cull.runs[c]),
                (&cull.moving_bufs[c].buf, &cull.moving_runs[c]),
            ];
            (c, lists.to_vec())
        });
        let whole = island
            .due
            .then(|| (last, vec![(&island.buf.buf, &island.runs)]));
        for (c, lists) in whole.into_iter().chain(lists) {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.layers[c],
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
            for (buf, runs) in lists {
                runs_of(&mut pass, meshes, buf, runs, &mut s);
            }
            stats.draws += s.draws;
            if c < last {
                stats.shadow[c.min(2)] = s.triangles;
            }
        }
    }
}
