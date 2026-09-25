// Convert between color modes.
// Source mode is params.mode; target mode is params.target_mode.
// Source mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA, 4=CMYK, 5=I;16*,
// 6=RGBX. Target mode codes: 0=L, 1=LA, 2=RGB, 3=RGBA, 4=CMYK,
// 5=YCbCr, 6=HSV, 7=I, 8=F.
// RGBX has RGBA-sized storage, but its fourth byte is padding and is ignored.
// I;16* uses the low 16 bits of one transport word as an unsigned sample.
// Packed u32 RGBA: byte0=R (luma/value for L/LA), byte1=G, byte2=B, byte3=A
//
// Conversion matrix (the byte targets produce packed output bytes; I/F write
// one little-endian scalar sample into the complete output word):
//   L(0):   R=value, G=0, B=0, A=255
//   LA(1):  R=value, G=0, B=0, A=alpha
//   RGB(2): R=red, G=green, B=blue, A=255
//   RGBA(3):R=red, G=green, B=blue, A=alpha
//
// Per-pixel dispatch (16x16 workgroups). Each thread converts its own pixel.

struct Params {
    width: u32,
    height: u32,
    mode: u32,         // source mode
    _pad: u32,
    target_mode: u32,  // 0=L, 1=LA, 2=RGB, 3=RGBA, 4=CMYK, 5=YCbCr, 6=HSV, 7=I, 8=F
}

// ── Mode helpers (for target mode) ──

fn target_has_g(m: u32) -> bool { return m >= 2u; }
fn target_has_b(m: u32) -> bool { return m >= 2u; }
fn target_has_a(m: u32) -> bool { return m == 1u || m == 3u || m == 4u; }

// Pillow's exact rounded BT.601 luma. The decimal 299/587/114 form differs
// from PIL's fixed-point conversion at boundary values.
fn bt601_luma(r: u32, g: u32, b: u32) -> u32 {
    return (19595u * r + 38470u * g + 7471u * b + 32768u) >> 16u;
}

// ConvertYCbCr.c builds 6-bit fixed-point tables as
// (value * coefficient * 64 + 0.5), then uses an arithmetic shift.  The
// bounded byte domain has no f32 rounding boundary for these coefficients, so
// this computes the same table entries without uploading eight 256-entry LUTs.
fn ycbcr_table(value: u32, coefficient: f32) -> i32 {
    return i32(f32(value) * coefficient * 64.0 + 0.5);
}

fn arithmetic_shift6(value: i32) -> i32 {
    if value < 0 {
        return -((-value + 63) / 64);
    }
    return value / 64;
}

fn source_rgb_r(src: u32, r: u32) -> u32 {
    return r;
}

fn source_rgb_g(src: u32, r: u32, g: u32) -> u32 {
    if src == 0u || src == 1u {
        return r;
    }
    return g;
}

fn source_rgb_b(src: u32, r: u32, b: u32) -> u32 {
    if src == 0u || src == 1u {
        return r;
    }
    return b;
}

// Correctly rounded positive n/d in [0,1]. HSV's denominator is at most
// 6*2^24, so normalization and doubled remainders fit u32. Construct the
// significand with integer division steps, retaining ties-to-even rounding;
// Metal's reciprocal-based float division can otherwise change output bytes.
fn hsv_ratio(n: u32, d: u32) -> f32 {
    if n == 0u { return 0.0; }
    var remainder = n;
    var exponent = 0i;
    while remainder < d {
        remainder = remainder << 1u;
        exponent = exponent - 1i;
    }
    remainder = remainder - d;
    var significand = 1u << 23u;
    for (var bit = 22i; bit >= 0i; bit = bit - 1i) {
        remainder = remainder << 1u;
        if remainder >= d {
            remainder = remainder - d;
            significand = significand | (1u << u32(bit));
        }
    }
    let twice = remainder * 2u;
    if twice > d || (twice == d && (significand & 1u) != 0u) {
        significand = significand + 1u;
    }
    if significand == (1u << 24u) {
        significand = significand >> 1u;
        exponent = exponent + 1i;
    }
    return bitcast<f32>((u32(exponent + 127i) << 23u) | (significand & 0x7fffffu));
}

