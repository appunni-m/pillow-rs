// 5x5 convolution filter — PIL-identical row-grouped f32 accumulation.
// Mode-aware: for L/LA (0/1) only convolves R channel (luma), preserves G/B/A.
// Mode 7 is I: one packed word is one signed little-endian i32 sample.
// Params: 25 pre-divided kernel f32 values (as u32 bits), offset (i32), and an
//         optional exact rational denominator.
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
    rational_denominator: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u || m == 4u; }

// PIL-identical clip8: truncating cast clamping to [0, 255]
fn clip8_filter(ss: f32) -> u32 {
    if ss <= 0.0 { return 0u; }
    if ss >= 255.0 { return 255u; }
    return u32(ss);
}

fn i32_sample(pixel: u32) -> f32 {
    return f32(bitcast<i32>(pixel));
}

fn i32_row_5(
    p0: u32, p1: u32, p2: u32, p3: u32, p4: u32,
    k0: f32, k1: f32, k2: f32, k3: f32, k4: f32,
) -> f32 {
    var sum = i32_sample(p1) * k1;
    sum = fma(i32_sample(p0), k0, sum);
    sum = fma(i32_sample(p2), k2, sum);
    sum = fma(i32_sample(p3), k3, sum);
    sum = fma(i32_sample(p4), k4, sum);
    return sum;
}

fn byte_row_5(
    p0: u32, p1: u32, p2: u32, p3: u32, p4: u32,
    k0: f32, k1: f32, k2: f32, k3: f32, k4: f32,
) -> f32 {
    var sum = f32(p1) * k1;
    sum = fma(f32(p0), k0, sum);
    sum = fma(f32(p2), k2, sum);
    sum = fma(f32(p3), k3, sum);
    sum = fma(f32(p4), k4, sum);
    return sum;
}

fn clip_i32_filter(ss: f32) -> u32 {
    if !(ss > 0.0) { return 0u; }
    if ss >= 2147483647.0 { return 0x7fffffffu; }
    return bitcast<u32>(i32(ss));
}

fn rational_num(bits: u32, denominator: u32) -> i32 {
    return i32(round(bitcast<f32>(bits) * f32(denominator)));
}

fn clip_rational_filter(sum: i32, denominator: u32) -> u32 {
    let d = i32(denominator);
    // The byte contract is trunc((sum / d) + offset + 0.5), with clipping
    // before the cast. Doubling keeps the half-unit bias exact for odd d.
    let doubled = sum * 2i + params.offset_val * d * 2i + d;
    let doubled_denominator = d * 2i;
    if doubled <= 0i {
        return 0u;
    }
    if doubled >= 255i * doubled_denominator {
        return 255u;
    }
    return u32(doubled / doubled_denominator);
}

fn rational_5x5_channel(
    values: ptr<function, array<u32, 25>>,
    coefficients: ptr<function, array<u32, 25>>,
    denominator: u32,
) -> u32 {
    var sum: i32 = 0i;
    for (var i = 0u; i < 25u; i++) {
        sum = sum + i32((*values)[i]) * rational_num((*coefficients)[i], denominator);
    }
    return clip_rational_filter(sum, denominator);
}

