use std::{default, sync::Arc};

use cga2d::Multivector;
use eframe::{
    egui::{mutex::RwLock, Context, TextureId},
    egui_wgpu::Renderer,
    wgpu::{
        include_wgsl, util::DeviceExt, vertex_attr_array, BindGroupDescriptor, BindGroupEntry,
        BindGroupLayoutDescriptor, BindGroupLayoutEntry, BlendState, Buffer, BufferBinding,
        BufferDescriptor, BufferUsages, Color, ColorTargetState, ColorWrites,
        CommandEncoderDescriptor, Device, Extent3d, FragmentState, ImageCopyTexture,
        ImageDataLayout, MultisampleState, Operations, Origin3d, PipelineCompilationOptions,
        PipelineLayoutDescriptor, PrimitiveState, Queue, RenderPassColorAttachment,
        RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor, ShaderStages, Texture,
        TextureDescriptor, TextureUsages, TextureViewDescriptor, VertexBufferLayout, VertexState,
    },
};
use wgpu::TextureFormat;

use crate::{
    group::{Generator, Point},
    PuzzleInfo,
};

pub(crate) struct GfxData {
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub texture: Texture,
    pub texture_id: TextureId,
    pub pipeline: RenderPipeline,
    pub vertex_buffer: Buffer,
    pub param_buffer: Buffer,
    pub puzzle_buffer: Option<Buffer>,
    pub renderer: Arc<RwLock<Renderer>>,
}
impl GfxData {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let render_state = cc
            .wgpu_render_state
            .as_ref()
            .expect("We're not using wgpu, so we're screwed");
        let device = render_state.device.clone();

        // Create and register the texture
        let texture = create_texture(
            &device,
            Extent3d {
                width: 100,
                height: 100,
                depth_or_array_layers: 1,
            },
        );
        let renderer = render_state.renderer.clone();
        let texture_id = renderer.write().register_native_texture(
            &device,
            &texture.create_view(&TextureViewDescriptor::default()),
            eframe::wgpu::FilterMode::Nearest,
        );

        let queue = render_state.queue.clone();

        let pipeline = create_pipeline(&device, texture.format());

        // Create buffers
        let vertex_buffer = device.create_buffer_init(&eframe::wgpu::util::BufferInitDescriptor {
            label: Some("It can do what it wants"),
            contents: bytemuck::cast_slice(&[
                VertexInput::new([-3., -1.], [1., 0., 0., 1.]),
                VertexInput::new([1., -1.], [0., 1., 0., 1.]),
                VertexInput::new([1., 3.], [0., 0., 1., 1.]),
            ]),
            usage: BufferUsages::VERTEX,
        });

        let param_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("You'll init it every frame"),
            size: std::mem::size_of::<Params>() as _,
            usage: BufferUsages::COPY_DST | BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });

        let puzzle_buffer = None;

        GfxData {
            device,
            queue,
            texture,
            texture_id,
            pipeline,
            vertex_buffer,
            param_buffer,
            puzzle_buffer,
            renderer,
        }
    }

    pub fn regenerate_puzzle_buffer(&mut self, puzzle_info: &PuzzleInfo) {
        // Generate puzzle buffer (TODO: only when changed)
        let big: Vec<u32> =
            (0..puzzle_info.element_group.point_count())
                .flat_map(|x| {
                    let mut v = vec![puzzle_info.inverse_map[x as usize].0 as u32];
                    v.extend((0..puzzle_info.element_group.generator_count()).map(|y| {
                        puzzle_info.element_group.mul_gen(Point(x), Generator(y)).0 as u32
                    }));
                    v
                })
                .collect();
        self.puzzle_buffer = Some(self.device.create_buffer_init(
            &eframe::wgpu::util::BufferInitDescriptor {
                label: Some("It's big"),
                contents: bytemuck::cast_slice(&big),
                usage: BufferUsages::STORAGE,
            },
        ));
    }

    pub fn frame(&mut self, params: Params, width: u32, height: u32) {
        // Resize texture if it needs to
        let new_size = Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        if self.texture.size() != new_size {
            self.texture = create_texture(&self.device, new_size);
            self.renderer.write().update_egui_texture_from_wgpu_texture(
                &self.device,
                &self.texture.create_view(&TextureViewDescriptor::default()),
                eframe::wgpu::FilterMode::Nearest,
                self.texture_id,
            );
        }

        // Write params to the buffer
        self.queue
            .write_buffer(&self.param_buffer, 0, bytemuck::bytes_of(&params));

        let mut ce = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("I don't like that"),
            });

        // RENDER PASS HOURS
        {
            let binding = self.texture.create_view(&TextureViewDescriptor::default());
            let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
                label: Some("That's nice"),
                layout: &self.pipeline.get_bind_group_layout(0),
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: eframe::wgpu::BindingResource::Buffer(BufferBinding {
                            buffer: &self.param_buffer,
                            offset: 0,
                            size: None,
                        }),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: eframe::wgpu::BindingResource::Buffer(BufferBinding {
                            buffer: self.puzzle_buffer.as_ref().expect("How did we get here?"),
                            offset: 0,
                            size: None,
                        }),
                    },
                ],
            });
            let mut render_pass = ce.begin_render_pass(&RenderPassDescriptor {
                label: Some("Why so many labels"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &binding,
                    resolve_target: None,
                    ops: Operations::default(),
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_bind_group(0, &bind_group, &[]);

            render_pass.draw(0..3, 0..1);

            drop(render_pass);
        }

        self.queue.submit([ce.finish()]);
    }
}

