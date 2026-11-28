//! GPU renderer: solid rectangles and cosmic-text glyphs drawn as textured quads.

mod clip;
mod icon_fonts;
mod images;
mod text;
pub use icon_fonts::IconFont;
pub use images::Image;
use text::measure_char_width;

use crate::atlas::{Slot, ATLAS_SIZE};
use crate::cache::{GlyphCache, TextCache};
use crate::icons;
use cosmic_text::{
    Attrs, Buffer, CacheKey, Family, FontSystem, Metrics, Shaping, SwashCache, SwashContent,
};
use std::{collections::HashMap, sync::Arc};
use winit::window::Window;

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("failed to create surface: {0}")]
    Surface(#[from] wgpu::CreateSurfaceError),
    #[error("no suitable GPU adapter: {0}")]
    Adapter(#[from] wgpu::RequestAdapterError),
    #[error("failed to create device: {0}")]
    Device(#[from] wgpu::RequestDeviceError),
}

pub type Color = [f32; 4];

pub fn rgb(hex: u32) -> Color {
    [
        ((hex >> 16) & 0xff) as f32 / 255.0,
        ((hex >> 8) & 0xff) as f32 / 255.0,
        (hex & 0xff) as f32 / 255.0,
        1.0,
    ]
}

/// One rectangle, drawn as an instance of six vertices the shader places at its corners.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Quad {
    /// Left, top, right, bottom, in pixels.
    rect: [f32; 4],
    /// Texture coordinates of the top-left and bottom-right corners.
    uv: [f32; 4],
    color: [f32; 4],
    /// 0 for rectangles and glyphs (the atlas gives coverage), 1 for images (the atlas gives
    /// color), 2 for tinted image masks.
    mode: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    screen: [f32; 2],
    _pad: [f32; 2],
}

/// Shaped lines kept per generation; see [`TextCache`].
const LINE_CACHE_LIMIT: usize = 2000;

#[derive(Clone, Copy)]
struct GlyphEntry {
    slot: Slot,
    left: i32,
    top: i32,
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    globals_buf: wgpu::Buffer,
    atlas_tex: wgpu::Texture,
    image_tex: wgpu::Texture,
    images: images::ImageAtlas,
    quad_buf: wgpu::Buffer,
    quad_capacity: usize,
    quads: Vec<Quad>,
    glyphs: GlyphCache<CacheKey, GlyphEntry>,
    font_system: FontSystem,
    swash: SwashCache,
    line_cache: TextCache<Buffer>,
    /// Reused for the glyphs of one line, so drawing text allocates nothing once warm.
    glyph_scratch: Vec<(cosmic_text::PhysicalGlyph, f32)>,
    /// Set when an atlas was emptied after a frame that could not show everything.
    redraw_needed: bool,
    /// Shaped icon glyphs by (font key, glyph, size bits); font key 0 is the embedded font.
    icon_cache: HashMap<(u64, char, u32), Buffer>,
    /// Family names of the icon fonts loaded at run time, by [`IconFont::id`]; `None` when the
    /// font could not be read, so it is not tried again.
    icon_fonts: HashMap<u64, Option<String>>,
    clear: Color,
    char_width: f32,
    metrics: Metrics,
    suspended: bool,
    clip: Option<[f32; 4]>,
    translation: (f32, f32),
    translation_stack: Vec<(f32, f32)>,
}

impl Renderer {
    pub fn new(window: Arc<Window>, font_size: f32) -> Result<Self, RenderError> {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance.create_surface(window)?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;

        let caps = surface.get_capabilities(&adapter);
        // Prefer a non-sRGB format so colors are rendered exactly as authored.
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let atlas_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph atlas"),
            size: wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        // Solid 2x2 block at the origin, used for rectangles.
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &atlas_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255u8; 4],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(2),
                rows_per_image: Some(2),
            },
            wgpu::Extent3d {
                width: 2,
                height: 2,
                depth_or_array_layers: 1,
            },
        );

        let image_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("image atlas"),
            size: wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let image_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let globals_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bgl = bind_group_layout(&device);
        let atlas_view = atlas_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let image_view = image_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bind group"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: globals_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&image_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&image_sampler),
                },
            ],
        });

        let pipeline = render_pipeline(&device, &bgl, format);

        let quad_capacity = 1024;
        let quad_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("quads"),
            size: (quad_capacity * std::mem::size_of::<Quad>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut font_system = FontSystem::new();
        font_system.db_mut().load_font_data(icons::FONT.to_vec());
        let metrics = Metrics::new(font_size, (font_size * 1.4).round());
        let char_width = measure_char_width(&mut font_system, metrics);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            pipeline,
            bind_group,
            globals_buf,
            atlas_tex,
            image_tex,
            images: images::ImageAtlas::new(),
            quad_buf,
            quad_capacity,
            quads: Vec::new(),
            glyphs: GlyphCache::new(),
            font_system,
            swash: SwashCache::new(),
            line_cache: TextCache::new(LINE_CACHE_LIMIT),
            glyph_scratch: Vec::new(),
            redraw_needed: false,
            icon_cache: HashMap::new(),
            icon_fonts: HashMap::new(),
            clear: rgb(0x1e1e1e),
            char_width,
            metrics,
            suspended: size.width == 0 || size.height == 0,
            clip: None,
            translation: (0.0, 0.0),
            translation_stack: Vec::new(),
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.suspended = width == 0 || height == 0;
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn size(&self) -> (f32, f32) {
        (self.config.width as f32, self.config.height as f32)
    }

    /// Width of one monospace cell, in pixels.
    pub fn char_width(&self) -> f32 {
        self.char_width
    }

    /// Height of one text line, in pixels.
    pub fn line_height(&self) -> f32 {
        self.metrics.line_height
    }

    pub fn set_font_size(&mut self, font_size: f32) {
        self.metrics = Metrics::new(font_size, (font_size * 1.4).round());
        self.char_width = measure_char_width(&mut self.font_system, self.metrics);
        self.line_cache.clear();
        self.icon_cache.clear();
    }

    pub fn set_clear_color(&mut self, color: Color) {
        self.clear = color;
    }

    /// Restrict subsequent rectangles and text to a pixel rectangle, or clear it.
    pub fn set_clip(&mut self, clip: Option<[f32; 4]>) {
        self.clip = clip.map(|[x, y, right, bottom]| {
            [
                x + self.translation.0,
                y + self.translation.1,
                right + self.translation.0,
                bottom + self.translation.1,
            ]
        });
    }

    /// Temporarily translate subsequent draw coordinates. Hit testing remains
    /// the caller's responsibility and should apply the inverse translation.
    pub fn push_translation(&mut self, x: f32, y: f32) {
        self.translation_stack.push(self.translation);
        self.translation.0 += x;
        self.translation.1 += y;
    }

    pub fn pop_translation(&mut self) {
        if let Some(previous) = self.translation_stack.pop() {
            self.translation = previous;
        }
    }

    pub fn draw_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        // Sample the middle of the reserved solid block.
        let uv = [1.0 / ATLAS_SIZE as f32, 1.0 / ATLAS_SIZE as f32];
        let (tx, ty) = self.translation;
        self.push_quad(x + tx, y + ty, x + w + tx, y + h + ty, uv, uv, color);
    }

    #[allow(clippy::too_many_arguments)]
    fn push_quad(
        &mut self,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        uv0: [f32; 2],
        uv1: [f32; 2],
        color: Color,
    ) {
        self.push_quad_mode(x0, y0, x1, y1, uv0, uv1, color, 0.0);
    }

    #[allow(clippy::too_many_arguments)]
    fn push_quad_mode(
        &mut self,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        uv0: [f32; 2],
        uv1: [f32; 2],
        color: Color,
        mode: f32,
    ) {
        let Some(([x0, y0, x1, y1], uv0, uv1)) =
            clip::clip_quad([x0, y0, x1, y1], uv0, uv1, self.clip)
        else {
            return;
        };
        self.quads.push(Quad {
            rect: [x0, y0, x1, y1],
            uv: [uv0[0], uv0[1], uv1[0], uv1[1]],
            color,
            mode,
        });
    }

    /// Submits everything drawn since the last call and presents the frame.
    pub fn present(&mut self) {
        if self.suspended {
            self.quads.clear();
            return;
        }
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f)
            | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            _ => {
                // Surface lost/outdated: reconfigure and skip this frame.
                self.surface.configure(&self.device, &self.config);
                self.quads.clear();
                return;
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        self.queue.write_buffer(
            &self.globals_buf,
            0,
            bytemuck::bytes_of(&Globals {
                screen: [self.config.width as f32, self.config.height as f32],
                _pad: [0.0; 2],
            }),
        );
        if self.quads.len() > self.quad_capacity {
            self.quad_capacity = self.quads.len().next_power_of_two();
            self.quad_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("quads"),
                size: (self.quad_capacity * std::mem::size_of::<Quad>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        self.queue
            .write_buffer(&self.quad_buf, 0, bytemuck::cast_slice(&self.quads));

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.clear[0] as f64,
                            g: self.clear[1] as f64,
                            b: self.clear[2] as f64,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.quad_buf.slice(..));
            // Six vertices, two triangles, for every rectangle.
            pass.draw(0..6, 0..self.quads.len() as u32);
        }
        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);
        if self.images.reset_pending {
            self.images.reset();
            self.redraw_needed = true;
        }
        if self.glyphs.reset_if_full() {
            self.redraw_needed = true;
        }
        self.quads.clear();
    }

    /// Whether the frame just presented left out glyphs or images because an atlas was full.
    /// The atlas has been emptied, so drawing again shows them; the request is cleared.
    pub fn take_redraw_request(&mut self) -> bool {
        std::mem::take(&mut self.redraw_needed)
    }
}

