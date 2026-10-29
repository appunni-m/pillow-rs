// 3x3 convolution filter — PIL-identical row-grouped f32 accumulation.
// Mode-aware: for L/LA (0/1) only convolves R channel (luma), preserves G/B/A.
// Mode 7 is I: one packed word is one signed little-endian i32 sample.
// Params: 9 pre-divided kernel f32 values (as u32 bits), offset (i32), and an
//         optional exact rational denominator. All sent from host via
//         extract_params.
//
// CPU reference (image.rs:2604-2624):
//   k[n] = kernel[n] / scale
//   rounding_bias = offset + 0.5
//   Row bottom (y+1): bp*k0 + cp*k1 + ap*k2
//   Row center (y):   bp*k3 + cp*k4 + ap*k5
//   Row top (y-1):    bp*k6 + cp*k7 + ap*k8
//   Accumulate ORDER: bias + row_b + row_c + row_t (f32, order matters!)
//   clip8_filter: if ss<=0 -> 0, ss>=255 -> 255, else ss as u8
// Border pixels (1-pixel edge) copied verbatim.
//
// Pixel format: packed u32 RGBA (R | G<<8 | B<<16 | A<<24)

struct Params {
    width: u32,
    height: u32,
    mode: u32,    // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    k0: u32,  // f32 bits, pre-divided by scale
    k1: u32,
    k2: u32,
    k3: u32,
    k4: u32,
    k5: u32,
    k6: u32,
    k7: u32,
    k8: u32,
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

fn byte_row_3(p0: u32, p1: u32, p2: u32, k0: f32, k1: f32, k2: f32) -> f32 {
    var sum = f32(p1) * k1;
    sum = fma(f32(p0), k0, sum);
    sum = fma(f32(p2), k2, sum);
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

fn rational_3x3_channel(
    bottom_left: u32,
    bottom_center: u32,
    bottom_right: u32,
    center_left: u32,
    center: u32,
    center_right: u32,
    top_left: u32,
    top_center: u32,
    top_right: u32,
    denominator: u32,
) -> u32 {
    let n0 = rational_num(params.k0, denominator);
    let n1 = rational_num(params.k1, denominator);
    let n2 = rational_num(params.k2, denominator);
    let n3 = rational_num(params.k3, denominator);
    let n4 = rational_num(params.k4, denominator);
    let n5 = rational_num(params.k5, denominator);
    let n6 = rational_num(params.k6, denominator);
    let n7 = rational_num(params.k7, denominator);
    let n8 = rational_num(params.k8, denominator);
    var sum = i32(bottom_center) * n1;
    sum = sum + i32(bottom_left) * n0;
    sum = sum + i32(bottom_right) * n2;
    sum = sum + i32(center) * n4;
    sum = sum + i32(center_left) * n3;
    sum = sum + i32(center_right) * n5;
    sum = sum + i32(top_center) * n7;
    sum = sum + i32(top_left) * n6;
    sum = sum + i32(top_right) * n8;
    return clip_rational_filter(sum, denominator);
}

fn process_i32_pixel(x: u32, y: u32) -> u32 {
    let w = params.width;
    let idx = y * w + x;
    if w < 3u || params.height < 3u {
        return input[idx];
    }
    if x == 0u || x >= w - 1u || y == 0u || y >= params.height - 1u {
        return input[idx];
    }

    let k0 = bitcast<f32>(params.k0);
    let k1 = bitcast<f32>(params.k1);
    let k2 = bitcast<f32>(params.k2);
    let k3 = bitcast<f32>(params.k3);
    let k4 = bitcast<f32>(params.k4);
    let k5 = bitcast<f32>(params.k5);
    let k6 = bitcast<f32>(params.k6);
    let k7 = bitcast<f32>(params.k7);
    let k8 = bitcast<f32>(params.k8);
    let p0 = input[(y - 1u) * w + (x - 1u)];
    let p1 = input[(y - 1u) * w + x];
    let p2 = input[(y - 1u) * w + (x + 1u)];
    let p3 = input[y * w + (x - 1u)];
    let p4 = input[y * w + x];
    let p5 = input[y * w + (x + 1u)];
    let p6 = input[(y + 1u) * w + (x - 1u)];
    let p7 = input[(y + 1u) * w + x];
    let p8 = input[(y + 1u) * w + (x + 1u)];

    // Keep the same middle, left, right contraction order and bottom-to-top
    // row accumulation used by Pillow's I-mode implementation.
    var bottom = i32_sample(p7) * k1;
    bottom = fma(i32_sample(p6), k0, bottom);
    bottom = fma(i32_sample(p8), k2, bottom);
    var middle = i32_sample(p4) * k4;
    middle = fma(i32_sample(p3), k3, middle);
    middle = fma(i32_sample(p5), k5, middle);
    var top = i32_sample(p1) * k7;
    top = fma(i32_sample(p0), k6, top);
    top = fma(i32_sample(p2), k8, top);
    var value = f32(params.offset_val) + 0.5;
    value = value + bottom;
    value = value + middle;
    value = value + top;
    return clip_i32_filter(value);
}

fn process_pixel(x: u32, y: u32) -> u32 {
    if params.mode == 7u {
        return process_i32_pixel(x, y);
    }
    let w = params.width;
    let idx = y * w + x;

    // No interior pixel exists in a smaller image. Keep this defense in the
    // shader as well as the host preflight so malformed dimensions cannot
    // reach the y-1/x-1 neighborhood loads.
    if w < 3u || params.height < 3u {
        return input[idx];
    }

    // Border: 1-pixel edge copied verbatim
    if x == 0u || x >= w - 1u || y == 0u || y >= params.height - 1u {
        return input[idx];
    }

    let k0 = bitcast<f32>(params.k0);
    let k1 = bitcast<f32>(params.k1);
    let k2 = bitcast<f32>(params.k2);
    let k3 = bitcast<f32>(params.k3);
    let k4 = bitcast<f32>(params.k4);
    let k5 = bitcast<f32>(params.k5);
    let k6 = bitcast<f32>(params.k6);
    let k7 = bitcast<f32>(params.k7);
    let k8 = bitcast<f32>(params.k8);
    let bias = f32(params.offset_val) + 0.5;

    // Load 9 neighborhood pixels from packed u32 buffer
    // Indices: pY_X where Y=row (0=top, 1=center, 2=bottom), X=col (0=left, 1=center, 2=right)
    let p0_0 = input[(y - 1u) * w + (x - 1u)];  // top-left      -> kernel k6
    let p0_1 = input[(y - 1u) * w + x];          // top-center    -> kernel k7
    let p0_2 = input[(y - 1u) * w + (x + 1u)];  // top-right     -> kernel k8
    let p1_0 = input[y * w + (x - 1u)];          // center-left   -> kernel k3
    let p1_1 = input[y * w + x];                  // center-center -> kernel k4
    let p1_2 = input[y * w + (x + 1u)];          // center-right  -> kernel k5
    let p2_0 = input[(y + 1u) * w + (x - 1u)];  // bottom-left   -> kernel k0
    let p2_1 = input[(y + 1u) * w + x];          // bottom-center -> kernel k1
    let p2_2 = input[(y + 1u) * w + (x + 1u)];  // bottom-right  -> kernel k2

    // Extract R, G, B, A from each pixel (byte 0=R, 1=G, 2=B, 3=A)
    let r00 = p0_0 & 0xffu;        let g00 = (p0_0 >> 8u) & 0xffu;
    let b00 = (p0_0 >> 16u) & 0xffu; let a00 = (p0_0 >> 24u) & 0xffu;
    let r01 = p0_1 & 0xffu;        let g01 = (p0_1 >> 8u) & 0xffu;
    let b01 = (p0_1 >> 16u) & 0xffu; let a01 = (p0_1 >> 24u) & 0xffu;
    let r02 = p0_2 & 0xffu;        let g02 = (p0_2 >> 8u) & 0xffu;
    let b02 = (p0_2 >> 16u) & 0xffu; let a02 = (p0_2 >> 24u) & 0xffu;

    let r10 = p1_0 & 0xffu;        let g10 = (p1_0 >> 8u) & 0xffu;
    let b10 = (p1_0 >> 16u) & 0xffu; let a10 = (p1_0 >> 24u) & 0xffu;
    let r11 = p1_1 & 0xffu;        let g11 = (p1_1 >> 8u) & 0xffu;
    let b11 = (p1_1 >> 16u) & 0xffu; let a11 = (p1_1 >> 24u) & 0xffu;
    let r12 = p1_2 & 0xffu;        let g12 = (p1_2 >> 8u) & 0xffu;
    let b12 = (p1_2 >> 16u) & 0xffu; let a12 = (p1_2 >> 24u) & 0xffu;

    let r20 = p2_0 & 0xffu;        let g20 = (p2_0 >> 8u) & 0xffu;
    let b20 = (p2_0 >> 16u) & 0xffu; let a20 = (p2_0 >> 24u) & 0xffu;
    let r21 = p2_1 & 0xffu;        let g21 = (p2_1 >> 8u) & 0xffu;
    let b21 = (p2_1 >> 16u) & 0xffu; let a21 = (p2_1 >> 24u) & 0xffu;
    let r22 = p2_2 & 0xffu;        let g22 = (p2_2 >> 8u) & 0xffu;
    let b22 = (p2_2 >> 16u) & 0xffu; let a22 = (p2_2 >> 24u) & 0xffu;

    // Mode-aware output: for L/LA modes, only R is convolved; G/B/A
    // preservation is applied identically by both byte arithmetic paths.
    let in_pixel = input[idx];
    let in_g = (in_pixel >> 8u) & 0xffu;
    let in_b = (in_pixel >> 16u) & 0xffu;
    let in_a = (in_pixel >> 24u) & 0xffu;

    if params.rational_denominator > 0u {
        let denominator = params.rational_denominator;
        let out_r = rational_3x3_channel(
            r20, r21, r22, r10, r11, r12, r00, r01, r02, denominator,
        );
        let out_g = select(
            in_g,
            rational_3x3_channel(
                g20, g21, g22, g10, g11, g12, g00, g01, g02, denominator,
            ),
            mode_has_g(params.mode),
        );
        let out_b = select(
            in_b,
            rational_3x3_channel(
                b20, b21, b22, b10, b11, b12, b00, b01, b02, denominator,
            ),
            mode_has_b(params.mode),
        );
        let out_a = select(
            255u,
            rational_3x3_channel(
                a20, a21, a22, a10, a11, a12, a00, a01, a02, denominator,
            ),
            mode_has_a(params.mode),
        );
        return out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
    }

    // ── Row bottom (y+1) — pixels p2_0..p2_2, kernel k0..k2 ──
    let row_b_r = byte_row_3(r20, r21, r22, k0, k1, k2);
    let row_b_g = byte_row_3(g20, g21, g22, k0, k1, k2);
    let row_b_b = byte_row_3(b20, b21, b22, k0, k1, k2);
    let row_b_a = byte_row_3(a20, a21, a22, k0, k1, k2);

    // ── Row center (y) — pixels p1_0..p1_2, kernel k3..k5 ──
    let row_c_r = byte_row_3(r10, r11, r12, k3, k4, k5);
    let row_c_g = byte_row_3(g10, g11, g12, k3, k4, k5);
    let row_c_b = byte_row_3(b10, b11, b12, k3, k4, k5);
    let row_c_a = byte_row_3(a10, a11, a12, k3, k4, k5);

    // ── Row top (y-1) — pixels p0_0..p0_2, kernel k6..k8 ──
    let row_t_r = byte_row_3(r00, r01, r02, k6, k7, k8);
    let row_t_g = byte_row_3(g00, g01, g02, k6, k7, k8);
    let row_t_b = byte_row_3(b00, b01, b02, k6, k7, k8);
    let row_t_a = byte_row_3(a00, a01, a02, k6, k7, k8);

    // Accumulate in PIL order: bias -> bottom -> center -> top
    let ss_r = bias + row_b_r + row_c_r + row_t_r;
    let ss_g = bias + row_b_g + row_c_g + row_t_g;
    let ss_b = bias + row_b_b + row_c_b + row_t_b;
    let ss_a = bias + row_b_a + row_c_a + row_t_a;

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
