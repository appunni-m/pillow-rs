// Exact native-L horizontal pass for GaussianBlur. Each invocation owns one
// packed output word and computes its four byte lanes independently. Lanes are
// linear pixels, so derive x/y before clamping; a word may cross an odd-width
// row boundary.

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

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn load_sample(pixel_index: u32) -> u32 {
    let word = input[pixel_index >> 2u];
    return (word >> ((pixel_index & 3u) * 8u)) & 0xffu;
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
    let low = sum * (params.weight_x & 4095u) + edge * (params.edge_weight_x & 4095u) + FIXED_BIAS;
    return min((high >> 12u) + ((((high & 4095u) << 12u) + low) >> 24u), 255u);
}

fn blur_pixel(pixel_index: u32) -> u32 {
    let width = params.width;
    let x = pixel_index % width;
    let y = pixel_index / width;
    let radius = min(params.radius_x, MAX_RADIUS);
    if radius == 0u && params.edge_weight_x == 0u {
        return load_sample(pixel_index);
    }

    let r = i32(radius);
    var sum = 0u;
    for (var dx = -r; dx <= r; dx += 1) {
        let source_x = clamp_offset(x, width, dx);
        sum += load_sample(y * width + source_x);
    }
    let left_x = clamp_offset(x, width, -r - 1);
    let right_x = clamp_offset(x, width, r + 1);
    let edge = load_sample(y * width + left_x) + load_sample(y * width + right_x);
    return fixed_weighted_average(sum, edge);
}

fn unpack_luma_word(word: u32) -> vec4<u32> {
    return vec4<u32>(
        word & 0xffu,
        (word >> 8u) & 0xffu,
        (word >> 16u) & 0xffu,
        (word >> 24u) & 0xffu,
    );
}

// An aligned word contains four consecutive pixels in one row. Neighboring
// words supply the two horizontal samples; the outer lanes replicate the
// first or last pixel at each image edge.
fn blur_radius_one_aligned_word(word_index: u32) -> u32 {
    let words_per_row = params.width >> 2u;
    let x_word = word_index % words_per_row;
    let current = unpack_luma_word(input[word_index]);
    var previous = current;
    var next = current;
    if x_word > 0u {
        previous = unpack_luma_word(input[word_index - 1u]);
    }
    if x_word + 1u < words_per_row {
        next = unpack_luma_word(input[word_index + 1u]);
    }

    var left = vec4<u32>(previous.w, current.x, current.y, current.z);
    var right = vec4<u32>(current.y, current.z, current.w, next.x);
    if x_word == 0u {
        left.x = current.x;
    }
    if x_word + 1u == words_per_row {
        right.w = current.w;
    }

    let sum = left + current + right;
    let average = ((sum + vec4<u32>(1u)) * vec4<u32>(21846u)) >> vec4<u32>(16u);
    return average.x | (average.y << 8u) | (average.z << 16u) | (average.w << 24u);
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

    if (params.width & 3u) == 0u
        && params.radius_x == 1u
        && params.edge_weight_x == 0u
    {
        output[word_index] = blur_radius_one_aligned_word(word_index);
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
