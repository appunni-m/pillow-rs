// LogicalXor: differing input truths => 255, otherwise zero.
// Mode-aware: only processes channels present in the image mode.
// Mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA, 9=four independent stored bytes
// Packed u32 RGBA: byte0=R, byte1=G, byte2=B, byte3=A

struct Params {
    width: u32,
    height: u32,
    mode: u32,    // 0=L, 1=LA, 2=RGB, 3=RGBA
    word_count: u32, // Active words for native byte mode; zero for pixel transport.
}

// ── Mode helpers ──

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u || m == 4u; }

fn truth_high_bits(word: u32) -> u32 {
    // The low-seven-bit sum cannot carry between bytes. Keep one high-bit
    // flag per sample until after XOR, then expand only the final flags.
    return (((word & 0x7f7f7f7fu) + 0x7f7f7f7fu) | word) & 0x80808080u;
}

@group(0) @binding(0) var<storage, read> input_a: array<u32>;
@group(0) @binding(1) var<storage, read> input_b: array<u32>;
@group(0) @binding(2) var<storage, read_write> output: array<u32>;
@group(0) @binding(3) var<uniform> params: Params;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let idx = gid.y * params.width + gid.x;
    if params.mode == 9u {
        if idx >= params.word_count { return; }
        let flags = truth_high_bits(input_a[idx]) ^ truth_high_bits(input_b[idx]);
        output[idx] = (flags >> 7u) * 255u;
        return;
    }

    let pa = input_a[idx];
    let pb = input_b[idx];
    let ar = pa & 0xffu;
    let ag = (pa >> 8u) & 0xffu;
    let ab = (pa >> 16u) & 0xffu;
    let aa = (pa >> 24u) & 0xffu;
    let br = pb & 0xffu;
    let bg = (pb >> 8u) & 0xffu;
    let bb = (pb >> 16u) & 0xffu;
    let ba = (pb >> 24u) & 0xffu;

    let out_r = select(0u, 255u, (ar != 0u) != (br != 0u));
    let out_g_raw = select(0u, 255u, (ag != 0u) != (bg != 0u));
    let out_b_raw = select(0u, 255u, (ab != 0u) != (bb != 0u));
    let out_a_raw = select(0u, 255u, (aa != 0u) != (ba != 0u));

    let out_g = select(ag, out_g_raw, mode_has_g(params.mode));
    let out_b = select(ab, out_b_raw, mode_has_b(params.mode));
    let out_a = select(255u, out_a_raw, mode_has_a(params.mode));

    output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}
