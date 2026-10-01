// Exact-layout RGB/HSV-to-same-mode masked Paste with an L mask. One invocation
// owns four three-byte pixels and emits their three aligned output words. Both
// modes blend the stored channels independently without RGBA staging.

struct Params {
    width: u32,
    height: u32,
    source_width: u32,
    source_height: u32,
    destination_pixels: u32,
    word_count: u32,
    paste_x: i32,
    paste_y: i32,
    mask_word_offset: u32,
    padding_1: u32,
    padding_2: u32,
    padding_3: u32,
}

@group(0) @binding(0) var<storage, read> input_destination: array<u32>;
@group(0) @binding(1) var<storage, read> input_source_mask: array<u32>;
@group(0) @binding(2) var<storage, read_write> output: array<u32>;
@group(0) @binding(3) var<uniform> params: Params;

fn read_destination(byte_index: u32) -> u32 {
    let word = input_destination[byte_index / 4u];
    return (word >> ((byte_index & 3u) * 8u)) & 0xffu;
}

fn read_source(byte_index: u32) -> u32 {
    let word = input_source_mask[byte_index / 4u];
    return (word >> ((byte_index & 3u) * 8u)) & 0xffu;
}

fn read_mask(pixel_index: u32) -> u32 {
    let word = input_source_mask[params.mask_word_offset + pixel_index / 4u];
    return (word >> ((pixel_index & 3u) * 8u)) & 0xffu;
}

fn blend(source: u32, destination: u32, mask: u32) -> u32 {
    return (source * mask + destination * (255u - mask) + 127u) / 255u;
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let first_pixel = gid.x * 4u;
    if first_pixel >= params.destination_pixels {
        return;
    }

    var packed_0 = 0u;
    var packed_1 = 0u;
    var packed_2 = 0u;
    for (var pixel_lane = 0u; pixel_lane < 4u; pixel_lane += 1u) {
        let destination_pixel_index = first_pixel + pixel_lane;
        if destination_pixel_index >= params.destination_pixels {
            continue;
        }

        let destination_x = destination_pixel_index % params.width;
        let destination_y = destination_pixel_index / params.width;
        var inside_x = true;
        var inside_y = true;
        var source_x = 0u;
        var source_y = 0u;
        if params.paste_x >= 0i {
            let origin_x = u32(params.paste_x);
            if destination_x < origin_x {
                inside_x = false;
            } else {
                source_x = destination_x - origin_x;
            }
        } else {
            let distance_x = 0u - bitcast<u32>(params.paste_x);
            if distance_x > 0xffffffffu - destination_x {
                inside_x = false;
            } else {
                source_x = destination_x + distance_x;
            }
        }
        if params.paste_y >= 0i {
            let origin_y = u32(params.paste_y);
            if destination_y < origin_y {
                inside_y = false;
            } else {
                source_y = destination_y - origin_y;
            }
        } else {
            let distance_y = 0u - bitcast<u32>(params.paste_y);
            if distance_y > 0xffffffffu - destination_y {
                inside_y = false;
            } else {
                source_y = destination_y + distance_y;
            }
        }

        var has_source = inside_x && inside_y;
        var source_pixel_index = 0u;
        if has_source {
            has_source = source_x < params.source_width && source_y < params.source_height;
            if has_source {
                source_pixel_index = source_y * params.source_width + source_x;
            }
        }
        var mask = 0u;
        if has_source {
            mask = read_mask(source_pixel_index);
        }

        for (var channel = 0u; channel < 3u; channel += 1u) {
            let destination_byte_index = destination_pixel_index * 3u + channel;
            var value = read_destination(destination_byte_index);
            if has_source {
                let source_byte_index = source_pixel_index * 3u + channel;
                value = blend(read_source(source_byte_index), value, mask);
            }

            let local_byte_index = pixel_lane * 3u + channel;
            let shift = (local_byte_index & 3u) * 8u;
            if local_byte_index < 4u {
                packed_0 |= value << shift;
            } else if local_byte_index < 8u {
                packed_1 |= value << shift;
            } else {
                packed_2 |= value << shift;
            }
        }
    }

    let first_output_word = gid.x * 3u;
    if first_output_word < params.word_count {
        output[first_output_word] = packed_0;
    }
    if first_output_word + 1u < params.word_count {
        output[first_output_word + 1u] = packed_1;
    }
    if first_output_word + 2u < params.word_count {
        output[first_output_word + 2u] = packed_2;
    }
}
