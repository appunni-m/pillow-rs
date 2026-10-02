// Exact per-channel 3x3 median for native RGB triples. Each invocation sorts
// four adjacent pixels in three independent vec4 lanes, then owns the three
// u32 words containing their twelve output bytes.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    flags: u32,
    size: u32,
}

struct RgbSamples {
    red: vec4<u32>,
    green: vec4<u32>,
    blue: vec4<u32>,
}

struct RgbPair {
    lower: RgbSamples,
    upper: RgbSamples,
}

const WINDOW: u32 = 9u;

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn native_rgb_pixel(pixel_index: u32) -> vec3<u32> {
    // RGB samples can straddle u32 words. The host rounds the compact input
    // transfer up to a complete word and zero-fills its final padding bytes.
    let byte_offset = pixel_index * 3u;
    let word_index = byte_offset >> 2u;
    let shift = (byte_offset & 3u) * 8u;
    var packed = input[word_index] >> shift;
    if shift > 8u {
        packed |= input[word_index + 1u] << (32u - shift);
    }
    return vec3<u32>(packed & 0xffu, (packed >> 8u) & 0xffu, (packed >> 16u) & 0xffu);
}

fn clamp_offset(coordinate: vec4<u32>, extent: u32, delta: i32) -> vec4<u32> {
    if delta < 0 {
        return select(coordinate, coordinate - vec4<u32>(1u), coordinate > vec4<u32>(0u));
    }
    if delta > 0 {
        return select(
            coordinate,
            coordinate + vec4<u32>(1u),
            coordinate < vec4<u32>(extent - 1u),
        );
    }
    return coordinate;
}

fn sample_four(pixels: vec4<u32>, dx: i32, dy: i32) -> RgbSamples {
    let x = clamp_offset(pixels % vec4<u32>(params.width), params.width, dx);
    let y = clamp_offset(pixels / vec4<u32>(params.width), params.height, dy);
    let source_pixels = y * vec4<u32>(params.width) + x;
    let pixel0 = native_rgb_pixel(source_pixels.x);
    let pixel1 = native_rgb_pixel(source_pixels.y);
    let pixel2 = native_rgb_pixel(source_pixels.z);
    let pixel3 = native_rgb_pixel(source_pixels.w);
    return RgbSamples(
        vec4<u32>(pixel0.x, pixel1.x, pixel2.x, pixel3.x),
        vec4<u32>(pixel0.y, pixel1.y, pixel2.y, pixel3.y),
        vec4<u32>(pixel0.z, pixel1.z, pixel2.z, pixel3.z),
    );
}

fn compare_exchange(a: RgbSamples, b: RgbSamples) -> RgbPair {
    return RgbPair(
        RgbSamples(min(a.red, b.red), min(a.green, b.green), min(a.blue, b.blue)),
        RgbSamples(max(a.red, b.red), max(a.green, b.green), max(a.blue, b.blue)),
    );
}

fn select_lower(a: RgbSamples, b: RgbSamples) -> RgbSamples {
    return RgbSamples(min(a.red, b.red), min(a.green, b.green), min(a.blue, b.blue));
}

fn select_upper(a: RgbSamples, b: RgbSamples) -> RgbSamples {
    return RgbSamples(max(a.red, b.red), max(a.green, b.green), max(a.blue, b.blue));
}

