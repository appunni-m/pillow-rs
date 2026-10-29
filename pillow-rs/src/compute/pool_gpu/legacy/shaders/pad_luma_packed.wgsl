// Identity-contain Pad for native L images. The source and destination stay
// four grayscale samples per storage word, including when a packed word
// crosses a row boundary.

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

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let output_word = gid.x;
    let pixel_count = params.dst_w * params.dst_h;
    let first_pixel = output_word * 4u;
    if first_pixel >= pixel_count {
        return;
    }

    var packed = 0u;
    for (var lane = 0u; lane < 4u; lane += 1u) {
        let pixel = first_pixel + lane;
        if pixel < pixel_count {
            let x = pixel % params.dst_w;
            let y = pixel / params.dst_w;
            var sample = params.fill & 0xffu;
            if x >= params.offset_x
                && y >= params.offset_y
                && x - params.offset_x < params.resized_w
                && y - params.offset_y < params.resized_h
            {
                let source_pixel = (y - params.offset_y) * params.resized_w
                    + (x - params.offset_x);
                let source_word = input[source_pixel / 4u];
                sample = (source_word >> ((source_pixel % 4u) * 8u)) & 0xffu;
            }
            packed |= sample << (lane * 8u);
        }
    }
    output[output_word] = packed;
}
