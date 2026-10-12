//! GPU renderer for the GlideDeck infinite canvas.
//!
//! Design:
//! * Every element is tessellated once into a mesh in its *local* space
//!   (lyon for shapes, glyph outlines for text) and cached on the GPU.
//!   Moving, scaling or rotating an element — or the camera — never
//!   re-tessellates; only the per-instance affine changes.
//! * The affine is composed on the CPU in f64 relative to the camera, which
//!   keeps vertices precise at extreme zoom depths.
//! * Curves are re-tessellated at coarse level-of-detail steps (powers of two
//!   of on-screen scale), so deep zooms stay smooth without faceting.
//! * Images are decoded off-thread into mipmapped textures and drawn as
//!   textured quads, interleaved with meshes in document z-order.
//! * Rendering goes to an offscreen MSAA [`RenderTarget`], so the same path
//!   serves the editor viewport, slide thumbnails, presentation and export.

mod headless;
mod icon_table;
mod images;
pub mod svg;
pub mod video;
mod tessellate;
mod text;

pub use headless::{read_back, HeadlessError, HeadlessRenderer};
pub use images::image_dimensions;
pub use tessellate::{geometry_hash, local_size, Vertex};
pub use icon_table::ICONS;
pub use cosmic_text::{Cursor as TextCursor, Motion as TextMotion};
pub use text::{icon_codepoint, EditAction, EditResult, EditState, PathCmd, TextEngine, TextLayout};

use bytemuck::{Pod, Zeroable};
use glam::{DAffine2, DMat2, DVec2};
use images::{ImageCache, ImageState};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use wgpu::util::DeviceExt;
use glide_core::{assets, Camera, Color, Document, Element, ElementKind};

/// Color format of the offscreen canvas texture.
pub const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
/// 4x MSAA is guaranteed for this format by the WebGPU spec.
pub const SAMPLE_COUNT: u32 = 4;

