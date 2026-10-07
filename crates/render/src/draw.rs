//! The renderer: pipelines, buffers, and the passes of a frame. Sky, then
//! what is solid (statics, then what moves), then what is see-through,
//! what glows, and sparks; then, over a cleared depth, the viewmodel.

use crate::grid::Grid;
use crate::{geo, shaders, Frame, Item, Mesh, Pass, M4};
use gpu::wgpu;
use std::ops::Range;

const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Floats an instance: a model matrix, a tint, and (glow, 0, 0, 0).
const INST: usize = 24;
/// Floats a spark: position and size, colour.
const SPARK: usize = 8;

/// What the last frame cost.
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub draws: u32,
    pub instances: u32,
    pub triangles: u64,
    pub lights: u32,
}

/// A buffer that grows to fit what is put in it.
struct Grow {
    buf: wgpu::Buffer,
    cap: u64,
    usage: wgpu::BufferUsages,
    label: &'static str,
}

impl Grow {
    fn new(device: &wgpu::Device, label: &'static str, usage: wgpu::BufferUsages) -> Grow {
        let usage = usage | wgpu::BufferUsages::COPY_DST;
        Grow {
            buf: make(device, label, 256, usage),
            cap: 256,
            usage,
            label,
        }
    }

    /// Put these bytes in (at least 16); whether the buffer was replaced.
    fn put(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[u8]) -> bool {
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

fn make(device: &wgpu::Device, label: &str, size: u64, usage: wgpu::BufferUsages) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage,
        mapped_at_creation: false,
    })
}

fn put_f32s(out: &mut Vec<u8>, v: &[f32]) {
    for f in v {
        out.extend_from_slice(&f.to_le_bytes());
    }
}

/// Draw runs of instances, each of one mesh.
fn runs_of(
    pass: &mut wgpu::RenderPass,
    meshes: &[Option<(wgpu::Buffer, u32)>],
    inst: &wgpu::Buffer,
    runs: &[Run],
    stats: &mut Stats,
) {
    pass.set_vertex_buffer(1, inst.slice(..));
    for r in runs {
        let Some(Some((buf, n))) = meshes.get(r.mesh.0 as usize) else {
            continue;
        };
        pass.set_vertex_buffer(0, buf.slice(..));
        pass.draw(0..*n, r.at.clone());
        stats.draws += 1;
        stats.instances += r.at.len() as u32;
        stats.triangles += *n as u64 / 3 * r.at.len() as u64;
    }
}

/// Instances of one mesh in a row of the instance buffer.
struct Run {
    mesh: Mesh,
    at: Range<u32>,
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    layout: wgpu::BindGroupLayout,
    opaque: wgpu::RenderPipeline,
    faint: wgpu::RenderPipeline,
    glow: wgpu::RenderPipeline,
    sky: wgpu::RenderPipeline,
    sparks: wgpu::RenderPipeline,
    globals: [wgpu::Buffer; 2],
    lights: Grow,
    cells: Grow,
    index: Grow,
    groups: Option<[wgpu::BindGroup; 2]>,
    moving: Grow,
    still: Grow,
    spark_buf: Grow,
    meshes: Vec<Option<(wgpu::Buffer, u32)>>,
    free: Vec<u32>,
    statics: Vec<Item>,
    statics_dirty: bool,
    still_runs: Vec<Run>,
    depth: Option<(wgpu::TextureView, (u32, u32))>,
    grid: Grid,
    bytes: Vec<u8>,
    pub stats: Stats,
}

/// What every pipeline shares.
struct Shared<'a> {
    device: &'a wgpu::Device,
    layout: &'a wgpu::PipelineLayout,
    module: &'a wgpu::ShaderModule,
    format: wgpu::TextureFormat,
}

