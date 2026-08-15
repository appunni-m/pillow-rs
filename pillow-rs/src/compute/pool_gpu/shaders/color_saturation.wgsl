// Color saturation: luma-preserving blend
// luma = BT.601, then lerp(luma, ch, factor)
// Mode-aware: only meaningful for RGB/RGBA; for L/LA output = input.
// Mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA
// Packed u32 RGBA: byte0=R, byte1=G, byte2=B, byte3=A

struct Params {
    width: u32,
    height: u32,
    mode: u32,    // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    factor_int: u32,
}

// ── Mode helpers ──

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

fn lerp_fn(ch: u32, luma: u32, f: u32) -> u32 {
    // The public enhancement API permits factors above 1.0. Signed math
    // preserves the extrapolation without unsigned underflow; the host
    // safety bound keeps these products inside i32.
    let fi = i32(f);
    let value = i32(luma) * (1000i - fi) + i32(ch) * fi;
    return u32(clamp(value / 1000i, 0i, 255i));
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

    // Pillow's rounded fixed-point BT.601 luma. For L/LA the result is not
    // used because those modes are passed through below.
    let luma = (19595u * r + 38470u * g + 7471u * b + 32768u) >> 16u;

    let f = params.factor_int;
    let val_r = lerp_fn(r, luma, f);
    let val_g = lerp_fn(g, luma, f);
    let val_b = lerp_fn(b, luma, f);

    // For L/LA modes, output = input (pass through all channels unchanged)
    let out_r = select(r, val_r, mode_has_g(params.mode));
    let out_g = select(g, val_g, mode_has_g(params.mode));
    let out_b = select(b, val_b, mode_has_b(params.mode));
    let out_a = select(255u, a, mode_has_a(params.mode));

    output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}
