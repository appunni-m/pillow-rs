// Exact-layout L-to-L masked Paste. Each invocation owns one packed output
// word containing up to four adjacent destination pixels.

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

fn read_mask(byte_index: u32) -> u32 {
    let word = input_source_mask[params.mask_word_offset + byte_index / 4u];
    return (word >> ((byte_index & 3u) * 8u)) & 0xffu;
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let word_index = gid.x;
    if word_index >= params.word_count {
        return;
    }

    var packed = 0u;
    for (var pixel_lane = 0u; pixel_lane < 4u; pixel_lane += 1u) {
        let destination_pixel_index = word_index * 4u + pixel_lane;
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

        let destination = read_destination(destination_pixel_index);
        var value = destination;
        if inside_x
            && inside_y
            && source_x < params.source_width
            && source_y < params.source_height
        {
            let source_pixel_index = source_y * params.source_width + source_x;
            let source = read_source(source_pixel_index);
            let mask = read_mask(source_pixel_index);
            // Pillow's ordinary byte BLEND rounds the weighted sum to nearest.
            value = (source * mask + destination * (255u - mask) + 127u) / 255u;
        }
        packed |= value << (pixel_lane * 8u);
    }
    output[word_index] = packed;
}
