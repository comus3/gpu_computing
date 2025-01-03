use wgpu_bootstrap::wgpu;
use wgpu_bootstrap::runner::Context;
use crate::cloth::geometry::{generate_cloth, Particle};
use wgpu_bootstrap::wgpu::util::DeviceExt;

pub struct ClothApp {
    particle_buffer: wgpu::Buffer,
    particle_count: usize,
    compute_pipeline: wgpu::ComputePipeline,
    particle_bind_group: wgpu::BindGroup,
}

impl ClothApp {
    pub fn new(context: &mut Context) -> Self {
        let particles = generate_cloth(10, 10, 0.1);

        let particle_buffer = context.device().create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Particle Buffer"),
            contents: bytemuck::cast_slice(&particles),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
        });

        let compute_shader = context.device().create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("compute.wgsl").into()),
        });

        let compute_pipeline = context.device().create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Compute Pipeline"),
            layout: None,
            module: &compute_shader,
            entry_point: "main",
        });        
        
        let particle_bind_group_layout = compute_pipeline.get_bind_group_layout(0);

        let particle_bind_group = context.device().create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &particle_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: particle_buffer.as_entire_binding(),
            }],
            label: Some("Particle Bind Group"),
        });

        Self {
            particle_buffer,
            particle_count: particles.len(),
            compute_pipeline,
            particle_bind_group,
        }
    }

    pub fn update(&self, context: &mut Context) {
        let mut encoder = context.device().create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Compute Encoder"),
        });
    
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Compute Pass"),
            });
            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(0, &self.particle_bind_group, &[]);
            compute_pass.dispatch_workgroups((self.particle_count as u32 + 63) / 64, 1, 1);
        }
    
        context.queue().submit(Some(encoder.finish()));
    }
    
}
