[[block]]
struct Particle {
    position: vec3<f32>;
    velocity: vec3<f32>;
};

[[group(0), binding(0)]]
var<storage, read_write> particles: array<Particle>;

[[stage(compute), workgroup_size(64)]]
fn main([[builtin(global_invocation_id)]] global_id: vec3<u32>) {
    let index = global_id.x;
    if (index >= arrayLength(&particles)) {
        return;
    }

    let gravity = vec3<f32>(0.0, -9.81, 0.0);
    let delta_time = 0.016; // 60 FPS approximation

    // Update velocity with gravity
    particles[index].velocity += gravity * delta_time;

    // Update position with velocity
    particles[index].position += particles[index].velocity * delta_time;
}