// Pillow promotes the stored float to double for *255, then truncates.
// Its 24-bit significand times 255 fits u32; shifting that exact product
// avoids the extra rounding from a float32 multiplication at byte boundaries.
fn hsv_byte(value: f32) -> u32 {
    if value == 0.0 { return 0u; }
    let bits = bitcast<u32>(value);
    let exponent = i32((bits >> 23u) & 255u) - 127i;
    let shift = 23i - exponent;
    if shift >= 32i { return 0u; }
    return (((bits & 0x7fffffu) | 0x800000u) * 255u) >> u32(shift);
}

fn rgb_to_hsv_pixel(r: u32, g: u32, b: u32) -> vec3<u32> {
    let maxc = max(r, max(g, b));
    let minc = min(r, min(g, b));
    if minc == maxc { return vec3<u32>(0u, 0u, maxc); }
    let range = maxc - minc;
    // One ratio is zero and another is one. Eliminate those exact terms
    // before narrowing the sector to f32, matching Pillow's double-constant
    // arithmetic without separately rounding 2+rc or 4+gc first.
    var h: f32;
    if r == maxc {
        if b == minc { h = 1.0 - hsv_ratio(maxc - g, range); }
        else { h = hsv_ratio(maxc - b, range) - 1.0; }
    } else if g == maxc {
        if b == minc { h = 1.0 + hsv_ratio(maxc - r, range); }
        else { h = 3.0 - hsv_ratio(maxc - b, range); }
    } else {
        if r == minc { h = 3.0 + hsv_ratio(maxc - g, range); }
        else { h = 5.0 - hsv_ratio(maxc - r, range); }
    }
    // The sector float is an exact multiple of 2^-24. Form its wrapped
    // h/6 as an integer rational, then round once to the stored hue float.
    // This also preserves negative red sectors and halfway ties.
    let magnitude = u32(abs(h) * 16777216.0);
    var numerator = magnitude;
    if h < 0.0 { numerator = 100663296u - magnitude; }
    let hue = hsv_ratio(numerator, 100663296u);
    return vec3<u32>(hsv_byte(hue), hsv_byte(hsv_ratio(range, maxc)), maxc);
}

