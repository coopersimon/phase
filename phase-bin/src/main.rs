mod debug;

use wgpu::Extent3d;
use winit::{
    application::ApplicationHandler, dpi::{
        LogicalSize, Size, PhysicalSize
    }, event::{
        ElementState, WindowEvent
    }, event_loop::{
        EventLoop
    }, window::Window, keyboard::{PhysicalKey, KeyCode}
};
use cpal::traits::StreamTrait;
use gilrs::{Gilrs};

use std::path::PathBuf;

use phase::*;
use clap::Parser;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    bios: String,

    #[arg(short, long)]
    debug: bool,

    #[arg(short, long)]
    game: Option<String>,

    #[arg(short = 'c', long)]
    memcard1: Option<String>,

    #[arg(long)]
    memcard2: Option<String>,

    #[arg(short, long)]
    mute: bool,
}

fn main() {
    let args = Args::parse();

    let config = PlayStationConfig {
        bios_path: PathBuf::from(args.bios)
    };
    let mut playstation = PlayStation::new(config);
    let game_disc = args.game.map(|s| s.try_into().expect("invalid path"));
    if let Some(memcard) = args.memcard1 {
        playstation.insert_mem_card(memcard.try_into().expect("invalid memcard path"), Port::One);
    }
    if let Some(memcard2) = args.memcard2 {
        playstation.insert_mem_card(memcard2.try_into().expect("invalid memcard2 path"), Port::Two);
    }

    if args.debug {
        debug::debug_mode(playstation.make_debugger());
    } else {
        run(playstation, game_disc, args.mute);
    }
}

/// Run playstation with visuals.
fn run(mut playstation: PlayStation, game_disc: Option<PathBuf>, mute: bool) {
    let event_loop = EventLoop::new().expect("Failed to create event loop");

    let audio_stream = make_audio_stream(&mut playstation, mute);

    let mut app = App::new(playstation, game_disc, audio_stream);

    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    event_loop.run_app(&mut app).unwrap();
}

// TODO: move inside phase core
const FRAME_TIME: chrono::Duration = chrono::Duration::nanoseconds(1_000_000_000 / 60);

struct WindowState {
    window:         std::sync::Arc<Window>,
    surface:        wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
}

impl WindowState {
    fn resize_surface(&mut self, size: PhysicalSize<u32>, device: &wgpu::Device) {
        self.surface_config.width = size.width;
        self.surface_config.height = size.height;
        self.surface.configure(device, &self.surface_config);
    }
}

struct App {
    window: Option<WindowState>,
    console: PlayStation,
    inserted_disc: Option<PathBuf>,

    // WGPU params
    instance:        wgpu::Instance,
    adapter:         wgpu::Adapter,
    device:          wgpu::Device,
    queue:           wgpu::Queue,
    render_pipeline: wgpu::RenderPipeline,

    sampler:         wgpu::Sampler,
    bind_group:      Option<wgpu::BindGroup>,
    texture:         Option<wgpu::Texture>,

    frame:           Frame,
    last_frame_time: chrono::DateTime<chrono::Utc>,

    audio_stream: cpal::Stream,

    controllers:    Gilrs,
}

