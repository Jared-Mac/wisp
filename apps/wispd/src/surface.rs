use anyhow::{Context, anyhow};
use std::{
    borrow::Cow,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
use tokio::sync::mpsc::UnboundedSender;
use wgpu::CurrentSurfaceTexture;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    platform::wayland::{EventLoopBuilderExtWayland, WindowAttributesExtWayland},
    window::{Window, WindowId},
};

use crate::media::MediaEvent;

const SHADER: &str = r"
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    let position = positions[index];
    var output: VertexOutput;
    output.position = vec4<f32>(position, 0.0, 1.0);
    output.uv = vec2<f32>((position.x + 1.0) * 0.5, 1.0 - (position.y + 1.0) * 0.5);
    return output;
}

@group(0) @binding(0) var frame_texture: texture_2d<f32>;
@group(0) @binding(1) var frame_sampler: sampler;

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(frame_texture, frame_sampler, input.uv);
}
";

#[derive(Debug)]
pub(crate) struct RgbaFrame {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

enum SurfaceCommand {
    Open,
    Close,
    FrameReady,
    Shutdown,
}

struct SurfaceShared {
    proxy: EventLoopProxy<SurfaceCommand>,
    latest_frame: Mutex<Option<RgbaFrame>>,
    frame_event_queued: AtomicBool,
    open: AtomicBool,
}

#[derive(Clone)]
pub(crate) struct SurfaceController {
    shared: Arc<SurfaceShared>,
}

impl SurfaceController {
    pub(crate) fn spawn(event_tx: UnboundedSender<MediaEvent>) -> anyhow::Result<Self> {
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("wisp-video-surface".into())
            .spawn(move || run_surface_thread(&ready_tx, &event_tx))
            .context("spawn Wayland surface thread")?;
        let shared = ready_rx
            .recv_timeout(Duration::from_secs(5))
            .context("Wayland surface thread did not initialize")?
            .map_err(anyhow::Error::msg)?;
        Ok(Self { shared })
    }

    pub(crate) fn open(&self) -> anyhow::Result<()> {
        self.shared
            .proxy
            .send_event(SurfaceCommand::Open)
            .map_err(|_| anyhow!("Wayland surface event loop is unavailable"))
    }

    pub(crate) fn close(&self) -> anyhow::Result<()> {
        self.shared
            .proxy
            .send_event(SurfaceCommand::Close)
            .map_err(|_| anyhow!("Wayland surface event loop is unavailable"))
    }

    pub(crate) fn send_frame(&self, frame: RgbaFrame) -> anyhow::Result<()> {
        *self
            .shared
            .latest_frame
            .lock()
            .expect("surface frame lock poisoned") = Some(frame);
        if !self.shared.open.load(Ordering::Acquire) {
            return Ok(());
        }
        if !self.shared.frame_event_queued.swap(true, Ordering::AcqRel)
            && self
                .shared
                .proxy
                .send_event(SurfaceCommand::FrameReady)
                .is_err()
        {
            self.shared
                .frame_event_queued
                .store(false, Ordering::Release);
            return Err(anyhow!("Wayland surface event loop is unavailable"));
        }
        Ok(())
    }

    pub(crate) fn shutdown(&self) {
        let _ = self.shared.proxy.send_event(SurfaceCommand::Shutdown);
    }
}

fn run_surface_thread(
    ready_tx: &mpsc::SyncSender<Result<Arc<SurfaceShared>, String>>,
    event_tx: &UnboundedSender<MediaEvent>,
) {
    let mut builder = EventLoop::<SurfaceCommand>::with_user_event();
    builder.with_wayland().with_any_thread(true);
    let event_loop = match builder.build() {
        Ok(event_loop) => event_loop,
        Err(error) => {
            let _ = ready_tx.send(Err(format!("create Wayland event loop: {error}")));
            return;
        }
    };
    let shared = Arc::new(SurfaceShared {
        proxy: event_loop.create_proxy(),
        latest_frame: Mutex::new(None),
        frame_event_queued: AtomicBool::new(false),
        open: AtomicBool::new(false),
    });
    if ready_tx.send(Ok(shared.clone())).is_err() {
        return;
    }
    let mut app = SurfaceApp {
        shared,
        event_tx: event_tx.clone(),
        window: None,
        renderer: None,
        rendered_frames: 0,
    };
    if let Err(error) = event_loop.run_app(&mut app) {
        let _ = event_tx.send(MediaEvent::SurfaceError {
            message: format!("Wayland event loop stopped: {error}"),
        });
    }
}

struct SurfaceApp {
    shared: Arc<SurfaceShared>,
    event_tx: UnboundedSender<MediaEvent>,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    rendered_frames: u64,
}

impl SurfaceApp {
    fn open(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("Wisp Video")
            .with_inner_size(LogicalSize::new(960.0, 540.0))
            .with_name("dev.wisp.surface", "dev.wisp.surface");
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                self.report_error(format!("create Wisp video window: {error}"));
                return;
            }
        };
        let mut renderer = match pollster::block_on(Renderer::new(window.clone())) {
            Ok(renderer) => renderer,
            Err(error) => {
                self.report_error(format!("initialize Wisp video renderer: {error:#}"));
                return;
            }
        };
        if let Some(frame) = self
            .shared
            .latest_frame
            .lock()
            .expect("surface frame lock poisoned")
            .take()
        {
            renderer.upload(&frame);
        }
        self.shared.open.store(true, Ordering::Release);
        self.renderer = Some(renderer);
        self.window = Some(window.clone());
        self.rendered_frames = 0;
        window.request_redraw();
        let _ = self.event_tx.send(MediaEvent::SurfaceOpened);
    }

    fn close(&mut self) {
        if self.window.is_none() {
            return;
        }
        self.shared.open.store(false, Ordering::Release);
        self.shared
            .frame_event_queued
            .store(false, Ordering::Release);
        self.renderer = None;
        self.window = None;
        let _ = self.event_tx.send(MediaEvent::SurfaceClosed);
    }

    fn take_latest_frame(&mut self) {
        self.shared
            .frame_event_queued
            .store(false, Ordering::Release);
        if let Some(frame) = self
            .shared
            .latest_frame
            .lock()
            .expect("surface frame lock poisoned")
            .take()
            && let Some(renderer) = &mut self.renderer
        {
            renderer.upload(&frame);
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }

    fn report_error(&self, message: String) {
        self.shared.open.store(false, Ordering::Release);
        let _ = self.event_tx.send(MediaEvent::SurfaceError { message });
    }
}

