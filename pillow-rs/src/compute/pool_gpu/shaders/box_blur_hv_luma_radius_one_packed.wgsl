// One-dispatch native-L BoxBlur(1). It preserves Pillow's horizontal byte
// rounding before applying the vertical byte rounding, while avoiding the
// full-frame intermediate written by the separable two-dispatch path.

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

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn load_sample(pixel_index: u32) -> u32 {
    let word = input[pixel_index >> 2u];
    return (word >> ((pixel_index & 3u) * 8u)) & 0xffu;
}

fn average_three(a: u32, b: u32, c: u32) -> u32 {
    // Exact rounded divide-by-three used by the radius-one fixed-point path.
    return ((a + b + c + 1u) * 21846u) >> 16u;
}

fn average_three_word(a: vec4<u32>, b: vec4<u32>, c: vec4<u32>) -> vec4<u32> {
    return ((a + b + c + vec4<u32>(1u)) * vec4<u32>(21846u)) >> vec4<u32>(16u);
}

fn horizontal_pixel(x: u32, y: u32) -> u32 {
    let left_x = select(x - 1u, 0u, x == 0u);
    let right_x = min(x + 1u, params.width - 1u);
    return average_three(
        load_sample(y * params.width + left_x),
        load_sample(y * params.width + x),
        load_sample(y * params.width + right_x),
    );
}

fn blur_pixel(pixel_index: u32) -> u32 {
    let x = pixel_index % params.width;
    let y = pixel_index / params.width;
    let top_y = select(y - 1u, 0u, y == 0u);
    let bottom_y = min(y + 1u, params.height - 1u);
    return average_three(
        horizontal_pixel(x, top_y),
        horizontal_pixel(x, y),
        horizontal_pixel(x, bottom_y),
    );
}

fn unpack_luma_word(word: u32) -> vec4<u32> {
    return vec4<u32>(
        word & 0xffu,
        (word >> 8u) & 0xffu,
        (word >> 16u) & 0xffu,
        (word >> 24u) & 0xffu,
    );
}

fn horizontal_word(y: u32, x_word: u32) -> vec4<u32> {
    let words_per_row = params.width >> 2u;
    let index = y * words_per_row + x_word;
    let current = unpack_luma_word(input[index]);
    var previous = current;
    var next = current;
    if x_word > 0u {
        previous = unpack_luma_word(input[index - 1u]);
    }
    if x_word + 1u < words_per_row {
        next = unpack_luma_word(input[index + 1u]);
    }

    var left = vec4<u32>(previous.w, current.x, current.y, current.z);
    var right = vec4<u32>(current.y, current.z, current.w, next.x);
    if x_word == 0u {
        left.x = current.x;
    }
    if x_word + 1u == words_per_row {
        right.w = current.w;
    }
    return average_three_word(left, current, right);
}

fn blur_aligned_word(word_index: u32) -> u32 {
    let words_per_row = params.width >> 2u;
    let x_word = word_index % words_per_row;
    let y = word_index / words_per_row;
    let top_y = select(y - 1u, 0u, y == 0u);
    let bottom_y = min(y + 1u, params.height - 1u);
    let values = average_three_word(
        horizontal_word(top_y, x_word),
        horizontal_word(y, x_word),
        horizontal_word(bottom_y, x_word),
    );
    return values.x | (values.y << 8u) | (values.z << 16u) | (values.w << 24u);
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pixel_count = params.width * params.height;
    if pixel_count == 0u {
        return;
    }
    let word_count = pixel_count / 4u + select(0u, 1u, (pixel_count & 3u) != 0u);
    let word_index = gid.x;
    if word_index >= word_count {
        return;
    }

    if (params.width & 3u) == 0u {
        output[word_index] = blur_aligned_word(word_index);
        return;
    }

    let base_pixel = word_index * 4u;
    var packed = 0u;
    for (var lane = 0u; lane < 4u; lane += 1u) {
        let pixel_index = base_pixel + lane;
        if pixel_index < pixel_count {
            packed |= blur_pixel(pixel_index) << (lane * 8u);
        }
    }
    output[word_index] = packed;
}
