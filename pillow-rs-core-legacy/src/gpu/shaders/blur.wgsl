// Box blur compute shader — separable 2-pass (horizontal + vertical)
// Shared between native wgpu and browser WebGPU

@group(0) @binding(0) var<uniform> radius: u32;
@group(0) @binding(1) var<uniform> width: u32;
@group(0) @binding(2) var<uniform> height: u32;
@group(0) @binding(3) var input: texture_2d<f32>;
@group(0) @binding(4) var output: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= width || gid.y >= height { return; }

    let r = i32(radius);
    var sum = vec4<f32>(0.0);
    var count: u32 = 0u;

    for (var dx = -r; dx <= r; dx++) {
        let sx = min(max(i32(gid.x) + dx, 0), i32(width) - 1);
        sum += textureLoad(input, vec2<u32>(u32(sx), gid.y), 0);
        count++;
    }

    let avg = sum / f32(count);
    textureStore(output, gid.xy, avg);
}
