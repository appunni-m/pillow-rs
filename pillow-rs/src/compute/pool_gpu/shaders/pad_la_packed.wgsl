// Identity-contain ImageOps.pad for native LA samples.
// Each u32 owns two complete [L, A] pixels. The flattened pixel mapping lets
// a word cross a row boundary safely, including odd-width output rows.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad0: u32,
    resized_w: u32,
    resized_h: u32,
    channels: u32,
    premultiply: u32,
    dst_w: u32,
    dst_h: u32,
    fill: u32,
    offset_x: u32,
    offset_y: u32,
    _pad1: u32,
    _pad2: u32,
    _pad3: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn la_pixel(x: u32, y: u32, fill_pixel: u32) -> u32 {
    if x >= params.offset_x
        && y >= params.offset_y
        && x - params.offset_x < params.resized_w
        && y - params.offset_y < params.resized_h
    {
        let source_pixel = (y - params.offset_y) * params.resized_w
            + (x - params.offset_x);
        let source_word = input[source_pixel / 2u];
        return (source_word >> ((source_pixel % 2u) * 16u)) & 0xffffu;
    }
    return fill_pixel;
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let output_word = gid.x;
    let pixel_count = params.dst_w * params.dst_h;
    let first_pixel = output_word * 2u;
    if first_pixel >= pixel_count {
        return;
    }

    let fill_pixel = (params.fill & 0xffu) | (((params.fill >> 24u) & 0xffu) << 8u);
    let first_x = first_pixel % params.dst_w;
    let first_y = first_pixel / params.dst_w;
    let first = la_pixel(first_x, first_y, fill_pixel);
    var packed = first;
    if first_pixel + 1u < pixel_count {
        let crosses_row = first_x + 1u >= params.dst_w;
        let second_x = select(first_x + 1u, 0u, crosses_row);
        let second_y = first_y + select(0u, 1u, crosses_row);
        let second = la_pixel(second_x, second_y, fill_pixel);
        packed |= second << 16u;
    }
    output[output_word] = packed;
}
