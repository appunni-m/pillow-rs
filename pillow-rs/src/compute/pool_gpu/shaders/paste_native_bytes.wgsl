// Exact-layout unmasked L/LA/RGB Paste. Each invocation owns one packed
// output word. RGB output bytes can cross pixel boundaries, so that branch
// maps each byte lane independently and still has exactly one writer.

struct Params {
    width: u32,
    height: u32,
    source_width: u32,
    source_height: u32,
    destination_pixels: u32,
    word_count: u32,
    bytes_per_pixel: u32,
    reserved: u32,
    paste_x: i32,
    paste_y: i32,
    padding_0: u32,
    padding_1: u32,
}

@group(0) @binding(0) var<storage, read> input_destination: array<u32>;
@group(0) @binding(1) var<storage, read> input_source: array<u32>;
@group(0) @binding(2) var<storage, read_write> output: array<u32>;
@group(0) @binding(3) var<uniform> params: Params;

fn read_destination(byte_index: u32) -> u32 {
    let word = input_destination[byte_index / 4u];
    return (word >> ((byte_index & 3u) * 8u)) & 0xffu;
}

fn read_source(byte_index: u32) -> u32 {
    let word = input_source[byte_index / 4u];
    return (word >> ((byte_index & 3u) * 8u)) & 0xffu;
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let word_index = gid.x;
    if word_index >= params.word_count {
        return;
    }

    if params.bytes_per_pixel == 3u {
        let destination_byte_count = params.destination_pixels * 3u;
        var packed = 0u;
        for (var byte_lane = 0u; byte_lane < 4u; byte_lane += 1u) {
            let destination_byte_index = word_index * 4u + byte_lane;
            if destination_byte_index >= destination_byte_count {
                continue;
            }

            let destination_pixel_index = destination_byte_index / 3u;
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

            var value = read_destination(destination_byte_index);
            if inside_x
                && inside_y
                && source_x < params.source_width
                && source_y < params.source_height
            {
                let source_pixel_index = source_y * params.source_width + source_x;
                let source_byte_index = source_pixel_index * 3u + destination_byte_index % 3u;
                value = read_source(source_byte_index);
            }
            packed |= value << (byte_lane * 8u);
        }
        output[word_index] = packed;
        return;
    }

    let pixels_per_word = 4u / params.bytes_per_pixel;
    var packed = 0u;
    for (var pixel_lane = 0u; pixel_lane < pixels_per_word; pixel_lane += 1u) {
        let destination_pixel_index = word_index * pixels_per_word + pixel_lane;
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

        for (var channel = 0u; channel < params.bytes_per_pixel; channel += 1u) {
            let destination_byte_index =
                destination_pixel_index * params.bytes_per_pixel + channel;
            var value = read_destination(destination_byte_index);
            if inside_x
                && inside_y
                && source_x < params.source_width
                && source_y < params.source_height
            {
                let source_pixel_index = source_y * params.source_width + source_x;
                let source_byte_index = source_pixel_index * params.bytes_per_pixel + channel;
                value = read_source(source_byte_index);
            }
            let shift = (pixel_lane * params.bytes_per_pixel + channel) * 8u;
            packed |= value << shift;
        }
    }
    output[word_index] = packed;
}
