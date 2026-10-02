// Identity-contain HSV Pad keeps the three stored HSV bytes native. Four
// pixels own exactly three packed output words; row-tiled dispatch requires
// the pixel width to be divisible by four so no word crosses a row boundary.

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

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pixel_group = gid.x;
    let y = gid.y;
    if y >= params.dst_h || pixel_group >= params.dst_w / 4u {
        return;
    }

    // Each row contains dst_w * 3 bytes and dst_w is divisible by four, so
    // both output rows and four-pixel groups start at a u32 boundary.
    let output_word = y * (params.dst_w * 3u / 4u) + pixel_group * 3u;
    if y >= params.offset_y && y - params.offset_y < params.resized_h {
        let source_y = y - params.offset_y;
        let source_word = source_y * (params.width * 3u / 4u) + pixel_group * 3u;
        output[output_word] = input[source_word];
        output[output_word + 1u] = input[source_word + 1u];
        output[output_word + 2u] = input[source_word + 2u];
        return;
    }

    let red = params.fill & 0xffu;
    let green = (params.fill >> 8u) & 0xffu;
    let blue = (params.fill >> 16u) & 0xffu;
    output[output_word] = red | (green << 8u) | (blue << 16u) | (red << 24u);
    output[output_word + 1u] = green | (blue << 8u) | (red << 16u) | (green << 24u);
    output[output_word + 2u] = blue | (red << 8u) | (green << 16u) | (blue << 24u);
}
