// Exact RankFilter(9) for native L bytes. Each invocation selects four
// adjacent output samples and writes one packed word. Its lower-bound binary
// search and replicated-edge sampling match Pillow's rank_filter_impl.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
    size: u32,
    rank: u32,
}

const WINDOW: u32 = 81u;

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn clamp_offset(coordinate: u32, extent: u32, delta: i32) -> u32 {
    if delta < 0 {
        let amount = u32(-delta);
        if coordinate < amount {
            return 0u;
        }
        return coordinate - amount;
    }
    let amount = u32(delta);
    let maximum = extent - 1u;
    if amount > maximum || coordinate > maximum - amount {
        return maximum;
    }
    return coordinate + amount;
}

fn sample_neighbor(x: u32, y: u32, dx: i32, dy: i32) -> u32 {
    let source_x = clamp_offset(x, params.width, dx);
    let source_y = clamp_offset(y, params.height, dy);
    let source_index = source_y * params.width + source_x;
    let packed = input[source_index >> 2u];
    return (packed >> ((source_index & 3u) * 8u)) & 0xffu;
}

fn four_neighbors(xs: vec4<u32>, ys: vec4<u32>, dx: i32, dy: i32) -> vec4<u32> {
    return vec4<u32>(
        sample_neighbor(xs.x, ys.x, dx, dy),
        sample_neighbor(xs.y, ys.y, dx, dy),
        sample_neighbor(xs.z, ys.z, dx, dy),
        sample_neighbor(xs.w, ys.w, dx, dy),
    );
}

fn select_rank_four(
    samples: ptr<function, array<vec4<u32>, 81>>,
    rank: vec4<u32>,
) -> vec4<u32> {
    var low = vec4<u32>(0u);
    var high = vec4<u32>(255u);

    // Lower-bound binary search is an exact order statistic even when many
    // samples are equal. `count > rank` is required because rank is zero-based.
    for (var search_step = 0u; search_step < 8u; search_step++) {
        let middle = (low + high) >> vec4<u32>(1u);
        var less_or_equal = vec4<u32>(0u);
        for (var sample_index = 0u; sample_index < WINDOW; sample_index++) {
            let sample = (*samples)[sample_index];
            less_or_equal += select(
                vec4<u32>(0u),
                vec4<u32>(1u),
                sample <= middle,
            );
        }
        let search_upper_half = less_or_equal > rank;
        low = select(middle + vec4<u32>(1u), low, search_upper_half);
        high = select(high, middle, search_upper_half);
    }
    return low;
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pixel_count = params.width * params.height;
    if pixel_count == 0u {
        return;
    }
    let word_count = pixel_count / 4u + select(0u, 1u, (pixel_count & 3u) != 0u);
    if gid.x >= word_count {
        return;
    }

    let base_pixel = gid.x * 4u;
    let last_pixel = pixel_count - 1u;
    let pixels = min(
        vec4<u32>(base_pixel, base_pixel + 1u, base_pixel + 2u, base_pixel + 3u),
        vec4<u32>(last_pixel),
    );
    let width4 = vec4<u32>(params.width);
    let xs = pixels % width4;
    let ys = pixels / width4;
    var samples: array<vec4<u32>, 81>;
    var sample_index = 0u;
    for (var dy = -4; dy <= 4; dy++) {
        for (var dx = -4; dx <= 4; dx++) {
            samples[sample_index] = four_neighbors(xs, ys, dx, dy);
            sample_index++;
        }
    }

    let selected = select_rank_four(&samples, vec4<u32>(min(params.rank, WINDOW - 1u)));
    let valid = vec4<bool>(
        base_pixel < pixel_count,
        base_pixel + 1u < pixel_count,
        base_pixel + 2u < pixel_count,
        base_pixel + 3u < pixel_count,
    );
    let result = select(vec4<u32>(0u), selected, valid);
    output[gid.x] = (result.x & 0xffu)
        | ((result.y & 0xffu) << 8u)
        | ((result.z & 0xffu) << 16u)
        | ((result.w & 0xffu) << 24u);
}
