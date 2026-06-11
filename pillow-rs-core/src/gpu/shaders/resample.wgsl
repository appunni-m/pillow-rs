// Bilinear resampling compute shader (resize/thumbnail)

@group(0) @binding(0) var<uniform> src_width: u32;
@group(0) @binding(1) var<uniform> src_height: u32;
@group(0) @binding(2) var<uniform> dst_width: u32;
@group(0) @binding(3) var<uniform> dst_height: u32;
@group(0) @binding(4) var input: texture_2d<f32>;
@group(0) @binding(5) var output: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(6) var sampler_: sampler;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= dst_width || gid.y >= dst_height { return; }

    let u = (f32(gid.x) + 0.5) * f32(src_width) / f32(dst_width) - 0.5;
    let v = (f32(gid.y) + 0.5) * f32(src_height) / f32(dst_height) - 0.5;
    let tc = vec2<f32>(u / f32(src_width), v / f32(src_height));

    let color = textureSampleLevel(input, sampler_, tc, 0.0);
    textureStore(output, gid.xy, color);
}
