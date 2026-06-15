// Convert between color modes.
// Source mode is params.mode; target mode is params.target_mode.
// Mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA
// Packed u32 RGBA: byte0=R (luma/value for L/LA), byte1=G, byte2=B, byte3=A
//
// Conversion matrix (16 pairs, each producing correct RGBA output bytes):
//   L(0):   R=value, G=0, B=0, A=255
//   LA(1):  R=value, G=0, B=0, A=alpha
//   RGB(2): R=red, G=green, B=blue, A=255
//   RGBA(3):R=red, G=green, B=blue, A=alpha
//
// Per-pixel dispatch (16x16 workgroups). Each thread converts its own pixel.

struct Params {
    width: u32,
    height: u32,
    mode: u32,         // source mode
    _pad: u32,
    target_mode: u32,  // 0=L, 1=LA, 2=RGB, 3=RGBA
}

// ── Mode helpers (for target mode) ──

fn target_has_g(m: u32) -> bool { return m >= 2u; }
fn target_has_b(m: u32) -> bool { return m >= 2u; }
fn target_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

fn source_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

// BT.601 luma: (299*R + 587*G + 114*B + 500) / 1000
fn bt601_luma(r: u32, g: u32, b: u32) -> u32 {
    return (299u * r + 587u * g + 114u * b + 500u) / 1000u;
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let idx = gid.y * params.width + gid.x;
    let pixel = input[idx];

    // Always unpack all 4 bytes from the source pixel.
    // For L mode: only byte0 (R) carries the value; G,B are typically 0.
    // For LA mode: byte0=R(value), byte3=A; G,B are typically 0.
    let r = pixel & 0xffu;
    let g = (pixel >> 8u) & 0xffu;
    let b = (pixel >> 16u) & 0xffu;
    let a = (pixel >> 24u) & 0xffu;

    let src = params.mode;
    let dst = params.target_mode;

    var out_r = r;
    var out_g = g;
    var out_b = b;
    var out_a = a;

    // ── Branch on (src, dst) pair ──
    //
    // L (0) → anything
    if src == 0u {
        if dst == 0u { /* L→L: passthrough */ }
        else if dst == 1u { /* L→LA: R carries value, A=255 */ out_a = 255u; }
        else if dst == 2u { /* L→RGB: replicate R to G,B */ out_g = r; out_b = r; out_a = 255u; }
        else if dst == 3u { /* L→RGBA: replicate R to G,B, A=255 */ out_g = r; out_b = r; out_a = 255u; }
    }
    // LA (1) → anything
    else if src == 1u {
        if dst == 0u { /* LA→L: keep R (value), drop A */ out_a = 255u; }
        else if dst == 1u { /* LA→LA: passthrough */ }
        else if dst == 2u { /* LA→RGB: replicate R to G,B, drop A */ out_g = r; out_b = r; out_a = 255u; }
        else if dst == 3u { /* LA→RGBA: replicate R to G,B, keep A */ out_g = r; out_b = r; }
    }
    // RGB (2) → anything
    else if src == 2u {
        if dst == 0u { /* RGB→L: BT.601 luma to R */ let l = bt601_luma(r, g, b); out_r = l; out_g = l; out_b = l; out_a = 255u; }
        else if dst == 1u { /* RGB→LA: luma to R, drop G,B, A=255 */ let l = bt601_luma(r, g, b); out_r = l; out_g = l; out_b = l; out_a = 255u; }
        else if dst == 2u { /* RGB→RGB: passthrough */ }
        else if dst == 3u { /* RGB→RGBA: add A=255 */ out_a = 255u; }
    }
    // RGBA (3) → anything
    else if src == 3u {
        if dst == 0u { /* RGBA→L: luma to R, drop alpha */ let l = bt601_luma(r, g, b); out_r = l; out_g = l; out_b = l; out_a = 255u; }
        else if dst == 1u { /* RGBA→LA: luma to R, keep A */ let l = bt601_luma(r, g, b); out_r = l; out_g = l; out_b = l; }
        else if dst == 2u { /* RGBA→RGB: drop A */ out_a = 255u; }
        else if dst == 3u { /* RGBA→RGBA: passthrough */ }
    }

    // Apply target mode-awareness: channels not present in target mode get zero
    let final_r = out_r;
    let final_g = select(0u, out_g, target_has_g(dst));
    let final_b = select(0u, out_b, target_has_b(dst));
    let final_a = select(255u, out_a, target_has_a(dst));

    output[idx] = final_r | (final_g << 8u) | (final_b << 16u) | (final_a << 24u);
}
