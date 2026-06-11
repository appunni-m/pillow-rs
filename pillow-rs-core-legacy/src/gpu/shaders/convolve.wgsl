// Generic 3x3 convolution shader (filter kernels: BLUR, CONTOUR, DETAIL, etc.)

@group(0) @binding(0) var<uniform> kernel: array<f32, 9>;
@group(0) @binding(1) var<uniform> scale: f32;
@group(0) @binding(2) var<uniform> offset: f32;
@group(0) @binding(3) var<uniform> width: u32;
@group(0) @binding(4) var<uniform> height: u32;
@group(0) @binding(5) var input: texture_2d<f32>;
@group(0) @binding(6) var output: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x == 0u || gid.y == 0u || gid.x >= width - 1u || gid.y >= height - 1u {
        textureStore(output, gid.xy, textureLoad(input, gid.xy, 0));
        return;
    }

    var sum = vec4<f32>(0.0);
    for (var dy = 0u; dy < 3u; dy++) {
        for (var dx = 0u; dx < 3u; dx++) {
            let sx = gid.x + dx - 1u;
            let sy = gid.y + dy - 1u;
            let k = kernel[dy * 3u + dx];
            sum += textureLoad(input, vec2<u32>(sx, sy), 0) * k;
        }
    }

    let result = sum / scale + offset;
    textureStore(output, gid.xy, vec4<f32>(
        clamp(result.r, 0.0, 1.0),
        clamp(result.g, 0.0, 1.0),
        clamp(result.b, 0.0, 1.0),
        clamp(result.a, 0.0, 1.0),
    ));
}