fn process_i32_pixel(x: u32, y: u32) -> u32 {
    let w = params.width;
    let h = params.height;
    let idx = y * w + x;
    if w < 5u || h < 5u {
        return input[idx];
    }
    if x < 2u || x >= w - 2u || y < 2u || y >= h - 2u {
        return input[idx];
    }

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

    let row0 = i32_row_5(
        input[(y + 2u) * w + (x - 2u)], input[(y + 2u) * w + (x - 1u)],
        input[(y + 2u) * w + x], input[(y + 2u) * w + (x + 1u)],
        input[(y + 2u) * w + (x + 2u)], k00, k01, k02, k03, k04,
    );
    let row1 = i32_row_5(
        input[(y + 1u) * w + (x - 2u)], input[(y + 1u) * w + (x - 1u)],
        input[(y + 1u) * w + x], input[(y + 1u) * w + (x + 1u)],
        input[(y + 1u) * w + (x + 2u)], k10, k11, k12, k13, k14,
    );
    let row2 = i32_row_5(
        input[y * w + (x - 2u)], input[y * w + (x - 1u)], input[y * w + x],
        input[y * w + (x + 1u)], input[y * w + (x + 2u)],
        k20, k21, k22, k23, k24,
    );
    let row3 = i32_row_5(
        input[(y - 1u) * w + (x - 2u)], input[(y - 1u) * w + (x - 1u)],
        input[(y - 1u) * w + x], input[(y - 1u) * w + (x + 1u)],
        input[(y - 1u) * w + (x + 2u)], k30, k31, k32, k33, k34,
    );
    let row4 = i32_row_5(
        input[(y - 2u) * w + (x - 2u)], input[(y - 2u) * w + (x - 1u)],
        input[(y - 2u) * w + x], input[(y - 2u) * w + (x + 1u)],
        input[(y - 2u) * w + (x + 2u)], k40, k41, k42, k43, k44,
    );
    var value = f32(params.offset_val) + 0.5;
    value = value + row0;
    value = value + row1;
    value = value + row2;
    value = value + row3;
    value = value + row4;
    return clip_i32_filter(value);
}