#[derive(Debug, Default, Copy, Clone, bytemuck::NoUninit, bytemuck::Zeroable)]
#[repr(C)]
struct VertexInput {
    position: [f32; 2],
    padding: [f32; 2],
    color: [f32; 4],
}
impl VertexInput {
    fn new(position: [f32; 2], color: [f32; 4]) -> Self {
        Self {
            position,
            padding: [0., 0.],
            color,
        }
    }
}

#[derive(Debug, Default, Copy, Clone, bytemuck::NoUninit, bytemuck::Zeroable)]
#[repr(C)]
pub(crate) struct Params {
    pub mirrors: [[f32; 4]; 4],
    pub edges: [u32; 4],
    pub point: [f32; 4],
    pub scale: [f32; 2],
    pub col_scale: f32,
    pub depth: u32,
    pub fundamental: u32,
    pub col_tiles: u32,
    pub inverse_col: u32,
    pub mirror_count: u32,
}
impl Params {
    pub fn new(
        mirrors: Vec<cga2d::Blade3>,
        edges: Vec<bool>,
        point: cga2d::Blade1,
        scale: [f32; 2],
        col_scale: f32,
        depth: u32,
        fundamental: bool,
        col_tiles: bool,
        inverse_col: bool,
    ) -> Self {
        let mirror_count = mirrors.len() as u32;

        let mut out_mirrors = [[0.; 4]; 4];
        let mut out_edges = [0; 4];

        for (i, &mirror) in mirrors.iter().enumerate() {
            out_mirrors[i] = rep_mirror(mirror);
            out_edges[i] = edges[i].into();
        }

        Self {
            mirrors: out_mirrors,
            edges: out_edges,
            point: [
                point.m as f32,
                point.p as f32,
                point.x as f32,
                point.y as f32,
            ],
            scale,
            col_scale,
            depth,
            fundamental: if fundamental { 1 } else { 0 },
            col_tiles: if col_tiles { 1 } else { 0 },
            inverse_col: if inverse_col { 1 } else { 0 },
            mirror_count,
        }
    }
}

fn rep_mirror(mirror: cga2d::Blade3) -> [f32; 4] {
    let m = !mirror.normalize();
    [m.m as f32, m.p as f32, m.x as f32, m.y as f32]
}

fn create_texture(device: &Device, size: Extent3d) -> Texture {
    device.create_texture(&TextureDescriptor {
        label: Some("Placeholder"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: eframe::wgpu::TextureDimension::D2,
        format: eframe::wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: TextureUsages::TEXTURE_BINDING
            | TextureUsages::RENDER_ATTACHMENT
            | TextureUsages::COPY_DST,
        view_formats: &[eframe::wgpu::TextureFormat::Rgba8UnormSrgb],
    })
}

fn create_pipeline(device: &Device, texture_format: TextureFormat) -> RenderPipeline {
    let module = device.create_shader_module(include_wgsl!("shader.wgsl"));

    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("Construct additional labels"),
        layout: Some(&device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Lay lay lay lay label"),
            bind_group_layouts: &[
                &device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                    label: Some("At some point I stopped labelling them"),
                    entries: &[
                        BindGroupLayoutEntry {
                            binding: 0,
                            visibility: ShaderStages::VERTEX_FRAGMENT,
                            ty: eframe::wgpu::BindingType::Buffer {
                                ty: eframe::wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        BindGroupLayoutEntry {
                            binding: 1,
                            visibility: ShaderStages::FRAGMENT,
                            ty: eframe::wgpu::BindingType::Buffer {
                                ty: eframe::wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                    ],
                }),
            ],
            push_constant_ranges: &[],
        })),
        vertex: VertexState {
            module: &module,
            entry_point: "vertex",
            compilation_options: PipelineCompilationOptions::default(),
            buffers: &[VertexBufferLayout {
                array_stride: 32,
                step_mode: eframe::wgpu::VertexStepMode::Vertex,
                attributes: &vertex_attr_array![0 => Float32x2, 10 => Float32x2, 1 => Float32x4],
            }],
        },
        primitive: PrimitiveState {
            topology: eframe::wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: MultisampleState::default(),
        fragment: Some(FragmentState {
            module: &module,
            entry_point: "fragment",
            compilation_options: PipelineCompilationOptions::default(),
            targets: &[Some(ColorTargetState {
                format: texture_format,
                blend: Some(BlendState::ALPHA_BLENDING),
                write_mask: ColorWrites::all(),
            })],
        }),
        multiview: None,
    })
}
