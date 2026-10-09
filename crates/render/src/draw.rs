//! The renderer: pipelines, buffers, and the passes of a frame. The sun's
//! shadow (a pass a cascade), then the scene in HDR, multisampled: the
//! sky, what is solid (statics, then what moves), grass, what is
//! see-through (the sea among it), what glows, and sparks, once what is
//! solid is occluded (`ao`); then, over a cleared depth, the viewmodel;
//! then bloom and the finish (`post`).

use crate::buffers::{make, put_f32s, runs_of, tiny_texture, Grow, MeshBuf, Run};
use crate::grid::Grid;
use crate::pipes::{buffer, pipeline, Kind, Shared};
use crate::post::{Post, HDR};
use crate::terrain::Terrain;
use crate::{geo, laws, m4, shaders, shadow, Frame, Item, Material, Mesh, Pass, Quality, M4};
use gpu::wgpu;

/// Floats an instance: a model matrix, a tint, (glow, rough, material,
/// detail).
const INST: usize = 24;
/// Floats a spark: position and size, colour, its streak and shape.
const SPARK: usize = 12;
/// How far the eye goes before the statics are laid again (near and far
/// meshes chosen anew).
const STILL_RELAY: f32 = crate::cull::EYE_SLACK;
const SHADOW: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// What the last frame cost.
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub draws: u32,
    pub instances: u32,
    pub triangles: u64,
    /// Triangles drawn into the sun's shadow, a cascade at a time.
    pub shadow: [u64; 3],
    pub lights: u32,
    pub grass: u32,
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    quality: Quality,
    layout: wgpu::BindGroupLayout,
    opaque: wgpu::RenderPipeline,
    faint: wgpu::RenderPipeline,
    glow: wgpu::RenderPipeline,
    sky: wgpu::RenderPipeline,
    sparks: wgpu::RenderPipeline,
    /// Sparks fading into what stands behind them, and the depth they
    /// read (made again with the screen's targets: their size).
    soft_sparks: wgpu::RenderPipeline,
    soft_layout: wgpu::BindGroupLayout,
    /// The see-through, the sea mirroring the picture so far; a sampler
    /// for that picture, and a stand-in when there is none.
    faint_ssr: wgpu::RenderPipeline,
    scene_samp: wgpu::Sampler,
    nothing: wgpu::TextureView,
    soft_group: Option<(wgpu::BindGroup, (u32, u32))>,
    grass: wgpu::RenderPipeline,
    globals: [wgpu::Buffer; 2],
    lights: Grow,
    cells: Grow,
    index: Grow,
    groups: Option<[wgpu::BindGroup; 2]>,
    moving: Grow,
    still: Grow,
    spark_buf: Grow,
    meshes: Vec<Option<MeshBuf>>,
    free: Vec<u32>,
    statics: Vec<Item>,
    statics_dirty: bool,
    /// Where the eye was when the statics were laid (their near or far
    /// meshes chosen from it).
    still_eye: [f32; 3],
    still_runs: Vec<Run>,
    /// What casts into each of the sun's cascades (`cull`).
    cull: crate::cull::Cull,
    grid: Grid,
    bytes: Vec<u8>,
    shadow_layers: Vec<wgpu::TextureView>,
    shadow_array: wgpu::TextureView,
    shadow_cmp: wgpu::Sampler,
    shadow_pipe: wgpu::RenderPipeline,
    casters: Vec<(wgpu::Buffer, wgpu::BindGroup)>,
    heights: wgpu::TextureView,
    terrain: [f32; 4],
    water: Mesh,
    post: Post,
    pub stats: Stats,
}

