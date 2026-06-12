// Bilinear resampling — storage buffer version (wgpu + WebGPU compatible)
// Manual bilinear interpolation (no hardware sampler needed)
struct Uniforms { src_w: u32, src_h: u32, dst_w: u32, dst_h: u32 }
@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var<storage, read> input: array<f32>;
@group(0) @binding(2) var<storage, read_write> output: array<f32>;

fn get_px(x: f32, y: f32) -> vec4<f32> {
    let x0 = u32(clamp(x, 0.0, f32(u.src_w)-1.0));
    let y0 = u32(clamp(y, 0.0, f32(u.src_h)-1.0));
    let i = (y0*u.src_w+x0)*4u;
    return vec4<f32>(input[i],input[i+1u],input[i+2u],input[i+3u]);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= u.dst_w || gid.y >= u.dst_h { return; }
    let sx = f32(gid.x)*f32(u.src_w)/f32(u.dst_w);
    let sy = f32(gid.y)*f32(u.src_h)/f32(u.dst_h);
    let fx = fract(sx); let fy = fract(sy);
    let p00 = get_px(sx, sy);
    let p10 = get_px(sx+1.0, sy);
    let p01 = get_px(sx, sy+1.0);
    let p11 = get_px(sx+1.0, sy+1.0);
    let r = mix(mix(p00,p10,fx), mix(p01,p11,fx), fy);
    let o = (gid.y*u.dst_w+gid.x)*4u;
    output[o]=r.r; output[o+1u]=r.g; output[o+2u]=r.b; output[o+3u]=r.a;
}
