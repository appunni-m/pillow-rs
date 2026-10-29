// Exact native-LA 3x3 RankFilter. Each invocation selects one order statistic
// for two interleaved pixels across four lanes: L0, A0, L1, A1. The output
// word remains native LA bytes and each channel is ranked independently.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
    size: u32,
    rank: u32,
}

const WINDOW: u32 = 9u;

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn sort_nine_samples(values: ptr<function, array<vec4<u32>, WINDOW>>) {
    for (var phase = 0u; phase < WINDOW; phase++) {
        var left = phase & 1u;
        loop {
            if left + 1u >= WINDOW {
                break;
            }
            let lower = min((*values)[left], (*values)[left + 1u]);
            let upper = max((*values)[left], (*values)[left + 1u]);
            (*values)[left] = lower;
            (*values)[left + 1u] = upper;
            left += 2u;
        }
    }
}

fn clamp_offset(coordinate: u32, extent: u32, delta: i32) -> u32 {
    if delta < 0 && coordinate > 0u {
        return coordinate - 1u;
    }
    if delta > 0 && coordinate < extent - 1u {
        return coordinate + 1u;
    }
    return coordinate;
}

fn load_la(pixel_index: u32) -> vec2<u32> {
    let packed = input[pixel_index >> 1u];
    let pixel = (packed >> ((pixel_index & 1u) * 16u)) & 0xffffu;
    return vec2<u32>(pixel & 0xffu, pixel >> 8u);
}

fn sample_pair(pixels: vec2<u32>, dx: i32, dy: i32) -> vec4<u32> {
    let x0 = pixels.x % params.width;
    let y0 = pixels.x / params.width;
    let x1 = pixels.y % params.width;
    let y1 = pixels.y / params.width;
    let sx0 = clamp_offset(x0, params.width, dx);
    let sy0 = clamp_offset(y0, params.height, dy);
    let sx1 = clamp_offset(x1, params.width, dx);
    let sy1 = clamp_offset(y1, params.height, dy);
    let value0 = load_la(sy0 * params.width + sx0);
    let value1 = load_la(sy1 * params.width + sx1);
    return vec4<u32>(value0.x, value0.y, value1.x, value1.y);
}

fn select_rank(values: ptr<function, array<vec4<u32>, WINDOW>>) -> vec4<u32> {
    if params.rank == 0u {
        var smallest = (*values)[0];
        for (var index = 1u; index < WINDOW; index++) {
            smallest = min(smallest, (*values)[index]);
        }
        return smallest;
    }
    if params.rank == 1u {
        var smallest = min((*values)[0], (*values)[1]);
        var second = max((*values)[0], (*values)[1]);
        for (var index = 2u; index < WINDOW; index++) {
            let lower = min(smallest, (*values)[index]);
            let upper = max(smallest, (*values)[index]);
            second = min(second, upper);
            smallest = lower;
        }
        return second;
    }
    if params.rank == WINDOW - 1u {
        var largest = (*values)[0];
        for (var index = 1u; index < WINDOW; index++) {
            largest = max(largest, (*values)[index]);
        }
        return largest;
    }
    sort_nine_samples(values);
    return (*values)[min(params.rank, WINDOW - 1u)];
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pixel_count = params.width * params.height;
    if pixel_count == 0u {
        return;
    }
    let word_count = pixel_count / 2u + select(0u, 1u, (pixel_count & 1u) != 0u);
    if gid.x >= word_count {
        return;
    }

    let first_pixel = gid.x * 2u;
    let pixels = vec2<u32>(first_pixel, min(first_pixel + 1u, pixel_count - 1u));
    var values: array<vec4<u32>, WINDOW>;
    var index = 0u;
    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            values[index] = sample_pair(pixels, dx, dy);
            index += 1u;
        }
    }
    let selected = select_rank(&values);
    let second_pixel_valid = first_pixel + 1u < pixel_count;
    let valid = vec4<bool>(true, true, second_pixel_valid, second_pixel_valid);
    let output_samples = select(vec4<u32>(0u), selected, valid);
    output[gid.x] = (output_samples.x & 0xffu)
        | ((output_samples.y & 0xffu) << 8u)
        | ((output_samples.z & 0xffu) << 16u)
        | ((output_samples.w & 0xffu) << 24u);
}
