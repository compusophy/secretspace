//! After the scene: its targets (an HDR picture, multisampled and
//! resolved, and its depth), ambient occlusion (`ao`), bloom (what of the
//! picture is past white, halved again and again, then back up, each step
//! a little wider, added together, the finer weighing more, and averaged:
//! a glow about what is bright, not a veil over all of it), and the
//! finish into the screen (the glow added, exposure, tone mapping, a
//! vignette, sRGB, the grade).

use gpu::wgpu;

use crate::ao::Ao;
use crate::buffers::bytes;
use crate::fullscreen;
use crate::shafts::{Shafts, SHAFT};
use crate::{laws, shaders, Camera, Look, Quality};

pub const HDR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// Bloom's chain, where the device can draw into it: half the bytes of
/// `HDR` (no alpha, which bloom never reads).
const BLOOM_SMALL: wgpu::TextureFormat = wgpu::TextureFormat::Rg11b10Ufloat;
pub const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Where the scene draws: the colour (multisampled, or the picture
/// itself), what it resolves into, and its depth.
pub struct Targets {
    pub color: wgpu::TextureView,
    pub resolve: Option<wgpu::TextureView>,
    pub depth: wgpu::TextureView,
}

/// What a pass drawing the scene does with its targets.
#[derive(Clone, Copy, Default)]
pub struct Ops {
    /// The colour cleared to this, or kept.
    pub clear: Option<[f32; 3]>,
    /// The depth cleared (to infinity) or kept, and stored after it or
    /// not (neither: only read, so the pass's shaders may read it too).
    pub depth: (bool, bool),
    /// The picture resolved (its samples to one) at its end.
    pub resolve: bool,
    /// The many-sampled colour let go at its end (it was resolved, and
    /// nothing reads it again): never when the colour is the picture.
    pub done: bool,
}

impl Targets {
    /// A pass drawing the scene, its targets treated as `ops` says.
    pub fn pass<'a>(
        &self,
        encoder: &'a mut wgpu::CommandEncoder,
        label: &str,
        ops: Ops,
    ) -> wgpu::RenderPass<'a> {
        let Ops {
            clear,
            depth,
            resolve,
            done,
        } = ops;
        let load = match clear {
            Some(c) => wgpu::LoadOp::Clear(wgpu::Color {
                r: c[0] as f64,
                g: c[1] as f64,
                b: c[2] as f64,
                a: 1.0,
            }),
            None => wgpu::LoadOp::Load,
        };
        let (clear_depth, keep_depth) = depth;
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(label),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.color,
                depth_slice: None,
                resolve_target: if resolve { self.resolve.as_ref() } else { None },
                ops: wgpu::Operations {
                    load,
                    store: if done && resolve && self.resolve.is_some() {
                        wgpu::StoreOp::Discard
                    } else {
                        wgpu::StoreOp::Store
                    },
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth,
                depth_ops: (clear_depth || keep_depth).then_some(wgpu::Operations {
                    load: if clear_depth {
                        wgpu::LoadOp::Clear(0.0)
                    } else {
                        wgpu::LoadOp::Load
                    },
                    store: if keep_depth {
                        wgpu::StoreOp::Store
                    } else {
                        wgpu::StoreOp::Discard
                    },
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        })
    }
}

struct Pass {
    group: wgpu::BindGroup,
    target: usize,
    up: bool,
}

pub struct Post {
    msaa: u32,
    levels: u32,
    /// Bloom's chain's format.
    bloom: wgpu::TextureFormat,
    /// Whether the scene's depth is read once what is solid is drawn.
    reads: bool,
    layout: wgpu::BindGroupLayout,
    finish_layout: wgpu::BindGroupLayout,
    down: wgpu::RenderPipeline,
    up: wgpu::RenderPipeline,
    finish: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    size: (u32, u32),
    pub targets: Option<Targets>,
    chain: Vec<wgpu::TextureView>,
    passes: Vec<Pass>,
    /// The first halving's numbers (its exposure is the frame's), and
    /// one texel of what it halves.
    first: Option<(wgpu::Buffer, [f32; 2])>,
    finish_group: Option<wgpu::BindGroup>,
    finish_buf: wgpu::Buffer,
    grade_buf: wgpu::Buffer,
    ao: Option<Ao>,
    shafts: Option<Shafts>,
    /// No shafts: a texel of nothing for the finish.
    dark: wgpu::TextureView,
}

