// Exact RankFilter(9) for native L images whose rows are word-aligned.
// One invocation owns four adjacent output pixels. Their nine windows share
// most source samples, so cache only the three packed words spanning each
// clamped source row and gather each four-lane window from that compact tile.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
    size: u32,
    rank: u32,
}

const WINDOW: u32 = 81u;
const ROW_WORDS: u32 = 3u;

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn repeated_byte(value: u32) -> u32 {
    let byte = value & 0xffu;
    return byte * 0x01010101u;
}

fn four_window_samples(
    samples: ptr<function, array<u32, 27>>,
    row: u32,
    dx: i32,
) -> vec4<u32> {
    let relative = u32(dx + 4);
    let word_offset = relative >> 2u;
    let shift = (relative & 3u) * 8u;
    let base = row * ROW_WORDS + word_offset;
    let lower = (*samples)[base];
    var packed = lower;
    if shift != 0u {
        let upper = (*samples)[base + 1u];
        packed = (lower >> shift) | (upper << (32u - shift));
    }
    return vec4<u32>(
        packed & 0xffu,
        (packed >> 8u) & 0xffu,
        (packed >> 16u) & 0xffu,
        (packed >> 24u) & 0xffu,
    );
}

fn select_rank_four(
    samples: ptr<function, array<u32, 27>>,
    rank: vec4<u32>,
) -> vec4<u32> {
    var low = vec4<u32>(0u);
    var high = vec4<u32>(255u);

    // Lower-bound search returns the first byte whose inclusive count exceeds
    // the zero-based rank, preserving exact behavior for duplicate samples.
    for (var search_step = 0u; search_step < 8u; search_step++) {
        let middle = (low + high) >> vec4<u32>(1u);
        var less_or_equal = vec4<u32>(0u);
        for (var dy = 0u; dy < 9u; dy++) {
            for (var dx = -4; dx <= 4; dx++) {
                let sample = four_window_samples(samples, dy, dx);
                less_or_equal += select(
                    vec4<u32>(0u),
                    vec4<u32>(1u),
                    sample <= middle,
                );
            }
        }
        let search_upper_half = less_or_equal > rank;
        low = select(middle + vec4<u32>(1u), low, search_upper_half);
        high = select(high, middle, search_upper_half);
    }
    return low;
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if params.width == 0u || params.height == 0u || gid.x >= (params.width / 4u) * params.height {
        return;
    }

    let words_per_row = params.width / 4u;
    let row = gid.x / words_per_row;
    let word_x = gid.x % words_per_row;
    var samples: array<u32, 27>;

    for (var dy = -4; dy <= 4; dy++) {
        let source_y = u32(clamp(i32(row) + dy, 0, i32(params.height) - 1));
        let source_words = source_y * words_per_row;
        let center = input[source_words + word_x];
        let sample_row = u32(dy + 4) * ROW_WORDS;
        samples[sample_row + 1u] = center;
        if word_x == 0u {
            samples[sample_row] = repeated_byte(center);
        } else {
            samples[sample_row] = input[source_words + word_x - 1u];
        }
        if word_x + 1u == words_per_row {
            samples[sample_row + 2u] = repeated_byte(center >> 24u);
        } else {
            samples[sample_row + 2u] = input[source_words + word_x + 1u];
        }
    }

    let selected = select_rank_four(
        &samples,
        vec4<u32>(min(params.rank, WINDOW - 1u)),
    );
    output[gid.x] = (selected.x & 0xffu)
        | ((selected.y & 0xffu) << 8u)
        | ((selected.z & 0xffu) << 16u)
        | ((selected.w & 0xffu) << 24u);
}
