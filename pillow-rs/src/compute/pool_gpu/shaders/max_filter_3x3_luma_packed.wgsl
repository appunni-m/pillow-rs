// Exact 3x3 maximum for native L bytes. Each invocation computes four
// adjacent row-major pixels and owns their complete packed output word.

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

fn sample_neighbor(pixel_index: u32, dx: i32, dy: i32) -> u32 {
    let x = pixel_index % params.width;
    let y = pixel_index / params.width;
    let source_x = clamp_offset(x, params.width, dx);
    let source_y = clamp_offset(y, params.height, dy);
    let source_index = source_y * params.width + source_x;
    let packed = input[source_index >> 2u];
    return (packed >> ((source_index & 3u) * 8u)) & 0xffu;
}

fn sample_four(pixels: vec4<u32>, dx: i32, dy: i32) -> vec4<u32> {
    return vec4<u32>(
        sample_neighbor(pixels.x, dx, dy),
        sample_neighbor(pixels.y, dx, dy),
        sample_neighbor(pixels.z, dx, dy),
        sample_neighbor(pixels.w, dx, dy),
    );
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

    let first_pixel = gid.x * 4u;
    let pixels = min(
        vec4<u32>(first_pixel, first_pixel + 1u, first_pixel + 2u, first_pixel + 3u),
        vec4<u32>(pixel_count - 1u),
    );
    let top = max(
        max(sample_four(pixels, -1, -1), sample_four(pixels, 0, -1)),
        sample_four(pixels, 1, -1),
    );
    let middle = max(
        max(sample_four(pixels, -1, 0), sample_four(pixels, 0, 0)),
        sample_four(pixels, 1, 0),
    );
    let bottom = max(
        max(sample_four(pixels, -1, 1), sample_four(pixels, 0, 1)),
        sample_four(pixels, 1, 1),
    );
    let maximum = max(max(top, middle), bottom);
    let valid = vec4<bool>(
        first_pixel < pixel_count,
        first_pixel + 1u < pixel_count,
        first_pixel + 2u < pixel_count,
        first_pixel + 3u < pixel_count,
    );
    let output_samples = select(vec4<u32>(0u), maximum, valid);
    output[gid.x] = (output_samples.x & 0xffu)
        | ((output_samples.y & 0xffu) << 8u)
        | ((output_samples.z & 0xffu) << 16u)
        | ((output_samples.w & 0xffu) << 24u);
}