impl ApplicationHandler<SurfaceCommand> for SurfaceApp {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: SurfaceCommand) {
        match event {
            SurfaceCommand::Open => self.open(event_loop),
            SurfaceCommand::Close => self.close(),
            SurfaceCommand::FrameReady => self.take_latest_frame(),
            SurfaceCommand::Shutdown => {
                self.close();
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => self.close(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                let render_result = self.renderer.as_mut().map(Renderer::render);
                match render_result {
                    Some(Ok(true)) => {
                        self.rendered_frames = self.rendered_frames.saturating_add(1);
                        if self.rendered_frames == 1 || self.rendered_frames.is_multiple_of(60) {
                            let _ = self.event_tx.send(MediaEvent::SurfaceRendered {
                                total: self.rendered_frames,
                            });
                        }
                    }
                    Some(Ok(false)) | None => {}
                    Some(Err(error)) => {
                        self.close();
                        self.report_error(error.to_string());
                    }
                }
            }
            WindowEvent::Occluded(false) => {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

struct FrameTexture {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

struct Renderer {
    instance: wgpu::Instance,
    window: Arc<Window>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    texture_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    frame: FrameTexture,
}

impl Renderer {
    async fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        let size = window.inner_size();
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window.clone())
            .context("create wgpu Wayland surface")?;
        let (adapter, device, queue) = request_gpu_device(&instance, &surface).await?;
        let config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .context("GPU surface has no supported configuration")?;
        surface.configure(&device, &config);

        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Wisp frame texture layout"),
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
            label: Some("Wisp frame sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Wisp video shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(SHADER)),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Wisp video pipeline layout"),
            bind_group_layouts: &[Some(&texture_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Wisp video pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(config.format.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let frame = create_frame_texture(&device, &texture_layout, &sampler, 2, 2);
        Ok(Self {
            instance,
            window,
            device,
            queue,
            surface,
            config,
            pipeline,
            texture_layout,
            sampler,
            frame,
        })
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.device, &self.config);
        self.window.request_redraw();
    }

    fn upload(&mut self, frame: &RgbaFrame) {
        if self.frame.width != frame.width || self.frame.height != frame.height {
            self.frame = create_frame_texture(
                &self.device,
                &self.texture_layout,
                &self.sampler,
                frame.width,
                frame.height,
            );
        }
        self.queue.write_texture(
            self.frame.texture.as_image_copy(),
            &frame.data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(frame.width * 4),
                rows_per_image: Some(frame.height),
            },
            wgpu::Extent3d {
                width: frame.width,
                height: frame.height,
                depth_or_array_layers: 1,
            },
        );
    }

    fn render(&mut self) -> anyhow::Result<bool> {
        let surface_frame = match self.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(frame) => frame,
            CurrentSurfaceTexture::Timeout | CurrentSurfaceTexture::Occluded => return Ok(false),
            CurrentSurfaceTexture::Suboptimal(frame) => {
                drop(frame);
                self.surface.configure(&self.device, &self.config);
                return Ok(false);
            }
            CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(false);
            }
            CurrentSurfaceTexture::Lost => {
                self.surface = self
                    .instance
                    .create_surface(self.window.clone())
                    .context("recreate lost Wisp GPU surface")?;
                self.surface.configure(&self.device, &self.config);
                return Ok(false);
            }
            CurrentSurfaceTexture::Validation => {
                return Err(anyhow!("wgpu rejected the Wisp surface frame"));
            }
        };
        let view = surface_frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Wisp frame encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Wisp frame pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.frame.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
        self.window.pre_present_notify();
        self.queue.present(surface_frame);
        Ok(true)
    }
}

async fn request_gpu_device(
    instance: &wgpu::Instance,
    surface: &wgpu::Surface<'_>,
) -> anyhow::Result<(wgpu::Adapter, wgpu::Device, wgpu::Queue)> {
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(surface),
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        })
        .await
        .context("find a GPU adapter for the Wisp surface")?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("Wisp video device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            trace: wgpu::Trace::Off,
        })
        .await
        .context("create Wisp GPU device")?;
    Ok((adapter, device, queue))
}

fn create_frame_texture(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    width: u32,
    height: u32,
) -> FrameTexture {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Wisp remote video frame"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Wisp remote video bind group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    });
    FrameTexture {
        texture,
        bind_group,
        width,
        height,
    }
}