fn pipeline(
    s: &Shared,
    entry: (&str, &str),
    buffers: &[Option<wgpu::VertexBufferLayout>],
    blend: Option<wgpu::BlendState>,
    depth: (bool, wgpu::CompareFunction),
    cull: Option<wgpu::Face>,
) -> wgpu::RenderPipeline {
    let (module, format) = (s.module, s.format);
    s.device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(entry.0),
            layout: Some(s.layout),
            vertex: wgpu::VertexState {
                module,
                entry_point: Some(entry.0),
                compilation_options: Default::default(),
                buffers,
            },
            primitive: wgpu::PrimitiveState {
                cull_mode: cull,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH,
                depth_write_enabled: Some(depth.0),
                depth_compare: Some(depth.1),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: Some(entry.1),
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
}

impl Renderer {
    /// A renderer drawing into targets of this format.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Renderer {
        let storage = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("engine"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                storage(1),
                storage(2),
                storage(3),
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("engine"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("engine"),
            source: wgpu::ShaderSource::Wgsl(shaders::wgsl().into()),
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
            attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4],
        };
        use wgpu::CompareFunction::{Always, GreaterEqual};
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
            format,
        };
        let p = |entry, buffers: &[Option<wgpu::VertexBufferLayout>], blend, depth, cull| {
            pipeline(&shared, entry, buffers, blend, depth, cull)
        };
        let opaque = p(
            ("world_vs", "world_fs"),
            &world,
            None,
            (true, GreaterEqual),
            Some(wgpu::Face::Back),
        );
        let faint = p(
            ("world_vs", "world_fs"),
            &world,
            Some(wgpu::BlendState::ALPHA_BLENDING),
            (false, GreaterEqual),
            None,
        );
        let glow = p(
            ("world_vs", "world_fs"),
            &world,
            Some(add),
            (false, GreaterEqual),
            None,
        );
        let sky = p(("sky_vs", "sky_fs"), &[], None, (false, Always), None);
        let sparks = p(
            ("spark_vs", "spark_fs"),
            &[Some(spark)],
            Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent::OVER,
            }),
            (false, GreaterEqual),
            None,
        );
        let uniform = wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST;
        let storage = wgpu::BufferUsages::STORAGE;
        let vertex = wgpu::BufferUsages::VERTEX;
        Renderer {
            device: device.clone(),
            queue: queue.clone(),
            layout,
            opaque,
            faint,
            glow,
            sky,
            sparks,
            globals: [
                make(device, "globals", shaders::GLOBALS, uniform),
                make(device, "globals (view)", shaders::GLOBALS, uniform),
            ],
            lights: Grow::new(device, "lights", storage),
            cells: Grow::new(device, "cells", storage),
            index: Grow::new(device, "index", storage),
            groups: None,
            moving: Grow::new(device, "moving", vertex),
            still: Grow::new(device, "still", vertex),
            spark_buf: Grow::new(device, "sparks", vertex),
            meshes: Vec::new(),
            free: Vec::new(),
            statics: Vec::new(),
            statics_dirty: false,
            still_runs: Vec::new(),
            depth: None,
            grid: Grid::default(),
            bytes: Vec::new(),
            stats: Stats::default(),
        }
    }

    /// Keep this geometry on the GPU.
    pub fn mesh(&mut self, g: &geo::Geo) -> Mesh {
        let mut bytes = Vec::with_capacity(g.v.len() * 4);
        put_f32s(&mut bytes, &g.v);
        let buf = make(
            &self.device,
            "mesh",
            (bytes.len() as u64).max(16),
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        );
        if !bytes.is_empty() {
            self.queue.write_buffer(&buf, 0, &bytes);
        }
        let m = (buf, g.len() as u32);
        match self.free.pop() {
            Some(i) => {
                self.meshes[i as usize] = Some(m);
                Mesh(i)
            }
            None => {
                self.meshes.push(Some(m));
                Mesh(self.meshes.len() as u32 - 1)
            }
        }
    }

    /// Let a mesh go (anything still drawing it is skipped).
    pub fn free(&mut self, m: Mesh) {
        if let Some(slot) = self.meshes.get_mut(m.0 as usize) {
            if slot.take().is_some() {
                self.free.push(m.0);
            }
        }
    }

    /// What never moves: drawn every frame, uploaded only when it changes.
    pub fn statics(&mut self, items: Vec<Item>) {
        self.statics = items;
        self.statics_dirty = true;
    }

    /// Lay items out as instances; the runs of one mesh each.
    fn lay(items: &mut [&Item], bytes: &mut Vec<u8>, base: u32) -> Vec<Run> {
        let mut runs: Vec<Run> = Vec::new();
        for (k, it) in items.iter().enumerate() {
            put_f32s(bytes, &it.model);
            put_f32s(bytes, &it.tint);
            put_f32s(bytes, &[it.glow, 0.0, 0.0, 0.0]);
            let i = base + k as u32;
            match runs.last_mut() {
                Some(r) if r.mesh == it.mesh => r.at.end = i + 1,
                _ => runs.push(Run {
                    mesh: it.mesh,
                    at: i..i + 1,
                }),
            }
        }
        runs
    }

    fn globals(&self, f: &Frame, fov: f32, size: (u32, u32)) -> Vec<u8> {
        let (vp, right, up): (M4, _, _) = f.cam.matrices(fov);
        let fwd = f.cam.forward();
        let t = (fov / 2.0).tan();
        let l = &f.look;
        let mut b = Vec::with_capacity(shaders::GLOBALS as usize);
        put_f32s(&mut b, &vp);
        let v4 = |b: &mut Vec<u8>, v: [f32; 3], w: f32| put_f32s(b, &[v[0], v[1], v[2], w]);
        v4(&mut b, f.cam.eye, f.time);
        v4(&mut b, fwd, t * f.cam.aspect);
        v4(&mut b, right, t);
        v4(&mut b, up, size.1 as f32 / 2.0 / t);
        v4(&mut b, l.fog, l.fog_range.0);
        v4(&mut b, l.sky, l.fog_range.1);
        v4(&mut b, l.low, 0.0);
        v4(&mut b, geo::norm(l.sun_dir), (l.sun_size / 2.0).cos());
        v4(&mut b, l.sun, l.stars);
        v4(&mut b, l.zenith, 0.0);
        v4(&mut b, l.deep, 0.0);
        let g = &self.grid;
        put_f32s(&mut b, &[g.origin[0], g.origin[1], g.cell, g.n as f32]);
        put_f32s(&mut b, &[size.0 as f32, size.1 as f32, 0.0, 0.0]);
        b
    }

    /// Draw a frame into `target` (`size` pixels).
    pub fn draw(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        f: &Frame,
    ) {
        let (device, queue) = (self.device.clone(), self.queue.clone());
        if self.depth.as_ref().map(|d| d.1) != Some(size) {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("depth"),
                size: wgpu::Extent3d {
                    width: size.0.max(1),
                    height: size.1.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            self.depth = Some((tex.create_view(&Default::default()), size));
        }
        let mut stats = Stats::default();

        // The lights, gridded about the eye.
        self.grid.build(f.lights, [f.cam.eye[0], f.cam.eye[2]]);
        stats.lights = self.grid.lights.len() as u32;
        let mut b = std::mem::take(&mut self.bytes);
        b.clear();
        for l in &self.grid.lights {
            put_f32s(
                &mut b,
                &[l.p[0], l.p[1], l.p[2], l.r, l.c[0], l.c[1], l.c[2], 0.0],
            );
        }
        let mut grew = self.lights.put(&device, &queue, &b);
        b.clear();
        for c in &self.grid.cells {
            b.extend_from_slice(&c[0].to_le_bytes());
            b.extend_from_slice(&c[1].to_le_bytes());
        }
        grew |= self.cells.put(&device, &queue, &b);
        b.clear();
        for i in &self.grid.index {
            b.extend_from_slice(&i.to_le_bytes());
        }
        grew |= self.index.put(&device, &queue, &b);
        queue.write_buffer(&self.globals[0], 0, &self.globals(f, f.cam.fov, size));
        queue.write_buffer(&self.globals[1], 0, &self.globals(f, f.view_fov, size));
        if grew || self.groups.is_none() {
            let group = |globals: &wgpu::Buffer| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("engine"),
                    layout: &self.layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: globals.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: self.lights.buf.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: self.cells.buf.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: self.index.buf.as_entire_binding(),
                        },
                    ],
                })
            };
            self.groups = Some([group(&self.globals[0]), group(&self.globals[1])]);
        }

        // What never moves, when it changed.
        if self.statics_dirty {
            self.statics_dirty = false;
            let mut items: Vec<&Item> = self.statics.iter().collect();
            items.sort_by_key(|i| i.mesh);
            b.clear();
            self.still_runs = Self::lay(&mut items, &mut b, 0);
            self.still.put(&device, &queue, &b);
        }

        // What moves, by pass: the see-through farthest first.
        let eye = f.cam.eye;
        let far = |i: &Item| {
            let d = [
                i.model[12] - eye[0],
                i.model[13] - eye[1],
                i.model[14] - eye[2],
            ];
            -(d[0] * d[0] + d[1] * d[1] + d[2] * d[2])
        };
        let mut by: [Vec<&Item>; 4] = Default::default();
        for it in f.items {
            by[it.pass as usize].push(it);
        }
        by[Pass::Faint as usize].sort_by(|a, b| far(a).total_cmp(&far(b)));
        for p in [Pass::Opaque, Pass::Glow, Pass::View] {
            by[p as usize].sort_by_key(|i| i.mesh);
        }
        b.clear();
        let mut runs: [Vec<Run>; 4] = Default::default();
        let mut base = 0;
        for p in 0..4 {
            runs[p] = Self::lay(&mut by[p], &mut b, base);
            base += by[p].len() as u32;
        }
        self.moving.put(&device, &queue, &b);
        b.clear();
        for s in f.sparks {
            put_f32s(&mut b, &[s.p[0], s.p[1], s.p[2], s.size]);
            put_f32s(&mut b, &s.c);
        }
        self.spark_buf.put(&device, &queue, &b);
        self.bytes = b;

        let Some(groups) = &self.groups else {
            return;
        };
        let Some((depth, _)) = &self.depth else {
            return;
        };
        let fog = f.look.fog;
        let meshes = &self.meshes;
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("world"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: fog[0] as f64,
                            g: fog[1] as f64,
                            b: fog[2] as f64,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &groups[0], &[]);
            pass.set_pipeline(&self.sky);
            pass.draw(0..3, 0..1);
            stats.draws += 1;
            pass.set_pipeline(&self.opaque);
            runs_of(
                &mut pass,
                meshes,
                &self.still.buf,
                &self.still_runs,
                &mut stats,
            );
            runs_of(
                &mut pass,
                meshes,
                &self.moving.buf,
                &runs[Pass::Opaque as usize],
                &mut stats,
            );
            pass.set_pipeline(&self.faint);
            runs_of(
                &mut pass,
                meshes,
                &self.moving.buf,
                &runs[Pass::Faint as usize],
                &mut stats,
            );
            pass.set_pipeline(&self.glow);
            runs_of(
                &mut pass,
                meshes,
                &self.moving.buf,
                &runs[Pass::Glow as usize],
                &mut stats,
            );
            if !f.sparks.is_empty() {
                pass.set_pipeline(&self.sparks);
                pass.set_vertex_buffer(0, self.spark_buf.buf.slice(..));
                pass.draw(0..6, 0..f.sparks.len() as u32);
                stats.draws += 1;
            }
        }
        if !runs[Pass::View as usize].is_empty() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("view"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &groups[1], &[]);
            pass.set_pipeline(&self.opaque);
            runs_of(
                &mut pass,
                meshes,
                &self.moving.buf,
                &runs[Pass::View as usize],
                &mut stats,
            );
        }
        self.stats = stats;
    }
}
