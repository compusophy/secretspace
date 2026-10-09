//! Drawing off any screen, read back as pixels.

use crate::{wanted, Caps};

/// Drawing off any screen: a device of its own, a target `size` pixels,
/// and the picture read back as pixels (a frame or two behind), for a page
/// that is not WebGPU itself to show a picture the engine drew (the hub's
/// card of a 3D match).
pub struct Offscreen {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub caps: Caps,
    target: Option<(wgpu::Texture, wgpu::TextureView, (u32, u32))>,
    slots: Vec<Slot>,
    /// The slot this frame copies into.
    pending: Option<usize>,
}

/// A buffer the picture is copied into, then read: 0 free, 1 being
/// mapped, 2 ready to read.
struct Slot {
    buf: wgpu::Buffer,
    size: (u32, u32),
    stride: u32,
    state: std::sync::Arc<std::sync::atomic::AtomicU8>,
}

impl Offscreen {
    /// The colour format to draw in (what `Renderer::new` must be given).
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

    pub async fn new() -> Result<Offscreen, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                ..Default::default()
            })
            .await
            .map_err(|e| format!("no adapter: {e}"))?;
        let features = adapter.features() & wanted();
        let limits = adapter.limits();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("offscreen"),
                required_features: features,
                required_limits: limits.clone(),
                ..Default::default()
            })
            .await
            .map_err(|e| format!("no device: {e}"))?;
        let info = adapter.get_info();
        let caps = Caps {
            adapter: info.name.clone(),
            software: info.device_type == wgpu::DeviceType::Cpu,
            features: device.features(),
            limits,
        };
        Ok(Offscreen {
            device,
            queue,
            caps,
            target: None,
            slots: Vec::new(),
            pending: None,
        })
    }

    /// Somewhere to draw this frame, `size` pixels, and an encoder; None
    /// while every buffer is still being read (skip a frame).
    pub fn begin(&mut self, size: (u32, u32)) -> Option<(wgpu::TextureView, wgpu::CommandEncoder)> {
        use std::sync::atomic::Ordering;
        let size = (size.0.max(1), size.1.max(1));
        if self.target.as_ref().map(|t| t.2) != Some(size) {
            let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("offscreen"),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: Self::FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = tex.create_view(&Default::default());
            self.target = Some((tex, view, size));
        }
        let stride = (size.0 * 4).div_ceil(256) * 256;
        let free = self.slots.iter().position(|s| {
            s.state.load(Ordering::Acquire) == 0 && s.size == size && s.stride == stride
        });
        let k = match free {
            Some(k) => k,
            None if self.slots.len() < 3
                || self
                    .slots
                    .iter()
                    .any(|s| s.state.load(Ordering::Acquire) == 0) =>
            {
                // A new buffer (or one of the wrong size made anew).
                let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("readback"),
                    size: (stride * size.1) as u64,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                });
                let slot = Slot {
                    buf,
                    size,
                    stride,
                    state: Default::default(),
                };
                match self
                    .slots
                    .iter()
                    .position(|s| s.state.load(Ordering::Acquire) == 0)
                {
                    Some(k) if self.slots.len() >= 3 => {
                        self.slots[k] = slot;
                        k
                    }
                    _ => {
                        self.slots.push(slot);
                        self.slots.len() - 1
                    }
                }
            }
            None => return None,
        };
        self.pending = Some(k);
        let view = self.target.as_ref().map(|t| t.1.clone())?;
        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("offscreen"),
            });
        Some((view, encoder))
    }

    /// The frame drawn: copied out to be read.
    pub fn end(&mut self, mut encoder: wgpu::CommandEncoder) {
        use std::sync::atomic::Ordering;
        let (Some(k), Some((tex, _, size))) = (self.pending.take(), self.target.as_ref()) else {
            return;
        };
        let slot = &self.slots[k];
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &slot.buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(slot.stride),
                    rows_per_image: Some(size.1),
                },
            },
            wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        slot.state.store(1, Ordering::Release);
        let state = slot.state.clone();
        slot.buf.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            state.store(if r.is_ok() { 2 } else { 0 }, Ordering::Release);
        });
    }

    /// The newest picture read back, into `into` (sized to it); false if
    /// none has come since the last.
    pub fn read(&mut self, into: &mut pixels::Canvas) -> bool {
        use std::sync::atomic::Ordering;
        let ready: Vec<usize> = (0..self.slots.len())
            .filter(|&k| self.slots[k].state.load(Ordering::Acquire) == 2)
            .collect();
        let Some(&newest) = ready.last() else {
            return false;
        };
        for &k in &ready {
            let s = &self.slots[k];
            if k == newest {
                let (w, h) = s.size;
                into.resize(w as i32, h as i32);
                if let Ok(data) = s.buf.slice(..).get_mapped_range() {
                    let row = (w * 4) as usize;
                    for y in 0..h as usize {
                        let from = y * s.stride as usize;
                        into.data[y * row..(y + 1) * row].copy_from_slice(&data[from..from + row]);
                    }
                }
            }
            s.buf.unmap();
            s.state.store(0, Ordering::Release);
        }
        true
    }
}
