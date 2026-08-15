// Exact vertical pass for BoxBlur and GaussianBlur.
// See box_blur_h.wgsl for the fixed-point parameter contract.  Each
// invocation owns one complete column and advances the same rolling window
// used by the horizontal pass, so work is O(pixels) and independent of the
// blur radius after initialization.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
    radius: u32,
    weight: u32,
    edge_weight: u32,
}

const MAX_RADIUS: u32 = 16u;
const FIXED_BIAS: u32 = 8388608u;

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

fn pixel_channel(pixel: u32, shift: u32) -> u32 {
    return (pixel >> shift) & 0xffu;
}

fn clamp_index(value: i32, limit: u32) -> u32 {
    return u32(clamp(value, 0, i32(limit) - 1));
}

fn fixed_weighted_average(sum: u32, edge: u32) -> u32 {
    let high = sum * (params.weight >> 12u) + edge * (params.edge_weight >> 12u);
    let low = sum * (params.weight & 4095u) + edge * (params.edge_weight & 4095u) + FIXED_BIAS;
    return min((high >> 12u) + ((((high & 4095u) << 12u) + low) >> 24u), 255u);
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(1, 1, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.y != 0u || gid.x >= params.width || params.width == 0u || params.height == 0u {
        return;
    }

    let width = params.width;
    let height = params.height;
    let radius = min(params.radius, MAX_RADIUS);
    let r = i32(radius);

    var sum_r = 0u;
    var sum_g = 0u;
    var sum_b = 0u;
    var sum_a = 0u;
    for (var dy = -r; dy <= r; dy++) {
        let sy = clamp_index(dy, height);
        let pixel = input[sy * width + gid.x];
        sum_r += pixel_channel(pixel, 0u);
        sum_g += pixel_channel(pixel, 8u);
        sum_b += pixel_channel(pixel, 16u);
        sum_a += pixel_channel(pixel, 24u);
    }

    for (var y = 0u; y < height; y++) {
        let idx = y * width + gid.x;
        let original = input[idx];
        if radius == 0u && params.edge_weight == 0u {
            output[idx] = original;
        } else {
            let top = clamp_index(i32(y) - r - 1, height);
            let bottom = clamp_index(i32(y) + r + 1, height);
            let edge_r = pixel_channel(input[top * width + gid.x], 0u) + pixel_channel(input[bottom * width + gid.x], 0u);
            let edge_g = pixel_channel(input[top * width + gid.x], 8u) + pixel_channel(input[bottom * width + gid.x], 8u);
            let edge_b = pixel_channel(input[top * width + gid.x], 16u) + pixel_channel(input[bottom * width + gid.x], 16u);
            let edge_a = pixel_channel(input[top * width + gid.x], 24u) + pixel_channel(input[bottom * width + gid.x], 24u);
            let out_r = fixed_weighted_average(sum_r, edge_r);
            let out_g = select(pixel_channel(original, 8u), fixed_weighted_average(sum_g, edge_g), mode_has_g(params.mode));
            let out_b = select(pixel_channel(original, 16u), fixed_weighted_average(sum_b, edge_b), mode_has_b(params.mode));
            let out_a = select(pixel_channel(original, 24u), fixed_weighted_average(sum_a, edge_a), mode_has_a(params.mode));
            output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
        }

        if y + 1u < height {
            let remove_pixel = input[clamp_index(i32(y) - r, height) * width + gid.x];
            let add_pixel = input[clamp_index(i32(y) + r + 1, height) * width + gid.x];
            sum_r = sum_r - pixel_channel(remove_pixel, 0u) + pixel_channel(add_pixel, 0u);
            sum_g = sum_g - pixel_channel(remove_pixel, 8u) + pixel_channel(add_pixel, 8u);
            sum_b = sum_b - pixel_channel(remove_pixel, 16u) + pixel_channel(add_pixel, 16u);
            sum_a = sum_a - pixel_channel(remove_pixel, 24u) + pixel_channel(add_pixel, 24u);
        }
    }
}
