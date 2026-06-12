// Image blending — storage buffer version (wgpu + WebGPU)
struct Uniforms { op_code: u32, width: u32, height: u32 }
@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var<storage, read> img_a: array<f32>;
@group(0) @binding(2) var<storage, read> img_b: array<f32>;
@group(0) @binding(3) var<storage, read_write> output: array<f32>;

fn px(buf: ptr<storage,array<f32>,read>, i: u32) -> vec4<f32> {
    return vec4<f32>((*buf)[i],(*buf)[i+1u],(*buf)[i+2u],(*buf)[i+3u]);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= u.width || gid.y >= u.height { return; }
    let i = (gid.y*u.width+gid.x)*4u;
    var a = px(&img_a,i); var b = px(&img_b,i);
    var r: vec4<f32>;
    switch u.op_code {
        case 0u: { r = vec4<f32>(a.rgb*b.rgb, a.a); }                              // multiply
        case 1u: { r = vec4<f32>(1.0-(1.0-a.rgb)*(1.0-b.rgb), a.a); }              // screen
        case 5u: { r = vec4<f32>(abs(a.rgb-b.rgb), a.a); }                          // difference
        case 6u: { r = vec4<f32>(min(a.rgb+b.rgb,vec3<f32>(1.0)), a.a); }           // add
        case 7u: { r = vec4<f32>(max(a.rgb-b.rgb,vec3<f32>(0.0)), a.a); }           // subtract
        default: { r = a; }
    }
    output[i]=r.r; output[i+1u]=r.g; output[i+2u]=r.b; output[i+3u]=r.a;
}
