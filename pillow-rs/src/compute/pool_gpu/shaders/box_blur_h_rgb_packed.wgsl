// Native RGB horizontal BoxBlur(1). Each invocation filters four consecutive
// pixels, then owns the three u32 words containing their twelve output bytes.
// The pixel group may cross an image-row boundary; each sample derives its
// own coordinates so the horizontal window clamps within that row.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    flags: u32,
    radius_x: u32,
    weight_x: u32,
    edge_weight_x: u32,
    radius_y: u32,
    weight_y: u32,
    edge_weight_y: u32,
}

const FIXED_BIAS: u32 = 8388608u;

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn load_channel(pixel_index: u32, channel: u32) -> u32 {
    let byte_index = pixel_index * 3u + channel;
    return (input[byte_index >> 2u] >> ((byte_index & 3u) * 8u)) & 0xffu;
}

fn clamp_offset(value: u32, extent: u32, delta: i32) -> u32 {
    if delta < 0 {
        let amount = u32(-delta);
        if value < amount {
            return 0u;
        }
        return value - amount;
    }
    let amount = u32(delta);
    let last = extent - 1u;
    if amount > last - value {
        return last;
    }
    return value + amount;
}

fn fixed_weighted_average(sum: u32, edge: u32) -> u32 {
    let high = sum * (params.weight_x >> 12u) + edge * (params.edge_weight_x >> 12u);
    let low = sum * (params.weight_x & 4095u)
        + edge * (params.edge_weight_x & 4095u)
        + FIXED_BIAS;
    return min((high >> 12u) + ((((high & 4095u) << 12u) + low) >> 24u), 255u);
}

fn blur_channel(pixel_index: u32, channel: u32) -> u32 {
    let x = pixel_index % params.width;
    let y = pixel_index / params.width;
    let radius = i32(params.radius_x);
    var sum = 0u;
    for (var dx = -radius; dx <= radius; dx += 1) {
        let source_x = clamp_offset(x, params.width, dx);
        sum += load_channel(y * params.width + source_x, channel);
    }
    let left_x = clamp_offset(x, params.width, -radius - 1);
    let right_x = clamp_offset(x, params.width, radius + 1);
    let edge = load_channel(y * params.width + left_x, channel)
        + load_channel(y * params.width + right_x, channel);
    return fixed_weighted_average(sum, edge);
}

fn blur_pixel(pixel_index: u32) -> u32 {
    return blur_channel(pixel_index, 0u)
        | (blur_channel(pixel_index, 1u) << 8u)
        | (blur_channel(pixel_index, 2u) << 16u);
}

@compute @workgroup_size(16, 16)
fn main(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(num_workgroups) num_workgroups: vec3<u32>,
) {
    let total_pixels = params.width * params.height;
    var output_group = gid.x + (num_workgroups.x * 16u) * gid.y;
    if (params.flags & 16u) != 0u {
        let groups_per_row = params.width >> 2u;
        if gid.x >= groups_per_row || gid.y >= params.height {
            return;
        }
        output_group = gid.y * groups_per_row + gid.x;
    }
    let output_group_count = (total_pixels >> 2u)
        + select(0u, 1u, (total_pixels & 3u) != 0u);
    if output_group >= output_group_count {
        return;
    }

    let first_pixel = output_group * 4u;
    var pixel0 = 0u;
    var pixel1 = 0u;
    var pixel2 = 0u;
    var pixel3 = 0u;
    if first_pixel < total_pixels {
        pixel0 = blur_pixel(first_pixel);
    }
    if first_pixel + 1u < total_pixels {
        pixel1 = blur_pixel(first_pixel + 1u);
    }
    if first_pixel + 2u < total_pixels {
        pixel2 = blur_pixel(first_pixel + 2u);
    }
    if first_pixel + 3u < total_pixels {
        pixel3 = blur_pixel(first_pixel + 3u);
    }

    let output_bytes = total_pixels * 3u;
    let output_word_count = (output_bytes >> 2u)
        + select(0u, 1u, (output_bytes & 3u) != 0u);
    let word = output_group * 3u;
    if word < output_word_count {
        output[word] = (pixel0 & 0x00ffffffu) | ((pixel1 & 0x000000ffu) << 24u);
    }
    if word + 1u < output_word_count {
        output[word + 1u] = ((pixel1 >> 8u) & 0x0000ffffu)
            | ((pixel2 & 0x0000ffffu) << 16u);
    }
    if word + 2u < output_word_count {
        output[word + 2u] = ((pixel2 >> 16u) & 0x000000ffu)
            | ((pixel3 & 0x00ffffffu) << 8u);
    }
}
