// Exact vertical pass for BoxBlur and GaussianBlur.
//
// Each invocation computes one output pixel, exposing the image's full 2D
// parallelism. Whole-window and fractional-edge samples use the same packed
// byte values and fixed-point weights as the CPU implementation.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
    radius_x: u32,
    weight_x: u32,
    edge_weight_x: u32,
    radius_y: u32,
    weight_y: u32,
    edge_weight_y: u32,
}

const MAX_RADIUS: u32 = 64u;
const FIXED_BIAS: u32 = 8388608u;
const TILE_SIZE: u32 = 16u;

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
    let high = sum * (params.weight_y >> 12u) + edge * (params.edge_weight_y >> 12u);
    let low = sum * (params.weight_y & 4095u) + edge * (params.edge_weight_y & 4095u) + FIXED_BIAS;
    return min((high >> 12u) + ((((high & 4095u) << 12u) + low) >> 24u), 255u);
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn blur_pixel(x: u32, y: u32) {
    let width = params.width;
    let height = params.height;
    let radius = min(params.radius_y, MAX_RADIUS);
    let index = y * width + x;
    let original = input[index];
    if radius == 0u && params.edge_weight_y == 0u {
        output[index] = original;
        return;
    }

    let r = i32(radius);
    var sum_r = 0u;
    var sum_g = 0u;
    var sum_b = 0u;
    var sum_a = 0u;
    for (var dy = -r; dy <= r; dy += 1) {
        let sy = clamp_index(i32(y) + dy, height);
        let pixel = input[sy * width + x];
        sum_r += pixel_channel(pixel, 0u);
        sum_g += pixel_channel(pixel, 8u);
        sum_b += pixel_channel(pixel, 16u);
        sum_a += pixel_channel(pixel, 24u);
    }

    let top = clamp_index(i32(y) - r - 1, height);
    let bottom = clamp_index(i32(y) + r + 1, height);
    let top_pixel = input[top * width + x];
    let bottom_pixel = input[bottom * width + x];
    let edge_r = pixel_channel(top_pixel, 0u) + pixel_channel(bottom_pixel, 0u);
    let edge_g = pixel_channel(top_pixel, 8u) + pixel_channel(bottom_pixel, 8u);
    let edge_b = pixel_channel(top_pixel, 16u) + pixel_channel(bottom_pixel, 16u);
    let edge_a = pixel_channel(top_pixel, 24u) + pixel_channel(bottom_pixel, 24u);
    let out_r = fixed_weighted_average(sum_r, edge_r);
    let out_g = select(pixel_channel(original, 8u), fixed_weighted_average(sum_g, edge_g), mode_has_g(params.mode));
    let out_b = select(pixel_channel(original, 16u), fixed_weighted_average(sum_b, edge_b), mode_has_b(params.mode));
    let out_a = select(pixel_channel(original, 24u), fixed_weighted_average(sum_a, edge_a), mode_has_a(params.mode));
    output[index] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}

@compute @workgroup_size(16, 16, 1)
fn main(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(num_workgroups) workgroups: vec3<u32>,
) {
    if params.width == 0u || params.height == 0u {
        return;
    }
    let stride_x = workgroups.x * TILE_SIZE;
    let stride_y = workgroups.y * TILE_SIZE;
    var y = gid.y;
    loop {
        if y >= params.height {
            break;
        }
        var x = gid.x;
        loop {
            if x >= params.width {
                break;
            }
            blur_pixel(x, y);
            if params.width - x <= stride_x {
                break;
            }
            x += stride_x;
        }
        if params.height - y <= stride_y {
            break;
        }
        y += stride_y;
    }
}
