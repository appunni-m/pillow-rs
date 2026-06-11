// Color operation shaders: invert, solarize, posterize, grayscale
// Operation selected via uniform op_code

@group(0) @binding(0) var<uniform> op_code: u32;
@group(0) @binding(1) var<uniform> param: f32;
@group(0) @binding(2) var<uniform> width: u32;
@group(0) @binding(3) var<uniform> height: u32;
@group(0) @binding(4) var input: texture_2d<f32>;
@group(0) @binding(5) var output: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= width || gid.y >= height { return; }

    var color = textureLoad(input, gid.xy, 0);

    switch op_code {
        case 0u: { // invert
            color = vec4<f32>(1.0 - color.rgb, color.a);
        }
        case 1u: { // solarize
            let threshold = param;
            if (color.r > threshold) { color.r = 1.0 - color.r; }
            if (color.g > threshold) { color.g = 1.0 - color.g; }
            if (color.b > threshold) { color.b = 1.0 - color.b; }
        }
        case 2u: { // posterize
            let bits = u32(param);
            let levels = f32((1u << bits) - 1u);
            color = vec4<f32>(floor(color.rgb * levels + 0.5) / levels, color.a);
        }
        case 3u: { // grayscale (luminance)
            let luma = 0.299 * color.r + 0.587 * color.g + 0.114 * color.b;
            color = vec4<f32>(luma, luma, luma, color.a);
        }
        default: {}
    }

    textureStore(output, gid.xy, color);
}
