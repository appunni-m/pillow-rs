// 3x3 convolution — storage buffer version (wgpu + WebGPU)
struct Uniforms { kernel: array<f32,9>, scale: f32, offset: f32, width: u32, height: u32 }
@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var<storage, read> input: array<f32>;
@group(0) @binding(2) var<storage, read_write> output: array<f32>;

fn get_px(x: i32, y: i32) -> vec4<f32> {
    let sx = clamp(x, 0, i32(u.width)-1);
    let sy = clamp(y, 0, i32(u.height)-1);
    let i = (u32(sy)*u.width+u32(sx))*4u;
    return vec4<f32>(input[i],input[i+1u],input[i+2u],input[i+3u]);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= u.width || gid.y >= u.height { return; }
    var sum = vec4<f32>(0.0);
    for (var dy=0u; dy<3u; dy++) {
        for (var dx=0u; dx<3u; dx++) {
            let k = u.kernel[dy*3u+dx];
            sum += get_px(i32(gid.x)+i32(dx)-1, i32(gid.y)+i32(dy)-1) * k;
        }
    }
    let r = sum/u.scale+u.offset;
    let o = (gid.y*u.width+gid.x)*4u;
    output[o]=clamp(r.r,0.0,1.0); output[o+1u]=clamp(r.g,0.0,1.0);
    output[o+2u]=clamp(r.b,0.0,1.0); output[o+3u]=clamp(r.a,0.0,1.0);
}
