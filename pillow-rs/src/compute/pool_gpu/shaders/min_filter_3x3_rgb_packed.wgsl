// Exact per-channel 3x3 minimum for native RGB triples. Each invocation
// evaluates four adjacent output pixels and owns the three u32 words that
// contain their twelve bytes.

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

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn native_rgb_pixel(pixel_index: u32) -> vec3<u32> {
    // The final compact upload is rounded to a complete word and zero-padded.
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

fn select_min(a: RgbSamples, b: RgbSamples) -> RgbSamples {
    return RgbSamples(min(a.red, b.red), min(a.green, b.green), min(a.blue, b.blue));
}

fn horizontal_minimum_four(row: u32, first_x: u32) -> RgbSamples {
    // Row-tiled dispatches assign four neighboring output pixels to each
    // invocation. Their 3x3 windows share most source pixels, so load the six
    // distinct columns once and reuse adjacent pairs for all four outputs.
    let last_x = params.width - 1u;
    let x0 = select(first_x, first_x - 1u, first_x > 0u);
    let x1 = first_x;
    let x2 = min(first_x + 1u, last_x);
    let x3 = min(first_x + 2u, last_x);
    let x4 = min(first_x + 3u, last_x);
    let x5 = min(first_x + 4u, last_x);
    let row_start = row * params.width;
    let p0 = native_rgb_pixel(row_start + x0);
    let p1 = native_rgb_pixel(row_start + x1);
    let p2 = native_rgb_pixel(row_start + x2);
    let p3 = native_rgb_pixel(row_start + x3);
    let p4 = native_rgb_pixel(row_start + x4);
    let p5 = native_rgb_pixel(row_start + x5);

    let min12 = min(p1, p2);
    let min34 = min(p3, p4);
    let out0 = min(p0, min12);
    let out1 = min(min12, p3);
    let out2 = min(p2, min34);
    let out3 = min(min34, p5);
    return RgbSamples(
        vec4<u32>(out0.x, out1.x, out2.x, out3.x),
        vec4<u32>(out0.y, out1.y, out2.y, out3.y),
        vec4<u32>(out0.z, out1.z, out2.z, out3.z),
    );
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
    var minimum: RgbSamples;
    if (params.flags & 16u) != 0u {
        let x = first_pixel % params.width;
        let y = first_pixel / params.width;
        let top_y = select(y, y - 1u, y > 0u);
        let bottom_y = min(y + 1u, params.height - 1u);
        minimum = select_min(
            select_min(
                horizontal_minimum_four(top_y, x),
                horizontal_minimum_four(y, x),
            ),
            horizontal_minimum_four(bottom_y, x),
        );
    } else {
        minimum = sample_four(pixels, -1, -1);
        minimum = select_min(minimum, sample_four(pixels, 0, -1));
        minimum = select_min(minimum, sample_four(pixels, 1, -1));
        minimum = select_min(minimum, sample_four(pixels, -1, 0));
        minimum = select_min(minimum, sample_four(pixels, 0, 0));
        minimum = select_min(minimum, sample_four(pixels, 1, 0));
        minimum = select_min(minimum, sample_four(pixels, -1, 1));
        minimum = select_min(minimum, sample_four(pixels, 0, 1));
        minimum = select_min(minimum, sample_four(pixels, 1, 1));
    }

    let valid = vec4<bool>(
        first_pixel < pixel_count,
        first_pixel + 1u < pixel_count,
        first_pixel + 2u < pixel_count,
        first_pixel + 3u < pixel_count,
    );
    let red = select(vec4<u32>(0u), minimum.red, valid);
    let green = select(vec4<u32>(0u), minimum.green, valid);
    let blue = select(vec4<u32>(0u), minimum.blue, valid);

    // Four RGB pixels occupy exactly three packed output words. The final
    // partial group writes zero to unused lanes and never crosses the buffer.
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
