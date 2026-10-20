//! Pictures read back from the GPU: a ring of up to three buffers, each
//! a picture copied out of a texture (rows padded to 256 bytes, as a copy
//! must be), mapped once the copy is submitted, read a frame or two later.
//! The newest that came back wins; the rest are let go unread.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

/// The most pictures in flight at once.
const SLOTS: usize = 3;

pub(crate) struct Readback {
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
    state: Arc<AtomicU8>,
}

impl Readback {
    pub(crate) fn new() -> Readback {
        Readback {
            slots: Vec::new(),
            pending: None,
        }
    }

    /// A buffer for this frame's picture, `size` pixels: false while every
    /// one is still being read (skip a frame).
    pub(crate) fn take(&mut self, device: &wgpu::Device, size: (u32, u32)) -> bool {
        let stride = (size.0 * 4).div_ceil(256) * 256;
        let free = self.slots.iter().position(|s| {
            s.state.load(Ordering::Acquire) == 0 && s.size == size && s.stride == stride
        });
        let k = match free {
            Some(k) => k,
            None if self.slots.len() < SLOTS
                || self
                    .slots
                    .iter()
                    .any(|s| s.state.load(Ordering::Acquire) == 0) =>
            {
                // A new buffer (or one of the wrong size made anew).
                let buf = device.create_buffer(&wgpu::BufferDescriptor {
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
                    Some(k) if self.slots.len() >= SLOTS => {
                        self.slots[k] = slot;
                        k
                    }
                    _ => {
                        self.slots.push(slot);
                        self.slots.len() - 1
                    }
                }
            }
            None => return false,
        };
        self.pending = Some(k);
        true
    }

    /// The frame drawn into `tex` (`size` pixels): copied out to the buffer
    /// `take` gave, submitted with everything `encoder` recorded, mapped.
    pub(crate) fn end(
        &mut self,
        queue: &wgpu::Queue,
        mut encoder: wgpu::CommandEncoder,
        tex: &wgpu::Texture,
        size: (u32, u32),
    ) {
        let Some(k) = self.pending.take() else {
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
        queue.submit([encoder.finish()]);
        slot.state.store(1, Ordering::Release);
        let state = slot.state.clone();
        slot.buf.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            state.store(if r.is_ok() { 2 } else { 0 }, Ordering::Release);
        });
    }

    /// The newest picture read back, into `into` (sized to it, rows packed);
    /// with `opaque`, every alpha 255. False if none has come since the last.
    pub(crate) fn read(&mut self, into: &mut pixels::Canvas, opaque: bool) -> bool {
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
                    unpad(
                        &data,
                        s.stride as usize,
                        (w as usize, h as usize),
                        &mut into.data,
                    );
                }
                if opaque {
                    for px in into.data.chunks_exact_mut(4) {
                        px[3] = 255;
                    }
                }
            }
            s.buf.unmap();
            s.state.store(0, Ordering::Release);
        }
        true
    }
}

/// A texture to draw a picture into and copy it out of, `size` pixels.
pub(crate) fn target(
    device: &wgpu::Device,
    size: (u32, u32),
    format: wgpu::TextureFormat,
) -> (wgpu::Texture, wgpu::TextureView, (u32, u32)) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("offscreen"),
        size: wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = tex.create_view(&Default::default());
    (tex, view, size)
}

/// Rows `stride` bytes apart in `from` as packed rows of `size.0` pixels.
fn unpad(from: &[u8], stride: usize, size: (usize, usize), into: &mut [u8]) {
    let row = size.0 * 4;
    for y in 0..size.1 {
        let at = y * stride;
        into[y * row..(y + 1) * row].copy_from_slice(&from[at..at + row]);
    }
}

#[cfg(test)]
mod tests {
    use super::unpad;

    #[test]
    fn rows_come_out_packed() {
        // Two rows of three pixels, each row padded to 256 bytes.
        let mut from = vec![0u8; 512];
        for (y, at) in [(0u8, 0usize), (1, 256)] {
            for b in 0..12 {
                from[at + b] = y * 100 + b as u8;
            }
        }
        let mut into = vec![9u8; 24];
        unpad(&from, 256, (3, 2), &mut into);
        assert_eq!(&into[..12], &(0..12).collect::<Vec<u8>>()[..]);
        assert_eq!(&into[12..], &(100..112).collect::<Vec<u8>>()[..]);
    }
}
