use std::sync::Arc;

use cga2d::Multivector;
use eframe::{
    egui::{mutex::RwLock, TextureId},
    egui_wgpu::Renderer,
    wgpu::{
        include_wgsl, util::DeviceExt, vertex_attr_array, BindGroupDescriptor, BindGroupEntry,
        BindGroupLayoutDescriptor, BindGroupLayoutEntry, BlendState, Buffer, BufferBinding,
        BufferDescriptor, BufferUsages, ColorTargetState, ColorWrites, CommandEncoderDescriptor,
        Device, Extent3d, FragmentState, MultisampleState, Operations, PipelineCompilationOptions,
        PipelineLayoutDescriptor, PrimitiveState, Queue, RenderPassColorAttachment,
        RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor, ShaderStages, Texture,
        TextureDescriptor, TextureUsages, TextureViewDescriptor, VertexBufferLayout, VertexState,
    },
};
use wgpu::TextureFormat;

use crate::{
    config::ViewSettings,
    conformal_puzzle::ConformalPuzzle,
    group::{Generator, Point},
};

pub(crate) struct GfxData {
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub texture: Texture,
    pub texture_id: TextureId,
    pub pipeline: RenderPipeline,
    pub vertex_buffer: Buffer,
    pub param_buffer: Buffer,
    pub coset_buffer: Option<Buffer>,
    pub sticker_buffer: Option<Buffer>,
    pub cut_buffer: Option<Buffer>,
    pub outline_buffer: Option<Buffer>,
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

        let coset_buffer = None;
        let sticker_buffer = None;
        let cut_buffer = None;
        let outline_buffer = None;

