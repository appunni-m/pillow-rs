// Exact vertical pass for Pillow's separable byte resize.
//
// The input is the packed intermediate produced by resize_convolution_h.  Its
// width is dst_w and its height is the original source height.  The vertical
// table uses the same metadata layout as the horizontal table, with dst_h
// metadata entries.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
    dst_w: u32,
    dst_h: u32,
    channels: u32,
    premultiply: u32,
}

const FIXED_BIAS: i32 = 2097152;

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;
@group(0) @binding(3) var<storage, read> coefficients: array<i32>;

fn pixel_channel(pixel: u32, channel: u32) -> u32 {
    // LA is transported as RGBA: logical alpha is stored in packed byte 3,
    // not byte 1.  Keep the logical channel numbering used by the coefficient
    // path while preserving the native packed upload representation.
    if params.channels == 2u && channel == 1u {
        return (pixel >> 24u) & 255u;
    }
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
    let weight_base = 3u * params.dst_h + u32(coefficients[metadata + 2u]);
    var sum: i32 = 0;
    for (var tap = 0u; tap < count; tap = tap + 1u) {
        let pixel = input[(source_y + tap) * params.dst_w + output_x];
        sum = sum + i32(pixel_channel(pixel, channel)) * coefficients[weight_base + tap];
    }
    return fixed_to_byte(sum);
}

fn filtered_float(output_x: u32, output_y: u32) -> f32 {
    let metadata = output_y * 3u;
    let source_y = u32(coefficients[metadata]);
    let count = u32(coefficients[metadata + 1u]);
    let weight_base = 3u * params.dst_h + u32(coefficients[metadata + 2u]);
    var sum: f32 = 0.0;
    for (var tap = 0u; tap < count; tap = tap + 1u) {
        let sample = bitcast<f32>(input[(source_y + tap) * params.dst_w + output_x]);
        sum = sum + sample * f32(coefficients[weight_base + tap]) / 4194304.0;
    }
    return sum;
}

fn unpremultiply(value: u32, alpha: u32) -> u32 {
    if alpha == 0u {
        return 0u;
    }
    return (value * 255u) / alpha;
}

fn pack_filtered(output_x: u32, output_y: u32) -> u32 {
    if params.mode == 8u {
        return bitcast<u32>(filtered_float(output_x, output_y));
    }
    let alpha = select(
        255u,
        filtered_channel(output_x, output_y, params.channels - 1u),
        params.channels == 2u || params.channels == 4u,
    );
    var red = filtered_channel(output_x, output_y, 0u);
    if params.premultiply != 0u {
        red = unpremultiply(red, alpha);
    }
    let green = select(
        0u,
        filtered_channel(output_x, output_y, 1u),
        params.channels >= 3u,
    );
    let blue = select(
        0u,
        filtered_channel(output_x, output_y, 2u),
        params.channels >= 3u,
    );
    var result_green = green;
    var result_blue = blue;
    if params.premultiply != 0u && params.channels == 4u {
        result_green = unpremultiply(green, alpha);
        result_blue = unpremultiply(blue, alpha);
    }
    return red | (result_green << 8u) | (result_blue << 16u) | (alpha << 24u);
}

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.dst_w || gid.y >= params.dst_h {
        return;
    }
    output[gid.y * params.dst_w + gid.x] = pack_filtered(gid.x, gid.y);
}
