// RemapPalette: remap palette indices via a 256-entry inverse lookup table.
// For P-mode images (stored as Luma8 indices): output[px] = lut[input[px]]
// For L-mode images: same — each luma value remapped through LUT
// For RGB images: each channel remapped independently through LUT
// Mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA
// Packed u32 RGBA: byte0=R, byte1=G, byte2=B, byte3=A

struct Params {
    width: u32,
    height: u32,
    mode: u32,     // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
}

// ── Mode helpers ──

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;
@group(0) @binding(3) var<storage, read> lut: array<u32>; // 256-entry LUT

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let idx = gid.y * params.width + gid.x;
    let pixel = input[idx];

    let r = pixel & 0xffu;
    let g = (pixel >> 8u) & 0xffu;
    let b = (pixel >> 16u) & 0xffu;
    let a = (pixel >> 24u) & 0xffu;

    // Remap each channel through LUT (clamped to 0-255)
    let new_r = lut[min(r, 255u)] & 0xffu;
    let new_g = select(0u, lut[min(g, 255u)] & 0xffu, mode_has_g(params.mode));
    let new_b = select(0u, lut[min(b, 255u)] & 0xffu, mode_has_b(params.mode));
    let new_a = select(255u, a, mode_has_a(params.mode));

    output[idx] = new_r | (new_g << 8u) | (new_b << 16u) | (new_a << 24u);
}
