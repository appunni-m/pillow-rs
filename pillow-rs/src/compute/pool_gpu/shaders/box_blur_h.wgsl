// Exact horizontal pass for BoxBlur and GaussianBlur.
//
// The CPU implementation uses Pillow's 24-bit fixed-point box accumulator.
// `radius`, `weight`, and `edge_weight` are supplied after the common header:
//   weight = floor(2^24 / (2*radius + 1))
//   edge_weight = fractional edge contribution for GaussianBlur
//
// Each invocation owns one complete row.  It initializes one radius-sized
// window and then advances that window with one remove/add pair per output
// pixel.  This keeps the shader work O(pixels) instead of re-reading the
// radius window for every output sample, while avoiding workgroup scratch and
// cross-invocation synchronization.

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

// Compute (sum*weight + edge*edge_weight + 2^23) >> 24 without u64.
// Both products are split into 12-bit pieces; every intermediate remains
// below the signed/unsigned 32-bit WGSL range for the bounded byte window.
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
    if gid.x != 0u || gid.y >= params.height || params.width == 0u || params.height == 0u {
        return;
    }

    let width = params.width;
    let radius = min(params.radius, MAX_RADIUS);
    let r = i32(radius);
    let base = gid.y * width;

    var sum_r = 0u;
    var sum_g = 0u;
    var sum_b = 0u;
    var sum_a = 0u;
    for (var dx = -r; dx <= r; dx++) {
        let pixel = input[base + clamp_index(dx, width)];
        sum_r += pixel_channel(pixel, 0u);
        sum_g += pixel_channel(pixel, 8u);
        sum_b += pixel_channel(pixel, 16u);
        sum_a += pixel_channel(pixel, 24u);
    }

    for (var x = 0u; x < width; x++) {
        let original = input[base + x];
        if radius == 0u && params.edge_weight == 0u {
            output[base + x] = original;
        } else {
            let left = clamp_index(i32(x) - r - 1, width);
            let right = clamp_index(i32(x) + r + 1, width);
            let edge_r = pixel_channel(input[base + left], 0u) + pixel_channel(input[base + right], 0u);
            let edge_g = pixel_channel(input[base + left], 8u) + pixel_channel(input[base + right], 8u);
            let edge_b = pixel_channel(input[base + left], 16u) + pixel_channel(input[base + right], 16u);
            let edge_a = pixel_channel(input[base + left], 24u) + pixel_channel(input[base + right], 24u);
            let out_r = fixed_weighted_average(sum_r, edge_r);
            let out_g = select(pixel_channel(original, 8u), fixed_weighted_average(sum_g, edge_g), mode_has_g(params.mode));
            let out_b = select(pixel_channel(original, 16u), fixed_weighted_average(sum_b, edge_b), mode_has_b(params.mode));
            let out_a = select(pixel_channel(original, 24u), fixed_weighted_average(sum_a, edge_a), mode_has_a(params.mode));
            output[base + x] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
        }

        if x + 1u < width {
            let remove_pixel = input[base + clamp_index(i32(x) - r, width)];
            let add_pixel = input[base + clamp_index(i32(x) + r + 1, width)];
            sum_r = sum_r - pixel_channel(remove_pixel, 0u) + pixel_channel(add_pixel, 0u);
            sum_g = sum_g - pixel_channel(remove_pixel, 8u) + pixel_channel(add_pixel, 8u);
            sum_b = sum_b - pixel_channel(remove_pixel, 16u) + pixel_channel(add_pixel, 16u);
            sum_a = sum_a - pixel_channel(remove_pixel, 24u) + pixel_channel(add_pixel, 24u);
        }
    }
}