        GfxData {
            device,
            queue,
            texture,
            texture_id,
            pipeline,
            vertex_buffer,
            param_buffer,
            coset_buffer,
            sticker_buffer,
            cut_buffer,
            outline_buffer,
            renderer,
        }
    }

    pub fn regenerate_puzzle_buffers(
        &mut self,
        camera_transform: cga2d::Rotoflector,
        puzzle: &ConformalPuzzle,
    ) {
        // Generate puzzle buffer (TODO: only when changed)

        // LUT to multiply group elements and find C0*E' from E
        let coset_buffer: Vec<u32> = (0..puzzle.puzzle.elem_group.point_count())
            .flat_map(|x| {
                let mut v = vec![
                    if let Some(p) = puzzle.quotient_group.inverse_map[x as usize] {
                        p.0 as u32
                    } else {
                        u32::MAX
                    },
                ];
                v.extend((0..puzzle.puzzle.elem_group.generator_count()).map(|y| {
                    if let Some(p) = puzzle.puzzle.elem_group.mul_gen(&Point(x), &Generator(y)) {
                        p.0 as u32
                    } else {
                        u32::MAX
                    }
                }));
                v
            })
            .collect();
        self.coset_buffer = Some(self.device.create_buffer_init(
            &eframe::wgpu::util::BufferInitDescriptor {
                label: Some("It's big"),
                contents: bytemuck::cast_slice(&coset_buffer),
                usage: BufferUsages::STORAGE,
            },
        ));

        self.regenerate_cut_buffer(camera_transform, puzzle);
        self.regenerate_sticker_buffer(puzzle);
    }

    pub fn regenerate_cut_buffer(
        &mut self,
        camera_transform: cga2d::Rotoflector,
        puzzle: &ConformalPuzzle,
    ) {
        let cut_buffer = get_cut_buffer(camera_transform, puzzle);
        self.cut_buffer = Some(self.device.create_buffer_init(
            &eframe::wgpu::util::BufferInitDescriptor {
                label: Some("It's small"),
                contents: bytemuck::cast_slice(&cut_buffer),
                usage: BufferUsages::STORAGE,
            },
        ));
    }

    pub fn regenerate_outline_buffer(
        &mut self,
        camera_transform: cga2d::Rotoflector,
        outlines: &Vec<cga2d::Blade3>,
    ) {
        let outline_buffer = get_outline_buffer(camera_transform, &outlines);
        self.outline_buffer = Some(self.device.create_buffer_init(
            &eframe::wgpu::util::BufferInitDescriptor {
                label: Some("It's small"),
                contents: bytemuck::cast_slice(&outline_buffer),
                usage: BufferUsages::STORAGE,
            },
        ))
    }

    pub fn regenerate_sticker_buffer(&mut self, puzzle: &ConformalPuzzle) {
        // LUT to get sticker colours from circle inclusion in the fundamental region
        let sticker_buffer: Vec<u32> = get_sticker_buffer(puzzle);
        self.sticker_buffer = Some(self.device.create_buffer_init(
            &eframe::wgpu::util::BufferInitDescriptor {
                label: Some("It's big"),
                contents: bytemuck::cast_slice(&sticker_buffer),
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
                            buffer: self.coset_buffer.as_ref().expect("How did we get here?"),
                            offset: 0,
                            size: None,
                        }),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: eframe::wgpu::BindingResource::Buffer(BufferBinding {
                            buffer: self.sticker_buffer.as_ref().expect("How did we get here?"),
                            offset: 0,
                            size: None,
                        }),
                    },
                    BindGroupEntry {
                        binding: 3,
                        resource: eframe::wgpu::BindingResource::Buffer(BufferBinding {
                            buffer: self.cut_buffer.as_ref().expect("How did we get here?"),
                            offset: 0,
                            size: None,
                        }),
                    },
                    BindGroupEntry {
                        binding: 4,
                        resource: eframe::wgpu::BindingResource::Buffer(BufferBinding {
                            buffer: self.outline_buffer.as_ref().expect("How did we get here?"),
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
    pub cut_circle_count: u32,
    pub outline_count: u32,
    pub col_scale: f32,
    pub depth: u32,
    /// fundamental = 1, col_tiles = 2, inverse_col = 4
    pub flags: u32,
    pub mirror_count: u32,
    padding: [f32; 1],
}
impl Params {
    pub fn new(
        mirrors: Vec<cga2d::Blade3>,
        edges: Vec<bool>,
        point: cga2d::Blade1,
        scale: [f32; 2],
        cut_circle_count: usize,
        outline_count: usize,
        depth: u32,
        view_settings: &ViewSettings,
    ) -> Self {
        let mirror_count = mirrors.len() as u32;

        let mut out_mirrors = [[0.; 4]; 4];
        let mut out_edges = [0; 4];

        for (i, &mirror) in mirrors.iter().enumerate() {
            out_mirrors[i] = rep_mirror(mirror);
            out_edges[i] = edges[i].into();
        }

        let mut flags = 0b0;
        if view_settings.fundamental {
            flags |= 1
        }
        if view_settings.col_tiles {
            flags |= 1 << 1
        }
        if view_settings.inverse_col {
            flags |= 1 << 2
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
            cut_circle_count: cut_circle_count as u32,
            outline_count: outline_count as u32,
            col_scale: view_settings.col_scale,
            depth,
            flags,
            mirror_count,
            padding: [0.; 1],
        }
    }
}

fn rep_mirror(mirror: cga2d::Blade3) -> [f32; 4] {
    let m = !mirror.normalize();
    [m.m as f32, m.p as f32, m.x as f32, m.y as f32]
}

fn get_sticker_buffer(puzzle: &ConformalPuzzle) -> Vec<u32> {
    (0..puzzle.puzzle.elem_group.point_count())
        .flat_map(|x| {
            (0..(1 << puzzle.cut_circles.len())).map(move |i| {
                if i < puzzle.cut_map.len() {
                    if let Some(i) = puzzle.cut_map[i] {
                        if i < puzzle.puzzle.piece_types.len() {
                            let sig = &puzzle.puzzle.piece_types[i];
                            // Does this have to use the attitude in element form?
                            let word = &puzzle.puzzle.elem_group.word_table[x as usize];
                            if let Ok(sig) = puzzle.puzzle.transform_signature(sig, &word.inverse())
                            {
                                if let Some(piece) = puzzle.puzzle.find_piece(sig) {
                                    // dbg!(piece);
                                    if let Some(attitude) =
                                        puzzle.puzzle.elem_group.mul_word(&piece.attitude, &word)
                                    {
                                        if let Some(res) = puzzle.puzzle.elem_group.mul_word(
                                            &Point::INIT,
                                            &puzzle.puzzle.elem_group.word_table
                                                [attitude.0 as usize],
                                        ) {
                                            return res.0 as u32;
                                        }
                                    }
                                }
                            }
                        }
                        return u32::MAX;
                    }
                }
                x as u32
            })
        })
        .collect()
}

fn get_cut_buffer(camera_transform: cga2d::Rotoflector, puzzle: &ConformalPuzzle) -> Vec<[f32; 4]> {
    puzzle
        .cut_circles
        .iter()
        .map(|&c| rep_mirror(camera_transform.sandwich(c)))
        .collect()
}

fn get_outline_buffer(
    camera_transform: cga2d::Rotoflector,
    outlines: &Vec<cga2d::Blade3>,
) -> Vec<[f32; 4]> {
    outlines
        .iter()
        .map(|&c| rep_mirror(camera_transform.sandwich(c)))
        .collect()
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
                        BindGroupLayoutEntry {
                            binding: 2,
                            visibility: ShaderStages::FRAGMENT,
                            ty: eframe::wgpu::BindingType::Buffer {
                                ty: eframe::wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        BindGroupLayoutEntry {
                            binding: 3,
                            visibility: ShaderStages::FRAGMENT,
                            ty: eframe::wgpu::BindingType::Buffer {
                                ty: eframe::wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        BindGroupLayoutEntry {
                            binding: 4,
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
