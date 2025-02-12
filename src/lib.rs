use wgpu::{PipelineCompilationOptions, RenderPipelineDescriptor, PipelineLayoutDescriptor, VertexBufferLayout, DepthStencilState, MultisampleState, RenderPipeline, PrimitiveState, VertexStepMode, FragmentState, TextureFormat, BufferAddress, BufferUsages, IndexFormat, VertexState, RenderPass, Device, Queue};

use wgpu_dyn_buffer::{DynamicBufferDescriptor, DynamicBuffer};

use ordered_float::OrderedFloat;

pub use cyat;
use cyat::{VertexBuffers, ShapeBuilder, Vertex};

type Bound = (u32, u32, u32, u32);
pub struct ShapeArea(pub ShapeBuilder<DefaultAttributes>, pub Bound);

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DefaultAttributes {
    pub color: [f32; 3],
    pub z: f32
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DefaultVertex {
    position: [f32; 2],
    color: [f32; 3],
    z: f32
}

impl DefaultVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 3] =
        wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x3, 2 => Float32];
}

impl DefaultVertex {
    fn layout() -> VertexBufferLayout<'static> {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

impl Vertex for DefaultVertex {
    type Attributes = DefaultAttributes;

    fn construct(position: [f32; 2], attrs: Self::Attributes) -> DefaultVertex {
        let c = |f: f32| OrderedFloat((f + 0.055) / 1.055).powf(2.4);
        DefaultVertex{
            position,
            color: [c(attrs.color[0]), c(attrs.color[1]), c(attrs.color[2])],
            z: attrs.z
        }
    }
}

pub struct CyatRenderer {
    render_pipeline: RenderPipeline,
    vertex_buffer: DynamicBuffer,
    index_buffer: DynamicBuffer,
    cyat_buffers: VertexBuffers<DefaultVertex, u16>,
    shape_buffer: Vec<(usize, usize, Bound)>
}

impl CyatRenderer {
    /// Create all unchanging resources here.
    pub fn new(
        device: &Device,
        texture_format: &TextureFormat,
        multisample: MultisampleState,
        depth_stencil: Option<DepthStencilState>,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor::default());
        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: PipelineCompilationOptions::default(),
                buffers: &[DefaultVertex::layout()]
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: PipelineCompilationOptions::default(),
                targets: &[Some((*texture_format).into())],
            }),
            primitive: PrimitiveState::default(),
            depth_stencil,
            multisample,
            multiview: None,
            cache: None
        });

        let vertex_buffer = DynamicBuffer::new(device, &DynamicBufferDescriptor {
            label: None,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        });

        let index_buffer = DynamicBuffer::new(device, &DynamicBufferDescriptor {
            label: None,
            usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
        });

        CyatRenderer{
            render_pipeline,
            vertex_buffer,
            index_buffer,
            cyat_buffers: VertexBuffers::new(),
            shape_buffer: Vec::new()
        }
    }

    /// Prepare for rendering this frame; create all resources that will be
    /// used during the next render that do not already exist.
    pub fn prepare(
        &mut self,
        device: &Device,
        queue: &Queue,
        shapes: Vec<ShapeArea>
    ) {
        self.cyat_buffers.clear();
        self.shape_buffer.clear();

        let mut index = 0;

        for ShapeArea(shape, bound) in shapes {
            shape.build(&mut self.cyat_buffers);

            let buffer_len = self.cyat_buffers.indices.len();
            self.shape_buffer.push((index, buffer_len, bound));
            index = buffer_len;
        }

        if self.cyat_buffers.vertices.is_empty() || self.cyat_buffers.indices.is_empty() {return;}

        self.vertex_buffer.write_buffer(device, queue, bytemuck::cast_slice(&self.cyat_buffers.vertices));
        self.index_buffer.write_buffer(device, queue, bytemuck::cast_slice(&self.cyat_buffers.indices));
    }

    /// Render using caller provided render pass.
    pub fn render(&self, render_pass: &mut RenderPass<'_>) {
        if self.cyat_buffers.vertices.is_empty() || self.cyat_buffers.indices.is_empty() {return;}

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.as_ref().slice(..));
        render_pass.set_index_buffer(self.index_buffer.as_ref().slice(..), IndexFormat::Uint16);
        for (start, end, bound) in &self.shape_buffer {
            render_pass.set_scissor_rect(bound.0, bound.1, bound.2, bound.3);
            render_pass.draw_indexed(*start as u32..*end as u32, 0, 0..1);
        }
    }
}