fn sort_nine_samples(values: ptr<function, array<RgbSamples, WINDOW>>) {
    // This is the median-output slice of the existing nine-phase sorting
    // network. Backward liveness removes compares whose results cannot reach
    // lane four and computes only the lower or upper result when the other is
    // dead. The exact network was checked against full sorting for random
    // nine-byte inputs.
    var pair: RgbPair;
    pair = compare_exchange((*values)[0], (*values)[1]);
    (*values)[0] = pair.lower;
    (*values)[1] = pair.upper;
    pair = compare_exchange((*values)[2], (*values)[3]);
    (*values)[2] = pair.lower;
    (*values)[3] = pair.upper;
    pair = compare_exchange((*values)[4], (*values)[5]);
    (*values)[4] = pair.lower;
    (*values)[5] = pair.upper;
    pair = compare_exchange((*values)[6], (*values)[7]);
    (*values)[6] = pair.lower;
    (*values)[7] = pair.upper;

    pair = compare_exchange((*values)[1], (*values)[2]);
    (*values)[1] = pair.lower;
    (*values)[2] = pair.upper;
    pair = compare_exchange((*values)[3], (*values)[4]);
    (*values)[3] = pair.lower;
    (*values)[4] = pair.upper;
    pair = compare_exchange((*values)[5], (*values)[6]);
    (*values)[5] = pair.lower;
    (*values)[6] = pair.upper;
    pair = compare_exchange((*values)[7], (*values)[8]);
    (*values)[7] = pair.lower;
    (*values)[8] = pair.upper;

    pair = compare_exchange((*values)[0], (*values)[1]);
    (*values)[0] = pair.lower;
    (*values)[1] = pair.upper;
    pair = compare_exchange((*values)[2], (*values)[3]);
    (*values)[2] = pair.lower;
    (*values)[3] = pair.upper;
    pair = compare_exchange((*values)[4], (*values)[5]);
    (*values)[4] = pair.lower;
    (*values)[5] = pair.upper;
    pair = compare_exchange((*values)[6], (*values)[7]);
    (*values)[6] = pair.lower;
    (*values)[7] = pair.upper;

    pair = compare_exchange((*values)[1], (*values)[2]);
    (*values)[1] = pair.lower;
    (*values)[2] = pair.upper;
    pair = compare_exchange((*values)[3], (*values)[4]);
    (*values)[3] = pair.lower;
    (*values)[4] = pair.upper;
    pair = compare_exchange((*values)[5], (*values)[6]);
    (*values)[5] = pair.lower;
    (*values)[6] = pair.upper;
    pair = compare_exchange((*values)[7], (*values)[8]);
    (*values)[7] = pair.lower;
    (*values)[8] = pair.upper;

    (*values)[1] = select_upper((*values)[0], (*values)[1]);
    pair = compare_exchange((*values)[2], (*values)[3]);
    (*values)[2] = pair.lower;
    (*values)[3] = pair.upper;
    pair = compare_exchange((*values)[4], (*values)[5]);
    (*values)[4] = pair.lower;
    (*values)[5] = pair.upper;
    pair = compare_exchange((*values)[6], (*values)[7]);
    (*values)[6] = pair.lower;
    (*values)[7] = pair.upper;

    (*values)[2] = select_upper((*values)[1], (*values)[2]);
    pair = compare_exchange((*values)[3], (*values)[4]);
    (*values)[3] = pair.lower;
    (*values)[4] = pair.upper;
    pair = compare_exchange((*values)[5], (*values)[6]);
    (*values)[5] = pair.lower;
    (*values)[6] = pair.upper;
    (*values)[7] = select_lower((*values)[7], (*values)[8]);

    (*values)[3] = select_upper((*values)[2], (*values)[3]);
    pair = compare_exchange((*values)[4], (*values)[5]);
    (*values)[4] = pair.lower;
    (*values)[5] = pair.upper;
    (*values)[6] = select_lower((*values)[6], (*values)[7]);

    (*values)[4] = select_upper((*values)[3], (*values)[4]);
    (*values)[5] = select_lower((*values)[5], (*values)[6]);
    (*values)[4] = select_lower((*values)[4], (*values)[5]);
}

@compute @workgroup_size(16, 16)
fn main(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(num_workgroups) num_workgroups: vec3<u32>,
) {
    let pixel_count = params.width * params.height;
    if pixel_count == 0u {
        return;
    }
    let output_group_count = pixel_count / 4u + select(0u, 1u, (pixel_count & 3u) != 0u);
    var output_group = gid.x + (num_workgroups.x * 16u) * gid.y;
    if (params.flags & 16u) != 0u {
        let groups_per_row = params.width >> 2u;
        if gid.x >= groups_per_row || gid.y >= params.height {
            return;
        }
        output_group = gid.y * groups_per_row + gid.x;
    }
    if output_group >= output_group_count {
        return;
    }

    let first_pixel = output_group * 4u;
    let pixels = min(
        vec4<u32>(first_pixel, first_pixel + 1u, first_pixel + 2u, first_pixel + 3u),
        vec4<u32>(pixel_count - 1u),
    );
    var values: array<RgbSamples, WINDOW>;
    values[0] = sample_four(pixels, -1, -1);
    values[1] = sample_four(pixels, 0, -1);
    values[2] = sample_four(pixels, 1, -1);
    values[3] = sample_four(pixels, -1, 0);
    values[4] = sample_four(pixels, 0, 0);
    values[5] = sample_four(pixels, 1, 0);
    values[6] = sample_four(pixels, -1, 1);
    values[7] = sample_four(pixels, 0, 1);
    values[8] = sample_four(pixels, 1, 1);
    sort_nine_samples(&values);
    let median = values[4u];
    let valid = vec4<bool>(
        first_pixel < pixel_count,
        first_pixel + 1u < pixel_count,
        first_pixel + 2u < pixel_count,
        first_pixel + 3u < pixel_count,
    );
    let red = select(vec4<u32>(0u), median.red, valid);
    let green = select(vec4<u32>(0u), median.green, valid);
    let blue = select(vec4<u32>(0u), median.blue, valid);

    // Four pixels occupy three packed output words. Exclusive word ownership
    // also covers the final partial group without byte-store races.
    let output_byte_count = pixel_count * 3u;
    let output_word_count = output_byte_count / 4u
        + select(0u, 1u, (output_byte_count & 3u) != 0u);
    let first_word = output_group * 3u;
    if first_word < output_word_count {
        output[first_word] = red.x | (green.x << 8u) | (blue.x << 16u) | (red.y << 24u);
    }
    if first_word + 1u < output_word_count {
        output[first_word + 1u] = green.y | (blue.y << 8u) | (red.z << 16u) | (green.z << 24u);
    }
    if first_word + 2u < output_word_count {
        output[first_word + 2u] = blue.z | (red.w << 8u) | (green.w << 16u) | (blue.w << 24u);
    }
}
