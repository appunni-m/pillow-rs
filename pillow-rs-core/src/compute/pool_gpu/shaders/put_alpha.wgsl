// Put alpha: set the alpha channel to a uniform value for all pixels.
// For non-alpha modes (L, RGB): alpha is already 255 in internal representation,
// so output is passthrough (alpha channel not present in these modes).
// For alpha modes (LA, RGBA): set byte 3 (alpha) to params.alpha.
// Mode-aware: only affects alpha if mode has an alpha channel.
// Mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA
// Packed u32 RGBA: byte0=R, byte1=G, byte2=B, byte3=A
// Per-pixel dispatch (16x16 workgroups).

struct Params {
    width: u32,
    height: u32,
    mode: u32,    // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    alpha: u32,   // alpha value 0-255
}

// ── Mode helpers ──

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

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

    // RGB channels passthrough unchanged.
    // Alpha: for modes that have alpha, set to params.alpha; for non-alpha modes, 255.
    let out_r = r;
    let out_g = select(0u, g, mode_has_g(params.mode));
    let out_b = select(0u, b, mode_has_b(params.mode));
    let out_a = select(255u, params.alpha, mode_has_a(params.mode));

    output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}
