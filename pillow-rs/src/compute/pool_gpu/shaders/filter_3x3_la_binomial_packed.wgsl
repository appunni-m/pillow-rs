// Exact native-LA binomial 3x3 convolution. Each u32 stores two adjacent
// [L, A] pixels; both channels use the same integer [1, 2, 1]^2 kernel.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
    k0: u32,
    k1: u32,
    k2: u32,
    k3: u32,
    k4: u32,
    k5: u32,
    k6: u32,
    k7: u32,
    k8: u32,
    offset_val: i32,
    rational_denominator: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn sample_la(pixel_index: u32, channel: u32) -> u32 {
    let packed_pixel = (input[pixel_index >> 1u] >> ((pixel_index & 1u) * 16u)) & 0xffffu;
    return (packed_pixel >> (channel * 8u)) & 0xffu;
}

fn horizontal_sum(x: u32, y: u32, channel: u32) -> u32 {
    let row = y * params.width;
    return sample_la(row + x - 1u, channel)
        + sample_la(row + x, channel) * 2u
        + sample_la(row + x + 1u, channel);
}

fn filter_sample(x: u32, y: u32, channel: u32) -> u32 {
    let width = params.width;
    let height = params.height;
    let pixel_index = y * width + x;
    if width < 3u || height < 3u
        || x == 0u || x >= width - 1u
        || y == 0u || y >= height - 1u {
        return sample_la(pixel_index, channel);
    }

    let sum = horizontal_sum(x, y - 1u, channel)
        + horizontal_sum(x, y, channel) * 2u
        + horizontal_sum(x, y + 1u, channel);
    return (sum + 8u) >> 4u;
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pixel_count = params.width * params.height;
    let packed_word_count = pixel_count / 2u + select(0u, 1u, (pixel_count & 1u) != 0u);
    if gid.x >= packed_word_count { return; }

    let first_pixel = gid.x * 2u;
    var packed = 0u;
    for (var lane = 0u; lane < 2u; lane += 1u) {
        let pixel_index = first_pixel + lane;
        if pixel_index < pixel_count {
            let x = pixel_index % params.width;
            let y = pixel_index / params.width;
            let luma = filter_sample(x, y, 0u);
            let alpha = filter_sample(x, y, 1u);
            packed = packed | ((luma | (alpha << 8u)) << (lane * 16u));
        }
    }
    output[gid.x] = packed;
}
