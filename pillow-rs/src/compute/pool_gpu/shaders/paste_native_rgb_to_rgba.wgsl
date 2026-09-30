// Exact unmasked RGB-source to RGBA-destination Paste. Every invocation owns
// one aligned destination pixel, while source channel reads handle packed RGB
// bytes that cross u32 boundaries.

struct Params {
    width: u32,
    height: u32,
    source_width: u32,
    source_height: u32,
    destination_pixels: u32,
    workgroups_x: u32,
    paste_x: i32,
    paste_y: i32,
    padding_0: u32,
    padding_1: u32,
    padding_2: u32,
    padding_3: u32,
}

@group(0) @binding(0) var<storage, read> input_destination: array<u32>;
@group(0) @binding(1) var<storage, read> input_source: array<u32>;
@group(0) @binding(2) var<storage, read_write> output: array<u32>;
@group(0) @binding(3) var<uniform> params: Params;

fn read_source(byte_index: u32) -> u32 {
    let word = input_source[byte_index / 4u];
    return (word >> ((byte_index & 3u) * 8u)) & 0xffu;
}

@compute @workgroup_size(64)
fn main(
    @builtin(workgroup_id) workgroup: vec3<u32>,
    @builtin(local_invocation_id) local: vec3<u32>,
) {
    let linear_workgroup = workgroup.y * params.workgroups_x + workgroup.x;
    let pixel_index = linear_workgroup * 64u + local.x;
    if pixel_index >= params.destination_pixels {
        return;
    }

    let destination_x = pixel_index % params.width;
    let destination_y = pixel_index / params.width;
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

    var pixel = input_destination[pixel_index];
    if inside_x
        && inside_y
        && source_x < params.source_width
        && source_y < params.source_height
    {
        let source_pixel = source_y * params.source_width + source_x;
        let source_byte = source_pixel * 3u;
        let red = read_source(source_byte);
        let green = read_source(source_byte + 1u);
        let blue = read_source(source_byte + 2u);
        pixel = red | (green << 8u) | (blue << 16u) | 0xff000000u;
    }
    output[pixel_index] = pixel;
}
