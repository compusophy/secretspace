//! The renderer: pipelines, buffers, and the passes of a frame. The sun's
//! shadow (a pass a cascade), then the scene in HDR, multisampled: the
//! sky, what is solid (statics, then what moves), grass, what is
//! see-through (the sea among it), what glows, and sparks; then, over a
//! cleared depth, the viewmodel; then bloom and the finish (`post`).

use crate::buffers::{make, put_f32s, runs_of, tiny_texture, Grow, MeshBuf, Run};
use crate::grid::Grid;
use crate::post::{Post, DEPTH, HDR};
use crate::terrain::Terrain;
use crate::{geo, laws, m4, shaders, shadow, Frame, Item, Look, Material, Mesh, Pass, Quality, M4};
use gpu::wgpu;

/// Floats an instance: a model matrix, a tint, (glow, rough, material,
/// detail).
const INST: usize = 24;
/// Floats a spark: position and size, colour.
const SPARK: usize = 8;
/// How far the eye goes before the statics are laid again (near and far
/// meshes chosen anew).
const STILL_RELAY: f32 = 6.0;
const SHADOW: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// What the last frame cost.
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub draws: u32,
    pub instances: u32,
    pub triangles: u64,
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

/// What every scene pipeline shares.
struct Shared<'a> {
    device: &'a wgpu::Device,
    layout: &'a wgpu::PipelineLayout,
    module: &'a wgpu::ShaderModule,
    samples: u32,
}

struct Kind<'a> {
    entry: (&'a str, &'a str),
    buffers: &'a [Option<wgpu::VertexBufferLayout<'a>>],
    blend: Option<wgpu::BlendState>,
    depth: (bool, wgpu::CompareFunction),
    cull: Option<wgpu::Face>,
}

fn pipeline(s: &Shared, k: Kind) -> wgpu::RenderPipeline {
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

impl Renderer {
    /// A renderer finishing into targets of `format`, doing as much as
    /// `quality` says.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        quality: Quality,
    ) -> Renderer {
        let frag = wgpu::ShaderStages::FRAGMENT;
        let both = wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT;
        let buffer = |binding, ty, visibility| wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let storage = wgpu::BufferBindingType::Storage { read_only: true };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("engine"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene"),
            source: wgpu::ShaderSource::Wgsl(shaders::scene().into()),
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
                buffers: &[Some(spark)],
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent::OVER,
                }),
                depth: (false, GreaterEqual),
                cull: None,
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
            post: Post::new(device, format, quality.msaa, quality.bloom_levels),
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
        let m = MeshBuf {
            v: vb,
            i: ib,
            count: g.i.len() as u32,
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
    fn lay(items: &mut [&Item], bytes: &mut Vec<u8>, base: u32) -> Vec<Run> {
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
        let (vp, right, up): (M4, _, _) = f.cam.matrices(fov);
        let fwd = f.cam.forward();
        let t = (fov / 2.0).tan();
        let l: &Look = &f.look;
        let lin = |c: [f32; 3]| c.map(|v| v.max(0.0).powf(2.2));
        let mut b = Vec::with_capacity(shaders::GLOBALS as usize);
        put_f32s(&mut b, &vp);
        for c in 0..3 {
            put_f32s(&mut b, cascades.get(c).unwrap_or(&m4::ID));
        }
        let v4 = |b: &mut Vec<u8>, v: [f32; 3], w: f32| put_f32s(b, &[v[0], v[1], v[2], w]);
        v4(&mut b, f.cam.eye, f.time);
        v4(&mut b, fwd, t * f.cam.aspect);
        v4(&mut b, right, t);
        v4(&mut b, up, size.1 as f32 / 2.0 / t);
        v4(&mut b, geo::norm(l.sun_dir), (l.sun_size / 2.0).cos());
        v4(&mut b, l.sun, l.stars);
        v4(&mut b, l.sky, l.fog);
        v4(&mut b, l.low, l.exposure);
        v4(&mut b, l.zenith, l.clouds);
        v4(&mut b, l.horizon, l.fog_falloff);
        v4(&mut b, l.deep, l.sea.unwrap_or(-1000.0));
        let g = &self.grid;
        put_f32s(&mut b, &[g.origin[0], g.origin[1], g.cell, g.n as f32]);
        let n = cascades.len() as f32;
        put_f32s(
            &mut b,
            &[
                size.0 as f32,
                size.1 as f32,
                1.0 / self.quality.shadow_size as f32,
                n,
            ],
        );
        let s = laws::CASCADES;
        put_f32s(&mut b, &[s[0], s[1], s[2], laws::SHADOW_STRENGTH]);
        put_f32s(&mut b, &self.terrain);
        put_f32s(&mut b, &[l.wind[0], l.wind[1], 0.0, 0.0]);
        v4(&mut b, lin(l.water), l.waves);
        let side = self.grass_side();
        let q = self.quality;
        put_f32s(
            &mut b,
            &[
                q.grass_spacing,
                side as f32,
                q.grass_reach,
                if side > 0 { 1.0 } else { 0.0 },
            ],
        );
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
        self.post.fit(&device, &queue, size);
        let mut stats = Stats::default();

        // The lights, gridded about the eye.
        self.grid.build(f.lights, [f.cam.eye[0], f.cam.eye[2]]);
        stats.lights = self.grid.lights.len() as u32;
        let mut b = std::mem::take(&mut self.bytes);
        b.clear();
        for l in &self.grid.lights {
            let c = l.c;
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
            let chosen: Vec<Item> = self
                .statics
                .iter()
                .map(|i| match i.far {
                    Some((m, d)) => {
                        let at = [i.model[12], i.model[13], i.model[14]];
                        let d2 = (0..3).map(|k| (at[k] - eye[k]).powi(2)).sum::<f32>();
                        Item {
                            mesh: if d2 > d * d { m } else { i.mesh },
                            ..*i
                        }
                    }
                    None => *i,
                })
                .collect();
            let mut items: Vec<&Item> = chosen.iter().collect();
            items.sort_by_key(|i| i.mesh);
            b.clear();
            self.still_runs = Self::lay(&mut items, &mut b, 0);
            self.still.put(&device, &queue, &b);
        }

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
            runs_of(&mut pass, meshes, &self.still.buf, &self.still_runs, &mut s);
            runs_of(
                &mut pass,
                meshes,
                &self.moving.buf,
                &runs[Pass::Opaque as usize],
                &mut s,
            );
            stats.draws += s.draws;
        }

        // The scene.
        let h = f.look.horizon;
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("world"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &t.color,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: h[0] as f64,
                            g: h[1] as f64,
                            b: h[2] as f64,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &t.depth,
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
            let side = self.grass_side();
            if side > 0 {
                pass.set_pipeline(&self.grass);
                pass.draw(0..9, 0..side * side);
                stats.draws += 1;
                stats.grass = side * side;
            }
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
        // The viewmodel, over a cleared depth; the picture resolves here.
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("view"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &t.color,
                    depth_slice: None,
                    resolve_target: t.resolve.as_ref(),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &t.depth,
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
        self.post.run(encoder, &queue, target, &f.look);
        self.stats = stats;
    }
}