pub(crate) fn texture(
    device: &wgpu::Device,
    label: &str,
    size: (u32, u32),
    format: wgpu::TextureFormat,
    samples: u32,
    sampled: bool,
) -> wgpu::TextureView {
    let mut usage = wgpu::TextureUsages::RENDER_ATTACHMENT;
    if sampled {
        usage |= wgpu::TextureUsages::TEXTURE_BINDING;
    }
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: size.0.max(1),
                height: size.1.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: samples,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

fn uniform(device: &wgpu::Device, queue: &wgpu::Queue, v: [f32; 8]) -> wgpu::Buffer {
    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("post"),
        size: 32,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&buf, 0, &bytes(&v));
    buf
}

impl Post {
    /// Post-processing into targets of `out` format, doing as much as
    /// `quality` says (occlusion, shafts, bloom's depth).
    pub fn new(device: &wgpu::Device, out: wgpu::TextureFormat, quality: Quality) -> Post {
        let (msaa, levels) = (quality.msaa, quality.bloom_levels);
        let (ao, shafts) = (quality.ao, quality.shafts);
        let small = wgpu::Features::RG11B10UFLOAT_RENDERABLE;
        let bloom = if device.features().contains(small) {
            BLOOM_SMALL
        } else {
            HDR
        };
        let layout = crate::slots::layout(device, "post", &crate::slots::POST);
        let finish_layout = crate::slots::layout(device, "finish", &crate::slots::FINISH);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("post"),
            source: wgpu::ShaderSource::Wgsl(shaders::post().into()),
        });
        let pipe = |layout: &wgpu::BindGroupLayout, entry: &str, format, blend| {
            fullscreen::pipeline(
                device,
                &[layout],
                &module,
                ("post_vs", entry),
                format,
                blend,
            )
        };
        let add = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::OVER,
        };
        Post {
            msaa,
            levels: levels.max(1),
            reads: ao > 0 || shafts || quality.ssr > 0 || quality.decals,
            down: pipe(&layout, "down_fs", bloom, None),
            up: pipe(&layout, "up_fs", bloom, Some(add)),
            bloom,
            finish: pipe(&finish_layout, "finish_fs", out, None),
            layout,
            finish_layout,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("post"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            size: (0, 0),
            targets: None,
            chain: Vec::new(),
            passes: Vec::new(),
            first: None,
            finish_group: None,
            finish_buf: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("finish"),
                size: 32,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            grade_buf: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("grade"),
                size: 48,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            ao: (ao > 0).then(|| Ao::new(device, msaa, ao)),
            shafts: shafts.then(|| Shafts::new(device, msaa)),
            dark: texture(device, "dark", (1, 1), SHAFT, 1, true),
        }
    }

    /// Whether the scene's depth is read once what is solid is drawn
    /// (occlusion, shafts, the sea's reflections, decals), before what
    /// glows: the scene is then drawn in passes that may read it.
    pub fn reads_depth(&self) -> bool {
        self.reads
    }

    /// From the depth of what is solid: ambient occlusion (laid over the
    /// picture by `apply`), and the sun's shafts; whether they show (for
    /// the finish: `run`).
    pub fn occlude(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        cam: &Camera,
        look: &Look,
    ) -> bool {
        if let Some(ao) = &self.ao {
            ao.run(encoder, queue, cam, look);
        }
        self.shafts
            .as_ref()
            .is_some_and(|sh| sh.run(encoder, queue, cam, look) > 0.0)
    }

    /// The occlusion laid over the picture, first in a scene pass that
    /// only reads the depth (its group 0 left as the occlusion's).
    pub fn apply(&self, pass: &mut wgpu::RenderPass) {
        if let Some(ao) = &self.ao {
            ao.apply(pass);
        }
    }

    /// Targets the size of the screen (made again when it changes).
    pub fn fit(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, size: (u32, u32)) {
        if self.size == size && self.targets.is_some() {
            return;
        }
        self.size = size;
        let hdr = texture(device, "hdr", size, HDR, 1, true);
        let read = self.reads;
        let depth = texture(device, "depth", size, DEPTH, self.msaa, read);
        if let Some(sh) = &mut self.shafts {
            sh.fit(device, &depth, size);
        }
        // The many-sampled picture is read only by the occlusion (how much
        // of its light is direct).
        let targets = if self.msaa > 1 {
            let read = self.ao.is_some();
            Targets {
                color: texture(device, "hdr (msaa)", size, HDR, self.msaa, read),
                resolve: Some(hdr.clone()),
                depth,
            }
        } else {
            Targets {
                color: hdr.clone(),
                resolve: None,
                depth,
            }
        };
        if let Some(ao) = &mut self.ao {
            ao.fit(device, (&targets.depth, &targets.color), size);
        }
        let sizes: Vec<(u32, u32)> = (1..=self.levels)
            .map(|l| ((size.0 >> l).max(1), (size.1 >> l).max(1)))
            .collect();
        self.chain = sizes
            .iter()
            .map(|&s| texture(device, "bloom", s, self.bloom, 1, true))
            .collect();
        let group = |src: &wgpu::TextureView, buf: &wgpu::Buffer| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("post"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(src),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: buf.as_entire_binding(),
                    },
                ],
            })
        };
        let mut passes = Vec::new();
        for l in 0..self.levels as usize {
            let (src, s) = if l == 0 {
                (&hdr, size)
            } else {
                (&self.chain[l - 1], sizes[l - 1])
            };
            // The first halving weighs bright specks down (so a spark
            // crossing a pixel does not make the bloom pulse) and keeps
            // only what is past white (its exposure written each frame).
            let first = if l == 0 { 1.0 } else { 0.0 };
            let buf = uniform(
                device,
                queue,
                [
                    1.0 / s.0 as f32,
                    1.0 / s.1 as f32,
                    0.0,
                    0.0,
                    first,
                    1.0,
                    0.0,
                    1.0,
                ],
            );
            if l == 0 {
                self.first = Some((buf.clone(), [1.0 / s.0 as f32, 1.0 / s.1 as f32]));
            }
            passes.push(Pass {
                group: group(src, &buf),
                target: l,
                up: false,
            });
        }
        for l in (0..self.levels as usize - 1).rev() {
            let s = sizes[l + 1];
            // Each wider level added at a share of the finer one.
            let buf = uniform(
                device,
                queue,
                [
                    1.0 / s.0 as f32,
                    1.0 / s.1 as f32,
                    0.0,
                    0.0,
                    laws::BLOOM_FALLOFF,
                    0.0,
                    0.0,
                    1.0,
                ],
            );
            passes.push(Pass {
                group: group(&self.chain[l + 1], &buf),
                target: l,
                up: true,
            });
        }
        self.passes = passes;
        self.finish_group = Some(
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("finish"),
                layout: &self.finish_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&hdr),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.finish_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&self.chain[0]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: self.grade_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::TextureView(
                            self.shafts
                                .as_ref()
                                .and_then(|s| s.view.as_ref())
                                .unwrap_or(&self.dark),
                        ),
                    },
                ],
            }),
        );
        self.targets = Some(targets);
    }

    /// Bloom, then the finish into `out` (the sun's shafts added if they
    /// were drawn this frame).
    pub fn run(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        out: &wgpu::TextureView,
        look: &Look,
        shafts: bool,
    ) {
        let sun = if shafts { look.sun } else { [0.0; 3] };
        // The levels added (each a share of the one before) come to this
        // many pictures: the finish divides by it.
        let all: f32 = (0..self.levels)
            .map(|l| laws::BLOOM_FALLOFF.powi(l as i32))
            .sum();
        let k = [
            sun[0],
            sun[1],
            sun[2],
            0.0,
            look.bloom,
            look.exposure,
            look.vignette,
            1.0 / all,
        ];
        queue.write_buffer(&self.finish_buf, 0, &bytes(&k));
        // What is past white is judged as the picture will be exposed.
        if let Some((buf, texel)) = &self.first {
            let first = [texel[0], texel[1], 0.0, 0.0, 1.0, look.exposure, 0.0, 1.0];
            queue.write_buffer(buf, 0, &bytes(&first));
        }
        let g = &look.grade;
        let grade = [
            g.lift[0],
            g.lift[1],
            g.lift[2],
            g.saturation,
            g.gamma[0],
            g.gamma[1],
            g.gamma[2],
            g.contrast,
            g.gain[0],
            g.gain[1],
            g.gain[2],
            0.0,
        ];
        queue.write_buffer(&self.grade_buf, 0, &bytes(&grade));
        for p in &self.passes {
            let load = if p.up {
                wgpu::LoadOp::Load
            } else {
                wgpu::LoadOp::Clear(wgpu::Color::BLACK)
            };
            let mut rp = fullscreen::pass(encoder, "bloom", &self.chain[p.target], load);
            rp.set_pipeline(if p.up { &self.up } else { &self.down });
            rp.set_bind_group(0, &p.group, &[]);
            rp.draw(0..3, 0..1);
        }
        if let Some(group) = &self.finish_group {
            let mut rp = fullscreen::pass(
                encoder,
                "finish",
                out,
                wgpu::LoadOp::Clear(wgpu::Color::BLACK),
            );
            rp.set_pipeline(&self.finish);
            rp.set_bind_group(0, group, &[]);
            rp.draw(0..3, 0..1);
        }
    }
}