/// Target accuracy of curve flattening, in physical pixels.
const TOLERANCE_PX: f64 = 0.2;
/// Elements smaller than this on screen (in physical pixels) are culled.
const MIN_VISIBLE_PX: f64 = 0.35;
/// Renders without use before a cached mesh/texture is dropped.
const CACHE_AGE: u64 = 600;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct Instance {
    col_a: [f32; 2],
    col_b: [f32; 2],
    translate: [f32; 2],
    opacity: f32,
    _pad: f32,
    /// Per-kind extras (images: aspect, corner radius / height).
    params: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct ShadowInstance {
    col_a: [f32; 2],
    col_b: [f32; 2],
    translate: [f32; 2],
    half: [f32; 2],
    /// x: corner radius, y: blur, z: 1 for ellipse.
    params: [f32; 4],
    offset: [f32; 2],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct BackdropUniform {
    top: [f32; 4],
    bottom: [f32; 4],
    height: f32,
    _pad: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct GridUniform {
    inv_a: [f32; 2],
    inv_b: [f32; 2],
    origin: [f32; 2],
    half_viewport: [f32; 2],
    fine: f32,
    coarse: f32,
    fine_alpha: f32,
    px_per_world: f32,
    color: [f32; 4],
}

struct GpuMesh {
    /// `None` for geometry that tessellates to nothing (e.g. invisible frames).
    buffers: Option<(wgpu::Buffer, wgpu::Buffer)>,
    index_count: u32,
    last_used: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct MeshKey {
    geometry: u64,
    lod: i32,
}

enum Draw {
    Mesh(MeshKey),
    Image(PathBuf),
    Video(PathBuf),
    /// Index into the shadow instance buffer.
    Shadow(u32),
}

/// An offscreen render target: an MSAA color buffer resolved into a
/// sampleable texture.
pub struct RenderTarget {
    pub size: [u32; 2],
    _msaa: wgpu::Texture,
    msaa_view: wgpu::TextureView,
    pub texture: wgpu::Texture,
    /// sRGB view used as the render/resolve target (linear blending).
    pub view: wgpu::TextureView,
    /// Non-sRGB view of the same pixels, for consumers that expect
    /// gamma-encoded `Rgba8Unorm` textures (egui).
    pub unorm_view: wgpu::TextureView,
}

impl RenderTarget {
    pub fn new(device: &wgpu::Device, size: [u32; 2]) -> Self {
        let size = [size[0].max(1), size[1].max(1)];
        let extent = wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 };
        let msaa = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glide canvas msaa"),
            size: extent,
            mip_level_count: 1,
            sample_count: SAMPLE_COUNT,
            dimension: wgpu::TextureDimension::D2,
            format: TARGET_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glide canvas"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TARGET_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[wgpu::TextureFormat::Rgba8Unorm],
        });
        RenderTarget {
            size,
            msaa_view: msaa.create_view(&Default::default()),
            _msaa: msaa,
            view: texture.create_view(&Default::default()),
            unorm_view: texture.create_view(&wgpu::TextureViewDescriptor {
                format: Some(wgpu::TextureFormat::Rgba8Unorm),
                ..Default::default()
            }),
            texture,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RenderOptions {
    pub show_grid: bool,
    /// Physical pixels per screen point (display scale factor).
    pub pixels_per_point: f64,
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions { show_grid: true, pixels_per_point: 1.0 }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RenderStats {
    pub drawn: usize,
    pub culled: usize,
    pub triangles: usize,
    pub cached_meshes: usize,
}

pub struct CanvasRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    mesh_pipeline: wgpu::RenderPipeline,
    image_pipeline: wgpu::RenderPipeline,
    shadow_pipeline: wgpu::RenderPipeline,
    backdrop_pipeline: wgpu::RenderPipeline,
    backdrop_uniform: wgpu::Buffer,
    backdrop_bind_group: wgpu::BindGroup,
    shadow_instances: wgpu::Buffer,
    shadow_capacity: usize,
    grid_pipeline: wgpu::RenderPipeline,
    grid_uniform: wgpu::Buffer,
    grid_bind_group: wgpu::BindGroup,
    instances: wgpu::Buffer,
    instance_capacity: usize,
    meshes: HashMap<MeshKey, GpuMesh>,
    images: ImageCache,
    svgs: HashMap<PathBuf, Option<std::sync::Arc<svg::SvgDoc>>>,
    videos: HashMap<PathBuf, VideoEntry>,
    video_tx: std::sync::mpsc::Sender<(PathBuf, Result<(video::VideoInfo, u32, u32, Vec<u8>), String>)>,
    video_rx: std::sync::mpsc::Receiver<(PathBuf, Result<(video::VideoInfo, u32, u32, Vec<u8>), String>)>,
    base_dir: Option<PathBuf>,
    frame: u64,
    pub text: TextEngine,
}

impl CanvasRenderer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("glide canvas shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("canvas.wgsl").into()),
        });
        let multisample = wgpu::MultisampleState { count: SAMPLE_COUNT, ..Default::default() };
        let targets = [Some(wgpu::ColorTargetState {
            format: TARGET_FORMAT,
            blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        })];
        let instance_attrs =
            wgpu::vertex_attr_array![2 => Float32x2, 3 => Float32x2, 4 => Float32x2, 5 => Float32];
        let instance_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Instance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &instance_attrs,
        };
        // Explicit offsets: `params` sits after the `_pad` field, which
        // `vertex_attr_array!` (tightly packed) would not account for.
        let attr = |shader_location, format, offset| wgpu::VertexAttribute { format, offset, shader_location };
        let image_instance_attrs = [
            attr(2, wgpu::VertexFormat::Float32x2, std::mem::offset_of!(Instance, col_a) as u64),
            attr(3, wgpu::VertexFormat::Float32x2, std::mem::offset_of!(Instance, col_b) as u64),
            attr(4, wgpu::VertexFormat::Float32x2, std::mem::offset_of!(Instance, translate) as u64),
            attr(5, wgpu::VertexFormat::Float32, std::mem::offset_of!(Instance, opacity) as u64),
            attr(6, wgpu::VertexFormat::Float32x4, std::mem::offset_of!(Instance, params) as u64),
        ];
        let image_instance_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Instance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &image_instance_attrs,
        };
        let shadow_attrs = wgpu::vertex_attr_array![
            0 => Float32x2, 1 => Float32x2, 2 => Float32x2, 3 => Float32x2,
            4 => Float32x4, 5 => Float32x2, 6 => Float32x4
        ];
        let shadow_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ShadowInstance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &shadow_attrs,
        };
        let pipeline = |label: &str,
                        layout: &wgpu::PipelineLayout,
                        vs: &str,
                        fs: &str,
                        buffers: &[Option<wgpu::VertexBufferLayout<'_>>],
                        topology: wgpu::PrimitiveTopology| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers,
                },
                primitive: wgpu::PrimitiveState { topology, ..Default::default() },
                depth_stencil: None,
                multisample,
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &targets,
                }),
                multiview_mask: None,
                cache: None,
            })
        };

        let mesh_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("glide mesh layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        let vertex_attrs = wgpu::vertex_attr_array![0 => Float32x2, 1 => Unorm8x4];
        let mesh_pipeline = pipeline(
            "glide mesh pipeline",
            &mesh_layout,
            "vs_mesh",
            "fs_mesh",
            &[
                Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &vertex_attrs,
                }),
                Some(instance_layout.clone()),
            ],
            wgpu::PrimitiveTopology::TriangleList,
        );

        let images = ImageCache::new(device);
        let image_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("glide image layout"),
            bind_group_layouts: &[Some(&images.layout)],
            immediate_size: 0,
        });
        let image_pipeline = pipeline(
            "glide image pipeline",
            &image_layout,
            "vs_image",
            "fs_image",
            &[None, Some(image_instance_layout)],
            wgpu::PrimitiveTopology::TriangleStrip,
        );
        let shadow_pipeline = pipeline(
            "glide shadow pipeline",
            &mesh_layout,
            "vs_shadow",
            "fs_shadow",
            &[Some(shadow_layout)],
            wgpu::PrimitiveTopology::TriangleStrip,
        );

        let grid_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("glide grid bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let grid_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("glide grid layout"),
            bind_group_layouts: &[Some(&grid_bgl)],
            immediate_size: 0,
        });
        let grid_pipeline = pipeline(
            "glide grid pipeline",
            &grid_layout,
            "vs_grid",
            "fs_grid",
            &[],
            wgpu::PrimitiveTopology::TriangleList,
        );
        let grid_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("glide grid uniform"),
            size: std::mem::size_of::<GridUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let grid_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("glide grid bind group"),
            layout: &grid_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 2, resource: grid_uniform.as_entire_binding() }],
        });

        let backdrop_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("glide backdrop bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let backdrop_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("glide backdrop layout"),
            bind_group_layouts: &[Some(&backdrop_bgl)],
            immediate_size: 0,
        });
        let backdrop_pipeline = pipeline(
            "glide backdrop pipeline",
            &backdrop_layout,
            "vs_grid",
            "fs_backdrop",
            &[],
            wgpu::PrimitiveTopology::TriangleList,
        );
        let backdrop_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("glide backdrop uniform"),
            size: std::mem::size_of::<BackdropUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let backdrop_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("glide backdrop bind group"),
            layout: &backdrop_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 3, resource: backdrop_uniform.as_entire_binding() }],
        });

        let instance_capacity = 256;
        let (video_tx, video_rx) = std::sync::mpsc::channel();
        CanvasRenderer {
            instances: Self::instance_buffer(device, instance_capacity),
            instance_capacity,
            device: device.clone(),
            queue: queue.clone(),
            mesh_pipeline,
            image_pipeline,
            shadow_pipeline,
            backdrop_pipeline,
            backdrop_uniform,
            backdrop_bind_group,
            shadow_instances: Self::sized_buffer::<ShadowInstance>(device, 64),
            shadow_capacity: 64,
            grid_pipeline,
            grid_uniform,
            grid_bind_group,
            meshes: HashMap::new(),
            images,
            svgs: HashMap::new(),
            videos: HashMap::new(),
            video_tx,
            video_rx,
            base_dir: None,
            frame: 0,
            text: TextEngine::new(),
        }
    }

    fn instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
        Self::sized_buffer::<Instance>(device, capacity)
    }

    fn sized_buffer<T>(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("glide instances"),
            size: (capacity * std::mem::size_of::<T>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Directory that relative media paths are resolved against.
    pub fn set_base_dir(&mut self, dir: Option<PathBuf>) {
        if self.base_dir != dir {
            self.base_dir = dir;
            self.images.retry_failed();
            self.svgs.clear();
        }
    }

    pub fn base_dir(&self) -> Option<&Path> {
        self.base_dir.as_deref()
    }

    /// Uploads finished background image decodes. Returns true while images
    /// are still loading (the caller should keep repainting).
    pub fn poll_images(&mut self) -> bool {
        self.images.poll(&self.device, &self.queue, self.frame);
        self.poll_videos();
        self.images.is_loading() || self.videos.values().any(|v| matches!(v, VideoEntry::Loading)) || self.any_video_playing()
    }

    /// Starts loading every image in `doc` and blocks until all are ready.
    pub fn load_images_blocking(&mut self, doc: &Document) {
        for el in &doc.elements {
            if let ElementKind::Image(m) = &el.kind {
                let path = assets::resolve(self.base_dir.as_deref(), &m.path);
                self.images.request(&path, self.frame);
            }
        }
        self.images.wait(&self.device, &self.queue, self.frame);
    }

    /// Renders one view offscreen (waiting for images) and returns sRGB RGBA8 pixels.
    pub fn snapshot(
        &mut self,
        doc: &Document,
        camera: &Camera,
        size: [u32; 2],
        show_grid: bool,
    ) -> Result<Vec<u8>, HeadlessError> {
        self.load_images_blocking(doc);
        let target = RenderTarget::new(&self.device, size);
        let viewport = DVec2::new(size[0] as f64, size[1] as f64);
        self.render(&target, doc, camera, viewport, &RenderOptions { show_grid, pixels_per_point: 1.0 });
        read_back(&self.device, &self.queue, &target)
    }

    // --- video -----------------------------------------------------------------

    fn video_entry(&mut self, reference: &str) -> (PathBuf, &mut VideoEntry) {
        let path = assets::resolve(self.base_dir.as_deref(), reference);
        if !self.videos.contains_key(&path) {
            let tx = self.video_tx.clone();
            let p = path.clone();
            std::thread::spawn(move || {
                let result = video::probe(&p).and_then(|info| {
                    let at = (info.duration * 0.1).min(1.0);
                    let (w, h, rgba) = video::frame_at(&p, &info, at).ok_or("could not decode a frame")?;
                    Ok((info, w, h, rgba))
                });
                let _ = tx.send((p, result));
            });
            self.videos.insert(path.clone(), VideoEntry::Loading);
        }
        let entry = self.videos.get_mut(&path).expect("inserted above");
        (path, entry)
    }

    fn poll_videos(&mut self) -> bool {
        let mut changed = false;
        while let Ok((path, result)) = self.video_rx.try_recv() {
            changed = true;
            let entry = match result {
                Ok((info, w, h, rgba)) => {
                    let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("glide video"),
                        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8UnormSrgb,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });
                    write_rgba(&self.queue, &texture, w, h, &rgba);
                    let view = texture.create_view(&Default::default());
                    let bind_group = self.images.bind_group(&self.device, &view);
                    VideoEntry::Ready { info, texture, bind_group, size: (w, h), player: None }
                }
                Err(e) => VideoEntry::Failed(e),
            };
            self.videos.insert(path, entry);
        }
        changed
    }

    /// Playback state of a video reference.
    pub fn video_status(&mut self, reference: &str) -> VideoStatus {
        self.poll_videos();
        if video::ffmpeg_path().is_none() {
            return VideoStatus::Unavailable("FFmpeg is not installed".into());
        }
        match self.video_entry(reference).1 {
            VideoEntry::Loading => VideoStatus::Loading,
            VideoEntry::Failed(e) => VideoStatus::Unavailable(e.clone()),
            VideoEntry::Ready { info, player, .. } => VideoStatus::Ready {
                playing: player.as_ref().is_some_and(|p| p.is_playing()),
                position: player.as_ref().map(|p| p.position()).unwrap_or(0.0),
                duration: info.duration,
            },
        }
    }

    /// Starts or pauses a video. Returns the new playing state.
    pub fn video_set_playing(&mut self, reference: &str, play: bool, looping: bool, muted: bool) -> bool {
        self.poll_videos();
        let (path, entry) = self.video_entry(reference);
        let VideoEntry::Ready { info, player, .. } = entry else { return false };
        if player.is_none() && play {
            *player = video::Player::new(&path, info.clone());
        }
        let Some(p) = player else { return false };
        p.set_looping(looping);
        p.set_muted(muted);
        if play { p.play() } else { p.pause() }
        play
    }

    pub fn video_restart(&mut self, reference: &str) {
        if let (_, VideoEntry::Ready { player: Some(p), .. }) = self.video_entry(reference) {
            p.restart();
        }
    }

    pub fn any_video_playing(&self) -> bool {
        self.videos.values().any(|v| matches!(v, VideoEntry::Ready { player: Some(p), .. } if p.is_playing()))
    }

    pub fn pause_all_videos(&mut self) {
        for v in self.videos.values_mut() {
            if let VideoEntry::Ready { player: Some(p), .. } = v {
                p.pause();
            }
        }
    }

    /// Local (untransformed) size of an element.
    pub fn local_size(&mut self, el: &Element) -> DVec2 {
        local_size(&mut self.text, &el.kind)
    }

    fn ensure_mesh(&mut self, key: MeshKey, kind: &ElementKind, tolerance: f32) -> &mut GpuMesh {
        if !self.meshes.contains_key(&key) {
            let svg_doc = match kind {
                ElementKind::Svg(m) => {
                    let path = assets::resolve(self.base_dir.as_deref(), &m.path);
                    self.svgs
                        .entry(path.clone())
                        .or_insert_with(|| match svg::load_svg(&path) {
                            Ok(d) => Some(std::sync::Arc::new(d)),
                            Err(e) => {
                                log::warn!("could not load {}: {e}", path.display());
                                None
                            }
                        })
                        .clone()
                }
                _ => None,
            };
            let mesh = match (kind, svg_doc) {
                (ElementKind::Svg(m), Some(doc)) => svg::tessellate_svg(&doc, m.width, m.height, tolerance),
                _ => tessellate::tessellate(&mut self.text, kind, tolerance),
            };
            let buffers = (!mesh.indices.is_empty()).then(|| {
                (
                    self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("glide mesh vertices"),
                        contents: bytemuck::cast_slice(&mesh.vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
                    self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("glide mesh indices"),
                        contents: bytemuck::cast_slice(&mesh.indices),
                        usage: wgpu::BufferUsages::INDEX,
                    }),
                )
            });
            let gpu = GpuMesh { buffers, index_count: mesh.indices.len() as u32, last_used: self.frame };
            self.meshes.insert(key, gpu);
        }
        let m = self.meshes.get_mut(&key).expect("inserted above");
        m.last_used = self.frame;
        m
    }

    /// Renders `doc` as seen by `camera` into `target`.
    ///
    /// `viewport` is the camera's viewport in screen points; the target is
    /// expected to be `viewport * pixels_per_point` physical pixels.
    pub fn render(
        &mut self,
        target: &RenderTarget,
        doc: &Document,
        camera: &Camera,
        viewport: DVec2,
        opts: &RenderOptions,
    ) -> RenderStats {
        self.frame += 1;
        self.images.poll(&self.device, &self.queue, self.frame);
        self.poll_videos();
        let mut stats = RenderStats::default();
        let ppp = opts.pixels_per_point;
        let px_size = DVec2::new(target.size[0] as f64, target.size[1] as f64);
        let screen_px = DAffine2::from_scale(DVec2::splat(ppp)) * camera.view_affine(viewport);
        let to_ndc = DAffine2::from_cols(
            DVec2::new(2.0 / px_size.x, 0.0),
            DVec2::new(0.0, -2.0 / px_size.y),
            DVec2::new(-1.0, 1.0),
        );
        let visible = camera.visible_world_aabb(viewport);

        let mut draws: Vec<(Draw, Instance)> = Vec::new();
        let mut shadows: Vec<ShadowInstance> = Vec::new();
        for el in doc.elements.iter().filter(|e| !e.hidden && e.opacity > 0.0) {
            if let ElementKind::Frame(f) = &el.kind
                && f.is_invisible()
            {
                continue;
            }
            let size = self.local_size(el);
            let aabb = el.world_aabb(size);
            let px_per_local = camera.zoom * ppp * el.transform.scale.abs();
            if !aabb.intersects(&visible) || size.max_element() * px_per_local < MIN_VISIBLE_PX {
                stats.culled += 1;
                continue;
            }
            let model = to_ndc * screen_px * el.transform.affine();
            let opacity = el.opacity.clamp(0.0, 1.0) as f32;

            // Soft shadow beneath shapes, visible frames and images.
            if let Some(sh) = el.shadow {
                let shape = match &el.kind {
                    ElementKind::Rect(r) => Some((r.corner_radius, false)),
                    ElementKind::Frame(f) => Some((f.corner_radius, false)),
                    ElementKind::Image(m) => Some((m.corner_radius, false)),
                    ElementKind::Ellipse(_) => Some((0.0, true)),
                    _ => None,
                };
                if let Some((radius, ellipse)) = shape {
                    let mut color = [sh.color.r() as f32 / 255.0, sh.color.g() as f32 / 255.0, sh.color.b() as f32 / 255.0, sh.color.a() as f32 / 255.0];
                    color[3] *= opacity;
                    let base = instance(model, opacity, [0.0; 4]);
                    shadows.push(ShadowInstance {
                        col_a: base.col_a,
                        col_b: base.col_b,
                        translate: base.translate,
                        half: (size * 0.5).as_vec2().to_array(),
                        params: [radius as f32, sh.blur.max(0.0) as f32, if ellipse { 1.0 } else { 0.0 }, 0.0],
                        offset: [sh.x as f32, sh.y as f32],
                        color,
                    });
                    draws.push((Draw::Shadow(shadows.len() as u32 - 1), base));
                }
            }

            // Images draw as a textured quad once loaded.
            if let ElementKind::Image(m) = &el.kind {
                let path = assets::resolve(self.base_dir.as_deref(), &m.path);
                if let ImageState::Ready { .. } = self.images.request(&path, self.frame) {
                    let quad = model * DAffine2::from_scale(DVec2::new(m.width, m.height));
                    let h = m.height.abs().max(1e-12);
                    let params = [(m.width / h) as f32, (m.corner_radius / h) as f32, 0.0, 0.0];
                    draws.push((Draw::Image(path), instance(quad, opacity, params)));
                    continue;
                }
            }

            // Videos draw their current frame (or poster) as a textured quad.
            if let ElementKind::Video(m) = &el.kind {
                let queue = self.queue.clone();
                let (path, entry) = self.video_entry(&m.path);
                if let VideoEntry::Ready { texture, player, size, .. } = entry {
                    if let Some(p) = player
                        && let Some(rgba) = p.take_frame()
                    {
                        write_rgba(&queue, texture, size.0, size.1, &rgba);
                    }
                    let quad = model * DAffine2::from_scale(DVec2::new(m.width, m.height));
                    let h = m.height.abs().max(1e-12);
                    let params = [(m.width / h) as f32, (m.corner_radius / h) as f32, 0.0, 0.0];
                    draws.push((Draw::Video(path), instance(quad, opacity, params)));
                    continue;
                }
            }

            // Level of detail: tolerance halves every time on-screen scale doubles,
            // bounded so huge elements do not explode the vertex count.
            let lod = px_per_local.log2().ceil().clamp(-60.0, 60.0) as i32;
            let min_tol = size.max_element().max(1e-12) * 2e-6;
            let tolerance = (TOLERANCE_PX / 2f64.powi(lod)).max(min_tol) as f32;
            let key = MeshKey { geometry: geometry_hash(&el.kind), lod };
            if self.ensure_mesh(key, &el.kind, tolerance).buffers.is_some() {
                draws.push((Draw::Mesh(key), instance(model, opacity, [0.0; 4])));
            }
        }

        if draws.len() > self.instance_capacity {
            self.instance_capacity = draws.len().next_power_of_two();
            self.instances = Self::instance_buffer(&self.device, self.instance_capacity);
        }
        let instance_data: Vec<Instance> = draws.iter().map(|(_, i)| *i).collect();
        if !instance_data.is_empty() {
            self.queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&instance_data));
        }
        if shadows.len() > self.shadow_capacity {
            self.shadow_capacity = shadows.len().next_power_of_two();
            self.shadow_instances = Self::sized_buffer::<ShadowInstance>(&self.device, self.shadow_capacity);
        }
        if !shadows.is_empty() {
            self.queue.write_buffer(&self.shadow_instances, 0, bytemuck::cast_slice(&shadows));
        }
        if opts.show_grid {
            let g = grid_uniform(camera, ppp, px_size, doc.background);
            self.queue.write_buffer(&self.grid_uniform, 0, bytemuck::bytes_of(&g));
        }
        if let Some(to) = doc.background_to {
            let f = |c: Color| [c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0, 1.0];
            let u = BackdropUniform { top: f(doc.background), bottom: f(to), height: px_size.y as f32, _pad: [0.0; 3] };
            self.queue.write_buffer(&self.backdrop_uniform, 0, bytemuck::bytes_of(&u));
        }

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("glide canvas"),
        });
        {
            let bg = linear(doc.background);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("glide canvas pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.msaa_view,
                    depth_slice: None,
                    resolve_target: Some(&target.view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: bg[0], g: bg[1], b: bg[2], a: 1.0 }),
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if doc.background_to.is_some() {
                pass.set_pipeline(&self.backdrop_pipeline);
                pass.set_bind_group(0, &self.backdrop_bind_group, &[]);
                pass.draw(0..3, 0..1);
            }
            if opts.show_grid {
                pass.set_pipeline(&self.grid_pipeline);
                pass.set_bind_group(0, &self.grid_bind_group, &[]);
                pass.draw(0..3, 0..1);
            }
            pass.set_vertex_buffer(1, self.instances.slice(..));
            #[derive(PartialEq, Clone, Copy)]
            enum Bound {
                Mesh,
                Image,
                Shadow,
            }
            let mut current: Option<Bound> = None;
            for (i, (draw, _)) in draws.iter().enumerate() {
                let inst = i as u32..i as u32 + 1;
                match draw {
                    Draw::Mesh(key) => {
                        let mesh = &self.meshes[key];
                        let Some((vb, ib)) = &mesh.buffers else { continue };
                        if current != Some(Bound::Mesh) {
                            pass.set_pipeline(&self.mesh_pipeline);
                            pass.set_vertex_buffer(1, self.instances.slice(..));
                            current = Some(Bound::Mesh);
                        }
                        pass.set_vertex_buffer(0, vb.slice(..));
                        pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                        pass.draw_indexed(0..mesh.index_count, 0, inst);
                        stats.triangles += mesh.index_count as usize / 3;
                    }
                    Draw::Image(path) => {
                        let ImageState::Ready { bind_group, .. } = self.images.request(path, self.frame) else {
                            continue;
                        };
                        if current != Some(Bound::Image) {
                            pass.set_pipeline(&self.image_pipeline);
                            pass.set_vertex_buffer(1, self.instances.slice(..));
                            current = Some(Bound::Image);
                        }
                        pass.set_bind_group(0, bind_group, &[]);
                        pass.draw(0..4, inst);
                        stats.triangles += 2;
                    }
                    Draw::Video(path) => {
                        let Some(VideoEntry::Ready { bind_group, .. }) = self.videos.get(path) else { continue };
                        if current != Some(Bound::Image) {
                            pass.set_pipeline(&self.image_pipeline);
                            pass.set_vertex_buffer(1, self.instances.slice(..));
                            current = Some(Bound::Image);
                        }
                        pass.set_bind_group(0, bind_group, &[]);
                        pass.draw(0..4, inst);
                        stats.triangles += 2;
                    }
                    Draw::Shadow(index) => {
                        if current != Some(Bound::Shadow) {
                            pass.set_pipeline(&self.shadow_pipeline);
                            pass.set_vertex_buffer(0, self.shadow_instances.slice(..));
                            current = Some(Bound::Shadow);
                        }
                        pass.draw(0..4, *index..*index + 1);
                        stats.triangles += 2;
                    }
                }
            }
        }
        self.queue.submit([encoder.finish()]);

        // Evict meshes and textures that have not been drawn for a while.
        let frame = self.frame;
        self.meshes.retain(|_, m| frame.saturating_sub(m.last_used) < CACHE_AGE);
        self.images.evict_unused(frame, CACHE_AGE);
        stats.drawn = draws.len();
        stats.cached_meshes = self.meshes.len();
        stats
    }
}

