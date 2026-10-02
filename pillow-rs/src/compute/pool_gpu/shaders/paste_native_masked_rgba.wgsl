// Native RGBA-to-RGBA masked Paste with a compact L mask. One invocation
// owns one aligned output pixel word, including all four stored channels.

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
    workgroups_x: u32,
    padding_2: u32,
    padding_3: u32,
}

@group(0) @binding(0) var<storage, read> input_destination: array<u32>;
@group(0) @binding(1) var<storage, read> input_source_mask: array<u32>;
@group(0) @binding(2) var<storage, read_write> output: array<u32>;
@group(0) @binding(3) var<uniform> params: Params;

fn read_mask(pixel_index: u32) -> u32 {
    let word = input_source_mask[params.mask_word_offset + pixel_index / 4u];
    return (word >> ((pixel_index & 3u) * 8u)) & 0xffu;
}

fn blend(source: u32, destination: u32, mask: u32) -> u32 {
    // Pillow byte BLEND rounds the weighted sum to nearest before /255.
    return (source * mask + destination * (255u - mask) + 127u) / 255u;
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    // The planner flattens a bounded 2D workgroup grid. The row stride is
    // groups_x * 64, and every flattened invocation has one unique word.
    let word_index = gid.x + gid.y * params.workgroups_x * 64u;
    if word_index >= params.word_count || word_index >= params.destination_pixels {
        return;
    }

    let destination_x = word_index % params.width;
    let destination_y = word_index / params.width;
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

    var value = input_destination[word_index];
    var has_source = inside_x && inside_y;
    if has_source {
        has_source = source_x < params.source_width && source_y < params.source_height;
    }
    if has_source {
        let source_pixel_index = source_y * params.source_width + source_x;
        let source_word = input_source_mask[source_pixel_index];
        let mask = read_mask(source_pixel_index);
        var packed = 0u;
        for (var channel = 0u; channel < 4u; channel += 1u) {
            let shift = channel * 8u;
            let source = (source_word >> shift) & 0xffu;
            let destination = (value >> shift) & 0xffu;
            packed |= blend(source, destination, mask) << shift;
        }
        value = packed;
    }
    output[word_index] = value;
}