impl App {
    fn new(console: PlayStation, game_disc: Option<PathBuf>, audio_stream: cpal::Stream) -> Self {
        // Setup wgpu
        let instance = wgpu::Instance::new(&Default::default());

        let adapter = futures::executor::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            compatible_surface: None,
        })).expect("Failed to find appropriate adapter");

        let (device, queue) = futures::executor::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            ..Default::default()
        })).expect("Failed to create device");

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None
                },
            ]
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter:     wgpu::FilterMode::Nearest,
            min_filter:     wgpu::FilterMode::Linear,
            mipmap_filter:  wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let shader_module = device.create_shader_module(wgpu::include_wgsl!("./shaders/display.wgsl"));

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default()
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                .. Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default()
            }),
            multiview_mask: None,
            cache: None
        });
        
        Self {
            window: None,
            console,
            inserted_disc: game_disc,

            instance,
            adapter,
            device,
            queue,
            render_pipeline,

            sampler,
            texture: None,
            bind_group: None,

            frame:           Frame::new(),
            last_frame_time: chrono::Utc::now(),

            audio_stream: audio_stream,

            controllers: Gilrs::new().unwrap(),
        }
    }

    fn create_texture(&mut self, size: (usize, usize)) {
        println!("output resolution: ({}, {})", size.0, size.1);

        let texture_extent = wgpu::Extent3d {
            width: size.0 as u32,
            height: size.1 as u32,
            depth_or_array_layers: 1
        };

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: texture_extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[wgpu::TextureFormat::Rgba8UnormSrgb]
        });
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.render_pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view)
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler)
                }
            ],
            label: None
        });

        self.texture = Some(texture);
        self.bind_group = Some(bind_group);
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.console.run_cpu();
        self.console.attach_controller(ControllerType::Digital, Port::One);
        if let Some(disc) = self.inserted_disc.clone() {
            self.console.insert_cd(disc);
        }

        let window_attrs = Window::default_attributes()
            .with_inner_size(Size::Logical(LogicalSize{width: 640.0, height: 480.0}))
            .with_title("Phase");
        let window = std::sync::Arc::new(event_loop.create_window(window_attrs).unwrap());

        // Setup wgpu
        let surface = self.instance.create_surface(window.clone()).expect("Failed to create surface");

        let size = window.inner_size();
        let surface_config = surface.get_default_config(&self.adapter, size.width, size.height).expect("Could not get default surface config");
        surface.configure(&self.device, &surface_config);

        self.window = Some(WindowState {
            window, surface, surface_config
        });

        self.last_frame_time = chrono::Utc::now();
    
        // AUDIO
        self.audio_stream.play().expect("Couldn't start audio stream");

        //let mut in_focus = true;
    }

    fn window_event(
            &mut self,
            event_loop: &winit::event_loop::ActiveEventLoop,
            _window_id: winit::window::WindowId,
            event: WindowEvent,
        ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            },
            WindowEvent::Resized(size) => {
                self.window.as_mut().unwrap().resize_surface(size, &self.device);
            },
            WindowEvent::RedrawRequested => {
                let now = chrono::Utc::now();
                let since_last_frame = now.signed_duration_since(self.last_frame_time);
                if since_last_frame >= FRAME_TIME {
                    //println!("frame time {}", since_last_frame);
                    self.last_frame_time = now;
    
                    self.console.frame(&mut self.frame);
    
                    if let Some(texture) = self.texture.as_ref() {
                        if texture.width() != (self.frame.size.0 as u32) ||
                            texture.height() != (self.frame.size.1 as u32) {
                            self.create_texture(self.frame.size);
                        }
                    } else {
                        self.create_texture(self.frame.size);
                    }

                    let texture = self.texture.as_ref().unwrap();
                    self.queue.write_texture(
                        texture.as_image_copy(),
                        &self.frame.frame_buffer, 
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(4 * texture.width()),
                            rows_per_image: None,
                        },
                        Extent3d {
                            width: texture.width(),
                            height: texture.height(),
                            depth_or_array_layers: 1
                        }
                    );
    
                    let frame = self.window.as_ref().unwrap().surface.get_current_texture().expect("Timeout when acquiring next swapchain tex.");
                    let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {label: None});
    
                    {
                        let view = frame.texture.create_view(&Default::default());
                        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: None,
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &view,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                                    store: wgpu::StoreOp::Store,
                                },
                                depth_slice: None,
                                resolve_target: None,
                            })],
                            depth_stencil_attachment: None,
                            ..Default::default()
                        });
                        rpass.set_pipeline(&self.render_pipeline);
                        rpass.set_bind_group(0, &self.bind_group, &[]);
                        rpass.draw(0..4, 0..1);
                    }
    
                    self.queue.submit([encoder.finish()]);
                    frame.present();

                    while let Some(gilrs::Event { event, .. }) = self.controllers.next_event() {
                        use gilrs::EventType::*;
                        match event {
                            Connected => {},
                            Disconnected => {},
                            ButtonPressed(button, _) => if let Some(button) = map_button(button) {
                                self.console.press_button(Port::One, button, true);
                            },
                            ButtonReleased(button, _) => if let Some(button) = map_button(button) {
                                self.console.press_button(Port::One, button, false);
                            },
                            AxisChanged(axis, value, _) => if let Some(stick) = map_stick(axis) {
                                self.console.update_stick_axis(Port::One, stick, value);
                            },
                            _ => {}
                        }
                    }
                }
                self.window.as_ref().unwrap().window.request_redraw();
            },
            WindowEvent::KeyboardInput { device_id: _, event, is_synthetic: _ } => {
                let pressed = match event.state {
                    ElementState::Pressed => true,
                    ElementState::Released => false,
                };
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::KeyX)        => self.console.press_button(Port::One, Button::Cross, pressed),
                    PhysicalKey::Code(KeyCode::KeyZ)        => self.console.press_button(Port::One, Button::Circle, pressed),
                    PhysicalKey::Code(KeyCode::KeyD)        => self.console.press_button(Port::One, Button::Triangle, pressed),
                    PhysicalKey::Code(KeyCode::KeyC)        => self.console.press_button(Port::One, Button::Square, pressed),
                    PhysicalKey::Code(KeyCode::KeyA)        => self.console.press_button(Port::One, Button::L1, pressed),
                    PhysicalKey::Code(KeyCode::KeyS)        => self.console.press_button(Port::One, Button::R1, pressed),
                    PhysicalKey::Code(KeyCode::KeyQ)        => self.console.press_button(Port::One, Button::L2, pressed),
                    PhysicalKey::Code(KeyCode::KeyW)        => self.console.press_button(Port::One, Button::R2, pressed),
                    PhysicalKey::Code(KeyCode::Space)       => self.console.press_button(Port::One, Button::Select, pressed),
                    PhysicalKey::Code(KeyCode::Enter)       => self.console.press_button(Port::One, Button::Start, pressed),
                    PhysicalKey::Code(KeyCode::ArrowUp)     => self.console.press_button(Port::One, Button::DUp, pressed),
                    PhysicalKey::Code(KeyCode::ArrowDown)   => self.console.press_button(Port::One, Button::DDown, pressed),
                    PhysicalKey::Code(KeyCode::ArrowLeft)   => self.console.press_button(Port::One, Button::DLeft, pressed),
                    PhysicalKey::Code(KeyCode::ArrowRight)  => self.console.press_button(Port::One, Button::DRight, pressed),
                    _ => {},
                }
            },
            _ => {}
        }
    }
}

