//! Drawing off any screen, read back as pixels.

use crate::readback::{target, Readback};
use crate::{wanted, Caps, Health};

/// Drawing off any screen: a device of its own, a target `size` pixels,
/// and the picture read back as pixels (a frame or two behind), for a page
/// that is not WebGPU itself to show a picture the engine drew (the hub's
/// card of a 3D match).
pub struct Offscreen {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub caps: Caps,
    /// Whether the device is lost (nothing is drawn then).
    pub health: Health,
    target: Option<(wgpu::Texture, wgpu::TextureView, (u32, u32))>,
    ring: Readback,
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
            health: Health::watch(&device),
            device,
            queue,
            caps,
            target: None,
            ring: Readback::new(),
        })
    }

    /// Somewhere to draw this frame, `size` pixels, and an encoder; None
    /// while every buffer is still being read (skip a frame), or once the
    /// device is lost.
    pub fn begin(&mut self, size: (u32, u32)) -> Option<(wgpu::TextureView, wgpu::CommandEncoder)> {
        if self.health.lost() {
            return None;
        }
        let size = (size.0.max(1), size.1.max(1));
        if self.target.as_ref().map(|t| t.2) != Some(size) {
            self.target = Some(target(&self.device, size, Self::FORMAT));
        }
        if !self.ring.take(&self.device, size) {
            return None;
        }
        let view = self.target.as_ref().map(|t| t.1.clone())?;
        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("offscreen"),
            });
        Some((view, encoder))
    }

    /// The frame drawn: copied out to be read.
    pub fn end(&mut self, encoder: wgpu::CommandEncoder) {
        if let Some((tex, _, size)) = self.target.as_ref() {
            self.ring.end(&self.queue, encoder, tex, *size);
        }
    }

    /// The newest picture read back, into `into` (sized to it); false if
    /// none has come since the last.
    pub fn read(&mut self, into: &mut pixels::Canvas) -> bool {
        self.ring.read(into, false)
    }
}
