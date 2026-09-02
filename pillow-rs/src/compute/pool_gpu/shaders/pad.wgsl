// Final placement pass for ImageOps.pad.
//
// The GPU planner performs Pillow's contain resize first with the exact
// fixed-point resize kernels. This pass only fills the requested canvas and
// copies the resized pixels at the scalar, ties-to-even offset chosen by the
// host. Keeping this separate from resize preserves the two-pass filter
// contract without a host readback or a CPU-side materialization.
//
// Packed transport: byte 0 = first sample, byte 1/2 = RGB samples, byte 3 =
// alpha/K/padding as selected by the logical mode.

struct Params {
    width: u32,          // original source width (unused by this pass)
    height: u32,         // original source height (unused by this pass)
    mode: u32,           // 0=L/P/1, 1=LA/PA, 2=RGB, 3=RGBA, 4=CMYK, 6=RGBX
    _pad0: u32,
    resized_w: u32,
    resized_h: u32,
    channels: u32,       // logical channel count used by resize
    premultiply: u32,    // resize control word (unused by this pass)
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

fn normalize_pixel(pixel: u32, mode: u32) -> u32 {
    // F stores one little-endian f32 sample per four-byte word.  Keep the
    // scalar representation opaque through both the resized image and the
    // pad fill; interpreting it as RGBA bytes would change the public value.
    if mode == 8u {
        return pixel;
    }
    let r = pixel & 0xffu;
    let g = (pixel >> 8u) & 0xffu;
    let b = (pixel >> 16u) & 0xffu;
    let a = (pixel >> 24u) & 0xffu;
    let has_rgb = mode == 2u || mode == 3u || mode == 4u || mode == 6u;
    let has_stored_fourth = mode == 3u || mode == 4u || mode == 6u;
    let out_g = select(0u, g, has_rgb);
    let out_b = select(0u, b, has_rgb);
    let out_a = select(255u, a, has_stored_fourth || mode == 1u);
    return r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.dst_w || gid.y >= params.dst_h {
        return;
    }

    let in_x = select(0u, gid.x - params.offset_x, gid.x >= params.offset_x);
    let in_y = select(0u, gid.y - params.offset_y, gid.y >= params.offset_y);
    let inside = gid.x >= params.offset_x
        && gid.y >= params.offset_y
        && in_x < params.resized_w
        && in_y < params.resized_h;
    let index = gid.y * params.dst_w + gid.x;
    if inside {
        output[index] = normalize_pixel(
            input[in_y * params.resized_w + in_x],
            params.mode,
        );
    } else {
        output[index] = normalize_pixel(params.fill, params.mode);
    }
}
