// Grayscale: Pillow's rounded fixed-point BT.601 luma.
// luma = (19595*r + 38470*g + 7471*b + 32768) >> 16
// Mode-aware: only processes channels present in the image mode.
// Mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA
// Packed u32 RGBA: byte0=R, byte1=G, byte2=B, byte3=A

struct Params {
    width: u32,
    height: u32,
    mode: u32,    // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
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

    // Pillow's fixed-point BT.601 luma: always computed from all 3 channels.
    // For L mode, r=g=b=luma so the result is correct (but still compute for safety).
    let luma = (19595u * r + 38470u * g + 7471u * b + 32768u) >> 16u;
    let luma_clamped = min(luma, 255u);

    // R channel always gets luma (present in all modes).
    // G and B get luma only in RGB/RGBA modes; in L/LA they stay as-is (typically 0).
    let out_r = luma_clamped;
    let out_g = select(g, luma_clamped, mode_has_g(params.mode));
    let out_b = select(b, luma_clamped, mode_has_b(params.mode));
    let out_a = select(255u, a, mode_has_a(params.mode));

    output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}
