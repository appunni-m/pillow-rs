// Colorize: lerp(black_color, white_color, luma/255)
// Param[0] = black_color packed as 0x00BBGGRR
// Param[1] = white_color packed as 0x00BBGGRR

struct Params {
    width: u32,
    height: u32,
    _pad0: u32,
    _pad1: u32,
    black_color: u32,
    white_color: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn colorize_lerp(black: u32, white: u32, luma: u32) -> u32 {
    let val = black * 255u + (white - black) * luma;
    return min((val + 127u) / 255u, 255u);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let idx = gid.y * params.width + gid.x;
    let pixel = input[idx];
    let r = pixel & 0xffu;
    let g = (pixel >> 8u) & 0xffu;
    let b = (pixel >> 16u) & 0xffu;

    // BT.601 luma
    let luma = (299u * r + 587u * g + 114u * b) / 1000u;

    // Extract black/white colors
    let bc = params.black_color;
    let wc = params.white_color;
    let br = bc & 0xffu;
    let bg = (bc >> 8u) & 0xffu;
    let bb = (bc >> 16u) & 0xffu;
    let wr = wc & 0xffu;
    let wg = (wc >> 8u) & 0xffu;
    let wb = (wc >> 16u) & 0xffu;

    // lerp: black + (white - black) * luma / 255
    let out_r = colorize_lerp(br, wr, luma);
    let out_g = colorize_lerp(bg, wg, luma);
    let out_b = colorize_lerp(bb, wb, luma);

    output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (255u << 24u);
}
