//! Window-less rendering: a standalone GPU device plus texture readback.
//! Used by `glidedeck render`, and the foundation for PDF and video export.

use crate::{CanvasRenderer, RenderOptions, RenderTarget};
use glam::DVec2;
use glide_core::{Aabb, Camera, Document};

#[derive(Debug, thiserror::Error)]
pub enum HeadlessError {
    #[error("no suitable GPU adapter: {0}")]
    Adapter(String),
    #[error("failed to create GPU device: {0}")]
    Device(String),
    #[error("GPU readback failed: {0}")]
    Readback(String),
}

pub struct HeadlessRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pub canvas: CanvasRenderer,
}

impl HeadlessRenderer {
    pub fn new() -> Result<Self, HeadlessError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
            ..Default::default()
        }))
        .map_err(|e| HeadlessError::Adapter(e.to_string()))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("glidedeck headless"),
            ..Default::default()
        }))
        .map_err(|e| HeadlessError::Device(e.to_string()))?;
        let canvas = CanvasRenderer::new(&device, &queue);
        Ok(HeadlessRenderer { device, queue, canvas })
    }

    /// World bounds of all visible elements.
    pub fn document_bounds(&mut self, doc: &Document) -> Aabb {
        doc.elements
            .iter()
            .filter(|e| !e.hidden)
            .fold(Aabb::EMPTY, |b, e| b.union(e.world_aabb(self.canvas.local_size(e))))
    }

    /// Renders one frame and returns tightly packed sRGB RGBA8 pixels.
    pub fn render_rgba(
        &mut self,
        doc: &Document,
        camera: &Camera,
        size: [u32; 2],
        show_grid: bool,
    ) -> Result<Vec<u8>, HeadlessError> {
        let target = RenderTarget::new(&self.device, size);
        let viewport = DVec2::new(size[0] as f64, size[1] as f64);
        let opts = RenderOptions { show_grid, pixels_per_point: 1.0 };
        self.canvas.load_images_blocking(doc);
        self.canvas.render(&target, doc, camera, viewport, &opts);
        read_back(&self.device, &self.queue, &target)
    }
}

/// Copies a render target back to the CPU as tightly packed sRGB RGBA8.
pub fn read_back(device: &wgpu::Device, queue: &wgpu::Queue, target: &RenderTarget) -> Result<Vec<u8>, HeadlessError> {
    let [w, h] = target.size;
    let unpadded = w * 4;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded = unpadded.div_ceil(align) * align;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("glide readback"),
        size: padded as u64 * h as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target.texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    queue.submit([encoder.finish()]);

    let (tx, rx) = std::sync::mpsc::channel();
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| HeadlessError::Readback(e.to_string()))?;
    rx.recv()
        .map_err(|e| HeadlessError::Readback(e.to_string()))?
        .map_err(|e| HeadlessError::Readback(e.to_string()))?;

    let data = buffer.get_mapped_range(..).map_err(|e| HeadlessError::Readback(e.to_string()))?;
    let mut out = Vec::with_capacity((unpadded * h) as usize);
    for row in data.chunks(padded as usize) {
        out.extend_from_slice(&row[..unpadded as usize]);
    }
    Ok(out)
}
