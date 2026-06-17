// Uniform color quantization: map each active channel to quantized levels.
// quantized = (ch / step) * step + step/2
// Mode-aware: only processes channels present in the image mode.
// Mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA
// Packed u32 RGBA: byte0=R, byte1=G, byte2=B, byte3=A

struct Params {
    width: u32,
    height: u32,
    mode: u32,    // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    levels: u32,  // number of quantization levels per channel (ceil(cbrt(colors)))
    step: u32,    // quantization step (256 / levels)
}

// ── Mode helpers ──

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

// Quantize a single channel value to the nearest level center.
fn quantize(val: u32, step: u32) -> u32 {
    // For step=0 (levels=0 or no quantization), passthrough
    if step <= 1u { return val; }
    let q = (val / step) * step + step / 2u;
    return min(q, 255u);
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

    let s = params.step;

    // Quantize each active channel; non-active channels pass through.
    // R channel is always present (luma/value in all modes).
    let out_r = quantize(r, s);
    let out_g = select(g, quantize(g, s), mode_has_g(params.mode));
    let out_b = select(b, quantize(b, s), mode_has_b(params.mode));
    let out_a = select(255u, a, mode_has_a(params.mode));

    output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}
