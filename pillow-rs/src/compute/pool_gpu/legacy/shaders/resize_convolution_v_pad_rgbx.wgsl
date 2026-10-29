// RGBX-only fused vertical resize and ImageOps.pad placement.
//
// The horizontal input is exactly the intermediate produced by
// resize_convolution_h.wgsl. Preserve its fixed-point coefficient order, but
// write the resized rows into their final canvas window and fill the rest in
// the same dispatch. RGBX's fourth byte is data padding, not alpha, so all
// four stored bytes pass through the same channel filter without premultiply.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad0: u32,
    resized_w: u32,
    resized_h: u32,
    channels: u32,
    premultiply: u32,
    dst_w: u32,
    dst_h: u32,
    fill: u32,
    offset_x: u32,
    offset_y: u32,
    _pad1: u32,
    _pad2: u32,
    _pad3: u32,
}

const FIXED_BIAS: i32 = 2097152;

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;
@group(0) @binding(3) var<storage, read> coefficients: array<i32>;

fn pixel_channel(pixel: u32, channel: u32) -> u32 {
    return (pixel >> (channel * 8u)) & 255u;
}

fn fixed_to_byte(sum: i32) -> u32 {
    let value = (sum + FIXED_BIAS) >> 22;
    return u32(clamp(value, 0, 255));
}

fn filtered_channel(output_x: u32, output_y: u32, channel: u32) -> u32 {
    let metadata = output_y * 3u;
    let source_y = u32(coefficients[metadata]);
    let count = u32(coefficients[metadata + 1u]);
    let weight_base = 3u * params.resized_h + u32(coefficients[metadata + 2u]);
    var sum: i32 = 0;
    for (var tap = 0u; tap < count; tap = tap + 1u) {
        let pixel = input[(source_y + tap) * params.resized_w + output_x];
        sum = sum + i32(pixel_channel(pixel, channel)) * coefficients[weight_base + tap];
    }
    return fixed_to_byte(sum);
}

fn filtered_pixel(output_x: u32, output_y: u32) -> u32 {
    let red = filtered_channel(output_x, output_y, 0u);
    let green = filtered_channel(output_x, output_y, 1u);
    let blue = filtered_channel(output_x, output_y, 2u);
    let padding = filtered_channel(output_x, output_y, 3u);
    return red | (green << 8u) | (blue << 16u) | (padding << 24u);
}

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.dst_w || gid.y >= params.dst_h {
        return;
    }

    let index = gid.y * params.dst_w + gid.x;
    if gid.x < params.offset_x || gid.y < params.offset_y {
        output[index] = params.fill;
        return;
    }

    let in_x = gid.x - params.offset_x;
    let in_y = gid.y - params.offset_y;
    if in_x >= params.resized_w || in_y >= params.resized_h {
        output[index] = params.fill;
        return;
    }

    output[index] = filtered_pixel(in_x, in_y);
}