fn make_audio_stream(console: &mut PlayStation, mute: bool) -> cpal::Stream {
    use cpal::traits::{
        DeviceTrait,
        HostTrait
    };

    let host = cpal::default_host();
    let device = host.default_output_device().expect("no output device available.");

    let config = pick_output_config(&device).with_max_sample_rate();
    let sample_rate = config.sample_rate().0 as f64;
    println!("Target audio sample rate {}", sample_rate);
    let mut audio_handler = console.enable_audio(sample_rate).unwrap();

    device.build_output_stream(
        &config.into(),
        move |data: &mut [f32], _| {
            audio_handler.get_audio_packet(data);
            if mute {
                for d in data.iter_mut() {
                    *d = 0.0;
                }
            }
        },
        move |err| {
            println!("Error occurred: {}", err);
        }
    ).unwrap()
}

fn pick_output_config(device: &cpal::Device) -> cpal::SupportedStreamConfigRange {
    use cpal::traits::DeviceTrait;

    const MIN: u32 = 32_000;

    let supported_configs_range = device.supported_output_configs()
        .expect("error while querying configs");

    for config in supported_configs_range {
        let cpal::SampleRate(v) = config.max_sample_rate();
        if v >= MIN {
            return config;
        }
    }

    device.supported_output_configs()
        .expect("error while querying formats")
        .next()
        .expect("No supported config")
}

fn map_button(input: gilrs::Button) -> Option<Button> {
    use gilrs::Button::*;
    match input {
        South           => Some(Button::Cross),
        East            => Some(Button::Circle),
        West            => Some(Button::Square),
        North           => Some(Button::Triangle),
        DPadUp          => Some(Button::DUp),
        DPadRight       => Some(Button::DRight),
        DPadLeft        => Some(Button::DLeft),
        DPadDown        => Some(Button::DDown),
        Select          => Some(Button::Select),
        Start           => Some(Button::Start),
        LeftTrigger     => Some(Button::L1),
        LeftTrigger2    => Some(Button::L2),
        RightTrigger    => Some(Button::R1),
        RightTrigger2   => Some(Button::R2),
        LeftThumb       => Some(Button::L3),
        RightThumb      => Some(Button::R3),
        _ => None,
    }
}

fn map_stick(input: gilrs::Axis) -> Option<AnalogStickAxis> {
    use gilrs::Axis::*;
    match input {
        LeftStickX  => Some(AnalogStickAxis::LeftX),
        LeftStickY  => Some(AnalogStickAxis::LeftY),
        RightStickX => Some(AnalogStickAxis::RightX),
        RightStickY => Some(AnalogStickAxis::RightY),
        _ => None,
    }
}