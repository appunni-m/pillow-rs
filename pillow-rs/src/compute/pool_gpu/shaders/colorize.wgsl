// Colorize: lerp(black_color, white_color, luma/255)
// Param[0] = black_color packed as 0x00BBGGRR
// Param[1] = white_color packed as 0x00BBGGRR
// Mode-aware: for L/LA (only R carries luma), only process R;
// for RGB/RGBA, process all RGB channels.
// Mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA
// Packed u32 RGBA: byte0=R, byte1=G, byte2=B, byte3=A

struct Params {
    width: u32,
    height: u32,
    mode: u32,    // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    black_color: u32,
    white_color: u32,
}

// ── Mode helpers ──

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

fn colorize_lerp(black: u32, white: u32, luma: u32) -> u32 {
    // Use signed arithmetic so a valid inverted ramp (white < black) does
    // not underflow the unsigned expression before the final clamp.
    let val = i32(black) * 255i + (i32(white) - i32(black)) * i32(luma);
    return u32(clamp((val + 127i) / 255i, 0i, 255i));
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let idx = gid.y * params.width + gid.x;
    let pixel = input[idx];
    let r = pixel & 0xffu;
    let g = (pixel >> 8u) & 0xffu;
    let b = (pixel >> 16u) & 0xffu;
    let a = (pixel >> 24u) & 0xffu;

    // For L/LA: luma = r (single channel, only R carries luma)
    // For RGB/RGBA: luma = BT.601
    let luma = select(r, (299u * r + 587u * g + 114u * b) / 1000u, mode_has_g(params.mode));

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
    let val_r = colorize_lerp(br, wr, luma);
    let val_g = colorize_lerp(bg, wg, luma);
    let val_b = colorize_lerp(bb, wb, luma);

    // For L/LA: only process R (luma); G and B pass through unchanged
    let out_r = val_r;
    let out_g = select(g, val_g, mode_has_g(params.mode));
    let out_b = select(b, val_b, mode_has_b(params.mode));
    let out_a = select(255u, a, mode_has_a(params.mode));

    output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}
