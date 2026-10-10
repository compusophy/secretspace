//! Ambient occlusion (`shaders::ao`): from the scene's depth once what is
//! solid is drawn, at half the screen's size, blurred, then multiplied
//! into the picture (the first draw of the pass that lays what is
//! see-through and what glows over it, so the many-sampled picture is not
//! read and written by a pass of its own). Where the picture's light is
//! direct (its alpha, as the solid pass writes it), it is eased.

use gpu::wgpu;

use crate::buffers::bytes;
use crate::fullscreen;
use crate::pipes::{pipeline, Kind, Shared};
use crate::post::texture;
use crate::{laws, shaders, Camera, Look};

const AO: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;

pub struct Ao {
    ways: u32,
    layout: wgpu::BindGroupLayout,
    ao: wgpu::RenderPipeline,
    blur: wgpu::RenderPipeline,
    apply: wgpu::RenderPipeline,
    buf: wgpu::Buffer,
    size: (u32, u32),
    /// The raw occlusion, the blurred, and the groups reading each.
    views: Option<[wgpu::TextureView; 2]>,
    groups: Option<[wgpu::BindGroup; 2]>,
    /// The picture of what is solid, as the occlusion reads it (how much
    /// of its light is direct).
    picture_layout: wgpu::BindGroupLayout,
    picture: Option<wgpu::BindGroup>,
}

impl Ao {
    /// Occlusion looking `ways` ways out from each pixel, over a picture
    /// of `msaa` samples.
    pub fn new(device: &wgpu::Device, msaa: u32, ways: u32) -> Ao {
        let layout = crate::slots::layout(device, "ao", &crate::slots::ao(msaa > 1));
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ao"),
            source: wgpu::ShaderSource::Wgsl(shaders::ao::ao(msaa > 1).into()),
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ao"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let picture_layout =
            crate::slots::layout(device, "ao picture", &crate::slots::picture(msaa > 1));
        let half = |layouts: &[&wgpu::BindGroupLayout], entry| {
            fullscreen::pipeline(device, layouts, &module, ("ao_vs", entry), AO, None)
        };
        // The picture times the occlusion; its alpha kept.
        let times = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Zero,
                dst_factor: wgpu::BlendFactor::Src,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Zero,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
        };
        // Over the scene, in its pass: its samples, its depth only read.
        let apply = pipeline(
            &Shared {
                device,
                layout: &pl,
                module: &module,
                samples: msaa,
            },
            Kind {
                entry: ("ao_vs", "apply_fs"),
                buffers: &[],
                blend: Some(times),
                depth: (false, wgpu::CompareFunction::Always),
                cull: None,
            },
        );
        Ao {
            ways,
            ao: half(&[&layout, &picture_layout], "ao_fs"),
            blur: half(&[&layout], "blur_fs"),
            apply,
            layout,
            buf: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("ao"),
                size: 80,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            size: (0, 0),
            views: None,
            groups: None,
            picture_layout,
            picture: None,
        }
    }

    fn half(&self) -> (u32, u32) {
        (
            self.size.0.div_ceil(2).max(1),
            self.size.1.div_ceil(2).max(1),
        )
    }

    /// Made again with the scene's depth and its picture (`size` pixels).
    pub fn fit(
        &mut self,
        device: &wgpu::Device,
        (depth, picture): (&wgpu::TextureView, &wgpu::TextureView),
        size: (u32, u32),
    ) {
        self.size = size;
        self.picture = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ao picture"),
            layout: &self.picture_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(picture),
            }],
        }));
        let half = self.half();
        let views = [
            texture(device, "ao", half, AO, 1, true),
            texture(device, "ao (soft)", half, AO, 1, true),
        ];
        let group = |src: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ao"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(depth),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(src),
                    },
                ],
            })
        };
        // The raw is made reading the soft (unused), the soft from the
        // raw, and the picture from the soft.
        self.groups = Some([group(&views[1]), group(&views[0])]);
        self.views = Some(views);
    }

    /// The occlusion as `cam` sees the scene under `look` (at half size,
    /// blurred), to be laid over it (`apply`).
    pub fn run(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        cam: &Camera,
        look: &Look,
    ) {
        let (Some(views), Some(groups), Some(picture)) = (&self.views, &self.groups, &self.picture)
        else {
            return;
        };
        let f = 1.0 / (cam.fov / 2.0).tan();
        let half = self.half();
        // The camera's axes (how far up each goes), for where a pixel is
        // in the air.
        let (_, right, up) = cam.matrices(cam.fov);
        let fwd = cam.forward();
        let k = [
            f / cam.aspect,
            f,
            laws::NEAR,
            laws::AO_REACH,
            self.size.0 as f32,
            self.size.1 as f32,
            half.0 as f32,
            half.1 as f32,
            laws::AO_POWER,
            laws::AO_SLACK,
            self.ways as f32,
            laws::AO_FADE,
            laws::AO_STRENGTH,
            right[1],
            up[1],
            fwd[1],
            cam.eye[1],
            look.fog,
            look.fog_falloff,
            laws::AO_DIRECT,
        ];
        queue.write_buffer(&self.buf, 0, &bytes(&k));
        let white = wgpu::LoadOp::Clear(wgpu::Color::WHITE);
        for (view, pipe, group, reads) in [
            (&views[0], &self.ao, &groups[0], Some(picture)),
            (&views[1], &self.blur, &groups[1], None),
        ] {
            let mut rp = fullscreen::pass(encoder, "ao", view, white);
            rp.set_pipeline(pipe);
            rp.set_bind_group(0, group, &[]);
            if let Some(picture) = reads {
                rp.set_bind_group(1, picture, &[]);
            }
            rp.draw(0..3, 0..1);
        }
    }

    /// Lay the occlusion over the picture: the first draw of a scene pass
    /// (its group 0 is then this one's: set the scene's again after).
    pub fn apply(&self, pass: &mut wgpu::RenderPass) {
        if let Some(groups) = &self.groups {
            pass.set_pipeline(&self.apply);
            pass.set_bind_group(0, &groups[0], &[]);
            pass.draw(0..3, 0..1);
        }
    }
}