fn process_pixel(x: u32, y: u32) -> u32 {
    if params.mode == 7u {
        return process_i32_pixel(x, y);
    }
    let w = params.width;
    let h = params.height;
    let idx = y * w + x;

    // No interior pixel exists in a smaller image. This guard must precede
    // the w-2/h-2 expressions below, so malformed dimensions cannot rely on
    // unsigned underflow to decide whether neighborhood loads are reached.
    if w < 5u || h < 5u {
        return input[idx];
    }

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

    if params.rational_denominator > 0u {
        var coefficients: array<u32, 25> = array<u32, 25>(
            params.k00, params.k01, params.k02, params.k03, params.k04,
            params.k10, params.k11, params.k12, params.k13, params.k14,
            params.k20, params.k21, params.k22, params.k23, params.k24,
            params.k30, params.k31, params.k32, params.k33, params.k34,
            params.k40, params.k41, params.k42, params.k43, params.k44,
        );
        var values_r: array<u32, 25> = array<u32, 25>(
            r0_0, r0_1, r0_2, r0_3, r0_4, r1_0, r1_1, r1_2, r1_3, r1_4,
            r2_0, r2_1, r2_2, r2_3, r2_4, r3_0, r3_1, r3_2, r3_3, r3_4,
            r4_0, r4_1, r4_2, r4_3, r4_4,
        );
        var values_g: array<u32, 25> = array<u32, 25>(
            g0_0, g0_1, g0_2, g0_3, g0_4, g1_0, g1_1, g1_2, g1_3, g1_4,
            g2_0, g2_1, g2_2, g2_3, g2_4, g3_0, g3_1, g3_2, g3_3, g3_4,
            g4_0, g4_1, g4_2, g4_3, g4_4,
        );
        var values_b: array<u32, 25> = array<u32, 25>(
            b0_0, b0_1, b0_2, b0_3, b0_4, b1_0, b1_1, b1_2, b1_3, b1_4,
            b2_0, b2_1, b2_2, b2_3, b2_4, b3_0, b3_1, b3_2, b3_3, b3_4,
            b4_0, b4_1, b4_2, b4_3, b4_4,
        );
        var values_a: array<u32, 25> = array<u32, 25>(
            a0_0, a0_1, a0_2, a0_3, a0_4, a1_0, a1_1, a1_2, a1_3, a1_4,
            a2_0, a2_1, a2_2, a2_3, a2_4, a3_0, a3_1, a3_2, a3_3, a3_4,
            a4_0, a4_1, a4_2, a4_3, a4_4,
        );
        let denominator = params.rational_denominator;
        let out_r = rational_5x5_channel(&values_r, &coefficients, denominator);
        let in_pixel = input[idx];
        let in_g = (in_pixel >> 8u) & 0xffu;
        let in_b = (in_pixel >> 16u) & 0xffu;
        let out_g = select(
            in_g,
            rational_5x5_channel(&values_g, &coefficients, denominator),
            mode_has_g(params.mode),
        );
        let out_b = select(
            in_b,
            rational_5x5_channel(&values_b, &coefficients, denominator),
            mode_has_b(params.mode),
        );
        let out_a = select(
            255u,
            rational_5x5_channel(&values_a, &coefficients, denominator),
            mode_has_a(params.mode),
        );
        return out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
    }

    // ── Row accumulation in PIL order (bottom to top) ──
    // Row +2 (bottom-most): dy=+2, kernel k40..k44
    let row0_r = byte_row_5(r4_0, r4_1, r4_2, r4_3, r4_4, k40, k41, k42, k43, k44);
    let row0_g = byte_row_5(g4_0, g4_1, g4_2, g4_3, g4_4, k40, k41, k42, k43, k44);
    let row0_b = byte_row_5(b4_0, b4_1, b4_2, b4_3, b4_4, k40, k41, k42, k43, k44);
    let row0_a = byte_row_5(a4_0, a4_1, a4_2, a4_3, a4_4, k40, k41, k42, k43, k44);

    // Row +1: dy=+1, kernel k30..k34
    let row1_r = byte_row_5(r3_0, r3_1, r3_2, r3_3, r3_4, k30, k31, k32, k33, k34);
    let row1_g = byte_row_5(g3_0, g3_1, g3_2, g3_3, g3_4, k30, k31, k32, k33, k34);
    let row1_b = byte_row_5(b3_0, b3_1, b3_2, b3_3, b3_4, k30, k31, k32, k33, k34);
    let row1_a = byte_row_5(a3_0, a3_1, a3_2, a3_3, a3_4, k30, k31, k32, k33, k34);

    // Row 0 (center): dy=0, kernel k20..k24
    let row2_r = byte_row_5(r2_0, r2_1, r2_2, r2_3, r2_4, k20, k21, k22, k23, k24);
    let row2_g = byte_row_5(g2_0, g2_1, g2_2, g2_3, g2_4, k20, k21, k22, k23, k24);
    let row2_b = byte_row_5(b2_0, b2_1, b2_2, b2_3, b2_4, k20, k21, k22, k23, k24);
    let row2_a = byte_row_5(a2_0, a2_1, a2_2, a2_3, a2_4, k20, k21, k22, k23, k24);

    // Row -1: dy=-1, kernel k10..k14
    let row3_r = byte_row_5(r1_0, r1_1, r1_2, r1_3, r1_4, k10, k11, k12, k13, k14);
    let row3_g = byte_row_5(g1_0, g1_1, g1_2, g1_3, g1_4, k10, k11, k12, k13, k14);
    let row3_b = byte_row_5(b1_0, b1_1, b1_2, b1_3, b1_4, k10, k11, k12, k13, k14);
    let row3_a = byte_row_5(a1_0, a1_1, a1_2, a1_3, a1_4, k10, k11, k12, k13, k14);

    // Row -2 (top-most): dy=-2, kernel k00..k04
    let row4_r = byte_row_5(r0_0, r0_1, r0_2, r0_3, r0_4, k00, k01, k02, k03, k04);
    let row4_g = byte_row_5(g0_0, g0_1, g0_2, g0_3, g0_4, k00, k01, k02, k03, k04);
    let row4_b = byte_row_5(b0_0, b0_1, b0_2, b0_3, b0_4, k00, k01, k02, k03, k04);
    let row4_a = byte_row_5(a0_0, a0_1, a0_2, a0_3, a0_4, k00, k01, k02, k03, k04);

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
