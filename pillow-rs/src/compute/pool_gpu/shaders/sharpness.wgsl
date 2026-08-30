// PIL ImageEnhance.Sharpness implementation.
// 1. Applies SMOOTH kernel [1,1,1, 1,5,1, 1,1,1] / 13 (NOT a sharpen kernel!)
// 2. Blends: output = blurred * (1 - factor) + original * factor
//
// factor: (factor * 1000) as u32 fixed-point.
//   factor=1000 (1.0) → identity (output = original)
//   factor<1000 (<1.0) → more blurring (anti-sharpen)
//   factor>1000 (>1.0) → unsharp mask (increased sharpness)
//
// CPU reference: pool_cpu/ops/enhance.rs op_enhance_sharpness
//   SMOOTH kernel: [1,1,1, 1,5,1, 1,1,1], scale=13, offset=0
//   Blend: blurred * (1-factor) + original * factor
//
// Mode-aware: for L/LA (0/1) only convolves R channel (luma), preserves G/B/A.
// Pixel format: packed u32 RGBA.
// Border pixels (1-pixel edge) copied verbatim.

struct Params {
    width: u32,
    height: u32,
    mode: u32,    // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    factor: u32,  // (factor * 1000) as u32, e.g. 1000 → identity
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn blend_fixed(blurred: u32, original: u32, factor: u32) -> u32 {
    // The host only advertises factors representable as factor*1000. Integer
    // truncation here matches the CPU's positive f64 result cast to u8 and
    // avoids f32 boundary drift in the unsharp blend.
    let fi = i32(factor);
    let value = i32(blurred) * (1000i - fi) + i32(original) * fi;
    return u32(clamp(value / 1000i, 0i, 255i));
}

fn sharpen_pixel(x: u32, y: u32) -> u32 {
    let w = params.width;
    let h = params.height;
    let idx = y * w + x;

    // Return the exact identity before loading a 3x3 neighborhood so this
    // endpoint cannot consume convolution work or trip a device watchdog.
    if params.factor == 1000u {
        return input[idx];
    }

    if w < 3u || h < 3u {
        return input[idx];
    }

    // Border: copy verbatim
    if x == 0u || x >= w - 1u || y == 0u || y >= h - 1u {
        return input[idx];
    }

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

    // The CPU computes 0.5 + weighted_sum / 13 and truncates to u8. All
    // samples are non-negative, so the equivalent integer rounding is
    // floor((2 * weighted_sum + 13) / 26), with no floating-point drift.
    let blur_r_u = (
        2u * (r00 + r01 + r02 + r10 + 5u * r11 + r12 + r20 + r21 + r22) + 13u
    ) / 26u;
    let blur_g_u = (
        2u * (g00 + g01 + g02 + g10 + 5u * g11 + g12 + g20 + g21 + g22) + 13u
    ) / 26u;
    let blur_b_u = (
        2u * (b00 + b01 + b02 + b10 + 5u * b11 + b12 + b20 + b21 + b22) + 13u
    ) / 26u;
    let blur_a_u = (
        2u * (a00 + a01 + a02 + a10 + 5u * a11 + a12 + a20 + a21 + a22) + 13u
    ) / 26u;

    // Original pixel channels (read from input since border cells may differ)
    let in_pixel = input[idx];
    let orig_r = in_pixel & 0xffu;
    let orig_g = (in_pixel >> 8u) & 0xffu;
    let orig_b = (in_pixel >> 16u) & 0xffu;
    let orig_a = (in_pixel >> 24u) & 0xffu;

    // Blend: out = blurred * (1.0 - factor) + original * factor
    // At factor=1.0: out = original (identity)
    // At factor=0.0: out = blurred (fully smooth)
    // At factor=2.0: out = original + (original - blurred) (unsharp mask)
    // Mode-aware output: for L/LA modes, only R is processed; G/B/A preserved from input
    let out_r = blend_fixed(blur_r_u, orig_r, params.factor);
    // L/LA are expanded to equal RGB transport bytes before the CPU
    // implementation sharpens them. Apply the same operation to all three
    // packed color bytes; changing only R would make preserve_mode compute a
    // different luma on the way back to L/LA.
    let out_g = blend_fixed(blur_g_u, orig_g, params.factor);
    let out_b = blend_fixed(blur_b_u, orig_b, params.factor);
    // ImageEnhance preserves alpha for LA/RGBA. CMYK uses byte three as the K
    // channel, so it follows the same sharpened-channel path as C/M/Y.
    var out_a = 255u;
    if (params.mode == 1u || params.mode == 3u) {
        out_a = orig_a;
    } else if (params.mode == 4u) {
        out_a = blend_fixed(blur_a_u, orig_a, params.factor);
    }

    return out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }
    let idx = gid.y * params.width + gid.x;
    output[idx] = sharpen_pixel(gid.x, gid.y);
}