enum VideoEntry {
    Loading,
    Ready {
        info: video::VideoInfo,
        texture: wgpu::Texture,
        bind_group: wgpu::BindGroup,
        size: (u32, u32),
        /// Created on first play (spawns the decoder processes).
        player: Option<video::Player>,
    },
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum VideoStatus {
    Loading,
    Ready { playing: bool, position: f64, duration: f64 },
    Unavailable(String),
}

fn write_rgba(queue: &wgpu::Queue, texture: &wgpu::Texture, w: u32, h: u32, rgba: &[u8]) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo { texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        rgba,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * w), rows_per_image: Some(h) },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
}

fn instance(m: DAffine2, opacity: f32, params: [f32; 4]) -> Instance {
    Instance {
        col_a: m.matrix2.x_axis.as_vec2().to_array(),
        col_b: m.matrix2.y_axis.as_vec2().to_array(),
        translate: m.translation.as_vec2().to_array(),
        opacity,
        _pad: 0.0,
        params,
    }
}

fn linear(c: Color) -> [f64; 4] {
    let f = |v: u8| {
        let v = v as f64 / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    [f(c.r()), f(c.g()), f(c.b()), c.a() as f64 / 255.0]
}

/// Picks two adjacent power-of-ten grid levels for the current zoom and
/// cross-fades the finer one so zooming never pops.
fn grid_uniform(camera: &Camera, ppp: f64, px_size: DVec2, background: Color) -> GridUniform {
    const MIN_SPACING_PX: f64 = 18.0;
    let px_per_world = camera.zoom * ppp;
    let level = (MIN_SPACING_PX * ppp / px_per_world).log10().ceil();
    let fine = 10f64.powf(level);
    let coarse = fine * 10.0;
    let fine_px = fine * px_per_world / ppp;
    let fine_alpha = ((fine_px - MIN_SPACING_PX) / (MIN_SPACING_PX * 1.5)).clamp(0.0, 1.0);
    // Only the camera center modulo the coarse spacing matters, which keeps the
    // shader's f32 math small no matter how far the camera has traveled.
    let origin = DVec2::new(camera.center.x.rem_euclid(coarse), camera.center.y.rem_euclid(coarse));
    let inv = DMat2::from_angle(camera.rotation) * (1.0 / px_per_world);
    let lum = 0.299 * background.r() as f64 + 0.587 * background.g() as f64 + 0.114 * background.b() as f64;
    let color = if lum > 128.0 { [0.0, 0.0, 0.0, 0.26] } else { [1.0, 1.0, 1.0, 0.2] };
    GridUniform {
        inv_a: inv.x_axis.as_vec2().to_array(),
        inv_b: inv.y_axis.as_vec2().to_array(),
        origin: origin.as_vec2().to_array(),
        half_viewport: (px_size * 0.5).as_vec2().to_array(),
        fine: fine as f32,
        coarse: coarse as f32,
        fine_alpha: fine_alpha as f32,
        px_per_world: px_per_world as f32,
        color,
    }
}
