// Box blur — storage buffer version (wgpu + WebGPU compatible)
struct Uniforms { radius: u32, width: u32, height: u32 }
@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var<storage, read> input: array<f32>;
@group(0) @binding(2) var<storage, read_write> output: array<f32>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= u.width || gid.y >= u.height { return; }
    let r = i32(u.radius);
    var sr=0.0; var sg=0.0; var sb=0.0; var sa=0.0; var n: u32 = 0u;
    for (var dx = -r; dx <= r; dx++) {
        let sx = clamp(i32(gid.x)+dx, 0, i32(u.width)-1);
        let i = (u32(sx) + gid.y * u.width) * 4u;
        sr+=input[i]; sg+=input[i+1u]; sb+=input[i+2u]; sa+=input[i+3u]; n++;
    }
    let fn = f32(n); let o = (gid.y*u.width+gid.x)*4u;
    output[o]=sr/fn; output[o+1u]=sg/fn; output[o+2u]=sb/fn; output[o+3u]=sa/fn;
}
