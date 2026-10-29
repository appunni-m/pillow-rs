// Exact native-byte masked Paste for L, RGB/HSV, and CMYK. A packed output
// word owns four destination bytes; each byte lane blends against the same L
// mask sample, while a one-pixel source may be broadcast across the region.

struct Params {
    width: u32,
    height: u32,
    source_width: u32,
    source_height: u32,
    mask_width: u32,
    mask_height: u32,
    destination_bytes: u32,
    word_count: u32,
    bytes_per_pixel: u32,
    paste_x: i32,
    paste_y: i32,
    source_is_broadcast: u32,
    padding_0: u32,
    padding_1: u32,
    padding_2: u32,
    padding_3: u32,
}

@group(0) @binding(0) var<storage, read> input_destination: array<u32>;
@group(0) @binding(1) var<storage, read> input_source: array<u32>;
@group(0) @binding(2) var<storage, read> input_mask: array<u32>;
@group(0) @binding(3) var<storage, read_write> output: array<u32>;
@group(0) @binding(4) var<uniform> params: Params;

fn read_destination(byte_index: u32) -> u32 {
    let word = input_destination[byte_index / 4u];
    return (word >> ((byte_index & 3u) * 8u)) & 0xffu;
}

fn read_source(byte_index: u32) -> u32 {
    let word = input_source[byte_index / 4u];
    return (word >> ((byte_index & 3u) * 8u)) & 0xffu;
}

fn read_mask(byte_index: u32) -> u32 {
    let word = input_mask[byte_index / 4u];
    return (word >> ((byte_index & 3u) * 8u)) & 0xffu;
}

fn paste_coordinate(destination: u32, origin: i32) -> vec2<u32> {
    if origin >= 0i {
        let start = u32(origin);
        return vec2<u32>(u32(destination >= start), destination - min(destination, start));
    }
    let distance = 0u - bitcast<u32>(origin);
    if distance > 0xffffffffu - destination {
        return vec2<u32>(0u, 0u);
    }
    return vec2<u32>(1u, destination + distance);
}

@compute @workgroup_size(64)
fn main(
    @builtin(workgroup_id) group: vec3<u32>,
    @builtin(num_workgroups) groups: vec3<u32>,
    @builtin(local_invocation_index) local: u32,
) {
    let word_index = (group.y * groups.x + group.x) * 64u + local;
    if word_index >= params.word_count {
        return;
    }
    var packed = 0u;
    for (var lane = 0u; lane < 4u; lane += 1u) {
        let destination_byte = word_index * 4u + lane;
        if destination_byte >= params.destination_bytes {
            continue;
        }
        let destination_pixel = destination_byte / params.bytes_per_pixel;
        let destination_x = destination_pixel % params.width;
        let destination_y = destination_pixel / params.width;
        let x = paste_coordinate(destination_x, params.paste_x);
        let y = paste_coordinate(destination_y, params.paste_y);
        var value = read_destination(destination_byte);
        if x.x != 0u
            && y.x != 0u
            && x.y < params.mask_width
            && y.y < params.mask_height
            && (params.source_is_broadcast != 0u
                || (x.y < params.source_width && y.y < params.source_height))
        {
            let region_index = y.y * params.mask_width + x.y;
            let source_pixel = select(
                region_index,
                0u,
                params.source_is_broadcast != 0u,
            );
            let source_byte = source_pixel * params.bytes_per_pixel
                + destination_byte % params.bytes_per_pixel;
            let source = read_source(source_byte);
            let mask = read_mask(region_index);
            value = (source * mask + value * (255u - mask) + 127u) / 255u;
        }
        packed |= value << (lane * 8u);
    }
    output[word_index] = packed;
}
