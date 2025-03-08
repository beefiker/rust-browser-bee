use wgpu::{ self, util::DeviceExt };
use winit::{
    event::{ Event, WindowEvent },
    event_loop::{ ControlFlow, EventLoop },
    window::{ Window, WindowBuilder },
};

use crate::layout;
use crate::command::DisplayCommand;

const SCREEN_WIDTH: u32 = 1024;
const SCREEN_HEIGHT: u32 = 768;
const CLEAR_COLOR: wgpu::Color = wgpu::Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],
    color: [f32; 3],
}

impl Vertex {
    const ATTRIBUTES: [
        wgpu::VertexAttribute;
        2
    ] = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x3];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

struct State {
    window: Window,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_indices: u32,
}

impl State {
    async fn new(window: Window, vertices: &[Vertex], indices: &[u16]) -> Self {
        let instance_desc = wgpu::InstanceDescriptor::default();
        let instance = wgpu::Instance::new(&instance_desc);
        let size = window.inner_size();

        let surface_target = (unsafe { wgpu::SurfaceTargetUnsafe::from_window(&window) }).unwrap();
        let surface = (unsafe { instance.create_surface_unsafe(surface_target) }).unwrap();

        let adapter = instance
            .request_adapter(
                &(wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::default(),
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                })
            ).await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &(wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: Default::default(),
                }),
                None
            ).await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps.formats
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
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let render_pipeline_layout = device.create_pipeline_layout(
            &(wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[],
                push_constant_ranges: &[],
            })
        );

        let render_pipeline = device.create_render_pipeline(
            &(wgpu::RenderPipelineDescriptor {
                label: Some("Render Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Vertex::desc()],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[
                        Some(wgpu::ColorTargetState {
                            format: config.format,
                            blend: Some(wgpu::BlendState {
                                color: wgpu::BlendComponent {
                                    src_factor: wgpu::BlendFactor::SrcAlpha,
                                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                                    operation: wgpu::BlendOperation::Add,
                                },
                                alpha: wgpu::BlendComponent {
                                    src_factor: wgpu::BlendFactor::One,
                                    dst_factor: wgpu::BlendFactor::One,
                                    operation: wgpu::BlendOperation::Add,
                                },
                            }),
                            write_mask: wgpu::ColorWrites::ALL,
                        }),
                    ],
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
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
                cache: None,
            })
        );

        let vertex_buffer = device.create_buffer_init(
            &(wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            })
        );

        let index_buffer = device.create_buffer_init(
            &(wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: bytemuck::cast_slice(indices),
                usage: wgpu::BufferUsages::INDEX,
            })
        );

        let num_indices = indices.len() as u32;

        Self {
            window,
            surface,
            device,
            queue,
            config,
            render_pipeline,
            vertex_buffer,
            index_buffer,
            num_indices,
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(
            &(wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            })
        );

        {
            let mut render_pass = encoder.begin_render_pass(
                &(wgpu::RenderPassDescriptor {
                    label: Some("Render Pass"),
                    color_attachments: &[
                        Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(CLEAR_COLOR),
                                store: wgpu::StoreOp::Store,
                            },
                        }),
                    ],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                })
            );

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..self.num_indices, 0, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

fn render_commands(command_list: &[DisplayCommand]) -> (Vec<Vertex>, Vec<u16>) {
    let mut vertices = Vec::new();
    let mut index_data = Vec::new();
    let mut rect_num: u16 = 0;

    for command in command_list {
        match *command {
            DisplayCommand::SolidRectangle(ref color, ref rect) => {
                let c = [color.r, color.g, color.b];
                let mut v = render_rectangle(&c, rect);
                vertices.append(&mut v);

                let index_base = rect_num * 4;
                // First triangle (top-left, top-right, bottom-left)
                index_data.extend_from_slice(&[index_base, index_base + 1, index_base + 2]);
                // Second triangle (bottom-left, top-right, bottom-right)
                index_data.extend_from_slice(&[index_base + 2, index_base + 1, index_base + 3]);
                rect_num += 1;
            }
        }
    }
    (vertices, index_data)
}

fn render_rectangle(c: &[f32; 3], rect: &layout::Rectangle) -> Vec<Vertex> {
    let (x, y, h, w) = transform_rectangle(rect);
    vec![
        // Top-left
        Vertex {
            position: [x, y],
            color: *c,
        },
        // Top-right
        Vertex {
            position: [x + w, y],
            color: *c,
        },
        // Bottom-left
        Vertex {
            position: [x, y - h],
            color: *c,
        },
        // Bottom-right
        Vertex {
            position: [x + w, y - h],
            color: *c,
        }
    ]
}

fn transform_rectangle(rect: &layout::Rectangle) -> (f32, f32, f32, f32) {
    let w = (rect.width / (SCREEN_WIDTH as f32)) * 2.0;
    let h = (rect.height / (SCREEN_HEIGHT as f32)) * 2.0;
    let x = (rect.x / (SCREEN_WIDTH as f32)) * 2.0 - 1.0;
    // Convert from screen coordinates (0,0 at top-left) to clip space (-1,1 at center)
    let y = 1.0 - (rect.y / (SCREEN_HEIGHT as f32)) * 2.0;
    (x, y, h, w)
}

pub fn render_loop(command_list: &[DisplayCommand]) {
    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new()
        .with_title("Browser")
        .with_inner_size(winit::dpi::PhysicalSize::new(SCREEN_WIDTH, SCREEN_HEIGHT))
        .build(&event_loop)
        .unwrap();

    let (vertices, indices) = render_commands(command_list);
    let mut state = pollster::block_on(State::new(window, &vertices, &indices));

    // Request initial redraw
    state.window.request_redraw();

    event_loop
        .run(move |event, target| {
            target.set_control_flow(ControlFlow::Wait);

            match event {
                Event::WindowEvent { ref event, window_id } if window_id == state.window.id() =>
                    match event {
                        | WindowEvent::CloseRequested
                        | WindowEvent::KeyboardInput {
                              event: winit::event::KeyEvent {
                                  logical_key: winit::keyboard::Key::Named(
                                      winit::keyboard::NamedKey::Escape,
                                  ),
                                  ..
                              },
                              ..
                          } => target.exit(),
                        WindowEvent::Resized(physical_size) => {
                            state.config.width = physical_size.width;
                            state.config.height = physical_size.height;
                            state.surface.configure(&state.device, &state.config);
                            state.window.request_redraw();
                        }
                        WindowEvent::RedrawRequested => {
                            match state.render() {
                                Ok(_) => {}
                                Err(wgpu::SurfaceError::Lost) => {
                                    state.surface.configure(&state.device, &state.config);
                                    state.window.request_redraw();
                                }
                                Err(wgpu::SurfaceError::OutOfMemory) => target.exit(),
                                Err(e) => eprintln!("{:?}", e),
                            }
                        }
                        _ => {}
                    }
                _ => {}
            }
        })
        .unwrap();
}
