use bytemuck::{Pod, Zeroable};
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::window::Window;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 2],
    pub tex_coords: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Uniforms {
    pub transform: [[f32; 4]; 4],
    pub viewport_size: [f32; 2],
    pub image_size: [f32; 2],
    pub zoom: f32,
    pub flags: u32,
    pub filter_mode: u32,
    pub tonemap_mode: u32,
    pub exposure: f32,
    pub color_space: u32,
    pub _padding: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct HudUniforms {
    pub viewport_size: [f32; 2],
    pub _padding: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct HudVertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
}

pub struct RenderState {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,

    // Image pipeline
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler_linear: wgpu::Sampler,
    sampler_nearest: wgpu::Sampler,
    tile_vertex_buffer: wgpu::Buffer,

    // Image texture state
    image_texture: Option<wgpu::Texture>,
    image_bind_group: Option<wgpu::BindGroup>,
    pub image_width: u32,
    pub image_height: u32,
    pub max_texture_dimension_2d: u32,

    // HUD pipeline
    hud_pipeline: wgpu::RenderPipeline,
    hud_uniform_buffer: wgpu::Buffer,
    hud_bind_group: wgpu::BindGroup,
    hud_vertex_buffer: wgpu::Buffer,
    hud_vertex_capacity: usize,

    pub window: Arc<Window>,
}

const QUAD_VERTICES: &[Vertex] = &[
    Vertex { position: [-0.5, -0.5], tex_coords: [0.0, 1.0] },
    Vertex { position: [ 0.5, -0.5], tex_coords: [1.0, 1.0] },
    Vertex { position: [ 0.5,  0.5], tex_coords: [1.0, 0.0] },
    Vertex { position: [-0.5,  0.5], tex_coords: [0.0, 0.0] },
];

const QUAD_INDICES: &[u16] = &[0, 1, 2, 0, 2, 3];

impl RenderState {
    pub async fn new(window: Arc<Window>) -> Result<Self, String> {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| format!("Failed to create surface: {}", e))?;

        let mut adapter_opt = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await;

        if adapter_opt.is_none() {
            log::warn!("HighPerformance GPU adapter not found, trying LowPower adapter");
            adapter_opt = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::LowPower,
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                })
                .await;
        }

        if adapter_opt.is_none() {
            log::warn!("Hardware GPU adapter not found, falling back to software rasterizer (WARP/llvmpipe)");
            adapter_opt = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::None,
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: true,
                })
                .await;
        }

        let adapter = adapter_opt.ok_or("Failed to find suitable GPU or software rendering adapter")?;
        let max_texture_dimension_2d = adapter.limits().max_texture_dimension_2d;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Photon Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: adapter.limits(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .map_err(|e| format!("Failed to request device: {}", e))?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Samplers
        let sampler_linear = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let sampler_nearest = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Buffers
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Quad Vertex Buffer"),
            contents: bytemuck::cast_slice(QUAD_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Quad Index Buffer"),
            contents: bytemuck::cast_slice(QUAD_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Image Uniform Buffer"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Image Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
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
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        });

        // Image Pipeline
        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Image Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        };

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Image Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[vertex_layout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // HUD Pipeline
        let hud_shader = device.create_shader_module(wgpu::include_wgsl!("hud_shader.wgsl"));
        let hud_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("HUD Uniform Buffer"),
            size: std::mem::size_of::<HudUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let hud_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("HUD Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let hud_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("HUD Bind Group"),
            layout: &hud_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: hud_uniform_buffer.as_entire_binding(),
            }],
        });

        let hud_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("HUD Pipeline Layout"),
            bind_group_layouts: &[&hud_bind_group_layout],
            push_constant_ranges: &[],
        });

        let hud_vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<HudVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        };

        let hud_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("HUD Render Pipeline"),
            layout: Some(&hud_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &hud_shader,
                entry_point: "vs_hud",
                buffers: &[hud_vertex_layout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &hud_shader,
                entry_point: "fs_hud",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let hud_vertex_capacity = 1024;
        let hud_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("HUD Vertex Buffer"),
            size: (hud_vertex_capacity * std::mem::size_of::<HudVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let tile_vertex_capacity = 512;
        let tile_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Tile Vertex Buffer"),
            size: (tile_vertex_capacity * std::mem::size_of::<Vertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            vertex_buffer,
            index_buffer,
            uniform_buffer,
            bind_group_layout,
            sampler_linear,
            sampler_nearest,
            tile_vertex_buffer,
            image_texture: None,
            image_bind_group: None,
            image_width: 0,
            image_height: 0,
            max_texture_dimension_2d,
            hud_pipeline,
            hud_uniform_buffer,
            hud_bind_group,
            hud_vertex_buffer,
            hud_vertex_capacity,
            window,
        })
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn uniform_buffer(&self) -> &wgpu::Buffer {
        &self.uniform_buffer
    }

    pub fn samplers(&self) -> (&wgpu::Sampler, &wgpu::Sampler) {
        (&self.sampler_linear, &self.sampler_nearest)
    }

    pub fn poll_device(&self) {
        self.device.poll(wgpu::Maintain::Poll);
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn upload_image(&mut self, width: u32, height: u32, rgba_pixels: &[u8], is_hdr: bool) {
        if width == 0 || height == 0 {
            return;
        }

        if width > self.max_texture_dimension_2d || height > self.max_texture_dimension_2d {
            log::warn!(
                "upload_image: dimensions {}x{} exceed GPU max limit {}",
                width,
                height,
                self.max_texture_dimension_2d
            );
            return;
        }

        let bytes_per_pixel: u32 = if is_hdr { 8 } else { 4 };
        let expected_bytes = (width as usize) * (height as usize) * (bytes_per_pixel as usize);
        if rgba_pixels.len() < expected_bytes {
            log::warn!(
                "upload_image: buffer length {} is smaller than expected {}",
                rgba_pixels.len(),
                expected_bytes
            );
            return;
        }

        // Drop previous texture and reclaim VRAM / staging buffers immediately
        if let Some(old_tex) = self.image_texture.take() {
            old_tex.destroy();
            self.device.poll(wgpu::Maintain::Poll);
        }

        let format = if is_hdr {
            wgpu::TextureFormat::Rgba16Float
        } else {
            wgpu::TextureFormat::Rgba8UnormSrgb
        };

        // Anti-aliasing: 3 mip levels (1x, 0.5x, 0.25x) eliminates fit-to-window moire
        // without consuming massive CPU time or bloating GPU memory
        let num_mips = if is_hdr || width < 128 || height < 128 {
            1
        } else {
            3
        };

        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Image Texture"),
            size,
            mip_level_count: num_mips,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload level 0
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba_pixels,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_pixel * width),
                rows_per_image: Some(height),
            },
            size,
        );

        // Generate and upload mip levels 1..num_mips using zero-copy 2x2 box filtering
        if num_mips > 1 {
            let mut prev_w = width;
            let mut prev_h = height;
            let mut prev_box: Option<Vec<u8>> = None;

            for level in 1..num_mips {
                let cur_w = (prev_w / 2).max(1);
                let cur_h = (prev_h / 2).max(1);
                let mut cur_data = vec![0u8; (cur_w * cur_h * 4) as usize];

                let prev_stride = (prev_w * 4) as usize;
                let cur_stride = (cur_w * 4) as usize;
                let prev_src: &[u8] = match prev_box.as_ref() {
                    Some(b) => b.as_slice(),
                    None => rgba_pixels,
                };
                let prev_len = prev_src.len();

                for y in 0..cur_h {
                    let py0 = (y * 2) as usize * prev_stride;
                    let py1 = if y * 2 + 1 < prev_h { (y * 2 + 1) as usize * prev_stride } else { py0 };
                    let dst_row = y as usize * cur_stride;

                    for x in 0..cur_w {
                        let px0 = (x * 2) as usize * 4;
                        let px1 = if x * 2 + 1 < prev_w { (x * 2 + 1) as usize * 4 } else { px0 };

                        let i00 = py0 + px0;
                        let i10 = py0 + px1;
                        let i01 = py1 + px0;
                        let i11 = py1 + px1;

                        let dst = dst_row + x as usize * 4;
                        if i11 + 3 < prev_len && dst + 3 < cur_data.len() {
                            cur_data[dst] = ((prev_src[i00] as u32 + prev_src[i10] as u32 + prev_src[i01] as u32 + prev_src[i11] as u32) / 4) as u8;
                            cur_data[dst + 1] = ((prev_src[i00 + 1] as u32 + prev_src[i10 + 1] as u32 + prev_src[i01 + 1] as u32 + prev_src[i11 + 1] as u32) / 4) as u8;
                            cur_data[dst + 2] = ((prev_src[i00 + 2] as u32 + prev_src[i10 + 2] as u32 + prev_src[i01 + 2] as u32 + prev_src[i11 + 2] as u32) / 4) as u8;
                            cur_data[dst + 3] = ((prev_src[i00 + 3] as u32 + prev_src[i10 + 3] as u32 + prev_src[i01 + 3] as u32 + prev_src[i11 + 3] as u32) / 4) as u8;
                        } else if dst + 3 < cur_data.len() {
                            let get_px = |idx: usize, ch: usize| -> u32 {
                                prev_src.get(idx + ch).copied().unwrap_or(0) as u32
                            };
                            cur_data[dst] = ((get_px(i00, 0) + get_px(i10, 0) + get_px(i01, 0) + get_px(i11, 0)) / 4) as u8;
                            cur_data[dst + 1] = ((get_px(i00, 1) + get_px(i10, 1) + get_px(i01, 1) + get_px(i11, 1)) / 4) as u8;
                            cur_data[dst + 2] = ((get_px(i00, 2) + get_px(i10, 2) + get_px(i01, 2) + get_px(i11, 2)) / 4) as u8;
                            cur_data[dst + 3] = ((get_px(i00, 3) + get_px(i10, 3) + get_px(i01, 3) + get_px(i11, 3)) / 4) as u8;
                        }
                    }
                }

                let mip_size = wgpu::Extent3d {
                    width: cur_w,
                    height: cur_h,
                    depth_or_array_layers: 1,
                };

                self.queue.write_texture(
                    wgpu::ImageCopyTexture {
                        texture: &texture,
                        mip_level: level,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &cur_data,
                    wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(4 * cur_w),
                        rows_per_image: Some(cur_h),
                    },
                    mip_size,
                );

                prev_w = cur_w;
                prev_h = cur_h;
                prev_box = Some(cur_data);
            }
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Image Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler_linear),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.sampler_nearest),
                },
            ],
        });

        self.image_texture = Some(texture);
        self.image_bind_group = Some(bind_group);
        self.image_width = width;
        self.image_height = height;
    }

    pub fn clear_image(&mut self) {
        if let Some(old_tex) = self.image_texture.take() {
            old_tex.destroy();
            self.device.poll(wgpu::Maintain::Poll);
        }
        self.image_bind_group = None;
        self.image_width = 0;
        self.image_height = 0;
    }

    pub fn render(
        &mut self,
        zoom: f32,
        pan_x: f32,
        pan_y: f32,
        rotation_deg: f32,
        flip_h: bool,
        flip_v: bool,
        filter_mode: u32,
        tonemap_mode: u32,
        exposure: f32,
        color_space: u32,
        is_oled_black: bool,
        tiles_to_render: &[(&wgpu::BindGroup, f32, f32, f32, f32)],
        hud_vertices: &[HudVertex],
    ) -> Result<(), wgpu::SurfaceError> {
        if self.size.width == 0 || self.size.height == 0 {
            return Ok(());
        }

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        let vw = self.size.width as f32;
        let vh = self.size.height as f32;

        if let Some(ref bind_group) = self.image_bind_group {
            let iw = self.image_width as f32;
            let ih = self.image_height as f32;

            let sh = if flip_h { -1.0 } else { 1.0 };
            let sv = if flip_v { -1.0 } else { 1.0 };

            // Rotation angle in radians (Clockwise on screen)
            let rad = rotation_deg.to_radians();
            let cos_theta = rad.cos();
            let sin_theta = rad.sin();

            // Transform quad vertices (centered at (0,0), [-0.5, 0.5]) to NDC [-1, 1]
            // Screen pixel width is iw * zoom, screen pixel height is ih * zoom.
            // Clockwise rotation maps:
            //   X_px = u * cos_theta + v * sin_theta
            //   Y_px = -u * sin_theta + v * cos_theta
            // Then converted to NDC by scaling X by 2.0 / vw and Y by 2.0 / vh.
            let m00 = (2.0 * iw * zoom * sh / vw) * cos_theta;
            let m01 = (2.0 * ih * zoom * sv / vw) * sin_theta;
            let m10 = -(2.0 * iw * zoom * sh / vh) * sin_theta;
            let m11 = (2.0 * ih * zoom * sv / vh) * cos_theta;

            // Screen space translation in NDC [-1, 1]
            let tx = (pan_x / vw) * 2.0;
            let ty = -(pan_y / vh) * 2.0; // Invert Y for NDC

            let transform: [[f32; 4]; 4] = [
                [m00, m10, 0.0, 0.0],
                [m01, m11, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [ tx,  ty, 0.0, 1.0],
            ];

            let uniforms = Uniforms {
                transform,
                viewport_size: [vw, vh],
                image_size: [iw, ih],
                zoom,
                flags: 1 | 2, // Enable checkerboard (bit 0) and pixel grid (bit 1)
                filter_mode,
                tonemap_mode,
                exposure,
                color_space,
                _padding: [0.0, 0.0],
            };

            self.queue
                .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

            if !tiles_to_render.is_empty() {
                let mut tile_verts = Vec::with_capacity(tiles_to_render.len() * 4);
                for &(_, u0, v0, u1, v1) in tiles_to_render {
                    let x0 = u0 - 0.5;
                    let x1 = u1 - 0.5;
                    let y_bottom = 0.5 - v1;
                    let y_top = 0.5 - v0;

                    tile_verts.push(Vertex { position: [x0, y_bottom], tex_coords: [0.0, 1.0] });
                    tile_verts.push(Vertex { position: [x1, y_bottom], tex_coords: [1.0, 1.0] });
                    tile_verts.push(Vertex { position: [x1, y_top],    tex_coords: [1.0, 0.0] });
                    tile_verts.push(Vertex { position: [x0, y_top],    tex_coords: [0.0, 0.0] });
                }
                self.queue.write_buffer(&self.tile_vertex_buffer, 0, bytemuck::cast_slice(&tile_verts));
            }

            let clear_color = if is_oled_black {
                wgpu::Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                }
            } else {
                wgpu::Color {
                    r: 0.07,
                    g: 0.07,
                    b: 0.08,
                    a: 1.0,
                }
            };

            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Image Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(clear_color),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                render_pass.set_pipeline(&self.render_pipeline);
                render_pass.set_bind_group(0, bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                render_pass.draw_indexed(0..6, 0, 0..1);

                // Draw high-resolution visible tiles on top of overview
                if !tiles_to_render.is_empty() {
                    render_pass.set_vertex_buffer(0, self.tile_vertex_buffer.slice(..));
                    for (i, &(tile_bg, ..)) in tiles_to_render.iter().enumerate() {
                        render_pass.set_bind_group(0, tile_bg, &[]);
                        render_pass.draw_indexed(0..6, (i * 4) as i32, 0..1);
                    }
                }
            }
        } else {
            // No image loaded yet: dark clear screen
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Empty Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.07,
                            g: 0.07,
                            b: 0.08,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        // Render HUD elements if any
        if !hud_vertices.is_empty() {
            let hud_uniforms = HudUniforms {
                viewport_size: [vw, vh],
                _padding: [0.0, 0.0],
            };
            self.queue.write_buffer(
                &self.hud_uniform_buffer,
                0,
                bytemuck::cast_slice(&[hud_uniforms]),
            );

            let vertex_count = hud_vertices.len();
            if vertex_count > self.hud_vertex_capacity {
                self.hud_vertex_capacity = vertex_count.next_power_of_two();
                self.hud_vertex_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Resized HUD Vertex Buffer"),
                    size: (self.hud_vertex_capacity * std::mem::size_of::<HudVertex>()) as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            }

            self.queue.write_buffer(
                &self.hud_vertex_buffer,
                0,
                bytemuck::cast_slice(hud_vertices),
            );

            let mut hud_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("HUD Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            hud_pass.set_pipeline(&self.hud_pipeline);
            hud_pass.set_bind_group(0, &self.hud_bind_group, &[]);
            hud_pass.set_vertex_buffer(0, self.hud_vertex_buffer.slice(..));
            hud_pass.draw(0..vertex_count as u32, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        self.device.poll(wgpu::Maintain::Poll);

        Ok(())
    }
}
