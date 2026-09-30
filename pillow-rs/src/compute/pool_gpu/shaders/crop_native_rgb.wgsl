// Crop native packed RGB bytes without expanding pixels to the generic RGBA
// transport. Each invocation owns four output pixels and their three words.
struct Params {
    source_width: u32,
    source_height: u32,
    left: u32,
    top: u32,
    output_width: u32,
    output_height: u32,
    source_transfer_bytes: u32,
    output_word_count: u32,
    output_pixel_groups: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn crop_rgb_pixel(output_pixel: u32) -> vec3<u32> {
    let output_pixel_count = params.output_width * params.output_height;
    if output_pixel >= output_pixel_count
        || params.left >= params.source_width
        || params.top >= params.source_height
        || params.output_width == 0u
    {
        return vec3<u32>(0u);
    }

    let output_y = output_pixel / params.output_width;
    if output_y >= params.output_height || output_y >= params.source_height - params.top {
        return vec3<u32>(0u);
    }
    let output_x = output_pixel % params.output_width;
    if output_x >= params.source_width - params.left {
        return vec3<u32>(0u);
    }

    let source_pixel = (params.top + output_y) * params.source_width + params.left + output_x;
    let source_byte = source_pixel * 3u;
    if source_byte >= params.source_transfer_bytes {
        return vec3<u32>(0u);
    }
    let word_index = source_byte / 4u;
    let byte_shift = (source_byte % 4u) * 8u;
    var packed = input[word_index] >> byte_shift;
    if byte_shift > 8u {
        packed |= input[word_index + 1u] << (32u - byte_shift);
    }
    return vec3<u32>(packed & 0xffu, (packed >> 8u) & 0xffu, (packed >> 16u) & 0xffu);
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.output_pixel_groups {
        return;
    }
    let first_pixel = gid.x * 4u;
    let p0 = crop_rgb_pixel(first_pixel);
    let p1 = crop_rgb_pixel(first_pixel + 1u);
    let p2 = crop_rgb_pixel(first_pixel + 2u);
    let p3 = crop_rgb_pixel(first_pixel + 3u);
    let first_word = gid.x * 3u;

    if first_word < params.output_word_count {
        output[first_word] = p0.x | (p0.y << 8u) | (p0.z << 16u) | (p1.x << 24u);
    }
    if first_word + 1u < params.output_word_count {
        output[first_word + 1u] = p1.y | (p1.z << 8u) | (p2.x << 16u) | (p2.y << 24u);
    }
    if first_word + 2u < params.output_word_count {
        output[first_word + 2u] = p2.z | (p3.x << 8u) | (p3.y << 16u) | (p3.z << 24u);
    }
}
