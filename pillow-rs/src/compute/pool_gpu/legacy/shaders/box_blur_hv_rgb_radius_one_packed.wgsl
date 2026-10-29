// One-dispatch native RGB BoxBlur(1). Preserve Pillow's horizontal byte
// rounding before vertical byte rounding while keeping compact RGB storage.

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

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn load_channel(pixel_index: u32, channel: u32) -> u32 {
    let byte_index = pixel_index * 3u + channel;
    return (input[byte_index >> 2u] >> ((byte_index & 3u) * 8u)) & 0xffu;
}

fn average_three(a: u32, b: u32, c: u32) -> u32 {
    // Exact rounded divide-by-three for every possible byte sum.
    return ((a + b + c + 1u) * 21846u) >> 16u;
}

fn average_three_four(a: vec4<u32>, b: vec4<u32>, c: vec4<u32>) -> vec4<u32> {
    return ((a + b + c + vec4<u32>(1u)) * vec4<u32>(21846u)) >> vec4<u32>(16u);
}

fn horizontal_channel(pixel_index: u32, channel: u32) -> u32 {
    let x = pixel_index % params.width;
    let y = pixel_index / params.width;
    let left_x = select(x - 1u, 0u, x == 0u);
    let right_x = min(x + 1u, params.width - 1u);
    return average_three(
        load_channel(y * params.width + left_x, channel),
        load_channel(pixel_index, channel),
        load_channel(y * params.width + right_x, channel),
    );
}

fn blur_channel(pixel_index: u32, channel: u32) -> u32 {
    let y = pixel_index / params.width;
    let top_y = select(y - 1u, 0u, y == 0u);
    let bottom_y = min(y + 1u, params.height - 1u);
    return average_three(
        horizontal_channel(top_y * params.width + pixel_index % params.width, channel),
        horizontal_channel(pixel_index, channel),
        horizontal_channel(bottom_y * params.width + pixel_index % params.width, channel),
    );
}

fn blur_pixel(pixel_index: u32) -> u32 {
    return blur_channel(pixel_index, 0u)
        | (blur_channel(pixel_index, 1u) << 8u)
        | (blur_channel(pixel_index, 2u) << 16u);
}

fn horizontal_four(first_x: u32, row: u32, channel: u32) -> vec4<u32> {
    let x_minus_one = select(first_x - 1u, 0u, first_x == 0u);
    let x_one = min(first_x + 1u, params.width - 1u);
    let x_two = min(first_x + 2u, params.width - 1u);
    let x_three = min(first_x + 3u, params.width - 1u);
    let x_four = min(first_x + 4u, params.width - 1u);
    let sample0 = load_channel(row * params.width + x_minus_one, channel);
    let sample1 = load_channel(row * params.width + first_x, channel);
    let sample2 = load_channel(row * params.width + x_one, channel);
    let sample3 = load_channel(row * params.width + x_two, channel);
    let sample4 = load_channel(row * params.width + x_three, channel);
    let sample5 = load_channel(row * params.width + x_four, channel);
    return average_three_four(
        vec4<u32>(sample0, sample1, sample2, sample3),
        vec4<u32>(sample1, sample2, sample3, sample4),
        vec4<u32>(sample2, sample3, sample4, sample5),
    );
}

fn blur_rgb_group_channel(first_pixel: u32, channel: u32) -> vec4<u32> {
    let x = first_pixel % params.width;
    let y = first_pixel / params.width;
    let top_y = select(y - 1u, 0u, y == 0u);
    let bottom_y = min(y + 1u, params.height - 1u);
    let top = horizontal_four(x, top_y, channel);
    let center = horizontal_four(x, y, channel);
    let bottom = horizontal_four(x, bottom_y, channel);
    return average_three_four(top, center, bottom);
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
    if (params.width & 3u) == 0u {
        let red = blur_rgb_group_channel(first_pixel, 0u);
        let green = blur_rgb_group_channel(first_pixel, 1u);
        let blue = blur_rgb_group_channel(first_pixel, 2u);
        pixel0 = red.x | (green.x << 8u) | (blue.x << 16u);
        pixel1 = red.y | (green.y << 8u) | (blue.y << 16u);
        pixel2 = red.z | (green.z << 8u) | (blue.z << 16u);
        pixel3 = red.w | (green.w << 8u) | (blue.w << 16u);
    } else {
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
