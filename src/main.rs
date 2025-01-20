use wgpu_bootstrap::{
    window::Window,
    frame::Frame,
    application::Application,
    context::Context,
    geometry::icosphere,
    camera::Camera,
    wgpu,
    cgmath,
    default::Vertex,
    computation::Computation,
    texture::create_texture_bind_group,
};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ComputeData {
    delta_time: f32,
    nb_vertices: f32,
    sphere_radius: f32,
    sphere_center_x: f32,
    sphere_center_y: f32,
    sphere_center_z: f32,
    vertex_mass: f32,
    structural_stiffness: f32,
    shear_stiffness: f32,
    bend_stiffness: f32,
    structural_damping: f32,
    shear_damping: f32,
    bend_damping: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Velocity {
    pub velocity: [f32; 3]
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Spring {
    pub index1: f32,
    pub index2: f32,
    pub rest_length: f32,
}
// =========================================================================================
//                                        PARAMETERS
// =========================================================================================
// CLOTH
const CLOTH_SIZE: f32 = 50.0;
const N_CLOTH_VERTICES_PER_ROW: u32 = 25; // the cloth is a square, the minimum is 2
const CLOTH_CENTER_X: f32 = 0.0;
const CLOTH_CENTER_Y: f32 = 10.0;
const CLOTH_CENTER_Z: f32 = 0.0;
// SPHERE
const SPHERE_RADIUS: f32 = 10.0;
const SPHERE_CENTER_X: f32 = 0.0;
const SPHERE_CENTER_Y: f32 = 0.0;
const SPHERE_CENTER_Z: f32 = 0.0;
// PHYSICS
const MASS: f32 = 200.0;
// const VERTEX_MASS: f32 = MASS / (N_CLOTH_VERTICES_PER_ROW * N_CLOTH_VERTICES_PER_ROW) as f32;
const VERTEX_MASS: f32 = 0.16;
const STRUCTURAL_STIFFNESS: f32 = 150.0;
const SHEAR_STIFFNESS: f32 = 5.0;
const BEND_STIFFNESS: f32 = 15.0;
const STRUCTURAL_DAMPING: f32 = 1.5;
const SHEAR_DAMPING: f32 = 0.05;
const BEND_DAMPING: f32 = 0.15;

const N_ITERATIONS: u32 = 500; 
// =========================================================================================

struct MyApp {
    camera_bind_group: wgpu::BindGroup,
    texture_bind_group: wgpu::BindGroup,
    // sphere
    sphere_pipeline: wgpu::RenderPipeline,
    sphere_vertex_buffer: wgpu::Buffer,
    sphere_index_buffer: wgpu::Buffer,
    sphere_indices: Vec<u16>,
    // cloth
    cloth_pipeline: wgpu::RenderPipeline,
    cloth_vertex_buffer: wgpu::Buffer,
    cloth_index_buffer: wgpu::Buffer,
    cloth_indices: Vec<u16>,
    // compute
    compute_pipeline: wgpu::ComputePipeline,
    forces_compute_pipeline: wgpu::ComputePipeline,
    compute_vertices_bind_group: wgpu::BindGroup,
    compute_data_bind_group: wgpu::BindGroup,
    compute_velocities_bind_group: wgpu::BindGroup,
    compute_data_buffer: wgpu::Buffer,
    compute_data: ComputeData,
    // spring
    springs_bind_group: wgpu::BindGroup,
}

impl MyApp {
    fn new(context: &Context) -> Self {
        let texture = context.create_texture(
            "English",
            include_bytes!("texture.jpg"),
        );

        let texture_bind_group = create_texture_bind_group(context, &texture);

        let camera = Camera {
            eye: (40.0, 30.0, 30.0).into(),
            target: (0.0, 0.0, 0.0).into(),
            up: cgmath::Vector3::unit_y(),
            aspect: context.get_aspect_ratio(),
            fovy: 45.0,
            znear: 0.1,
            zfar: 1000.0,
        };

        // camera ------------------------------------------------------------
        let (_camera_buffer, camera_bind_group) = camera.create_camera_bind_group(context);

        // sphere ------------------------------------------------------------
        let sphere_pipeline = context.create_render_pipeline(
            "Render Pipeline Sphere",
            include_str!("sphere.wgsl"),
            &[Vertex::desc()],
            &[&context.camera_bind_group_layout],
            wgpu::PrimitiveTopology::LineList
        );

        let (mut sphere_vertices, sphere_indices) = icosphere(4);

        // change the radius of the sphere :
        for vertex in sphere_vertices.iter_mut() {
            let mut posn = cgmath::Vector3::from(vertex.position);
            posn *= SPHERE_RADIUS as f32;
            vertex.position = posn.into()
        }
        // change the center of the sphere :
        for vertex in sphere_vertices.iter_mut() {
            vertex.position[0] += SPHERE_CENTER_X;
            vertex.position[1] += SPHERE_CENTER_Y;
            vertex.position[2] += SPHERE_CENTER_Z;
        }

        // create a buffer for the sphere
        let sphere_vertex_buffer = context.create_buffer(
            &sphere_vertices,
            wgpu::BufferUsages::VERTEX
        );
        let sphere_index_buffer = context.create_buffer(
            &sphere_indices,
            wgpu::BufferUsages::INDEX
        );

        // Cloth ------------------------------------------------------------
        let cloth_pipeline = context.create_render_pipeline(
            "Render Pipeline Cloth",
            include_str!("red.wgsl"),
            &[Vertex::desc()],
            &[
                &context.texture_bind_group_layout,
                &context.camera_bind_group_layout,
                ],
            wgpu::PrimitiveTopology::TriangleList
        );

        
        // create the cloth
        let mut cloth_vertices = Vec::new();
        let mut cloth_indices: Vec<u16> = Vec::new();
        
        // create the vertices
        for i in 0..N_CLOTH_VERTICES_PER_ROW {
            for j in 0..N_CLOTH_VERTICES_PER_ROW {
                cloth_vertices.push(Vertex {
                    position: [
                        CLOTH_CENTER_X + i as f32 * (CLOTH_SIZE / (N_CLOTH_VERTICES_PER_ROW - 1) as f32) - (CLOTH_SIZE / 2.0),
                        CLOTH_CENTER_Y,
                        CLOTH_CENTER_Z + j as f32 * (CLOTH_SIZE / (N_CLOTH_VERTICES_PER_ROW - 1) as f32) - (CLOTH_SIZE / 2.0),
                    ],
                    normal: [0.0, 0.0, 0.0],
                    tangent: [0.0, 0.0, 0.0],
                    tex_coords: [
                        i as f32 * (1.0 / (N_CLOTH_VERTICES_PER_ROW - 1) as f32),
                        j as f32 * (1.0 / (N_CLOTH_VERTICES_PER_ROW - 1) as f32),
                    ],
                });
            }
        }

        // create the indices
        for i in 0..N_CLOTH_VERTICES_PER_ROW - 1 {
            for j in 0..N_CLOTH_VERTICES_PER_ROW - 1 {
                // first triangle
                cloth_indices.push((i * N_CLOTH_VERTICES_PER_ROW + j) as u16);
                cloth_indices.push((i * N_CLOTH_VERTICES_PER_ROW + j + 1) as u16);
                cloth_indices.push(((i + 1) * N_CLOTH_VERTICES_PER_ROW + j) as u16);
                // second triangle
                cloth_indices.push((i * N_CLOTH_VERTICES_PER_ROW + j + 1) as u16);
                cloth_indices.push(((i + 1) * N_CLOTH_VERTICES_PER_ROW + j + 1) as u16);
                cloth_indices.push(((i + 1) * N_CLOTH_VERTICES_PER_ROW + j) as u16);
            }
        }

        // set the default speed of the cloth
        let mut cloth_velocities: Vec<Velocity> = Vec::new();
        for _i in cloth_vertices.iter_mut() {
            cloth_velocities.push(Velocity {
                velocity: [0.0, 0.0, 0.0],
            });
        }

        // create a buffer for the cloth
        let cloth_vertex_buffer = context.create_buffer(
            &cloth_vertices,
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE
        );
        let cloth_index_buffer = context.create_buffer(
            &cloth_indices,
            wgpu::BufferUsages::INDEX
        );
        let cloth_velocities_buffer = context.create_buffer(
            &cloth_velocities,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX
        );

        // create the compute pipeline
        let compute_pipeline = context.create_compute_pipeline(
            "Compute Pipeline",
            include_str!("compute.wgsl"),
        );
        // create the force compute pipeline
        let forces_compute_pipeline = context.create_compute_pipeline(
            "Forces Compute Pipeline",
            include_str!("forces.wgsl")
        );

        let compute_vertices_bind_group = context.create_bind_group(
            "compute vertices bind group",
            &compute_pipeline.get_bind_group_layout(0),
            &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: cloth_vertex_buffer.as_entire_binding(),
                },
            ],
        );
        let compute_velocities_bind_group = context.create_bind_group(
            "compute velocities bind group",
            &compute_pipeline.get_bind_group_layout(1),
            &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: cloth_velocities_buffer.as_entire_binding(),
                },
            ],
        );

        // compute data -----------------------------------------------------
        let compute_data = ComputeData {
            delta_time: 0.01,
            nb_vertices: (N_CLOTH_VERTICES_PER_ROW*N_CLOTH_VERTICES_PER_ROW) as f32,
            sphere_radius: SPHERE_RADIUS,
            sphere_center_x: SPHERE_CENTER_X,
            sphere_center_y: SPHERE_CENTER_Y,
            sphere_center_z: SPHERE_CENTER_Z,
            vertex_mass: VERTEX_MASS,
            structural_stiffness: STRUCTURAL_STIFFNESS,
            shear_stiffness: SHEAR_STIFFNESS,
            bend_stiffness: BEND_STIFFNESS,
            structural_damping: STRUCTURAL_DAMPING,
            shear_damping: SHEAR_DAMPING,
            bend_damping: BEND_DAMPING,
        };

        let compute_data_buffer = context.create_buffer(
            &[compute_data],
            wgpu::BufferUsages::UNIFORM,
        );

        let compute_data_bind_group = context.create_bind_group(
            "compute data bind group",
            &compute_pipeline.get_bind_group_layout(2),
            &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: compute_data_buffer.as_entire_binding(),
                },
            ],
        );

        // Springs ----------------------------------------------------------
        let mut springs: Vec<Spring> = Vec::new();
        for i in 0..N_CLOTH_VERTICES_PER_ROW * N_CLOTH_VERTICES_PER_ROW {
            let col: i32 = (i % N_CLOTH_VERTICES_PER_ROW) as i32;
            let row: i32 = (i / N_CLOTH_VERTICES_PER_ROW) as i32;
            // structural springs
            for j in [-1,1] as [i32; 2] {
                // col +- 1
                let mut index2 = row * N_CLOTH_VERTICES_PER_ROW as i32 + col + j;
                if col + j > N_CLOTH_VERTICES_PER_ROW as i32 - 1 || col + j < 0 {
                    index2 = (N_CLOTH_VERTICES_PER_ROW * N_CLOTH_VERTICES_PER_ROW + 1) as i32;
                }
                springs.push(Spring {
                    index1: i as f32,
                    index2: index2 as f32,
                    rest_length: (CLOTH_SIZE / (N_CLOTH_VERTICES_PER_ROW - 1) as f32),
                });
                // row +- 1
                index2 = (row + j) * N_CLOTH_VERTICES_PER_ROW as i32 + col;
                if row + j > N_CLOTH_VERTICES_PER_ROW as i32 - 1 || row + j < 0 {
                    index2 = (N_CLOTH_VERTICES_PER_ROW * N_CLOTH_VERTICES_PER_ROW + 1) as i32;
                }
                springs.push(Spring {
                    index1: i as f32,
                    index2: index2 as f32,
                    rest_length: (CLOTH_SIZE / (N_CLOTH_VERTICES_PER_ROW - 1) as f32),
                });
            }
            // shear springs
            for j in [-1,1] as [i32; 2] {
                // col + j and row + j
                let mut index2 = (row + j) * N_CLOTH_VERTICES_PER_ROW as i32 + col + j;
                if col + j > N_CLOTH_VERTICES_PER_ROW as i32 - 1 || col + j < 0 || row + j > N_CLOTH_VERTICES_PER_ROW as i32 - 1 || row + j < 0 {
                    index2 = (N_CLOTH_VERTICES_PER_ROW * N_CLOTH_VERTICES_PER_ROW + 1) as i32;
                }
                springs.push(Spring {
                    index1: i as f32,
                    index2: index2 as f32,
                    rest_length: (CLOTH_SIZE / (N_CLOTH_VERTICES_PER_ROW - 1) as f32) * 1.41421356237,
                });
                // col + j and row - j
                index2 = (row - j) * N_CLOTH_VERTICES_PER_ROW as i32 + col + j;
                if col + j > N_CLOTH_VERTICES_PER_ROW as i32 - 1 || col + j < 0 || row - j > N_CLOTH_VERTICES_PER_ROW as i32 - 1 || row - j < 0 {
                    index2 = (N_CLOTH_VERTICES_PER_ROW * N_CLOTH_VERTICES_PER_ROW + 1) as i32;
                }
                springs.push(Spring {
                    index1: i as f32,
                    index2: index2 as f32,
                    rest_length: (CLOTH_SIZE / (N_CLOTH_VERTICES_PER_ROW - 1) as f32) * 1.41421356237,
                });
            }
            // bend springs
            for j in [-1,1] as [i32; 2] {
                // col +- 2j
                let mut index2 = row * N_CLOTH_VERTICES_PER_ROW as i32 + col + 2 * j;
                if col + 2 * j > N_CLOTH_VERTICES_PER_ROW as i32 - 1 || col + 2 * j < 0 {
                    index2 = (N_CLOTH_VERTICES_PER_ROW * N_CLOTH_VERTICES_PER_ROW + 1) as i32;
                }
                springs.push(Spring {
                    index1: i as f32,
                    index2: index2 as f32,
                    rest_length: (CLOTH_SIZE / (N_CLOTH_VERTICES_PER_ROW - 1) as f32) * 2.0,
                });
                // row +- 2j
                index2 = (row + 2 * j) * N_CLOTH_VERTICES_PER_ROW as i32 + col;
                if row + 2 * j > N_CLOTH_VERTICES_PER_ROW as i32 - 1 || row + 2 * j < 0 {
                    index2 = (N_CLOTH_VERTICES_PER_ROW * N_CLOTH_VERTICES_PER_ROW + 1) as i32;
                }
                springs.push(Spring {
                    index1: i as f32,
                    index2: index2 as f32,
                    rest_length: (CLOTH_SIZE / (N_CLOTH_VERTICES_PER_ROW - 1) as f32) * 2.0,
                });
            }
        }

        // create a buffer for the springs
        let springs_buffer = context.create_buffer(
            springs.as_slice(),
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX,
        );
        // create a bind group for the springs
        let springs_bind_group = context.create_bind_group(
            "Sping Bind Group",
            &compute_pipeline.get_bind_group_layout(3),
            &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: springs_buffer.as_entire_binding(),
                },
            ]
        );

        return Self {
            camera_bind_group,
            texture_bind_group,
            // sphere
            sphere_pipeline,
            sphere_vertex_buffer,
            sphere_index_buffer,
            sphere_indices,
            // cloth
            cloth_pipeline,
            cloth_vertex_buffer,
            cloth_index_buffer,
            cloth_indices,
            // compute
            compute_pipeline,
            forces_compute_pipeline,
            compute_vertices_bind_group,
            compute_velocities_bind_group,
            compute_data_bind_group,
            compute_data_buffer,
            compute_data,
            // springs
            springs_bind_group,
        };
    }
    
}

