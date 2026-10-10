// Exact native-L vertical pass for GaussianBlur. Each invocation owns one
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
    let high = sum * (params.weight_y >> 12u) + edge * (params.edge_weight_y >> 12u);
    let low = sum * (params.weight_y & 4095u) + edge * (params.edge_weight_y & 4095u) + FIXED_BIAS;
    return min((high >> 12u) + ((((high & 4095u) << 12u) + low) >> 24u), 255u);
}

fn unpack_luma_word(word: u32) -> vec4<u32> {
    return vec4<u32>(
        word & 0xffu,
        (word >> 8u) & 0xffu,
        (word >> 16u) & 0xffu,
        (word >> 24u) & 0xffu,
    );
}

fn fixed_weighted_average_word(sum: vec4<u32>, edge: vec4<u32>) -> u32 {
    let whole_high = params.weight_y >> 12u;
    let whole_low = params.weight_y & 4095u;
    let edge_high = params.edge_weight_y >> 12u;
    let edge_low = params.edge_weight_y & 4095u;
    let high = sum * vec4<u32>(whole_high) + edge * vec4<u32>(edge_high);
    let low = sum * vec4<u32>(whole_low) + edge * vec4<u32>(edge_low) + vec4<u32>(FIXED_BIAS);
    let values = min(
        (high >> vec4<u32>(12u))
            + ((((high & vec4<u32>(4095u)) << vec4<u32>(12u)) + low) >> vec4<u32>(24u)),
        vec4<u32>(255u),
    );
    return values.x | (values.y << 8u) | (values.z << 16u) | (values.w << 24u);
}

// Width-aligned rows let one invocation reuse the packed word containing all
// four x lanes for every vertical tap, instead of loading that word once per
// output byte. Odd widths use the linear-pixel fallback below because packed
// words can cross row boundaries there.
fn blur_aligned_word(word_index: u32) -> u32 {
    let words_per_row = params.width >> 2u;
    let x_word = word_index % words_per_row;
    let y = word_index / words_per_row;
    let radius = min(params.radius_y, MAX_RADIUS);
    if radius == 0u && params.edge_weight_y == 0u {
        return input[word_index];
    }

    let r = i32(radius);
    var sum = vec4<u32>(0u);
    for (var dy = -r; dy <= r; dy += 1) {
        let source_y = clamp_offset(y, params.height, dy);
        sum += unpack_luma_word(input[source_y * words_per_row + x_word]);
    }
    let top_y = clamp_offset(y, params.height, -r - 1);
    let bottom_y = clamp_offset(y, params.height, r + 1);
    let edge = unpack_luma_word(input[top_y * words_per_row + x_word])
        + unpack_luma_word(input[bottom_y * words_per_row + x_word]);
    return fixed_weighted_average_word(sum, edge);
}

fn blur_radius_one_aligned_word(word_index: u32) -> u32 {
    let words_per_row = params.width >> 2u;
    let x_word = word_index % words_per_row;
    let y = word_index / words_per_row;
    let top_y = clamp_offset(y, params.height, -1);
    let bottom_y = clamp_offset(y, params.height, 1);
    let top = unpack_luma_word(input[top_y * words_per_row + x_word]);
    let center = unpack_luma_word(input[word_index]);
    let bottom = unpack_luma_word(input[bottom_y * words_per_row + x_word]);
    let sum = top + center + bottom;
    let average = ((sum + vec4<u32>(1u)) * vec4<u32>(21846u)) >> vec4<u32>(16u);
    return average.x | (average.y << 8u) | (average.z << 16u) | (average.w << 24u);
}

fn blur_pixel(pixel_index: u32) -> u32 {
    let width = params.width;
    let height = params.height;
    let x = pixel_index % width;
    let y = pixel_index / width;
    let radius = min(params.radius_y, MAX_RADIUS);
    if radius == 0u && params.edge_weight_y == 0u {
        return load_sample(pixel_index);
    }

    let r = i32(radius);
    var sum = 0u;
    for (var dy = -r; dy <= r; dy += 1) {
        let source_y = clamp_offset(y, height, dy);
        sum += load_sample(source_y * width + x);
    }
    let top_y = clamp_offset(y, height, -r - 1);
    let bottom_y = clamp_offset(y, height, r + 1);
    let edge = load_sample(top_y * width + x) + load_sample(bottom_y * width + x);
    return fixed_weighted_average(sum, edge);
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
        if params.radius_y == 1u && params.edge_weight_y == 0u {
            output[word_index] = blur_radius_one_aligned_word(word_index);
            return;
        }
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
