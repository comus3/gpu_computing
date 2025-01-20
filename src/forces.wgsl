struct Position {
    position_x: f32,
    position_y: f32,
    position_z: f32,
    normal_x: f32,
    normal_y: f32,
    normal_z: f32,
    tangent_x: f32,
    tangent_y: f32,
    tangent_z: f32,
    tex_coords_x: f32,
    tex_coords_y: f32,
}

struct Velocity {
    velocity_x: f32,
    velocity_y: f32,
    velocity_z: f32,
}

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

struct Spring {
    vertex_index_1: f32,
    vertex_index_2: f32,
    rest_length: f32,
}

@group(0) @binding(0) var<storage, read_write> verticiesPositions: array<Position>;
@group(1) @binding(0) var<storage, read_write> verticiesVelocities: array<Velocity>;
@group(2) @binding(0) var<uniform> data: ComputeData;
@group(3) @binding(0) var<storage, read> springsR: array<Spring>;

@compute @workgroup_size(128, 1, 1)
fn main(@builtin(global_invocation_id) param: vec3<u32>) {
    if (param.x >= u32(data.nb_vertices)) {
          return;
    }

    var force_sum = vec3<f32>(0.0, 0.0, 0.0);
    for (var i = 0 ; i < 12; i++) {
        let spring = springsR[param.x*u32(12) + u32(i)];
        let vertex_index_1 = u32(spring.vertex_index_1);
        let vertex_index_2 = u32(spring.vertex_index_2);
        let rest_length = spring.rest_length;

        if u32(spring.vertex_index_2) <= u32(data.nb_vertices) {
            // calculate the distance between the two vertices
            let position_1 = vec3<f32>(verticiesPositions[vertex_index_1].position_x, verticiesPositions[vertex_index_1].position_y, verticiesPositions[vertex_index_1].position_z);
            let position_2 = vec3<f32>(verticiesPositions[vertex_index_2].position_x, verticiesPositions[vertex_index_2].position_y, verticiesPositions[vertex_index_2].position_z);
            var distance = length(position_1 - position_2);
            var direction = normalize(position_1 - position_2);

            // calculate the speed of the first vertex relative to the second
            let velocity_1 = vec3<f32>(verticiesVelocities[vertex_index_1].velocity_x, verticiesVelocities[vertex_index_1].velocity_y, verticiesVelocities[vertex_index_1].velocity_z);
            let velocity_2 = vec3<f32>(verticiesVelocities[vertex_index_2].velocity_x, verticiesVelocities[vertex_index_2].velocity_y, verticiesVelocities[vertex_index_2].velocity_z);
            let relative_velocity = length(velocity_1 - velocity_2);
            let velocity_direction = normalize(velocity_1 - velocity_2);
            

            if i < 4 {
                let force = -data.structural_stiffness * (distance - rest_length);
                force_sum += force * direction;
                if relative_velocity != 0.0 {
                    let damping_force = -data.structural_damping * relative_velocity;
                    force_sum += damping_force * velocity_direction;
                }
            } else if i < 8 {
                let force = -data.shear_stiffness * (distance - rest_length);
                force_sum += force * direction;
                if relative_velocity != 0.0 {
                    let damping_force = -data.shear_damping * relative_velocity;
                    force_sum += damping_force * velocity_direction;
                }
            } else if i < 12 {
                // direction.z = 0.0;
                let force = -data.bend_stiffness * (distance - rest_length) - data.bend_damping * relative_velocity;
                force_sum += force * direction;
                if relative_velocity != 0.0 {
                    let damping_force = -data.bend_damping * relative_velocity;
                    force_sum += damping_force * velocity_direction;
                }
            }
        }
    }
    force_sum.y += -9.81 * data.vertex_mass;

    // update the velocity of the vertex
    verticiesVelocities[param.x].velocity_x += (force_sum.x / data.vertex_mass) * data.delta_time;
    verticiesVelocities[param.x].velocity_y += (force_sum.y / data.vertex_mass) * data.delta_time;
    verticiesVelocities[param.x].velocity_z += (force_sum.z / data.vertex_mass) * data.delta_time;

    // update the position of the vertex, or it crashes
    verticiesPositions[param.x].position_x += 0.0;
}