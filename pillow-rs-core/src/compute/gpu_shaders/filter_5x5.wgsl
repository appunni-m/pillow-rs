// 5x5 convolution filter — PIL-identical row-grouped f32 accumulation.
// Mode-aware: for L/LA (0/1) only convolves R channel (luma), preserves G/B/A.
// Params: 25 pre-divided kernel f32 values (as u32 bits), scale (f32 as u32),
//         offset (i32).
//
// CPU reference (image.rs:2670-2710):
//   kn[n] = kernel[n] / scale,  rounding_bias = offset + 0.5
//   Row accumulation ORDER (critical for float rounding):
//     ss = rounding_bias
//     ss += row0 (y+2, bottom-most)
//     ss += row1 (y+1)
//     ss += row2 (y, center)
//     ss += row3 (y-1)
//     ss += row4 (y-2, top-most)
//   clip8_filter: same as 3x3
//   Border: 2-pixel edge copied verbatim
//
// Kernel layout (PIL convention, bottom row first in memory):
//   k00..k04 = y+2 (bottom-most)  k10..k14 = y+1
//   k20..k24 = y    (center)      k30..k34 = y-1
//   k40..k44 = y-2 (top-most)

struct Params {
    width: u32,
    height: u32,
    mode: u32,    // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    // 25 kernel values, host pre-divides by scale
    k00: u32, k01: u32, k02: u32, k03: u32, k04: u32,
    k10: u32, k11: u32, k12: u32, k13: u32, k14: u32,
    k20: u32, k21: u32, k22: u32, k23: u32, k24: u32,
    k30: u32, k31: u32, k32: u32, k33: u32, k34: u32,
    k40: u32, k41: u32, k42: u32, k43: u32, k44: u32,
    offset_val: i32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

// PIL-identical clip8: truncating cast clamping to [0, 255]
fn clip8_filter(ss: f32) -> u32 {
    if ss <= 0.0 { return 0u; }
    if ss >= 255.0 { return 255u; }
    return u32(ss);
}

fn process_pixel(x: u32, y: u32) -> u32 {
    let w = params.width;
    let h = params.height;
    let idx = y * w + x;

    // Border: 2-pixel edge copied verbatim
    if x < 2u || x >= w - 2u || y < 2u || y >= h - 2u {
        return input[idx];
    }

    // Decode kernel values from Params
    let k00 = bitcast<f32>(params.k00); let k01 = bitcast<f32>(params.k01);
    let k02 = bitcast<f32>(params.k02); let k03 = bitcast<f32>(params.k03);
    let k04 = bitcast<f32>(params.k04);
    let k10 = bitcast<f32>(params.k10); let k11 = bitcast<f32>(params.k11);
    let k12 = bitcast<f32>(params.k12); let k13 = bitcast<f32>(params.k13);
    let k14 = bitcast<f32>(params.k14);
    let k20 = bitcast<f32>(params.k20); let k21 = bitcast<f32>(params.k21);
    let k22 = bitcast<f32>(params.k22); let k23 = bitcast<f32>(params.k23);
    let k24 = bitcast<f32>(params.k24);
    let k30 = bitcast<f32>(params.k30); let k31 = bitcast<f32>(params.k31);
    let k32 = bitcast<f32>(params.k32); let k33 = bitcast<f32>(params.k33);
    let k34 = bitcast<f32>(params.k34);
    let k40 = bitcast<f32>(params.k40); let k41 = bitcast<f32>(params.k41);
    let k42 = bitcast<f32>(params.k42); let k43 = bitcast<f32>(params.k43);
    let k44 = bitcast<f32>(params.k44);
    let bias = f32(params.offset_val) + 0.5;

    // Extract R,G,B,A from all 25 neighborhood pixels
    // Row -2 (top-most): dy=-2
    let p0_0 = input[(y - 2u) * w + (x - 2u)];
    let p0_1 = input[(y - 2u) * w + (x - 1u)];
    let p0_2 = input[(y - 2u) * w + x];
    let p0_3 = input[(y - 2u) * w + (x + 1u)];
    let p0_4 = input[(y - 2u) * w + (x + 2u)];
    // Row -1
    let p1_0 = input[(y - 1u) * w + (x - 2u)];
    let p1_1 = input[(y - 1u) * w + (x - 1u)];
    let p1_2 = input[(y - 1u) * w + x];
    let p1_3 = input[(y - 1u) * w + (x + 1u)];
    let p1_4 = input[(y - 1u) * w + (x + 2u)];
    // Row 0 (center)
    let p2_0 = input[y * w + (x - 2u)];
    let p2_1 = input[y * w + (x - 1u)];
    let p2_2 = input[y * w + x];
    let p2_3 = input[y * w + (x + 1u)];
    let p2_4 = input[y * w + (x + 2u)];
    // Row +1
    let p3_0 = input[(y + 1u) * w + (x - 2u)];
    let p3_1 = input[(y + 1u) * w + (x - 1u)];
    let p3_2 = input[(y + 1u) * w + x];
    let p3_3 = input[(y + 1u) * w + (x + 1u)];
    let p3_4 = input[(y + 1u) * w + (x + 2u)];
    // Row +2 (bottom-most)
    let p4_0 = input[(y + 2u) * w + (x - 2u)];
    let p4_1 = input[(y + 2u) * w + (x - 1u)];
    let p4_2 = input[(y + 2u) * w + x];
    let p4_3 = input[(y + 2u) * w + (x + 1u)];
    let p4_4 = input[(y + 2u) * w + (x + 2u)];

    // R channel (byte 0):
    let r0_0 = p0_0 & 0xffu; let r0_1 = p0_1 & 0xffu; let r0_2 = p0_2 & 0xffu;
    let r0_3 = p0_3 & 0xffu; let r0_4 = p0_4 & 0xffu;
    let r1_0 = p1_0 & 0xffu; let r1_1 = p1_1 & 0xffu; let r1_2 = p1_2 & 0xffu;
    let r1_3 = p1_3 & 0xffu; let r1_4 = p1_4 & 0xffu;
    let r2_0 = p2_0 & 0xffu; let r2_1 = p2_1 & 0xffu; let r2_2 = p2_2 & 0xffu;
    let r2_3 = p2_3 & 0xffu; let r2_4 = p2_4 & 0xffu;
    let r3_0 = p3_0 & 0xffu; let r3_1 = p3_1 & 0xffu; let r3_2 = p3_2 & 0xffu;
    let r3_3 = p3_3 & 0xffu; let r3_4 = p3_4 & 0xffu;
    let r4_0 = p4_0 & 0xffu; let r4_1 = p4_1 & 0xffu; let r4_2 = p4_2 & 0xffu;
    let r4_3 = p4_3 & 0xffu; let r4_4 = p4_4 & 0xffu;

    // G channel (byte 1):
    let g0_0 = (p0_0 >> 8u) & 0xffu; let g0_1 = (p0_1 >> 8u) & 0xffu;
    let g0_2 = (p0_2 >> 8u) & 0xffu; let g0_3 = (p0_3 >> 8u) & 0xffu;
    let g0_4 = (p0_4 >> 8u) & 0xffu;
    let g1_0 = (p1_0 >> 8u) & 0xffu; let g1_1 = (p1_1 >> 8u) & 0xffu;
    let g1_2 = (p1_2 >> 8u) & 0xffu; let g1_3 = (p1_3 >> 8u) & 0xffu;
    let g1_4 = (p1_4 >> 8u) & 0xffu;
    let g2_0 = (p2_0 >> 8u) & 0xffu; let g2_1 = (p2_1 >> 8u) & 0xffu;
    let g2_2 = (p2_2 >> 8u) & 0xffu; let g2_3 = (p2_3 >> 8u) & 0xffu;
    let g2_4 = (p2_4 >> 8u) & 0xffu;
    let g3_0 = (p3_0 >> 8u) & 0xffu; let g3_1 = (p3_1 >> 8u) & 0xffu;
    let g3_2 = (p3_2 >> 8u) & 0xffu; let g3_3 = (p3_3 >> 8u) & 0xffu;
    let g3_4 = (p3_4 >> 8u) & 0xffu;
    let g4_0 = (p4_0 >> 8u) & 0xffu; let g4_1 = (p4_1 >> 8u) & 0xffu;
    let g4_2 = (p4_2 >> 8u) & 0xffu; let g4_3 = (p4_3 >> 8u) & 0xffu;
    let g4_4 = (p4_4 >> 8u) & 0xffu;

    // B channel (byte 2):
    let b0_0 = (p0_0 >> 16u) & 0xffu; let b0_1 = (p0_1 >> 16u) & 0xffu;
    let b0_2 = (p0_2 >> 16u) & 0xffu; let b0_3 = (p0_3 >> 16u) & 0xffu;
    let b0_4 = (p0_4 >> 16u) & 0xffu;
    let b1_0 = (p1_0 >> 16u) & 0xffu; let b1_1 = (p1_1 >> 16u) & 0xffu;
    let b1_2 = (p1_2 >> 16u) & 0xffu; let b1_3 = (p1_3 >> 16u) & 0xffu;
    let b1_4 = (p1_4 >> 16u) & 0xffu;
    let b2_0 = (p2_0 >> 16u) & 0xffu; let b2_1 = (p2_1 >> 16u) & 0xffu;
    let b2_2 = (p2_2 >> 16u) & 0xffu; let b2_3 = (p2_3 >> 16u) & 0xffu;
    let b2_4 = (p2_4 >> 16u) & 0xffu;
    let b3_0 = (p3_0 >> 16u) & 0xffu; let b3_1 = (p3_1 >> 16u) & 0xffu;
    let b3_2 = (p3_2 >> 16u) & 0xffu; let b3_3 = (p3_3 >> 16u) & 0xffu;
    let b3_4 = (p3_4 >> 16u) & 0xffu;
    let b4_0 = (p4_0 >> 16u) & 0xffu; let b4_1 = (p4_1 >> 16u) & 0xffu;
    let b4_2 = (p4_2 >> 16u) & 0xffu; let b4_3 = (p4_3 >> 16u) & 0xffu;
    let b4_4 = (p4_4 >> 16u) & 0xffu;

    // A channel (byte 3):
    let a0_0 = (p0_0 >> 24u); let a0_1 = (p0_1 >> 24u); let a0_2 = (p0_2 >> 24u);
    let a0_3 = (p0_3 >> 24u); let a0_4 = (p0_4 >> 24u);
    let a1_0 = (p1_0 >> 24u); let a1_1 = (p1_1 >> 24u); let a1_2 = (p1_2 >> 24u);
    let a1_3 = (p1_3 >> 24u); let a1_4 = (p1_4 >> 24u);
    let a2_0 = (p2_0 >> 24u); let a2_1 = (p2_1 >> 24u); let a2_2 = (p2_2 >> 24u);
    let a2_3 = (p2_3 >> 24u); let a2_4 = (p2_4 >> 24u);
    let a3_0 = (p3_0 >> 24u); let a3_1 = (p3_1 >> 24u); let a3_2 = (p3_2 >> 24u);
    let a3_3 = (p3_3 >> 24u); let a3_4 = (p3_4 >> 24u);
    let a4_0 = (p4_0 >> 24u); let a4_1 = (p4_1 >> 24u); let a4_2 = (p4_2 >> 24u);
    let a4_3 = (p4_3 >> 24u); let a4_4 = (p4_4 >> 24u);

    // ── Row accumulation in PIL order (bottom to top) ──
    // Row +2 (bottom-most): dy=+2, kernel k40..k44
    let row0_r = f32(r4_0)*k40 + f32(r4_1)*k41 + f32(r4_2)*k42 + f32(r4_3)*k43 + f32(r4_4)*k44;
    let row0_g = f32(g4_0)*k40 + f32(g4_1)*k41 + f32(g4_2)*k42 + f32(g4_3)*k43 + f32(g4_4)*k44;
    let row0_b = f32(b4_0)*k40 + f32(b4_1)*k41 + f32(b4_2)*k42 + f32(b4_3)*k43 + f32(b4_4)*k44;
    let row0_a = f32(a4_0)*k40 + f32(a4_1)*k41 + f32(a4_2)*k42 + f32(a4_3)*k43 + f32(a4_4)*k44;

    // Row +1: dy=+1, kernel k30..k34
    let row1_r = f32(r3_0)*k30 + f32(r3_1)*k31 + f32(r3_2)*k32 + f32(r3_3)*k33 + f32(r3_4)*k34;
    let row1_g = f32(g3_0)*k30 + f32(g3_1)*k31 + f32(g3_2)*k32 + f32(g3_3)*k33 + f32(g3_4)*k34;
    let row1_b = f32(b3_0)*k30 + f32(b3_1)*k31 + f32(b3_2)*k32 + f32(b3_3)*k33 + f32(b3_4)*k34;
    let row1_a = f32(a3_0)*k30 + f32(a3_1)*k31 + f32(a3_2)*k32 + f32(a3_3)*k33 + f32(a3_4)*k34;

    // Row 0 (center): dy=0, kernel k20..k24
    let row2_r = f32(r2_0)*k20 + f32(r2_1)*k21 + f32(r2_2)*k22 + f32(r2_3)*k23 + f32(r2_4)*k24;
    let row2_g = f32(g2_0)*k20 + f32(g2_1)*k21 + f32(g2_2)*k22 + f32(g2_3)*k23 + f32(g2_4)*k24;
    let row2_b = f32(b2_0)*k20 + f32(b2_1)*k21 + f32(b2_2)*k22 + f32(b2_3)*k23 + f32(b2_4)*k24;
    let row2_a = f32(a2_0)*k20 + f32(a2_1)*k21 + f32(a2_2)*k22 + f32(a2_3)*k23 + f32(a2_4)*k24;

    // Row -1: dy=-1, kernel k10..k14
    let row3_r = f32(r1_0)*k10 + f32(r1_1)*k11 + f32(r1_2)*k12 + f32(r1_3)*k13 + f32(r1_4)*k14;
    let row3_g = f32(g1_0)*k10 + f32(g1_1)*k11 + f32(g1_2)*k12 + f32(g1_3)*k13 + f32(g1_4)*k14;
    let row3_b = f32(b1_0)*k10 + f32(b1_1)*k11 + f32(b1_2)*k12 + f32(b1_3)*k13 + f32(b1_4)*k14;
    let row3_a = f32(a1_0)*k10 + f32(a1_1)*k11 + f32(a1_2)*k12 + f32(a1_3)*k13 + f32(a1_4)*k14;

    // Row -2 (top-most): dy=-2, kernel k00..k04
    let row4_r = f32(r0_0)*k00 + f32(r0_1)*k01 + f32(r0_2)*k02 + f32(r0_3)*k03 + f32(r0_4)*k04;
    let row4_g = f32(g0_0)*k00 + f32(g0_1)*k01 + f32(g0_2)*k02 + f32(g0_3)*k03 + f32(g0_4)*k04;
    let row4_b = f32(b0_0)*k00 + f32(b0_1)*k01 + f32(b0_2)*k02 + f32(b0_3)*k03 + f32(b0_4)*k04;
    let row4_a = f32(a0_0)*k00 + f32(a0_1)*k01 + f32(a0_2)*k02 + f32(a0_3)*k03 + f32(a0_4)*k04;

    // Accumulate in PIL order: bias, +row0, +row1, +row2, +row3, +row4
    var ss_r = bias + row0_r; ss_r = ss_r + row1_r; ss_r = ss_r + row2_r;
    ss_r = ss_r + row3_r; ss_r = ss_r + row4_r;
    var ss_g = bias + row0_g; ss_g = ss_g + row1_g; ss_g = ss_g + row2_g;
    ss_g = ss_g + row3_g; ss_g = ss_g + row4_g;
    var ss_b = bias + row0_b; ss_b = ss_b + row1_b; ss_b = ss_b + row2_b;
    ss_b = ss_b + row3_b; ss_b = ss_b + row4_b;
    var ss_a = bias + row0_a; ss_a = ss_a + row1_a; ss_a = ss_a + row2_a;
    ss_a = ss_a + row3_a; ss_a = ss_a + row4_a;

    // Mode-aware output: for L/LA modes, only R is convolved; G/B/A preserved from input
    let in_pixel = input[idx];
    let in_g = (in_pixel >> 8u) & 0xffu;
    let in_b = (in_pixel >> 16u) & 0xffu;
    let in_a = (in_pixel >> 24u) & 0xffu;

    let out_r = clip8_filter(ss_r);
    let out_g = select(in_g, clip8_filter(ss_g), mode_has_g(params.mode));
    let out_b = select(in_b, clip8_filter(ss_b), mode_has_b(params.mode));
    let out_a = select(255u, clip8_filter(ss_a), mode_has_a(params.mode));

    return out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }
    let idx = gid.y * params.width + gid.x;
    output[idx] = process_pixel(gid.x, gid.y);
}
