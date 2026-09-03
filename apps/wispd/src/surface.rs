use anyhow::{Context, anyhow};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, mpsc},
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
    platform::x11::{EventLoopBuilderExtX11, WindowAttributesExtX11},
    window::{Window, WindowId},
};
use wisp_protocol::{RemoteVideoTarget, VideoSource};

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
    Open(RemoteVideoTarget),
    Close(RemoteVideoTarget),
    FrameReady(RemoteVideoTarget),
    Shutdown,
}

struct SurfaceShared {
    proxy: EventLoopProxy<SurfaceCommand>,
    latest_frames: Mutex<HashMap<RemoteVideoTarget, RgbaFrame>>,
    frame_events_queued: Mutex<HashSet<RemoteVideoTarget>>,
    open_targets: Mutex<HashSet<RemoteVideoTarget>>,
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
            .context("spawn video surface thread")?;
        let shared = ready_rx
            .recv_timeout(Duration::from_secs(5))
            .context("video surface thread did not initialize")?
            .map_err(anyhow::Error::msg)?;
        Ok(Self { shared })
    }

    pub(crate) fn open(&self, target: RemoteVideoTarget) -> anyhow::Result<()> {
        self.shared
            .proxy
            .send_event(SurfaceCommand::Open(target))
            .map_err(|_| anyhow!("video surface event loop is unavailable"))
    }

    pub(crate) fn close(&self, target: RemoteVideoTarget) -> anyhow::Result<()> {
        self.shared
            .proxy
            .send_event(SurfaceCommand::Close(target))
            .map_err(|_| anyhow!("video surface event loop is unavailable"))
    }

    pub(crate) fn send_frame(
        &self,
        target: &RemoteVideoTarget,
        frame: RgbaFrame,
    ) -> anyhow::Result<()> {
        self.shared
            .latest_frames
            .lock()
            .expect("surface frame lock poisoned")
            .insert(target.clone(), frame);
        if !self
            .shared
            .open_targets
            .lock()
            .expect("surface open-target lock poisoned")
            .contains(target)
        {
            return Ok(());
        }
        let queued = self
            .shared
            .frame_events_queued
            .lock()
            .expect("surface queued-frame lock poisoned")
            .insert(target.clone());
        if queued
            && self
                .shared
                .proxy
                .send_event(SurfaceCommand::FrameReady(target.clone()))
                .is_err()
        {
            self.shared
                .frame_events_queued
                .lock()
                .expect("surface queued-frame lock poisoned")
                .remove(target);
            return Err(anyhow!("video surface event loop is unavailable"));
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
    // NVIDIA's Vulkan Wayland swapchain teardown can jump through a null
    // driver function when multiple surfaces are alive. XWayland supports
    // actually unmapping closed windows without taking down the voice daemon.
    builder.with_x11().with_any_thread(true);
    let event_loop = match builder.build() {
        Ok(event_loop) => event_loop,
        Err(error) => {
            let _ = ready_tx.send(Err(format!("create X11 event loop: {error}")));
            return;
        }
    };
    let shared = Arc::new(SurfaceShared {
        proxy: event_loop.create_proxy(),
        latest_frames: Mutex::new(HashMap::new()),
        frame_events_queued: Mutex::new(HashSet::new()),
        open_targets: Mutex::new(HashSet::new()),
    });
    if ready_tx.send(Ok(shared.clone())).is_err() {
        return;
    }
    let mut app = SurfaceApp {
        shared,
        event_tx: event_tx.clone(),
        windows: HashMap::new(),
        targets: HashMap::new(),
    };
    let run_result = event_loop.run_app(&mut app);
    if let Err(error) = run_result {
        let _ = event_tx.send(MediaEvent::SurfaceError {
            target: None,
            message: format!("surface event loop stopped: {error}"),
        });
    }
    // The process is already exiting, and the NVIDIA driver can fault in
    // vkDestroyDevice while tearing down a renderer that is still open. Leave
    // cached GPU objects to the operating system. Normal per-window closes only
    // unmap them, avoiding the same faulty driver teardown path.
    std::mem::forget(app);
}

struct SurfaceWindow {
    target: RemoteVideoTarget,
    window: Arc<Window>,
    renderer: Renderer,
    rendered_frames: u64,
}

struct SurfaceApp {
    shared: Arc<SurfaceShared>,
    event_tx: UnboundedSender<MediaEvent>,
    windows: HashMap<WindowId, SurfaceWindow>,
    targets: HashMap<RemoteVideoTarget, WindowId>,
}

impl SurfaceApp {
    fn open(&mut self, event_loop: &ActiveEventLoop, target: RemoteVideoTarget) {
        if self.targets.contains_key(&target) {
            return;
        }
        if let Some((window_id, surface)) = self
            .windows
            .iter_mut()
            .find(|(_, surface)| surface.target == target)
        {
            let window_id = *window_id;
            surface.window.set_visible(true);
            surface.window.request_redraw();
            self.shared
                .open_targets
                .lock()
                .expect("surface open-target lock poisoned")
                .insert(target.clone());
            self.targets.insert(target.clone(), window_id);
            let _ = self.event_tx.send(MediaEvent::SurfaceOpened { target });
            return;
        }
        let source = match target.source {
            VideoSource::ScreenShare => "screen",
            VideoSource::Camera => "camera",
        };
        let attributes = Window::default_attributes()
            .with_title(format!("Wisp — {} {source}", target.participant))
            .with_inner_size(LogicalSize::new(960.0, 540.0))
            .with_name("dev.wisp.surface", "dev.wisp.surface");
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                self.report_error(Some(target), format!("create Wisp video window: {error}"));
                return;
            }
        };
        let mut renderer = match pollster::block_on(Renderer::new(window.clone())) {
            Ok(renderer) => renderer,
            Err(error) => {
                self.report_error(
                    Some(target),
                    format!("initialize Wisp video renderer: {error:#}"),
                );
                return;
            }
        };
        if let Some(frame) = self
            .shared
            .latest_frames
            .lock()
            .expect("surface frame lock poisoned")
            .remove(&target)
        {
            renderer.upload(&frame);
        }
        let window_id = window.id();
        self.shared
            .open_targets
            .lock()
            .expect("surface open-target lock poisoned")
            .insert(target.clone());
        self.targets.insert(target.clone(), window_id);
        self.windows.insert(
            window_id,
            SurfaceWindow {
                target: target.clone(),
                window: window.clone(),
                renderer,
                rendered_frames: 0,
            },
        );
        window.request_redraw();
        let _ = self.event_tx.send(MediaEvent::SurfaceOpened { target });
    }

    fn close(&mut self, target: &RemoteVideoTarget) {
        let Some(window_id) = self.targets.remove(target) else {
            return;
        };
        self.shared
            .open_targets
            .lock()
            .expect("surface open-target lock poisoned")
            .remove(target);
        self.shared
            .frame_events_queued
            .lock()
            .expect("surface queued-frame lock poisoned")
            .remove(target);
        if let Some(surface) = self.windows.get(&window_id) {
            // Unlike Wayland, X11 can actually unmap the window. Keep its GPU
            // objects cached because this NVIDIA driver can fault while wgpu
            // destroys a live Vulkan device; reopening maps the same window.
            surface.window.set_visible(false);
        }
        let _ = self.event_tx.send(MediaEvent::SurfaceClosed {
            target: target.clone(),
        });
    }

    fn take_latest_frame(&mut self, target: &RemoteVideoTarget) {
        self.shared
            .frame_events_queued
            .lock()
            .expect("surface queued-frame lock poisoned")
            .remove(target);
        if let Some(frame) = self
            .shared
            .latest_frames
            .lock()
            .expect("surface frame lock poisoned")
            .remove(target)
            && let Some(window_id) = self.targets.get(target)
            && let Some(surface) = self.windows.get_mut(window_id)
        {
            surface.renderer.upload(&frame);
            surface.window.request_redraw();
        }
    }

    fn report_error(&self, target: Option<RemoteVideoTarget>, message: String) {
        let _ = self
            .event_tx
            .send(MediaEvent::SurfaceError { target, message });
    }
}