// Construct the correctly rounded IEEE-754 f32 bit pattern for the positive
// rational value `sum / 1000`.  Some Metal drivers implement f32 division
// with a reciprocal approximation, which can be one ULP above Rust/Pillow's
// `f32` result.  Integer long division keeps the GPU result deterministic
// without a host round-trip; `sum` is bounded by the RGB 299/587/114 sum.
fn integer_sum_to_f32_bits(sum: u32) -> u32 {
    if sum == 0u {
        return 0u;
    }

    // Smallest exponent is -10 for 1/1000; thresholds are ceil(1000 * 2^e).
    var exponent: i32;
    if sum >= 128000u { exponent = 7; }
    else if sum >= 64000u { exponent = 6; }
    else if sum >= 32000u { exponent = 5; }
    else if sum >= 16000u { exponent = 4; }
    else if sum >= 8000u { exponent = 3; }
    else if sum >= 4000u { exponent = 2; }
    else if sum >= 2000u { exponent = 1; }
    else if sum >= 1000u { exponent = 0; }
    else if sum >= 500u { exponent = -1; }
    else if sum >= 250u { exponent = -2; }
    else if sum >= 125u { exponent = -3; }
    else if sum >= 63u { exponent = -4; }
    else if sum >= 32u { exponent = -5; }
    else if sum >= 16u { exponent = -6; }
    else if sum >= 8u { exponent = -7; }
    else if sum >= 4u { exponent = -8; }
    else if sum >= 2u { exponent = -9; }
    else { exponent = -10; }

    // (sum / 1000) * 2^23 / 2^exponent is
    // (sum * 2^(20-exponent)) / 125.  The quotient stays below 2^24.
    let shifts = u32(20 - exponent);
    var quotient = sum / 125u;
    var remainder = sum % 125u;
    for (var shift = 0u; shift < 30u; shift++) {
        if shift >= shifts {
            break;
        }
        let doubled = remainder * 2u;
        quotient = quotient * 2u + doubled / 125u;
        remainder = doubled % 125u;
    }

    // Round to nearest, ties to even, matching IEEE-754 integer-to-f32
    // division for this bounded positive domain.
    let twice_remainder = remainder * 2u;
    if twice_remainder > 125u ||
        (twice_remainder == 125u && (quotient & 1u) == 1u)
    {
        quotient += 1u;
    }
    if quotient >= 16777216u {
        quotient = 8388608u;
        exponent += 1;
    }
    return (u32(exponent + 127) << 23u) | (quotient - 8388608u);
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let idx = gid.y * params.width + gid.x;
    let pixel = input[idx];

    // Always unpack all 4 bytes from the source pixel.
    // For L mode: only byte0 (R) carries the value; G,B are typically 0.
    // For LA mode: byte0=R(value), byte3=A; G,B are typically 0.
    let r = pixel & 0xffu;
    let g = (pixel >> 8u) & 0xffu;
    let b = (pixel >> 16u) & 0xffu;
    let a = (pixel >> 24u) & 0xffu;

    let src = params.mode;
    let dst = params.target_mode;

    // I and F are scalar Pillow modes, but their four-byte little-endian
    // sample is still carried by the packed transport. Write the complete
    // word and return before byte-channel normalization can reinterpret it as
    // RGBA. The CPU converter first promotes L/LA to replicated RGB, so use
    // the same logical source channels here.
    let rgb_r = source_rgb_r(src, r);
    let rgb_g = source_rgb_g(src, r, g);
    let rgb_b = source_rgb_b(src, r, b);
    if dst == 7u {
        let l = bt601_luma(rgb_r, rgb_g, rgb_b);
        output[idx] = bitcast<u32>(i32(l));
        return;
    }
    if dst == 8u {
        // Pillow's RGB->F converter forms the integer 299/587/114 sum before
        // the final f32 division, rather than evaluating three f32 products.
        let sum = 299u * rgb_r + 587u * rgb_g + 114u * rgb_b;
        output[idx] = integer_sum_to_f32_bits(sum);
        return;
    }

    var out_r = r;
    var out_g = g;
    var out_b = b;
    var out_a = a;

    // ── Branch on (src, dst) pair ──
    //
    // L (0) → anything
    if src == 0u {
        if dst == 0u { /* L→L: passthrough */ }
        else if dst == 1u { /* L→LA: R carries value, A=255 */ out_a = 255u; }
        else if dst == 2u { /* L→RGB: replicate R to G,B */ out_g = r; out_b = r; out_a = 255u; }
        else if dst == 3u { /* L→RGBA: replicate R to G,B, A=255 */ out_g = r; out_b = r; out_a = 255u; }
    }
    // LA (1) → anything
    else if src == 1u {
        if dst == 0u { /* LA→L: keep R (value), drop A */ out_a = 255u; }
        else if dst == 1u { /* LA→LA: passthrough */ }
        else if dst == 2u { /* LA→RGB: replicate R to G,B, drop A */ out_g = r; out_b = r; out_a = 255u; }
        else if dst == 3u { /* LA→RGBA: replicate R to G,B, keep A */ out_g = r; out_b = r; }
    }
    // RGB (2) → anything
    else if src == 2u {
        if dst == 0u { /* RGB→L: BT.601 luma to R */ let l = bt601_luma(r, g, b); out_r = l; out_g = l; out_b = l; out_a = 255u; }
        else if dst == 1u { /* RGB→LA: luma to R, drop G,B, A=255 */ let l = bt601_luma(r, g, b); out_r = l; out_g = l; out_b = l; out_a = 255u; }
        else if dst == 2u { /* RGB→RGB: passthrough */ }
        else if dst == 3u { /* RGB→RGBA: add A=255 */ out_a = 255u; }
    }
    // RGBA (3) → anything
    else if src == 3u {
        if dst == 0u { /* RGBA→L: luma to R, drop alpha */ let l = bt601_luma(r, g, b); out_r = l; out_g = l; out_b = l; out_a = 255u; }
        else if dst == 1u { /* RGBA→LA: luma to R, keep A */ let l = bt601_luma(r, g, b); out_r = l; out_g = l; out_b = l; }
        else if dst == 2u { /* RGBA→RGB: drop A */ out_a = 255u; }
        else if dst == 3u { /* RGBA→RGBA: passthrough */ }
    }
    // RGBX (6) → anything. Byte three is padding, never an alpha sample.
    else if src == 6u {
        if dst == 0u { /* RGBX→L: luma to R, ignore padding */ let l = bt601_luma(r, g, b); out_r = l; out_g = l; out_b = l; out_a = 255u; }
        else if dst == 1u { /* RGBX→LA: luma to R, A=255 */ let l = bt601_luma(r, g, b); out_r = l; out_g = l; out_b = l; out_a = 255u; }
        else if dst == 2u { /* RGBX→RGB: drop padding */ out_a = 255u; }
        else if dst == 3u { /* RGBX→RGBA: replace padding with opaque A */ out_a = 255u; }
    }

    // L/LA use Pillow's direct luma-to-YCbCr path: retain the luma byte and
    // install neutral chroma. RGB-family sources use ConvertYCbCr's 6-bit
    // fixed-point BT.601 lookup-table construction.
    if dst == 5u {
        if src == 0u || src == 1u {
            out_r = r;
            out_g = 128u;
            out_b = 128u;
        } else {
            let y = arithmetic_shift6(
                ycbcr_table(rgb_r, 0.299) +
                ycbcr_table(rgb_g, 0.587) +
                ycbcr_table(rgb_b, 0.114),
            );
            let cb = arithmetic_shift6(
                ycbcr_table(rgb_r, -0.16874) +
                ycbcr_table(rgb_g, -0.33126) +
                ycbcr_table(rgb_b, 0.5),
            ) + 128;
            let cr = arithmetic_shift6(
                ycbcr_table(rgb_r, 0.5) +
                ycbcr_table(rgb_g, -0.41869) +
                ycbcr_table(rgb_b, -0.08131),
            ) + 128;
            out_r = u32(clamp(y, 0, 255));
            out_g = u32(clamp(cb, 0, 255));
            out_b = u32(clamp(cr, 0, 255));
        }
        out_a = 255u;
    }

    // HSV is stored as H/S/V in the three-byte RGB transport. L/LA are
    // promoted to replicated RGB, matching the CPU converter's to_rgb8().
    if dst == 6u {
        let hsv = rgb_to_hsv_pixel(rgb_r, rgb_g, rgb_b);
        out_r = hsv.x;
        out_g = hsv.y;
        out_b = hsv.z;
        out_a = 255u;
    }
    // CMYK is stored as four native bytes, not as RGBA alpha. Pillow's
    // byte-mode conversion uses the grayscale branch for L/LA and the RGB
    // inverse for RGB-family sources.
    if dst == 4u {
        if src == 0u || src == 1u {
            out_r = 0u;
            out_g = 0u;
            out_b = 0u;
            out_a = 255u - r;
        } else {
            out_r = 255u - r;
            out_g = 255u - g;
            out_b = 255u - b;
            out_a = 0u;
        }
    }
    // I;16* → byte modes. Pillow clips the unsigned sample to 255 rather
    // than scaling the full 0..65535 range. The source upload places the
    // decoded numeric sample in the low 16 bits of the word.
    else if src == 5u {
        let l = min(pixel & 0xffffu, 255u);
        out_r = l;
        out_g = select(0u, l, dst >= 2u);
        out_b = select(0u, l, dst >= 2u);
        out_a = 255u;
    }

    // Apply target mode-awareness: channels not present in target mode get zero
    let final_r = out_r;
    let final_g = select(0u, out_g, target_has_g(dst));
    let final_b = select(0u, out_b, target_has_b(dst));
    let final_a = select(255u, out_a, target_has_a(dst));

    output[idx] = final_r | (final_g << 8u) | (final_b << 16u) | (final_a << 24u);
}
