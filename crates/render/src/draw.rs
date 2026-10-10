//! The renderer: what it holds (meshes, statics, the terrain, buffers)
//! and the passes of a frame. The sun's shadow (a pass a cascade), then
//! the scene in HDR, multisampled: the sky, what is solid (statics, then
//! what moves), grass, what is see-through (the sea among it), what
//! glows, and sparks, once what is solid is occluded (`ao`); decals laid
//! on what stands still before what moves is drawn; then, over a cleared
//! depth, the viewmodel; then bloom and the finish (`post`).

use crate::buffers::{lay, make, put_f32s, runs_of, tiny_texture, Grow, MeshBuf, Run};
use crate::grid::{Grid, Lists};
use crate::pipes::Pipes;
use crate::post::{Ops, Post, HDR};
use crate::shadows::Shadows;
use crate::terrain::Terrain;
use crate::{geo, laws, m4, shaders, shadow, Frame, Item, Material, Mesh, Pass, Quality};
use gpu::wgpu;

/// How far the eye goes before the statics are laid again (near and far
/// meshes chosen anew).
const STILL_RELAY: f32 = crate::cull::EYE_SLACK;

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
    /// The format it finishes into.
    format: wgpu::TextureFormat,
    quality: Quality,
    layout: wgpu::BindGroupLayout,
    pipes: Pipes,
    /// A sampler for the picture so far (the sea mirrors it), and a stand
    /// in when there is none.
    scene_samp: wgpu::Sampler,
    nothing: wgpu::TextureView,
    /// What the pass after the solid one reads (made again with the
    /// screen's targets: their size).
    soft_group: Option<(wgpu::BindGroup, (u32, u32))>,
    globals: [wgpu::Buffer; 2],
    lists: Lists,
    groups: Option<[wgpu::BindGroup; 2]>,
    moving: Grow,
    still: Grow,
    spark_buf: Grow,
    /// Every mesh made, by its id (none once let go: an id is never
    /// handed out again, so what still holds one draws nothing).
    meshes: Vec<Option<MeshBuf>>,
    /// The statics: the solid ones (culled, laid once); the rest, drawn
    /// in their passes with what moves.
    statics: Vec<Item>,
    statics_lit: Vec<Item>,
    statics_dirty: bool,
    /// Where the eye was when the statics were laid (their near or far
    /// meshes chosen from it).
    still_eye: [f32; 3],
    still_runs: Vec<Run>,
    /// What casts into each of the sun's cascades (`cull`).
    cull: crate::cull::Cull,
    grid: Grid,
    bytes: Vec<u8>,
    shadows: Shadows,
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
        let uniform = wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST;
        let vbuf = wgpu::BufferUsages::VERTEX;
        let mut r = Renderer {
            device: device.clone(),
            queue: queue.clone(),
            format,
            quality,
            pipes: Pipes::new(device, quality, &layout),
            layout,
            scene_samp: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("scene"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            nothing: crate::post::texture(device, "nothing", (1, 1), HDR, 1, true),
            soft_group: None,
            globals: [
                make(device, "globals", shaders::GLOBALS, uniform),
                make(device, "globals (view)", shaders::GLOBALS, uniform),
            ],
            lists: Lists::new(device),
            groups: None,
            moving: Grow::new(device, "moving", vbuf),
            still: Grow::new(device, "still", vbuf),
            spark_buf: Grow::new(device, "sparks", vbuf),
            meshes: Vec::new(),
            statics: Vec::new(),
            statics_lit: Vec::new(),
            statics_dirty: false,
            still_eye: [0.0; 3],
            still_runs: Vec::new(),
            cull: crate::cull::Cull::new(device, 3),
            grid: Grid::default(),
            bytes: Vec::new(),
            shadows: Shadows::new(device, quality),
            heights: tiny_texture(device, queue),
            terrain: [0.0; 4],
            water: Mesh(0),
            post: Post::new(device, format, quality),
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

    /// Do as much as `quality` says from now on: what depends on the tier
    /// (the pipelines, the shadow map, the targets) made again, and what
    /// does not (meshes, statics, the terrain) kept, so nothing need be
    /// built again. Only the scale changed: only the targets' size.
    pub fn set_quality(&mut self, quality: Quality) {
        let same = Quality {
            scale: self.quality.scale,
            ..quality
        } == self.quality;
        self.quality = quality;
        if same {
            return;
        }
        let device = &self.device;
        self.pipes = Pipes::new(device, quality, &self.layout);
        self.shadows = Shadows::new(device, quality);
        self.post = Post::new(device, self.format, quality);
        self.groups = None;
        self.soft_group = None;
        self.statics_dirty = true;
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
        self.meshes.push(Some(m));
        Mesh(self.meshes.len() as u32 - 1)
    }

    /// Let a mesh go (anything still drawing it is skipped).
    pub fn free(&mut self, m: Mesh) {
        if let Some(slot) = self.meshes.get_mut(m.0 as usize) {
            *slot = None;
        }
    }

    /// What never moves: drawn every frame, uploaded only when it changes
    /// (what is see-through or glows, with what moves, in its pass).
    pub fn statics(&mut self, items: Vec<Item>) {
        (self.statics, self.statics_lit) = items.into_iter().partition(|i| i.pass == Pass::Opaque);
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

    fn grass_side(&self) -> u32 {
        let q = self.quality;
        if q.grass_spacing <= 0.0 || self.terrain[3] < 2.0 {
            return 0;
        }
        (2.0 * q.grass_reach / q.grass_spacing).ceil() as u32
    }

    fn globals(&self, f: &Frame, fov: f32, size: (u32, u32), sun: &crate::globals::Sun) -> Vec<u8> {
        let shared = (&self.grid, self.quality, self.terrain, self.grass_side());
        crate::globals::bytes(f, fov, size, sun, shared)
    }

    /// Draw a frame into `target` (`size` pixels): the scene at the
    /// tier's size of it (`Quality::scene`), scaled up as it is finished.
    pub fn draw(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        f: &Frame,
    ) {
        let (device, queue) = (self.device.clone(), self.queue.clone());
        let size = self.quality.scene(size);
        self.post.fit(&device, &queue, size);
        let mut stats = Stats::default();

        // The lights, gridded about the eye.
        let most = self.quality.lights;
        self.grid
            .build(f.lights, [f.cam.eye[0], f.cam.eye[2]], most);
        stats.lights = self.grid.lights.len() as u32;
        let mut b = std::mem::take(&mut self.bytes);
        let grew = self
            .lists
            .put((&device, &queue), &self.grid, f.look.glow, &mut b);

        // The sun's cascades, and the island's layer (none of them when
        // the sun is down).
        let sun_up = geo::norm(f.look.sun_dir)[1] > 0.02;
        let ends = laws::CASCADES[(self.quality.cascades.clamp(1, 3) - 1) as usize];
        let count = if sun_up {
            (self.quality.cascades as usize).min(ends.len())
        } else {
            0
        };
        let cascades = shadow::fit(
            &f.cam,
            f.look.sun_dir,
            &ends[..count],
            self.quality.shadow_size,
        );
        self.shadows.aim(&queue, &cascades);
        if self.statics_dirty {
            let still = (&self.statics, &self.meshes);
            self.shadows
                .statics((&device, &queue), still.0, still.1, &mut b);
        }
        let island = self
            .shadows
            .island(&queue, sun_up.then_some(f.look.sun_dir));
        let sun = crate::globals::Sun {
            cascades: &cascades,
            island: island.map(|l| (l, self.shadows.island_layer())),
        };
        queue.write_buffer(&self.globals[0], 0, &self.globals(f, f.cam.fov, size, &sun));
        queue.write_buffer(
            &self.globals[1],
            0,
            &self.globals(f, f.view_fov, size, &sun),
        );
        if grew || self.groups.is_none() {
            let group = |globals: &wgpu::Buffer| {
                let (lists, shadows) = (&self.lists, &self.shadows);
                crate::pipes::scene_group(
                    &device,
                    &self.layout,
                    globals,
                    lists,
                    shadows,
                    &self.heights,
                )
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
            self.still_runs = lay(&mut items, &mut b, 0);
            self.still.put(&device, &queue, &b);
        }
        let whole = island.is_some();
        self.cull
            .lay((&device, &queue), &cascades, whole, &self.meshes, &mut b);

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
        let lit = self.statics_lit.clone();
        let mut by: [Vec<&Item>; 4] = Default::default();
        for it in f.items.iter().chain(&lit).chain(sea.iter()) {
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
            runs[p] = lay(&mut by[p], &mut b, base);
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
        self.pipes.decals.put((&device, &queue), f.decals, &mut b);
        self.bytes = b;

        let Some(groups) = &self.groups else {
            return;
        };
        let Some(t) = &self.post.targets else {
            return;
        };
        let meshes = &self.meshes;

        // The sun's shadow, a cascade at a time.
        self.shadows
            .record(encoder, &self.cull, meshes, cascades.len(), &mut stats);

        // The scene: what is solid, then (once it is occluded, if it is)
        // what is see-through or glows.
        let split = self.post.reads_depth();
        if split && self.soft_group.as_ref().is_none_or(|g| g.1 != size) {
            let scene = t.resolve.as_ref().unwrap_or(&self.nothing);
            let group = crate::pipes::soft_group(
                &device,
                &self.pipes.soft_layout,
                &t.depth,
                scene,
                &self.scene_samp,
            );
            self.soft_group = Some((group, size));
        }
        let soft = self.soft_group.as_ref().filter(|_| split).map(|g| &g.0);
        let marks = soft.filter(|_| self.quality.decals && !f.decals.is_empty());
        // The sea mirrors the picture of what is solid (resolved into a
        // copy at the end of the last pass that draws it).
        let ssr = split && self.quality.ssr > 0 && t.resolve.is_some();
        let pipes = &self.pipes;
        let moving = &self.moving.buf;
        let solid = |pass: &mut wgpu::RenderPass, stats: &mut Stats| {
            pass.set_pipeline(&pipes.opaque);
            runs_of(pass, meshes, moving, &runs[Pass::Opaque as usize], stats);
        };
        let lit = |pass: &mut wgpu::RenderPass, stats: &mut Stats| {
            match soft.filter(|_| ssr) {
                Some(group) => {
                    pass.set_pipeline(&pipes.faint_ssr);
                    pass.set_bind_group(1, group, &[]);
                }
                None => pass.set_pipeline(&pipes.faint),
            }
            runs_of(pass, meshes, moving, &runs[Pass::Faint as usize], stats);
            pass.set_pipeline(&pipes.glow);
            runs_of(pass, meshes, moving, &runs[Pass::Glow as usize], stats);
            if !f.sparks.is_empty() {
                match soft {
                    Some(group) => {
                        pass.set_pipeline(&pipes.soft_sparks);
                        pass.set_bind_group(1, group, &[]);
                    }
                    None => pass.set_pipeline(&pipes.sparks),
                }
                pass.set_vertex_buffer(0, self.spark_buf.buf.slice(..));
                pass.draw(0..6, 0..f.sparks.len() as u32);
                stats.draws += 1;
            }
        };
        // What stands still, grass, what moves (unless decals go under
        // it first), then the sky wherever none of it stands (so it is
        // shaded only there); with no depth to read, the rest too.
        {
            let ops = Ops {
                clear: Some(f.look.horizon),
                depth: (true, true),
                resolve: ssr && marks.is_none(),
                done: false,
            };
            let mut pass = t.pass(encoder, "world", ops);
            pass.set_bind_group(0, &groups[0], &[]);
            pass.set_pipeline(&pipes.opaque);
            runs_of(
                &mut pass,
                meshes,
                &self.still.buf,
                &self.still_runs,
                &mut stats,
            );
            let side = self.grass_side();
            if side > 0 {
                pass.set_pipeline(&pipes.grass);
                pass.set_index_buffer(pipes.blade.slice(..), wgpu::IndexFormat::Uint16);
                pass.draw_indexed(0..9, 0, 0..side * side);
                stats.draws += 1;
                stats.grass = side * side;
            }
            if marks.is_none() {
                solid(&mut pass, &mut stats);
            }
            pass.set_pipeline(&pipes.sky);
            pass.draw(0..3, 0..1);
            stats.draws += 1;
            if !split {
                lit(&mut pass, &mut stats);
            }
        }
        // Decals on what stands still (the depth only read), then what
        // moves over them.
        if let Some(group) = marks {
            {
                let mut pass = t.pass(encoder, "marks", Ops::default());
                pass.set_bind_group(0, &groups[0], &[]);
                pipes.decals.draw(&mut pass, group, &mut stats);
            }
            let ops = Ops {
                depth: (false, true),
                resolve: ssr,
                ..Ops::default()
            };
            let mut pass = t.pass(encoder, "moving", ops);
            pass.set_bind_group(0, &groups[0], &[]);
            solid(&mut pass, &mut stats);
        }
        // Occlusion and shafts from the depth; then, over the occluded
        // picture, what is see-through and what glows.
        let mut shafts = false;
        if split {
            shafts = self.post.occlude(encoder, &queue, &f.cam, &f.look);
            let mut pass = t.pass(encoder, "lit", Ops::default());
            self.post.apply(&mut pass);
            pass.set_bind_group(0, &groups[0], &[]);
            lit(&mut pass, &mut stats);
        }
        // The viewmodel, over a cleared depth; the picture resolves here
        // (and its samples are let go).
        {
            let ops = Ops {
                depth: (true, false),
                resolve: true,
                done: true,
                ..Ops::default()
            };
            let mut pass = t.pass(encoder, "view", ops);
            pass.set_bind_group(0, &groups[1], &[]);
            pass.set_pipeline(&pipes.opaque);
            runs_of(
                &mut pass,
                meshes,
                moving,
                &runs[Pass::View as usize],
                &mut stats,
            );
        }
        self.post.run(encoder, &queue, target, &f.look, shafts);
        self.stats = stats;
    }
}