impl ApplicationHandler<SurfaceCommand> for SurfaceApp {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: SurfaceCommand) {
        match event {
            SurfaceCommand::Open(target) => self.open(event_loop, target),
            SurfaceCommand::Close(target) => self.close(&target),
            SurfaceCommand::FrameReady(target) => self.take_latest_frame(&target),
            SurfaceCommand::Shutdown => event_loop.exit(),
        }
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(target) = self
            .windows
            .get(&window_id)
            .map(|surface| surface.target.clone())
        else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => self.close(&target),
            WindowEvent::Resized(size) => {
                if let Some(surface) = self.windows.get_mut(&window_id) {
                    surface.renderer.resize(size.width, size.height);
                }
                let _ = self.event_tx.send(MediaEvent::SurfaceResized {
                    target,
                    width: size.width,
                    height: size.height,
                });
            }
            WindowEvent::RedrawRequested => {
                let render_result = self
                    .windows
                    .get_mut(&window_id)
                    .map(|surface| surface.renderer.render());
                match render_result {
                    Some(Ok(true)) => {
                        let surface = self
                            .windows
                            .get_mut(&window_id)
                            .expect("surface disappeared during redraw");
                        surface.rendered_frames = surface.rendered_frames.saturating_add(1);
                        if surface.rendered_frames == 1
                            || surface.rendered_frames.is_multiple_of(60)
                        {
                            let _ = self.event_tx.send(MediaEvent::SurfaceRendered {
                                target,
                                total: surface.rendered_frames,
                            });
                        }
                    }
                    Some(Ok(false)) | None => {}
                    Some(Err(error)) => {
                        self.close(&target);
                        self.report_error(Some(target), error.to_string());
                    }
                }
            }
            WindowEvent::Occluded(occluded) => {
                let _ = self.event_tx.send(MediaEvent::SurfaceVisibilityChanged {
                    target: target.clone(),
                    visible: !occluded,
                });
                if !occluded && let Some(surface) = self.windows.get(&window_id) {
                    surface.window.request_redraw();
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
    // Fields are dropped in declaration order. GPU resources and the surface
    // must be released before their device, window, and instance owners.
    frame: FrameTexture,
    sampler: wgpu::Sampler,
    texture_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    config: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    queue: wgpu::Queue,
    device: wgpu::Device,
    window: Arc<Window>,
    instance: wgpu::Instance,
}

impl Drop for Renderer {
    fn drop(&mut self) {
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
    }
}

impl Renderer {
    async fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        let size = window.inner_size();
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window.clone())
            .context("create wgpu X11 surface")?;
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
            frame,
            sampler,
            texture_layout,
            pipeline,
            config,
            surface,
            queue,
            device,
            window,
            instance,
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
