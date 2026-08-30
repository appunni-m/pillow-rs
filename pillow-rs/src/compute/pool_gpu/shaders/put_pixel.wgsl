// Put pixel: set a single pixel at (x, y) to a given color.
// Per-pixel dispatch (16x16 workgroups). Only the thread at (params.x, params.y)
// writes the new color; all other threads passthrough.
// Mode-aware: only writes channels present in the image mode.
//   For L mode: writes byte 0 (R) from the low byte of the color.
//   For LA mode: writes bytes 0 and 3 (R and A).
//   For RGB mode: writes bytes 0, 1, 2 (R, G, B).
//   For RGBA/CMYK mode: writes all four bytes.
// Mode codes: 0=L/P, 1=LA/PA, 2=RGB, 3=RGBA, 4=CMYK, 6=RGBX
// Packed u32 RGBA: byte0=R, byte1=G, byte2=B, byte3=A
// color is packed RGBA: byte0=R, byte1=G, byte2=B, byte3=A

struct Params {
    width: u32,
    height: u32,
    mode: u32,    // 0=L/P, 1=LA/PA, 2=RGB, 3=RGBA, 4=CMYK
    _pad: u32,
    x: u32,       // target x coordinate
    y: u32,       // target y coordinate
    color: u32,   // new pixel color (packed RGBA)
}

// ── Mode helpers ──

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u || m == 4u || m == 6u || m == 7u || m == 8u; }

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

    let is_target = (gid.x == params.x && gid.y == params.y);

    // Extract new color channels
    let cr = params.color & 0xffu;
    let cg = (params.color >> 8u) & 0xffu;
    let cb = (params.color >> 16u) & 0xffu;
    let ca = (params.color >> 24u) & 0xffu;

    // For the target pixel: write the new color channels that exist in this mode.
    // Non-target pixels: passthrough (with mode-aware G/B/A cleanup).
    let out_r = select(r, cr, is_target);
    let out_g = select(g, cg, is_target && mode_has_g(params.mode));
    let out_b = select(b, cb, is_target && mode_has_b(params.mode));
    let out_a = select(a, ca, is_target && mode_has_a(params.mode));

    output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}
