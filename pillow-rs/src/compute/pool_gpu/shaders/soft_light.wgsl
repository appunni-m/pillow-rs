// SoftLight: Two-term formula, NO clamping (CHOP2 behavior)
// term1 = ((255-a)*a*b)/65536
// term2 = (a*(255-(255-a)*(255-b)/255))/255
// out = term1 + term2  (no clamping)
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
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u || m == 4u; }

fn softlight_ch(a: u32, b: u32) -> u32 {
    let term1 = ((255u - a) * a * b) / 65536u;
    let term2 = (a * (255u - ((255u - a) * (255u - b) / 255u))) / 255u;
    return term1 + term2;
}

@group(0) @binding(0) var<storage, read> input_a: array<u32>;
@group(0) @binding(1) var<storage, read> input_b: array<u32>;
@group(0) @binding(2) var<storage, read_write> output: array<u32>;
@group(0) @binding(3) var<uniform> params: Params;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let idx = gid.y * params.width + gid.x;

    let pa = input_a[idx];
    let pb = input_b[idx];
    let ar = pa & 0xffu;
    let ag = (pa >> 8u) & 0xffu;
    let ab = (pa >> 16u) & 0xffu;
    let aa = (pa >> 24u) & 0xffu;
    let br = pb & 0xffu;
    let bg = (pb >> 8u) & 0xffu;
    let bb = (pb >> 16u) & 0xffu;

    let out_r = softlight_ch(ar, br);
    let out_g_raw = softlight_ch(ag, bg);
    let out_b_raw = softlight_ch(ab, bb);

    let out_g = select(ag, out_g_raw, mode_has_g(params.mode));
    let out_b = select(ab, out_b_raw, mode_has_b(params.mode));
    let ba = (pb >> 24u) & 0xffu;
    let out_a = select(255u, softlight_ch(aa, ba), mode_has_a(params.mode));

    output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}
