use egui_winit::winit;
use egui_winit::winit::dpi::PhysicalSize;
use egui_winit::winit::{
    event::*,
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};
use mupdf::document::Document;
use mupdf::{self, Point};

struct State {
    //graphics data
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    window: Window,

    //interal data
    doc: mupdf::Document,
    toc: bool,
    pos: Point,
    scale: i32,
    color: wgpu::Color,
}

impl State {
    // Creating some of the wgpu types requires async code
    // Generate a new State from a window and doc
    async fn new(window: Window, doc: mupdf::Document) -> Self {
        let size = window.inner_size();

        // The instance is a handle to our GPU
        // Backends::all => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
        });

        // # Safety
        //
        // The surface needs to live as long as the window that created it.
        // State owns the window so this should be safe.
        let surface = unsafe { instance.create_surface(&window) }.unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::empty(),
                    // WebGL doesn't support all of wgpu's features, so if
                    // we're building for the web we'll have to disable some.
                    limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                    label: None,
                },
                None, // Trace path
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        // Shader code in this tutorial assumes an sRGB surface texture. Using a different
        // one will result all the colors coming out darker. If you want to support non
        // sRGB surfaces, you'll need to account for that when drawing to the frame.
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        Self {
            //graphics data
            window,
            surface,
            device,
            queue,
            config,
            size,
            //internal data
            doc,
            toc: false,
            pos: Point { x: 0.0, y: 0.0 },
            scale: 1,
            color: wgpu::Color {r: 0.0, g: 0.0, b: 0.0, a: 0.0},
        }
    }

    // simply return the window
    fn window(&self) -> &Window {
        return &self.window;
    }

    // handle resizing
    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.size = new_size;
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
    }
    // whether there's an input event
    fn input(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                self.color = wgpu::Color {
                    r: position.x / self.size.width as f64,
                    g: position.y / self.size.width as f64,
                    b: 1.0,
                    a: 1.0,
                };
                return true;
            }
            _ => {
                return false;
            }
        }
    }

    fn update(&mut self) {
        //todo!()
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });
        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.color),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });
        }

        // submit will accept anything that implements IntoIter
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

pub async fn run() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        Err::<(), &str>("Please provide the file name").unwrap();
    }
    let filename = args.last().take().unwrap().to_string();
    let prettyname = {
        let path = std::path::Path::new(&filename);
        String::from(path.file_name().unwrap().to_str().unwrap())
    };
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title(format!("{}", prettyname))
        .with_inner_size(winit::dpi::LogicalSize::new(500, 500))
        .build(&event_loop)
        .unwrap();

    let doc = Document::open(filename.as_str()).unwrap();

    let mut state = State::new(window, doc).await;

    let pdf_doc = state
        .doc
        .convert_to_pdf(0, state.doc.page_count().unwrap(), 0)
        .unwrap();
    let page0 = pdf_doc.load_page(0).unwrap();

    event_loop.run(move |event, _, control_flow| {
        control_flow.set_wait();
        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == state.window().id() && !state.input(event) => match event {
                WindowEvent::Resized(physical_size) => {
                    state.resize(*physical_size);
                }
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    // new_inner_size is &&mut so we have to dereference it twice
                    state.resize(**new_inner_size);
                }
                WindowEvent::CloseRequested => {
                    control_flow.set_exit();
                }
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            state: ElementState::Pressed,
                            virtual_keycode: Some(VirtualKeyCode::Escape),
                            ..
                        },
                    ..
                } => {
                    control_flow.set_exit();
                }
                _ => {}
            },
            Event::MainEventsCleared => {
                // Application update code.

                // Queue a RedrawRequested event.
                //
                // You only need to call this if you've determined that you need to redraw, in
                // applications which do not always need to. Applications that redraw continuously
                // can just render here instead.
                state.window.request_redraw();
            }
            Event::RedrawRequested(window_id) if window_id == state.window().id() => {
                // Redraw the application.
                //
                // It's preferable for applications that do not render continuously to render in
                // this event rather than in MainEventsCleared, since rendering in here allows
                // the program to gracefully handle redraws requested by the OS.
                state.update();
                match state.render() {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Timeout) => state.resize(state.size),
                    Err(wgpu::SurfaceError::OutOfMemory) => {
                        eprintln!("Out of memory. Exiting.");
                        control_flow.set_exit();
                    }
                    Err(e) => {
                        eprint!("{:?}", e)
                    }
                };
            }
            _ => (),
        }
    });
}