impl Renderer {
    /// A renderer finishing into targets of `format`, doing as much as
    /// `quality` says.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        quality: Quality,
    ) -> Renderer {
        let layout = crate::pipes::scene_layout(device);
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("engine"),
            bind_group_layouts: &[Some(&layout)],
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
        use wgpu::CompareFunction::{Always, GreaterEqual};
        let world = [Some(vertex.clone()), Some(instance)];
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
        let opaque = pipeline(&shared, world_kind(None, true, Some(wgpu::Face::Back)));
        let faint = pipeline(
            &shared,
            world_kind(Some(wgpu::BlendState::ALPHA_BLENDING), false, None),
        );
        let glow = pipeline(&shared, world_kind(Some(add), false, None));
        let sky = pipeline(
            &shared,
            Kind {
                entry: ("sky_vs", "sky_fs"),
                buffers: &[],
                blend: None,
                depth: (false, Always),
                cull: None,
            },
        );
        let sparks = pipeline(
            &shared,
            Kind {
                entry: ("spark_vs", "spark_fs"),
                buffers: &[Some(spark.clone())],
                // Premultiplied: light (alpha 0) adds, smoke lays over.
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                depth: (false, GreaterEqual),
                cull: None,
            },
        );
        // Sparks that fade into what stands behind them read the depth (a
        // second group), in the pass that only reads it.
        let soft_layout = crate::pipes::soft_layout(device, quality.msaa > 1);
        let soft_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("soft"),
            bind_group_layouts: &[Some(&layout), Some(&soft_layout)],
            immediate_size: 0,
        });
        let soft_sparks = pipeline(
            &Shared {
                layout: &soft_pl,
                ..shared
            },
            Kind {
                entry: ("spark_vs", "spark_soft_fs"),
                buffers: &[Some(spark.clone())],
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                depth: (false, GreaterEqual),
                cull: None,
            },
        );
        let faint_ssr = pipeline(
            &Shared {
                layout: &soft_pl,
                ..shared
            },
            Kind {
                entry: ("world_vs", "faint_fs"),
                ..world_kind(Some(wgpu::BlendState::ALPHA_BLENDING), false, None)
            },
        );
        let grass = pipeline(
            &shared,
            Kind {
                entry: ("grass_vs", "grass_fs"),
                buffers: &[],
                blend: None,
                depth: (true, GreaterEqual),
                cull: None,
            },
        );
        // The sun's shadow: a depth-only pipeline, a cascade a layer.
        let cascades = quality.cascades.max(1);
        let shadow_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow"),
            size: wgpu::Extent3d {
                width: quality.shadow_size,
                height: quality.shadow_size,
                depth_or_array_layers: cascades,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SHADOW,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_layers = (0..cascades)
            .map(|c| {
                shadow_tex.create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: c,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let shadow_array = shadow_tex.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let caster_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("caster"),
            entries: &[buffer(
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
        let shadow_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shadow"),
            source: wgpu::ShaderSource::Wgsl(shaders::shadow().into()),
        });
        let shadow_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shadow"),
            bind_group_layouts: &[Some(&caster_layout)],
            immediate_size: 0,
        });
        let shadow_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow"),
            layout: Some(&shadow_pl),
            vertex: wgpu::VertexState {
                module: &shadow_module,
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
                format: SHADOW,
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
        let uniform = wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST;
        let vbuf = wgpu::BufferUsages::VERTEX;
        let mut r = Renderer {
            device: device.clone(),
            queue: queue.clone(),
            quality,
            layout,
            opaque,
            faint,
            glow,
            sky,
            sparks,
            soft_sparks,
            soft_layout,
            faint_ssr,
            scene_samp: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("scene"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            nothing: crate::post::texture(device, "nothing", (1, 1), HDR, 1, true),
            soft_group: None,
            grass,
            globals: [
                make(device, "globals", shaders::GLOBALS, uniform),
                make(device, "globals (view)", shaders::GLOBALS, uniform),
            ],
            lights: Grow::new(device, "lights", wgpu::BufferUsages::STORAGE),
            cells: Grow::new(device, "cells", wgpu::BufferUsages::STORAGE),
            index: Grow::new(device, "index", wgpu::BufferUsages::STORAGE),
            groups: None,
            moving: Grow::new(device, "moving", vbuf),
            still: Grow::new(device, "still", vbuf),
            spark_buf: Grow::new(device, "sparks", vbuf),
            meshes: Vec::new(),
            free: Vec::new(),
            statics: Vec::new(),
            statics_dirty: false,
            still_eye: [0.0; 3],
            still_runs: Vec::new(),
            cull: crate::cull::Cull::new(device, laws::CASCADES.len()),
            grid: Grid::default(),
            bytes: Vec::new(),
            shadow_layers,
            shadow_array,
            shadow_cmp: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("shadow"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                compare: Some(wgpu::CompareFunction::LessEqual),
                ..Default::default()
            }),
            shadow_pipe,
            casters,
            heights: tiny_texture(device, queue),
            terrain: [0.0; 4],
            water: Mesh(0),
            post: Post::new(
                device,
                format,
                quality.msaa,
                quality.bloom_levels,
                (quality.ao, quality.shafts),
            ),
            stats: Stats::default(),
        };
        let mut sea = geo::Geo::default();
        sea.floor((-1.0, -1.0), (1.0, 1.0), 0.0, [1.0; 3], 0.0);
        r.water = r.mesh(&sea);
        r
    }

    pub fn quality(&self) -> Quality {
        self.quality
    }

    /// Keep this geometry on the GPU.
    pub fn mesh(&mut self, g: &geo::Geo) -> Mesh {
        let mut bytes = Vec::with_capacity(g.v.len() * 4);
        put_f32s(&mut bytes, &g.v);
        let vb = make(
            &self.device,
            "mesh",
            (bytes.len() as u64).max(16),
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        );
        if !bytes.is_empty() {
            self.queue.write_buffer(&vb, 0, &bytes);
        }
        let ib_bytes: Vec<u8> = g.i.iter().flat_map(|i| i.to_le_bytes()).collect();
        let ib = make(
            &self.device,
            "mesh (index)",
            (ib_bytes.len() as u64).max(16),
            wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        );
        if !ib_bytes.is_empty() {
            self.queue.write_buffer(&ib, 0, &ib_bytes);
        }
        let r =
            g.v.chunks(geo::STRIDE)
                .map(|v| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt())
                .fold(0.0, f32::max);
        let m = MeshBuf {
            v: vb,
            i: ib,
            count: g.i.len() as u32,
            r,
        };
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

    /// The ground's heights and grass, for the GPU (grass, the sea's
    /// shallows).
    pub fn terrain(&mut self, t: &Terrain) {
        let n = t.n as u32;
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("heights"),
            size: wgpu::Extent3d {
                width: n,
                height: n,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rg32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let bytes: Vec<u8> = t
            .heights
            .iter()
            .zip(&t.lush)
            .flat_map(|(h, l)| [h.to_le_bytes(), l.to_le_bytes()])
            .flatten()
            .collect();
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(n * 8),
                rows_per_image: Some(n),
            },
            wgpu::Extent3d {
                width: n,
                height: n,
                depth_or_array_layers: 1,
            },
        );
        self.heights = tex.create_view(&Default::default());
        self.terrain = [t.origin[0], t.origin[1], t.cell, n as f32];
        self.groups = None;
    }

    /// Lay items out as instances; the runs of one mesh each.
    pub(crate) fn lay(items: &mut [&Item], bytes: &mut Vec<u8>, base: u32) -> Vec<Run> {
        let mut runs: Vec<Run> = Vec::new();
        for (k, it) in items.iter().enumerate() {
            put_f32s(bytes, &it.model);
            put_f32s(bytes, &it.tint);
            put_f32s(
                bytes,
                &[it.glow, it.rough, it.material as i32 as f32, it.detail],
            );
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

    fn grass_side(&self) -> u32 {
        let q = self.quality;
        if q.grass_spacing <= 0.0 || self.terrain[3] < 2.0 {
            return 0;
        }
        (2.0 * q.grass_reach / q.grass_spacing).ceil() as u32
    }

    fn globals(&self, f: &Frame, fov: f32, size: (u32, u32), cascades: &[M4]) -> Vec<u8> {
        let shared = (&self.grid, self.quality, self.terrain, self.grass_side());
        crate::globals::bytes(f, fov, size, cascades, shared)
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
        self.post.fit(&device, &queue, size);
        let mut stats = Stats::default();

        // The lights, gridded about the eye.
        self.grid.build(f.lights, [f.cam.eye[0], f.cam.eye[2]]);
        stats.lights = self.grid.lights.len() as u32;
        let mut b = std::mem::take(&mut self.bytes);
        b.clear();
        for l in &self.grid.lights {
            let c = geo::scale(l.c, f.look.glow);
            put_f32s(
                &mut b,
                &[l.p[0], l.p[1], l.p[2], l.r, c[0], c[1], c[2], 0.0],
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

        // The sun's cascades (none when it is down).
        let sun_up = geo::norm(f.look.sun_dir)[1] > 0.02;
        let count = if sun_up {
            self.quality.cascades as usize
        } else {
            0
        };
        let cascades = shadow::fit(
            &f.cam,
            f.look.sun_dir,
            &laws::CASCADES[..count],
            self.quality.shadow_size,
        );
        for (c, m) in cascades.iter().enumerate() {
            let mut cb = Vec::with_capacity(64);
            put_f32s(&mut cb, m);
            queue.write_buffer(&self.casters[c].0, 0, &cb);
        }
        queue.write_buffer(
            &self.globals[0],
            0,
            &self.globals(f, f.cam.fov, size, &cascades),
        );
        queue.write_buffer(
            &self.globals[1],
            0,
            &self.globals(f, f.view_fov, size, &cascades),
        );
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
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: wgpu::BindingResource::TextureView(&self.shadow_array),
                        },
                        wgpu::BindGroupEntry {
                            binding: 5,
                            resource: wgpu::BindingResource::Sampler(&self.shadow_cmp),
                        },
                        wgpu::BindGroupEntry {
                            binding: 6,
                            resource: wgpu::BindingResource::TextureView(&self.heights),
                        },
                    ],
                })
            };
            self.groups = Some([group(&self.globals[0]), group(&self.globals[1])]);
        }

        // What never moves, when it changed or the eye has gone far
        // enough to change which are drawn near and which far.
        let eye = f.cam.eye;
        let moved = (0..3)
            .map(|k| (eye[k] - self.still_eye[k]).powi(2))
            .sum::<f32>();
        if self.statics_dirty || moved > STILL_RELAY * STILL_RELAY {
            self.statics_dirty = false;
            self.still_eye = eye;
            self.cull.set(&self.statics, eye);
        }
        // Of them, what the view could see (when it has turned).
        if let Some(mut items) = self.cull.view(&f.cam, &self.meshes) {
            b.clear();
            self.still_runs = Self::lay(&mut items, &mut b, 0);
            self.still.put(&device, &queue, &b);
        }
        self.cull
            .lay((&device, &queue), &cascades, &self.meshes, &mut b);

        // What moves, by pass (the sea among the see-through): the
        // see-through farthest first.
        let sea = f.look.sea.map(|y| {
            let snap = |v: f32| (v / 50.0).round() * 50.0;
            Item::new(
                self.water,
                m4::place([snap(eye[0]), y, snap(eye[2])], 0.0, [1600.0, 1.0, 1600.0]),
            )
            .material(Material::Water)
            .pass(Pass::Faint)
        });
        let far = |i: &Item| {
            if i.material == Material::Water {
                return f32::MIN;
            }
            let d = [
                i.model[12] - eye[0],
                i.model[13] - eye[1],
                i.model[14] - eye[2],
            ];
            -(d[0] * d[0] + d[1] * d[1] + d[2] * d[2])
        };
        let mut by: [Vec<&Item>; 4] = Default::default();
        for it in f.items.iter().chain(sea.iter()) {
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
        self.cull.moving(
            (&device, &queue),
            &cascades,
            &by[Pass::Opaque as usize],
            &self.meshes,
            &mut b,
        );
        b.clear();
        for s in f.sparks {
            put_f32s(&mut b, &[s.p[0], s.p[1], s.p[2], s.size]);
            put_f32s(&mut b, &s.c);
            let shape = s.shape as u32 as f32 + s.seed.clamp(0.0, 0.999);
            put_f32s(&mut b, &[s.v[0], s.v[1], s.v[2], shape]);
        }
        self.spark_buf.put(&device, &queue, &b);
        self.bytes = b;

        let Some(groups) = &self.groups else {
            return;
        };
        let Some(t) = &self.post.targets else {
            return;
        };
        let meshes = &self.meshes;

        // The sun's shadow, a cascade at a time.
        for (c, layer) in self.shadow_layers.iter().enumerate().take(cascades.len()) {
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
            pass.set_pipeline(&self.shadow_pipe);
            pass.set_bind_group(0, &self.casters[c].1, &[]);
            let mut s = Stats::default();
            runs_of(
                &mut pass,
                meshes,
                &self.cull.bufs[c].buf,
                &self.cull.runs[c],
                &mut s,
            );
            runs_of(
                &mut pass,
                meshes,
                &self.cull.moving_bufs[c].buf,
                &self.cull.moving_runs[c],
                &mut s,
            );
            stats.draws += s.draws;
            stats.shadow[c.min(2)] = s.triangles;
        }

        // The scene: what is solid, then (once it is occluded, if it is)
        // what is see-through or glows.
        let split = self.post.occludes();
        if split && self.soft_group.as_ref().is_none_or(|g| g.1 != size) {
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("soft"),
                layout: &self.soft_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&t.depth),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(
                            t.resolve.as_ref().unwrap_or(&self.nothing),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.scene_samp),
                    },
                ],
            });
            self.soft_group = Some((group, size));
        }
        let soft = self.soft_group.as_ref().filter(|_| split).map(|g| &g.0);
        // The sea mirrors the picture so far (resolved into a copy first).
        let ssr = split && self.quality.ssr > 0 && t.resolve.is_some();
        let lit = |pass: &mut wgpu::RenderPass, stats: &mut Stats| {
            match soft.filter(|_| ssr) {
                Some(group) => {
                    pass.set_pipeline(&self.faint_ssr);
                    pass.set_bind_group(1, group, &[]);
                }
                None => pass.set_pipeline(&self.faint),
            }
            runs_of(
                pass,
                meshes,
                &self.moving.buf,
                &runs[Pass::Faint as usize],
                stats,
            );
            pass.set_pipeline(&self.glow);
            runs_of(
                pass,
                meshes,
                &self.moving.buf,
                &runs[Pass::Glow as usize],
                stats,
            );
            if !f.sparks.is_empty() {
                match soft {
                    Some(group) => {
                        pass.set_pipeline(&self.soft_sparks);
                        pass.set_bind_group(1, group, &[]);
                    }
                    None => pass.set_pipeline(&self.sparks),
                }
                pass.set_vertex_buffer(0, self.spark_buf.buf.slice(..));
                pass.draw(0..6, 0..f.sparks.len() as u32);
                stats.draws += 1;
            }
        };
        {
            let mut pass = t.pass(encoder, "world", Some(f.look.horizon), (true, true), false);
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
            let side = self.grass_side();
            if side > 0 {
                pass.set_pipeline(&self.grass);
                pass.draw(0..9, 0..side * side);
                stats.draws += 1;
                stats.grass = side * side;
            }
            if !split {
                lit(&mut pass, &mut stats);
            }
        }
        if split {
            self.post.occlude(encoder, &queue, &f.cam, &f.look);
            if ssr {
                // Nothing drawn: the picture so far, resolved for the sea.
                drop(t.pass(encoder, "scene", None, (false, false), true));
            }
            let mut pass = t.pass(encoder, "lit", None, (false, false), false);
            pass.set_bind_group(0, &groups[0], &[]);
            lit(&mut pass, &mut stats);
        }
        // The viewmodel, over a cleared depth; the picture resolves here.
        {
            let mut pass = t.pass(encoder, "view", None, (true, false), true);
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
        self.post.run(encoder, &queue, target, &f.look);
        self.stats = stats;
    }
}
