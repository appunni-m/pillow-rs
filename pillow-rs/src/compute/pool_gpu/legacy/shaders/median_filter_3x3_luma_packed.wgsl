// Exact 3x3 median for native L bytes. Each invocation filters four adjacent
// output pixels, using vec4 lanes for those pixels and one packed word for the
// result. The host admits only dimensions whose pixel count fits this shader's
// u32 indexing and checks the packed dispatch against the adapter limits.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
    size: u32,
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

fn sample_neighbor(pixel_index: u32, dx: i32, dy: i32) -> u32 {
    let x = pixel_index % params.width;
    let y = pixel_index / params.width;
    let source_x = clamp_offset(x, params.width, dx);
    let source_y = clamp_offset(y, params.height, dy);
    let source_index = source_y * params.width + source_x;
    let packed = input[source_index >> 2u];
    let shift = (source_index & 3u) * 8u;
    return (packed >> shift) & 0xffu;
}

fn four_neighbors(pixels: vec4<u32>, dx: i32, dy: i32) -> vec4<u32> {
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

    let base_pixel = gid.x * 4u;
    let pixels = min(
        vec4<u32>(base_pixel, base_pixel + 1u, base_pixel + 2u, base_pixel + 3u),
        vec4<u32>(pixel_count - 1u),
    );
    var values: array<vec4<u32>, WINDOW>;
    var index = 0u;
    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            values[index] = four_neighbors(pixels, dx, dy);
            index += 1u;
        }
    }
    sort_nine_samples(&values);
    let median = values[4u];
    let valid = vec4<bool>(
        base_pixel < pixel_count,
        base_pixel + 1u < pixel_count,
        base_pixel + 2u < pixel_count,
        base_pixel + 3u < pixel_count,
    );
    let output_samples = select(vec4<u32>(0u), median, valid);
    output[gid.x] = (output_samples.x & 0xffu)
        | ((output_samples.y & 0xffu) << 8u)
        | ((output_samples.z & 0xffu) << 16u)
        | ((output_samples.w & 0xffu) << 24u);
}
