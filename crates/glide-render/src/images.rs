//! Image textures: decoded on background threads, uploaded with a full
//! mip chain so images stay clean when zoomed far out, and evicted when unused.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};

/// Images larger than this (per side) are downscaled before upload.
const MAX_SIDE: u32 = 8192;

pub(crate) enum ImageState {
    Loading,
    Ready { bind_group: wgpu::BindGroup, last_used: u64 },
    Failed,
}

struct Decoded {
    path: PathBuf,
    result: Result<Vec<image::RgbaImage>, String>,
}

pub(crate) struct ImageCache {
    entries: HashMap<PathBuf, ImageState>,
    tx: Sender<Decoded>,
    rx: Receiver<Decoded>,
    pub layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    pending: usize,
}

impl ImageCache {
    pub fn new(device: &wgpu::Device) -> Self {
        let (tx, rx) = channel();
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("glide image bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("glide image sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 16,
            ..Default::default()
        });
        ImageCache { entries: HashMap::new(), tx, rx, layout, sampler, pending: 0 }
    }

    pub fn is_loading(&self) -> bool {
        self.pending > 0
    }

    /// Returns the state for `path`, starting a background decode on first use.
    pub fn request(&mut self, path: &Path, frame: u64) -> &ImageState {
        if !self.entries.contains_key(path) {
            self.entries.insert(path.to_path_buf(), ImageState::Loading);
            self.pending += 1;
            let tx = self.tx.clone();
            let path = path.to_path_buf();
            std::thread::spawn(move || {
                let result = decode(&path);
                let _ = tx.send(Decoded { path, result });
            });
        }
        let state = self.entries.get_mut(path).expect("inserted above");
        if let ImageState::Ready { last_used, .. } = state {
            *last_used = frame;
        }
        state
    }

    /// Uploads finished decodes. Returns true if anything changed.
    pub fn poll(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, frame: u64) -> bool {
        let mut changed = false;
        while let Ok(d) = self.rx.try_recv() {
            self.pending = self.pending.saturating_sub(1);
            changed = true;
            let state = match d.result {
                Ok(mips) => self.upload(device, queue, &mips, frame),
                Err(e) => {
                    log::warn!("could not load {}: {e}", d.path.display());
                    ImageState::Failed
                }
            };
            self.entries.insert(d.path, state);
        }
        changed
    }

    /// Blocks until every requested image has finished decoding.
    pub fn wait(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, frame: u64) {
        while self.pending > 0 {
            match self.rx.recv() {
                Ok(d) => {
                    self.pending -= 1;
                    let state = match d.result {
                        Ok(mips) => self.upload(device, queue, &mips, frame),
                        Err(_) => ImageState::Failed,
                    };
                    self.entries.insert(d.path, state);
                }
                Err(_) => break,
            }
        }
    }

    pub fn evict_unused(&mut self, frame: u64, max_age: u64) {
        self.entries.retain(|_, s| match s {
            ImageState::Ready { last_used, .. } => frame.saturating_sub(*last_used) < max_age,
            _ => true,
        });
    }

    /// Forgets failures so edited/added files are retried.
    pub fn retry_failed(&mut self) {
        self.entries.retain(|_, s| !matches!(s, ImageState::Failed));
    }

    /// A bind group for any texture view, usable with the image pipeline.
    pub fn bind_group(&self, device: &wgpu::Device, view: &wgpu::TextureView) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("glide texture bind group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        })
    }

    fn upload(&self, device: &wgpu::Device, queue: &wgpu::Queue, mips: &[image::RgbaImage], frame: u64) -> ImageState {
        let (w, h) = mips[0].dimensions();
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glide image"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: mips.len() as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (level, img) in mips.iter().enumerate() {
            let (mw, mh) = img.dimensions();
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                img.as_raw(),
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * mw), rows_per_image: Some(mh) },
                wgpu::Extent3d { width: mw, height: mh, depth_or_array_layers: 1 },
            );
        }
        let view = texture.create_view(&Default::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("glide image bind group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        });
        ImageState::Ready { bind_group, last_used: frame }
    }
}

/// Decodes an image file and builds its mip chain on the calling thread.
fn decode(path: &Path) -> Result<Vec<image::RgbaImage>, String> {
    let img = image::ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .decode()
        .map_err(|e| e.to_string())?;
    let mut base = img.to_rgba8();
    let (w, h) = base.dimensions();
    if w.max(h) > MAX_SIDE {
        let k = MAX_SIDE as f64 / w.max(h) as f64;
        let (nw, nh) = (((w as f64 * k).round() as u32).max(1), ((h as f64 * k).round() as u32).max(1));
        base = image::imageops::resize(&base, nw, nh, image::imageops::FilterType::Lanczos3);
    }
    let mut mips = vec![base];
    loop {
        let (w, h) = mips.last().expect("non-empty").dimensions();
        if w == 1 && h == 1 {
            break;
        }
        let next = image::imageops::resize(
            mips.last().expect("non-empty"),
            (w / 2).max(1),
            (h / 2).max(1),
            image::imageops::FilterType::Triangle,
        );
        mips.push(next);
    }
    Ok(mips)
}

/// Reads only the pixel dimensions of an image file.
pub fn image_dimensions(path: &Path) -> Option<(u32, u32)> {
    image::ImageReader::open(path).ok()?.with_guessed_format().ok()?.into_dimensions().ok()
}