/// The bindings the shader reads: the screen size, the glyph atlas and the image atlas.
fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

/// The pipeline that draws every rectangle as an instance, into `format` targets.
fn render_pipeline(
    device: &wgpu::Device,
    bgl: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("pipeline layout"),
        bind_group_layouts: &[Some(bgl)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Quad>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32],
            })],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

#[cfg(test)]
mod tests {
    /// Draws `quads` into a 64x64 RGBA texture on a GPU without a window and returns its pixels,
    /// or `None` when this machine has no adapter.
    fn render_offscreen(quads: &[super::Quad]) -> Option<Vec<[u8; 4]>> {
        const SIZE: u32 = 64;
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .ok()?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()?;
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let bgl = super::bind_group_layout(&device);
        let pipeline = super::render_pipeline(&device, &bgl, format);
        let texture = |format, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: SIZE,
                    height: SIZE,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let sampled = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;
        let atlas = texture(wgpu::TextureFormat::R8Unorm, sampled);
        // Full coverage everywhere, like the solid block rectangles sample.
        queue.write_texture(
            atlas.as_image_copy(),
            &vec![255u8; (SIZE * SIZE) as usize],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(SIZE),
                rows_per_image: Some(SIZE),
            },
            atlas.size(),
        );
        let images = texture(wgpu::TextureFormat::Rgba8Unorm, sampled);
        let target = texture(
            format,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let globals = wgpu::util::DeviceExt::create_buffer_init(
            &device,
            &wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::bytes_of(&super::Globals {
                    screen: [SIZE as f32, SIZE as f32],
                    _pad: [0.0; 2],
                }),
                usage: wgpu::BufferUsages::UNIFORM,
            },
        );
        let instances = wgpu::util::DeviceExt::create_buffer_init(
            &device,
            &wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(quads),
                usage: wgpu::BufferUsages::VERTEX,
            },
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
        let atlas_view = atlas.create_view(&Default::default());
        let images_view = images.create_view(&Default::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: globals.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&images_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(SIZE * SIZE * 4),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let view = target.create_view(&Default::default());
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.set_vertex_buffer(0, instances.slice(..));
            pass.draw(0..6, 0..quads.len() as u32);
        }
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(SIZE * 4),
                    rows_per_image: Some(SIZE),
                },
            },
            target.size(),
        );
        queue.submit(Some(encoder.finish()));
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
        let pixels = slice.get_mapped_range().ok()?.as_chunks::<4>().0.to_vec();
        Some(pixels)
    }

    #[test]
    fn instanced_rectangles_are_drawn_where_they_were_placed() {
        let quad = |rect, color| super::Quad {
            rect,
            uv: [0.0, 0.0, 1.0 / 64.0, 1.0 / 64.0],
            color,
            mode: 0.0,
        };
        let Some(pixels) = render_offscreen(&[
            quad([8.0, 8.0, 24.0, 24.0], [1.0, 0.0, 0.0, 1.0]),
            quad([40.0, 32.0, 56.0, 60.0], [0.0, 1.0, 0.0, 1.0]),
        ]) else {
            eprintln!("No GPU adapter on this machine; the drawing check was skipped.");
            return;
        };
        let at = |x: usize, y: usize| pixels[y * 64 + x];
        assert_eq!(at(16, 16), [255, 0, 0, 255], "inside the red rectangle");
        assert_eq!(at(48, 46), [0, 255, 0, 255], "inside the green rectangle");
        assert_eq!(at(32, 16), [0, 0, 0, 255], "between them");
        assert_eq!(at(7, 16), [0, 0, 0, 255], "left of the red rectangle");
        assert_eq!(at(16, 24), [0, 0, 0, 255], "below the red rectangle");
        assert_eq!(at(48, 31), [0, 0, 0, 255], "above the green rectangle");
    }

    #[test]
    fn the_shader_parses_and_validates() {
        let module = wgpu::naga::front::wgsl::parse_str(include_str!("shader.wgsl"))
            .expect("the shader parses");
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("the shader is valid");
    }

    #[test]
    fn the_shader_reads_each_rectangle_as_the_pipeline_lays_it_out() {
        use wgpu::naga::{Binding, BuiltIn, ScalarKind, TypeInner, VectorSize};
        let module = wgpu::naga::front::wgsl::parse_str(include_str!("shader.wgsl")).unwrap();
        let entry = module
            .entry_points
            .iter()
            .find(|entry| entry.name == "vs_main")
            .unwrap();
        let input = &module.types[entry.function.arguments[0].ty];
        let TypeInner::Struct { members, .. } = &input.inner else {
            panic!("vs_main takes a struct");
        };
        let mut locations = Vec::new();
        let mut corner_index = false;
        for member in members {
            match member.binding {
                Some(Binding::BuiltIn(BuiltIn::VertexIndex)) => corner_index = true,
                Some(Binding::Location { location, .. }) => {
                    let width = match module.types[member.ty].inner {
                        TypeInner::Vector {
                            size: VectorSize::Quad,
                            scalar,
                        } if scalar.kind == ScalarKind::Float => 4,
                        TypeInner::Scalar(scalar) if scalar.kind == ScalarKind::Float => 1,
                        ref other => panic!("unexpected input {other:?}"),
                    };
                    locations.push((location, width));
                }
                _ => {}
            }
        }
        assert!(corner_index, "the corner comes from the vertex index");
        locations.sort();
        // Float32x4 rect, Float32x4 uv, Float32x4 color, Float32 mode, as in the pipeline.
        assert_eq!(locations, vec![(0, 4), (1, 4), (2, 4), (3, 1)]);
    }

    #[test]
    fn quads_keep_a_tight_layout() {
        // Corners, texture corners, color and mode: 52 bytes per rectangle, where six vertices
        // used to take 216.
        assert_eq!(std::mem::size_of::<super::Quad>(), 52);
    }
}