impl Application for MyApp {
    fn render(&self, context: &Context) -> Result<(), wgpu::SurfaceError> {
        let mut frame = Frame::new(context)?;

        {
            let mut render_pass = frame.begin_render_pass(wgpu::Color {r: 0.85, g: 0.85, b: 0.85, a: 1.0});
            // render the sphere
            render_pass.set_pipeline(&self.sphere_pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.sphere_vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.sphere_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..self.sphere_indices.len() as u32, 0, 0..1);
            // render the cloth as a triangle list
            render_pass.set_pipeline(&self.cloth_pipeline);
            render_pass.set_bind_group(0, &self.texture_bind_group, &[]);
            render_pass.set_bind_group(1, &self.camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.cloth_vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.cloth_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..self.cloth_indices.len() as u32, 0, 0..1);
        }

        frame.present();
        
        Ok(())
    }

    fn update(&mut self, context: &Context, delta_time: f32) {
        // update the compute data
        let compute_data = ComputeData {
            delta_time: delta_time/N_ITERATIONS as f32,
            nb_vertices: (N_CLOTH_VERTICES_PER_ROW*N_CLOTH_VERTICES_PER_ROW) as f32,
            sphere_radius: SPHERE_RADIUS,
            sphere_center_x: SPHERE_CENTER_X,
            sphere_center_y: SPHERE_CENTER_Y,
            sphere_center_z: SPHERE_CENTER_Z,
            vertex_mass: VERTEX_MASS,
            structural_stiffness: STRUCTURAL_STIFFNESS,
            shear_stiffness: SHEAR_STIFFNESS,
            bend_stiffness: BEND_STIFFNESS,
            structural_damping: STRUCTURAL_DAMPING,
            shear_damping: SHEAR_DAMPING,
            bend_damping: BEND_DAMPING,
        };
        context.update_buffer(&self.compute_data_buffer, &[compute_data]);

        let mut computation = Computation::new(context);

        for _ in 0..N_ITERATIONS
        {
            let mut compute_pass = computation.begin_compute_pass();
            // calculate the forces
            compute_pass.set_pipeline(&self.forces_compute_pipeline);
            compute_pass.set_bind_group(0, &self.compute_vertices_bind_group, &[]);
            compute_pass.set_bind_group(1, &self.compute_velocities_bind_group, &[]);
            compute_pass.set_bind_group(2, &self.compute_data_bind_group, &[]);
            compute_pass.set_bind_group(3, &self.springs_bind_group, &[]);
            compute_pass.dispatch_workgroups(((N_CLOTH_VERTICES_PER_ROW*N_CLOTH_VERTICES_PER_ROW) as f64/128.0).ceil() as u32, 1, 1);

            // update the positions and collisions
            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(0, &self.compute_vertices_bind_group, &[]);
            compute_pass.set_bind_group(1, &self.compute_velocities_bind_group, &[]);
            compute_pass.set_bind_group(2, &self.compute_data_bind_group, &[]);
            compute_pass.set_bind_group(3, &self.springs_bind_group, &[]);
            compute_pass.dispatch_workgroups(((N_CLOTH_VERTICES_PER_ROW*N_CLOTH_VERTICES_PER_ROW) as f32/128.0).ceil() as u32, 1, 1);
        }
        computation.submit();
    }
}

fn main() {
    let window = Window::new();


    let context = window.get_context();

    let my_app = MyApp::new(context);

    window.run(my_app);
}