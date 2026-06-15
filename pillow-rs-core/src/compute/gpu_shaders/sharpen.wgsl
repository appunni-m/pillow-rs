// Fixed sharpen kernel (same as Filter3x3 with hardcoded kernel).
// Mode-aware: for L/LA (0/1) only convolves R channel (luma), preserves G/B/A.
// Kernel: [-2, -2, -2, -2, 32, -2, -2, -2, -2], scale=16, offset=0.
//
// CPU reference (ops/filter.rs:71-75):
//   SHARPEN = FilterKernel::new([-2,-2,-2,-2,32,-2,-2,-2,-2], 16.0, 0)
//   Executed as filter_3x3 with identical accumulation and clip8_filter.
//
// factor: f32 bits, scale factor for varying sharpen strength (1.0 = default).
// Pixel format: packed u32 RGBA.
// Border pixels (1-pixel edge) copied verbatim.

struct Params {
    width: u32,
    height: u32,
    mode: u32,    // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    factor: u32,  // f32 bits, sharpen strength factor
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u; }

fn clip8_filter(ss: f32) -> u32 {
    if ss <= 0.0 { return 0u; }
    if ss >= 255.0 { return 255u; }
    return u32(ss);
}

fn sharpen_pixel(x: u32, y: u32) -> u32 {
    let w = params.width;
    let h = params.height;
    let idx = y * w + x;

    // Border: copy verbatim
    if x == 0u || x >= w - 1u || y == 0u || y >= h - 1u {
        return input[idx];
    }

    // Sharpen kernel: [-2, -2, -2, -2, 32, -2, -2, -2, -2] / 16
    let k0 = -2.0 / 16.0; let k1 = -2.0 / 16.0; let k2 = -2.0 / 16.0;
    let k3 = -2.0 / 16.0; let k4 = 32.0 / 16.0; let k5 = -2.0 / 16.0;
    let k6 = -2.0 / 16.0; let k7 = -2.0 / 16.0; let k8 = -2.0 / 16.0;
    let bias = 0.5;  // offset=0, so bias = 0.0 + 0.5 = 0.5

    // Load 9 neighborhood pixels (same indexed as filter_3x3)
    // pY_X: Y=row (0=top, 1=center, 2=bottom), X=col
    let p0_0 = input[(y - 1u) * w + (x - 1u)];
    let p0_1 = input[(y - 1u) * w + x];
    let p0_2 = input[(y - 1u) * w + (x + 1u)];
    let p1_0 = input[y * w + (x - 1u)];
    let p1_1 = input[y * w + x];
    let p1_2 = input[y * w + (x + 1u)];
    let p2_0 = input[(y + 1u) * w + (x - 1u)];
    let p2_1 = input[(y + 1u) * w + x];
    let p2_2 = input[(y + 1u) * w + (x + 1u)];

    // Extract channels from all 9 pixels
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

    // Row bottom (y+1): kernel k0,k1,k2 on p2_0,p2_1,p2_2
    let row_b_r = f32(r20) * k0 + f32(r21) * k1 + f32(r22) * k2;
    let row_b_g = f32(g20) * k0 + f32(g21) * k1 + f32(g22) * k2;
    let row_b_b = f32(b20) * k0 + f32(b21) * k1 + f32(b22) * k2;
    let row_b_a = f32(a20) * k0 + f32(a21) * k1 + f32(a22) * k2;

    // Row center: kernel k3,k4,k5 on p1_0,p1_1,p1_2
    let row_c_r = f32(r10) * k3 + f32(r11) * k4 + f32(r12) * k5;
    let row_c_g = f32(g10) * k3 + f32(g11) * k4 + f32(g12) * k5;
    let row_c_b = f32(b10) * k3 + f32(b11) * k4 + f32(b12) * k5;
    let row_c_a = f32(a10) * k3 + f32(a11) * k4 + f32(a12) * k5;

    // Row top (y-1): kernel k6,k7,k8 on p0_0,p0_1,p0_2
    let row_t_r = f32(r00) * k6 + f32(r01) * k7 + f32(r02) * k8;
    let row_t_g = f32(g00) * k6 + f32(g01) * k7 + f32(g02) * k8;
    let row_t_b = f32(b00) * k6 + f32(b01) * k7 + f32(b02) * k8;
    let row_t_a = f32(a00) * k6 + f32(a01) * k7 + f32(a02) * k8;

    // Accumulate: bias -> bottom -> center -> top
    let ss_r = bias + row_b_r + row_c_r + row_t_r;
    let ss_g = bias + row_b_g + row_c_g + row_t_g;
    let ss_b = bias + row_b_b + row_c_b + row_t_b;
    let ss_a = bias + row_b_a + row_c_a + row_t_a;

    // Mode-aware output: for L/LA modes, only R is sharpened; G/B/A preserved from input
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
    output[idx] = sharpen_pixel(gid.x, gid.y);
}
