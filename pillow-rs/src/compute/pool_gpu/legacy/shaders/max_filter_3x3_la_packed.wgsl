// Exact native-LA 3x3 maximum. Each invocation computes two interleaved
// pixels (L0, A0, L1, A1) and owns their complete packed output word.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
    size: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

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
    let top = max(
        max(sample_pair(pixels, -1, -1), sample_pair(pixels, 0, -1)),
        sample_pair(pixels, 1, -1),
    );
    let middle = max(
        max(sample_pair(pixels, -1, 0), sample_pair(pixels, 0, 0)),
        sample_pair(pixels, 1, 0),
    );
    let bottom = max(
        max(sample_pair(pixels, -1, 1), sample_pair(pixels, 0, 1)),
        sample_pair(pixels, 1, 1),
    );
    let maximum = max(max(top, middle), bottom);
    let second_pixel_valid = first_pixel + 1u < pixel_count;
    let valid = vec4<bool>(true, true, second_pixel_valid, second_pixel_valid);
    let output_samples = select(vec4<u32>(0u), maximum, valid);
    output[gid.x] = (output_samples.x & 0xffu)
        | ((output_samples.y & 0xffu) << 8u)
        | ((output_samples.z & 0xffu) << 16u)
        | ((output_samples.w & 0xffu) << 24u);
}
