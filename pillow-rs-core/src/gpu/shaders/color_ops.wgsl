// Color operation shaders: invert, solarize, posterize, grayscale
// Storage buffer version — compatible with wgpu and WebGPU
// Input: RGBA f32 pixels (values 0.0-1.0), Output: same format

struct Uniforms {
    op_code: u32,   // 0=invert, 1=solarize, 2=posterize, 3=grayscale
    param: f32,     // threshold/bits
    width: u32,
    height: u32,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var<storage, read> input: array<f32>;
@group(0) @binding(2) var<storage, read_write> output: array<f32>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= uniforms.width || gid.y >= uniforms.height { return; }

    let idx = (gid.y * uniforms.width + gid.x) * 4u;
    var r = input[idx];
    var g = input[idx + 1u];
    var b = input[idx + 2u];
    var a = input[idx + 3u];

    switch uniforms.op_code {
        case 0u: { // invert
            r = 1.0 - r; g = 1.0 - g; b = 1.0 - b;
        }
        case 1u: { // solarize
            let threshold = uniforms.param;
            if (r > threshold) { r = 1.0 - r; }
            if (g > threshold) { g = 1.0 - g; }
            if (b > threshold) { b = 1.0 - b; }
        }
        case 2u: { // posterize
            let bits = u32(uniforms.param);
            let levels = f32((1u << bits) - 1u);
            r = floor(r * levels + 0.5) / levels;
            g = floor(g * levels + 0.5) / levels;
            b = floor(b * levels + 0.5) / levels;
        }
        case 3u: { // grayscale (BT.601 luma)
            let luma = 0.299 * r + 0.587 * g + 0.114 * b;
            r = luma; g = luma; b = luma;
        }
        default: {}
    }

    output[idx] = r;
    output[idx + 1u] = g;
    output[idx + 2u] = b;
    output[idx + 3u] = a;
}
