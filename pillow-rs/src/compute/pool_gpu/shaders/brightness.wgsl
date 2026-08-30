// Brightness: clamp(ch * factor_int / 1000, 0, 255)
// Mode-aware: preserves alpha for LA/RGBA and processes all four bytes for
// explicit CMYK (mode 4, where byte 3 is K rather than alpha).
// Mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA, 4=CMYK
// Packed u32 RGBA: byte0=R, byte1=G, byte2=B, byte3=A

struct Params {
    width: u32,
    height: u32,
    mode: u32,    // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    factor_int: u32,
}

// ── Mode helpers ──

fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }
fn mode_is_cmyk(m: u32) -> bool { return m == 4u; }

fn brightness_apply(c: u32, f: u32) -> u32 {
    return min((c * f) / 1000u, 255u);
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

    let f = params.factor_int;
    let val_r = brightness_apply(r, f);
    let val_g = brightness_apply(g, f);
    let val_b = brightness_apply(b, f);

    // The host transport expands L/LA into equal RGB bytes before calling the
    // CPU implementation. Process all three transport channels so restoring
    // L/LA with preserve_mode sees the same luma instead of a weighted mix of
    // one enhanced channel and two original channels.
    let out_r = val_r;
    let out_g = val_g;
    let out_b = val_b;
    let out_a = select(
        select(255u, a, mode_has_a(params.mode)),
        brightness_apply(a, f),
        mode_is_cmyk(params.mode),
    );

    output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}
