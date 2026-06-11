// Image blending operations

@group(0) @binding(0) var<uniform> op_code: u32;
@group(0) @binding(1) var<uniform> width: u32;
@group(0) @binding(2) var<uniform> height: u32;
@group(0) @binding(3) var img_a: texture_2d<f32>;
@group(0) @binding(4) var img_b: texture_2d<f32>;
@group(0) @binding(5) var output: texture_storage_2d<rgba8unorm, write>;

fn blend_screen(a: vec3<f32>, b: vec3<f32>) -> vec3<f32> {
    return 1.0 - (1.0 - a) * (1.0 - b);
}

fn blend_overlay(a: vec3<f32>, b: vec3<f32>) -> vec3<f32> {
    var result: vec3<f32>;
    for (var i = 0u; i < 3u; i++) {
        if (a[i] < 0.5) {
            result[i] = 2.0 * a[i] * b[i];
        } else {
            result[i] = 1.0 - 2.0 * (1.0 - a[i]) * (1.0 - b[i]);
        }
    }
    return result;
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= width || gid.y >= height { return; }

    let a = textureLoad(img_a, gid.xy, 0);
    let b = textureLoad(img_b, gid.xy, 0);
    var result: vec4<f32>;

    switch op_code {
        case 0u: { result = vec4<f32>(a.rgb * b.rgb, a.a); }                          // multiply
        case 1u: { result = vec4<f32>(blend_screen(a.rgb, b.rgb), a.a); }              // screen
        case 2u: { result = vec4<f32>(blend_overlay(a.rgb, b.rgb), a.a); }             // overlay
        case 5u: { result = vec4<f32>(abs(a.rgb - b.rgb), a.a); }                      // difference
        case 6u: { result = vec4<f32>(min(a.rgb + b.rgb, vec3<f32>(1.0)), a.a); }     // add
        case 7u: { result = vec4<f32>(max(a.rgb - b.rgb, vec3<f32>(0.0)), a.a); }     // subtract
        default: { result = a; }
    }

    textureStore(output, gid.xy, result);
}
