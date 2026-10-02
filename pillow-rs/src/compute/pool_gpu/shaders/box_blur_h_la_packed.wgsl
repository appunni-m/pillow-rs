// Exact native-LA horizontal pass for BoxBlur and GaussianBlur. Each u32
// contains two interleaved [L, A] pixels; pixel coordinates are derived from
// the linear index so an odd-width row boundary remains correct.

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

fn load_la(pixel_index: u32) -> vec2<u32> {
    let word = input[pixel_index >> 1u];
    let shift = (pixel_index & 1u) * 16u;
    let pixel = (word >> shift) & 0xffffu;
    return vec2<u32>(pixel & 0xffu, pixel >> 8u);
}

fn clamp_offset(value: u32, extent: u32, delta: i32) -> u32 {
    if delta < 0 {
        let amount = u32(-delta);
        if value < amount { return 0u; }
        return value - amount;
    }
    let amount = u32(delta);
    let last = extent - 1u;
    if amount > last - value { return last; }
    return value + amount;
}

fn fixed_weighted_average(sum: vec2<u32>, edge: vec2<u32>) -> vec2<u32> {
    let whole_high = params.weight_x >> 12u;
    let whole_low = params.weight_x & 4095u;
    let edge_high = params.edge_weight_x >> 12u;
    let edge_low = params.edge_weight_x & 4095u;
    let high = sum * vec2<u32>(whole_high) + edge * vec2<u32>(edge_high);
    let low = sum * vec2<u32>(whole_low) + edge * vec2<u32>(edge_low) + vec2<u32>(FIXED_BIAS);
    return min(
        (high >> vec2<u32>(12u))
            + ((((high & vec2<u32>(4095u)) << vec2<u32>(12u)) + low) >> vec2<u32>(24u)),
        vec2<u32>(255u),
    );
}

fn blur_pixel(pixel_index: u32) -> u32 {
    let width = params.width;
    let x = pixel_index % width;
    let y = pixel_index / width;
    let radius = min(params.radius_x, MAX_RADIUS);
    if radius == 0u && params.edge_weight_x == 0u {
        return input[pixel_index >> 1u] >> ((pixel_index & 1u) * 16u) & 0xffffu;
    }

    let r = i32(radius);
    var sum = vec2<u32>(0u);
    for (var dx = -r; dx <= r; dx += 1) {
        let sx = clamp_offset(x, width, dx);
        sum += load_la(y * width + sx);
    }
    let left = clamp_offset(x, width, -r - 1);
    let right = clamp_offset(x, width, r + 1);
    let edge = load_la(y * width + left) + load_la(y * width + right);
    let value = fixed_weighted_average(sum, edge);
    return value.x | (value.y << 8u);
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pixel_count = params.width * params.height;
    if pixel_count == 0u { return; }
    let word_count = pixel_count / 2u + select(0u, 1u, (pixel_count & 1u) != 0u);
    let word_index = gid.x;
    if word_index >= word_count { return; }

    let base_pixel = word_index * 2u;
    var packed = 0u;
    for (var lane = 0u; lane < 2u; lane += 1u) {
        let pixel_index = base_pixel + lane;
        if pixel_index < pixel_count {
            packed |= blur_pixel(pixel_index) << (lane * 16u);
        }
    }
    output[word_index] = packed;
}
