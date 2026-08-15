// Fused ImageChops.multiply(other) -> ImageChops.screen(other).
//
// The multiply result is truncated before screen consumes it, matching the
// two public operations exactly while keeping the pair in one dispatch.
// Packed u32 RGBA: byte0=R, byte1=G, byte2=B, byte3=A.

struct Params {
    width: u32,
    height: u32,
    mode: u32,    // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
}

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

fn multiply_screen(a: u32, b: u32) -> u32 {
    let multiplied = (a * b) / 255u;
    return 255u - ((255u - multiplied) * (255u - b) / 255u);
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
    let ba = (pb >> 24u) & 0xffu;

    let out_r = multiply_screen(ar, br);
    let out_g_raw = multiply_screen(ag, bg);
    let out_b_raw = multiply_screen(ab, bb);
    let out_a_raw = multiply_screen(aa, ba);

    let out_g = select(ag, out_g_raw, mode_has_g(params.mode));
    let out_b = select(ab, out_b_raw, mode_has_b(params.mode));
    let out_a = select(255u, out_a_raw, mode_has_a(params.mode));
    output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}
