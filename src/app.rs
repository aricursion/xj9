use egui_winit::winit;
use egui_winit::winit::{
    event::*,
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};
use wgpu::util::DeviceExt;

use mupdf::document::Document;
use mupdf::{self, Matrix, Point};

use crate::texture;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    tex_coords: [f32; 2],
    bg_color: [f32; 3]
}

const SQUARE: &[Vertex] = &[
    Vertex {
        position: [-1.0, 1.0, 0.0],
        tex_coords: [0.0, 0.0],
        bg_color: [255.0, 255.0, 255.0]
    },
    Vertex {
        position: [-1.0, -1.0, 0.0],
        tex_coords: [0.0, 1.0],
        bg_color: [255.0, 255.0, 255.0]
    },
    Vertex {
        position: [1.0, -1.0, 0.0],
        tex_coords: [1.0, 1.0],
        bg_color: [255.0, 255.0, 255.0]
    },
    Vertex {
        position: [1.0, 1.0, 0.0],
        tex_coords: [1.0, 0.0],
        bg_color: [255.0, 255.0, 255.0]
    },
];

const SQUARE_INDICES: &[u16] = &[0, 1, 3, 1, 2, 3];

impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 5]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

struct State {
    //graphics data
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    window: Window,
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_indices: u32,
    diffuse_bind_group: wgpu::BindGroup,
    diffuse_texture: texture::Texture,
    //interal data
    doc: mupdf::Document,
    toc: bool,
    page: i32,
    pos: Point,
    res: f32,
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
        let res = 3.0;

        let page0 = doc.load_page(0).unwrap();
        let pixmap0 = page0
            .to_pixmap(
                &Matrix::new_scale(res, res),
                &mupdf::Colorspace::device_rgb(),
                1.0,
                false,
            )
            .unwrap();
        let pixels: &[u8] = bytemuck::cast_slice(pixmap0.pixels().unwrap());
        let mut new_pixels: Vec<u8> = pixels.iter().map(|&i| i as u8).collect();
        let diffuse_texture = texture::Texture::from_bytes(
            &device,
            &queue,
            &*(new_pixels.into_boxed_slice()),
            pixmap0.width(),
            pixmap0.height(),
            "page0",
        )
        .unwrap();


        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        // This should match the filterable field of the
                        // corresponding Texture entry above.
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        let diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                },
            ],
            label: Some("diffuse_bind_group"),
        });

        let shader = device.create_shader_module(wgpu::include_wgsl!("./shader.wgsl"));

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&texture_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                polygon_mode: wgpu::PolygonMode::Fill,
                // Requires Features::DEPTH_CLIP_CONTROL
                unclipped_depth: false,
                // Requires Features::CONSERVATIVE_RASTERIZATION
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(SQUARE),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(SQUARE_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });
        let num_indices = SQUARE_INDICES.len() as u32;

        Self {
            //graphics data
            window,
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            vertex_buffer,
            index_buffer,
            num_indices,
            diffuse_bind_group,
            diffuse_texture,
            //internal data
            doc,
            page: 0,
            toc: false,
            pos: Point { x: 0.0, y: 0.0 },
            res,
            color: wgpu::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            },
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
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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

            let buf;
            let idxbuf;
            let numidx;
            buf = &self.vertex_buffer;
            idxbuf = &self.index_buffer;
            numidx = self.num_indices;

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.diffuse_bind_group, &[]);
            render_pass.set_vertex_buffer(0, buf.slice(..));
            render_pass.set_index_buffer(idxbuf.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..numidx, 0, 0..1);
        }

        // submit will accept anything that implements IntoIter
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    fn update_page_texture(&mut self) {
        let page = self.doc.load_page(self.page).unwrap();
        let mut pixmap = page
            .to_pixmap(
                &Matrix::new_scale(self.res, self.res),
                &mupdf::Colorspace::device_rgb(),
                1.0,
                false,
            )
            .unwrap();
        pixmap.set_resolution(pixmap.resolution().0 *10, pixmap.resolution().1 *10);
        let pixels: &[u8] = bytemuck::cast_slice(pixmap.pixels().unwrap());
        let mut new_pixels: Vec<u8> = pixels.iter().map(|&i| i as u8).collect();

        let new_texture = texture::Texture::from_bytes(
            &self.device,
            &self.queue,
            &*new_pixels.into_boxed_slice(),
            pixmap.width(),
            pixmap.height(),
            "newpage",
        )
        .unwrap();
        self.diffuse_texture = new_texture;
        let texture_bind_group_layout =
            self.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: false,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            // This should match the filterable field of the
                            // corresponding Texture entry above.
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                    label: Some("texture_bind_group_layout"),
                });

        self.diffuse_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.diffuse_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.diffuse_texture.sampler),
                },
            ],
            label: Some("diffuse_bind_group"),
        });
        return;
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

    let doc = Document::open(filename.as_str()).unwrap();
    let pixmap = doc
        .load_page(0)
        .unwrap()
        .to_pixmap(
            &Matrix::IDENTITY,
            &mupdf::Colorspace::device_rgb(),
            1.0,
            false,
        )
        .unwrap();

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title(format!("{}", prettyname))
        .with_inner_size(winit::dpi::PhysicalSize::new(pixmap.width(), pixmap.height()))
        .build(&event_loop)
        .unwrap();

    let mut state = State::new(window, doc).await;

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
                            virtual_keycode: kc,
                            ..
                        },
                    ..
                } => match kc {
                    Some(VirtualKeyCode::Escape) => {
                        control_flow.set_exit();
                    }
                    Some(VirtualKeyCode::Down) => {
                        state.page += 1;
                        state.update_page_texture();
                    }
                    Some(VirtualKeyCode::Up) => {
                        state.page -= 1;
                        state.update_page_texture();
                    }
                    _ => {}
                },
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
                        eprint!("{:?}", e);
                        control_flow.set_exit();
                    }
                };
            }
            _ => (),
        }
    });
}
