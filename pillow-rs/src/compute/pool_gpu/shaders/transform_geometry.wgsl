// Exact Geometry.c sampling. The host supplies indices and binary64
// fractional coordinates only; this shader owns source sampling, the f32
// coefficient stores, ordered f64 FMA/Horner evaluation, and alpha conversion.
struct Params {
    width: u32, height: u32, mode: u32, _pad: u32,
    dst_w: u32, dst_h: u32,
    a: u32, b: u32, c: u32, d: u32, e: u32, f: u32,
    fill_color: u32, filter_code: u32, premultiply: u32, method: u32,
}
@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read> geometry: array<u32>;
@group(0) @binding(2) var<storage, read_write> output: array<u32>;
@group(0) @binding(3) var<uniform> params: Params;

fn mode_has_g(m: u32) -> bool {
    return m == 2u || m == 3u || m == 4u || m == 6u;
}
fn mode_has_b(m: u32) -> bool {
    return m == 2u || m == 3u || m == 4u || m == 6u;
}
fn mode_has_fourth(m: u32) -> bool {
    // CMYK's fourth byte is K and RGBX's fourth byte is padding. I/F use
    // all four bytes as one opaque typed sample and return early in the
    // nearest sampler, but keeping the helper truthful documents the packed
    // transport for those modes as well.
    return m == 4u || m == 6u || m == 7u || m == 8u;
}
fn mode_has_a(m: u32) -> bool {
    return m == 1u || m == 3u;
}

fn get_fill_pixel() -> u32 {
    let fc = params.fill_color;
    if params.mode == 5u {
        // I;16 is uploaded as one numeric little-endian u16 per word.
        return fc & 0xffffu;
    }
    if params.mode == 7u || params.mode == 8u {
        // I and F retain the complete four-byte scalar representation.
        return fc;
    }
    let fr = fc & 0xffu;
    let fg = (fc >> 8u) & 0xffu;
    let fb = (fc >> 16u) & 0xffu;
    let fa = (fc >> 24u) & 0xffu;
    // Mode-aware: only keep channels present in the image mode
    let out_g = select(0u, fg, mode_has_g(params.mode));
    let out_b = select(0u, fb, mode_has_b(params.mode));
    let out_a = select(255u, fa, mode_has_a(params.mode));
    var out_fourth = out_a;
    if mode_has_fourth(params.mode) {
        out_fourth = fa;
    }
    if params.premultiply == 0u || out_a == 0u {
        return fr | (out_g << 8u) | (out_b << 16u) | (out_fourth << 24u);
    }
    let unpremul_r = min(fr * 255u / out_a, 255u);
    let unpremul_g = min(out_g * 255u / out_a, 255u);
    let unpremul_b = min(out_b * 255u / out_a, 255u);
    return unpremul_r | (unpremul_g << 8u) | (unpremul_b << 16u) | (out_fourth << 24u);
}


struct U64 {
    lo: u32,
    hi: u32,
}

fn u64_add(left: U64, right: U64) -> U64 {
    let lo = left.lo + right.lo;
    let carry = select(0u, 1u, lo < left.lo);
    return U64(lo, left.hi + right.hi + carry);
}

fn u64_less(left: U64, right: U64) -> bool {
    if left.hi != right.hi {
        return left.hi < right.hi;
    }
    return left.lo < right.lo;
}

fn u64_sub(left: U64, right: U64) -> U64 {
    let borrow = select(0u, 1u, left.lo < right.lo);
    return U64(left.lo - right.lo, left.hi - right.hi - borrow);
}

fn u64_mul_mantissa_weight(mantissa: u32, weight: u32) -> U64 {
    // Splitting both operands into 16-bit limbs keeps every intermediate
    // product and carry inside u32 while covering the complete 32x32-bit
    // product. The signed coefficient is converted to magnitude at the
    // callsite before entering this unsigned multiplication.
    let mantissa_lo = mantissa & 65535u;
    let mantissa_hi = mantissa >> 16u;
    let weight_lo = weight & 65535u;
    let weight_hi = weight >> 16u;
    let product0 = mantissa_lo * weight_lo;
    let product1 = mantissa_lo * weight_hi;
    let product2 = mantissa_hi * weight_lo;
    let product3 = mantissa_hi * weight_hi;
    let limb0 = product0 & 65535u;
    let limb1_sum = (product1 & 65535u) + (product2 & 65535u) + (product0 >> 16u);
    let limb1 = limb1_sum & 65535u;
    let carry1 = (product1 >> 16u) + (product2 >> 16u) + (limb1_sum >> 16u);
    let limb2_sum = (product3 & 65535u) + carry1;
    let limb2 = limb2_sum & 65535u;
    let limb3 = (product3 >> 16u) + (limb2_sum >> 16u);
    return U64(limb0 | (limb1 << 16u), limb2 | (limb3 << 16u));
}


struct U128 {
    a: u32,
    b: u32,
    c: u32,
    d: u32,
}

fn u128_add(left: U128, right: U128) -> U128 {
    let a = left.a + right.a;
    let carry_a = select(0u, 1u, a < left.a);
    let b0 = left.b + right.b;
    let carry_b0 = select(0u, 1u, b0 < left.b);
    let b = b0 + carry_a;
    let carry_b1 = select(0u, 1u, b < b0);
    let carry_b = carry_b0 + carry_b1;
    let c0 = left.c + right.c;
    let carry_c0 = select(0u, 1u, c0 < left.c);
    let c = c0 + carry_b;
    let carry_c1 = select(0u, 1u, c < c0);
    let carry_c = carry_c0 + carry_c1;
    let d0 = left.d + right.d;
    let d = d0 + carry_c;
    return U128(a, b, c, d);
}

fn u128_sub(left: U128, right: U128) -> U128 {
    let a = left.a - right.a;
    let borrow_a = select(0u, 1u, left.a < right.a);
    let b0 = left.b - right.b;
    let borrow_b0 = select(0u, 1u, left.b < right.b);
    let b = b0 - borrow_a;
    let borrow_b1 = select(0u, 1u, b0 < borrow_a);
    let borrow_b = borrow_b0 + borrow_b1;
    let c0 = left.c - right.c;
    let borrow_c0 = select(0u, 1u, left.c < right.c);
    let c = c0 - borrow_b;
    let borrow_c1 = select(0u, 1u, c0 < borrow_b);
    let borrow_c = borrow_c0 + borrow_c1;
    let d0 = left.d - right.d;
    let d = d0 - borrow_c;
    return U128(a, b, c, d);
}

fn u128_less(left: U128, right: U128) -> bool {
    if left.d != right.d {
        return left.d < right.d;
    }
    if left.c != right.c {
        return left.c < right.c;
    }
    if left.b != right.b {
        return left.b < right.b;
    }
    return left.a < right.a;
}

fn u128_shl(value: U128, shift: u32) -> U128 {
    if shift == 0u {
        return value;
    }
    if shift < 32u {
        return U128(
            value.a << shift,
            (value.b << shift) | (value.a >> (32u - shift)),
            (value.c << shift) | (value.b >> (32u - shift)),
            (value.d << shift) | (value.c >> (32u - shift)),
        );
    }
    if shift < 64u {
        let small = shift - 32u;
        if small == 0u {
            return U128(0u, value.a, value.b, value.c);
        }
        return U128(0u, value.a << small, (value.b << small) | (value.a >> (32u - small)),
            (value.c << small) | (value.b >> (32u - small)));
    }
    if shift < 96u {
        let small = shift - 64u;
        if small == 0u {
            return U128(0u, 0u, value.a, value.b);
        }
        return U128(0u, 0u, value.a << small, (value.b << small) | (value.a >> (32u - small)));
    }
    if shift < 128u {
        return U128(0u, 0u, 0u, value.a << (shift - 96u));
    }
    return U128(0u, 0u, 0u, 0u);
}

fn u128_shr(value: U128, shift: u32) -> U128 {
    if shift == 0u {
        return value;
    }
    if shift < 32u {
        return U128(
            (value.a >> shift) | (value.b << (32u - shift)),
            (value.b >> shift) | (value.c << (32u - shift)),
            (value.c >> shift) | (value.d << (32u - shift)),
            value.d >> shift,
        );
    }
    if shift < 64u {
        let small = shift - 32u;
        if small == 0u {
            return U128(value.b, value.c, value.d, 0u);
        }
        return U128((value.b >> small) | (value.c << (32u - small)),
            (value.c >> small) | (value.d << (32u - small)), value.d >> small, 0u);
    }
    if shift < 96u {
        let small = shift - 64u;
        if small == 0u {
            return U128(value.c, value.d, 0u, 0u);
        }
        return U128((value.c >> small) | (value.d << (32u - small)), value.d >> small, 0u, 0u);
    }
    if shift < 128u {
        return U128(value.d >> (shift - 96u), 0u, 0u, 0u);
    }
    return U128(0u, 0u, 0u, 0u);
}

fn u128_bit_length(value: U128) -> u32 {
    if value.d != 0u {
        return 96u + 32u - countLeadingZeros(value.d);
    }
    if value.c != 0u {
        return 64u + 32u - countLeadingZeros(value.c);
    }
    if value.b != 0u {
        return 32u + 32u - countLeadingZeros(value.b);
    }
    if value.a != 0u {
        return 32u - countLeadingZeros(value.a);
    }
    return 0u;
}

fn u128_low_bits(value: U128, bits: u32) -> U128 {
    if bits == 0u {
        return U128(0u, 0u, 0u, 0u);
    }
    if bits < 32u {
        return U128(value.a & ((1u << bits) - 1u), 0u, 0u, 0u);
    }
    if bits == 32u {
        return U128(value.a, 0u, 0u, 0u);
    }
    if bits < 64u {
        return U128(value.a, value.b & ((1u << (bits - 32u)) - 1u), 0u, 0u);
    }
    if bits == 64u {
        return U128(value.a, value.b, 0u, 0u);
    }
    if bits < 96u {
        return U128(value.a, value.b, value.c & ((1u << (bits - 64u)) - 1u), 0u);
    }
    if bits == 96u {
        return U128(value.a, value.b, value.c, 0u);
    }
    if bits < 128u {
        return U128(value.a, value.b, value.c, value.d & ((1u << (bits - 96u)) - 1u));
    }
    return value;
}

struct SignedU128 {
    magnitude: U128,
    negative: bool,
}

fn signed_u128_add(sum: SignedU128, term: U128, term_negative: bool) -> SignedU128 {
    if term.a == 0u && term.b == 0u && term.c == 0u && term.d == 0u {
        return sum;
    }
    if sum.magnitude.a == 0u && sum.magnitude.b == 0u && sum.magnitude.c == 0u && sum.magnitude.d == 0u {
        return SignedU128(term, term_negative);
    }
    if sum.negative == term_negative {
        return SignedU128(u128_add(sum.magnitude, term), sum.negative);
    }
    if u128_less(sum.magnitude, term) {
        return SignedU128(u128_sub(term, sum.magnitude), term_negative);
    }
    return SignedU128(u128_sub(sum.magnitude, term), sum.negative);
}


fn f64_sum_to_f32(sum: SignedU128, minimum_exponent: i32) -> u32 {
    let bit_length = u128_bit_length(sum.magnitude);
    if bit_length == 0u {
        return 0u;
    }
    var exponent = minimum_exponent + i32(bit_length) - 1;

    if exponent > 127 {
        return select(0x7f800000u, 0xff800000u, sum.negative);
    }

    // Round values below the normal range in units of 2^-149.  The host
    // admission proof bounds the integer sum, so the same limb operations are
    // sufficient for subnormal output without device floating-point arithmetic.
    if exponent < -126 {
        let target_shift = minimum_exponent + 149;
        var subnormal: U128;
        if target_shift >= 0 {
            subnormal = u128_shl(sum.magnitude, u32(target_shift));
        } else {
            let shift = u32(-target_shift);
            if shift >= 128u {
                subnormal = U128(0u, 0u, 0u, 0u);
            } else if shift == 0u {
                subnormal = sum.magnitude;
            } else {
                subnormal = u128_shr(sum.magnitude, shift);
                let remainder = u128_low_bits(sum.magnitude, shift);
                let halfway = u128_shl(U128(1u, 0u, 0u, 0u), shift - 1u);
                let greater = u128_less(halfway, remainder);
                let equal = remainder.a == halfway.a && remainder.b == halfway.b
                    && remainder.c == halfway.c && remainder.d == halfway.d;
                if greater || (equal && (subnormal.a & 1u) != 0u) {
                    subnormal = u128_add(subnormal, U128(1u, 0u, 0u, 0u));
                }
            }
        }
        // Rounding the largest subnormal upward produces the smallest normal.
        if !u128_less(subnormal, U128(0x00800000u, 0u, 0u, 0u)) {
            return select(0x00800000u, 0x80800000u, sum.negative);
        }
        return select(subnormal.a, subnormal.a | 0x80000000u, sum.negative);
    }
    var mantissa: u32;
    if bit_length > 24u {
        let shift = bit_length - 24u;
        var rounded = u128_shr(sum.magnitude, shift).a;
        let remainder = u128_low_bits(sum.magnitude, shift);
        let halfway = u128_shl(U128(1u, 0u, 0u, 0u), shift - 1u);
        let greater = u128_less(halfway, remainder);
        let equal = remainder.a == halfway.a && remainder.b == halfway.b
            && remainder.c == halfway.c && remainder.d == halfway.d;
        if greater || (equal && (rounded & 1u) != 0u) {
            rounded = rounded + 1u;
        }
        mantissa = rounded;
        if mantissa == (1u << 24u) {
            mantissa = mantissa >> 1u;
            exponent = exponent + 1;
        }
    } else {
        mantissa = u128_shl(sum.magnitude, 24u - bit_length).a;
    }
    if exponent > 127 {
        return select(0x7f800000u, 0xff800000u, sum.negative);
    }
    let result = (u32(exponent + 127) << 23u) | (mantissa & 0x7fffffu);
    if sum.negative {
        return result | 0x80000000u;
    }
    return result;
}


struct F64OrderedState {
    magnitude: U128,
    exponent: i32,
    negative: bool,
    valid: bool,
}

fn f64_ordered_round(sum: SignedU128, scale_exp: i32) -> F64OrderedState {
    if sum.magnitude.a == 0u && sum.magnitude.b == 0u
        && sum.magnitude.c == 0u && sum.magnitude.d == 0u {
        return F64OrderedState(U128(0u, 0u, 0u, 0u), 0, false, true);
    }
    let bit_length = u128_bit_length(sum.magnitude);
    var exponent = scale_exp + i32(bit_length) - 1;
    if exponent < -1022 || exponent > 1023 {
        return F64OrderedState(U128(0u, 0u, 0u, 0u), 0, false, false);
    }
    var mantissa: U128;
    if bit_length > 53u {
        let shift = bit_length - 53u;
        mantissa = u128_shr(sum.magnitude, shift);
        let remainder = u128_low_bits(sum.magnitude, shift);
        let halfway = u128_shl(U128(1u, 0u, 0u, 0u), shift - 1u);
        let greater = u128_less(halfway, remainder);
        let equal = remainder.a == halfway.a && remainder.b == halfway.b
            && remainder.c == halfway.c && remainder.d == halfway.d;
        if greater || (equal && (mantissa.a & 1u) != 0u) {
            mantissa = u128_add(mantissa, U128(1u, 0u, 0u, 0u));
        }
    } else {
        mantissa = u128_shl(sum.magnitude, 53u - bit_length);
    }
    if mantissa.a == 0u && mantissa.b == 0x00200000u
        && mantissa.c == 0u && mantissa.d == 0u {
        mantissa = u128_shr(mantissa, 1u);
        exponent = exponent + 1;
        if exponent > 1023 {
            return F64OrderedState(U128(0u, 0u, 0u, 0u), 0, false, false);
        }
    }
    return F64OrderedState(mantissa, exponent - 52, sum.negative, true);
}

fn f64_align_jammed(magnitude: U128, shift: i32) -> U128 {
    if shift >= 0 {
        return u128_shl(magnitude, u32(shift));
    }
    let discarded = u128_low_bits(magnitude, u32(-shift));
    var result = u128_shr(magnitude, u32(-shift));
    if discarded.a != 0u || discarded.b != 0u || discarded.c != 0u || discarded.d != 0u {
        result.a = result.a | 1u;
    }
    return result;
}


// A finite value uses magnitude * 2^exponent. Special values retain their
// FLOAT32 payload in magnitude.a with valid=false. All geometry fractions
// and finite f32 polynomial intermediates are normal binary64 or zero.
fn number(bits: u32) -> F64OrderedState {
    let negative = (bits & 0x80000000u) != 0u;
    let exponent = (bits >> 23u) & 255u;
    if exponent == 255u {
        return F64OrderedState(U128(bits, 0u, 0u, 0u), 0, negative, false);
    }
    if exponent == 0u {
        return F64OrderedState(U128(bits & 0x7fffffu, 0u, 0u, 0u), -149, negative, true);
    }
    return F64OrderedState(U128((bits & 0x7fffffu) | 0x800000u, 0u, 0u, 0u), i32(exponent) - 150, negative, true);
}
fn fraction(lo: u32, hi: u32) -> F64OrderedState {
    let exponent = (hi >> 20u) & 2047u;
    return F64OrderedState(U128(lo, (hi & 0xfffffu) | select(0u, 0x100000u, exponent != 0u), 0u, 0u),
        select(-1074, i32(exponent) - 1075, exponent != 0u), false, true);
}
fn zero() -> F64OrderedState { return number(0u); }
fn negate(value: F64OrderedState) -> F64OrderedState {
    var result = value;
    result.negative = !result.negative;
    if !result.valid { result.magnitude.a = result.magnitude.a ^ 0x80000000u; }
    return result;
}
fn is_zero(value: F64OrderedState) -> bool {
    return value.valid && u128_bit_length(value.magnitude) == 0u;
}
fn is_nan(value: F64OrderedState) -> bool {
    return !value.valid && (value.magnitude.a & 0x7fffffu) != 0u;
}
fn is_signaling(value: F64OrderedState) -> bool {
    return is_nan(value) && (value.magnitude.a & 0x00400000u) == 0u;
}
fn promote(value: F64OrderedState) -> F64OrderedState {
    if is_nan(value) { return quiet(value); }
    return value;
}
fn quiet(value: F64OrderedState) -> F64OrderedState {
    var result = value;
    result.magnitude.a = result.magnitude.a | 0x00400000u;
    return result;
}
fn product53(left: U128, right: U128) -> U128 {
    let a = u64_mul_mantissa_weight(left.a, right.a);
    let b = u64_mul_mantissa_weight(left.a, right.b);
    let c = u64_mul_mantissa_weight(left.b, right.a);
    let d = u64_mul_mantissa_weight(left.b, right.b);
    return u128_add(u128_add(U128(a.lo, a.hi, d.lo, d.hi), U128(0u, b.lo, b.hi, 0u)), U128(0u, c.lo, c.hi, 0u));
}
fn fused(left: F64OrderedState, right: F64OrderedState, addend: F64OrderedState) -> F64OrderedState {
    // AArch64 FPMulAdd processes the addend before the two multiplicands
    // when propagating quiet NaNs (Geometry.c's fused Horner evaluation).
    if is_signaling(addend) { return quiet(addend); }
    if is_signaling(left) { return quiet(left); }
    if is_signaling(right) { return quiet(right); }
    if is_nan(addend) { return quiet(addend); }
    if is_nan(left) { return quiet(left); }
    if is_nan(right) { return quiet(right); }
    let negative = left.negative != right.negative;
    if !left.valid || !right.valid {
        if is_zero(left) || is_zero(right) || (!addend.valid && negative != addend.negative) {
            return number(0x7fc00000u);
        }
        return number(select(0x7f800000u, 0xff800000u, negative));
    }
    if !addend.valid { return addend; }
    let product = product53(left.magnitude, right.magnitude);
    let product_exp = left.exponent + right.exponent;
    if u128_bit_length(product) == 0u {
        if is_zero(addend) { return number(select(0u, 0x80000000u, negative && addend.negative)); }
        return addend;
    }
    if is_zero(addend) { return f64_ordered_round(SignedU128(product, negative), product_exp); }
    // Two binary64 mantissas produce at most 106 significant bits. Keeping
    // 127 aligned bits leaves every cancellation bit plus a sticky tail;
    // round-to-odd therefore preserves the correctly rounded FMA result.
    let high_bit = max(product_exp + i32(u128_bit_length(product)), addend.exponent + i32(u128_bit_length(addend.magnitude)));
    let exponent = max(min(product_exp, addend.exponent), high_bit - 127);
    let sum = signed_u128_add(SignedU128(f64_align_jammed(product, product_exp - exponent), negative),
        f64_align_jammed(addend.magnitude, addend.exponent - exponent), addend.negative);
    return f64_ordered_round(sum, exponent);
}
fn plus(left: F64OrderedState, right: F64OrderedState) -> F64OrderedState {
    if is_signaling(left) { return quiet(left); }
    if is_signaling(right) { return quiet(right); }
    if is_nan(left) { return quiet(left); }
    if is_nan(right) { return quiet(right); }
    return fused(left, number(0x3f800000u), right);
}
fn minus(left: F64OrderedState, right: F64OrderedState) -> F64OrderedState {
    // ARM FSUB propagates a NaN operand before applying subtraction's sign;
    // rewriting subtraction as addition of a negated NaN changes its bits.
    if is_signaling(left) { return quiet(left); }
    if is_signaling(right) { return quiet(right); }
    if is_nan(left) { return quiet(left); }
    if is_nan(right) { return quiet(right); }
    return plus(left, negate(right));
}
fn float_bits(value: F64OrderedState) -> u32 {
    if !value.valid { return value.magnitude.a; }
    if is_zero(value) { return select(0u, 0x80000000u, value.negative); }
    return f64_sum_to_f32(SignedU128(value.magnitude, value.negative), value.exponent);
}
fn float_store(value: F64OrderedState) -> F64OrderedState { return number(float_bits(value)); }
fn coefficient_sub(left: F64OrderedState, right: F64OrderedState, typed: bool) -> F64OrderedState {
    let value = minus(left, right);
    if typed { return float_store(value); }
    return value;
}
fn cubic(samples: array<F64OrderedState, 4>, distance: F64OrderedState, typed: bool) -> F64OrderedState {
    let p1 = promote(samples[1]);
    let p2 = coefficient_sub(samples[2], samples[0], typed);
    var p3 = fused(coefficient_sub(samples[0], samples[1], typed), number(0x40000000u), samples[2]);
    if typed { p3 = float_store(p3); }
    p3 = coefficient_sub(p3, samples[3], typed);
    let p4 = coefficient_sub(coefficient_sub(samples[1], samples[0], typed), samples[2], typed);
    var p4_final = plus(p4, samples[3]);
    if typed { p4_final = float_store(p4_final); }
    return fused(distance, fused(distance, fused(distance, p4_final, p3), p2), p1);
}
fn channel_sample(word: u32, channel: u32) -> F64OrderedState {
    if params.mode == 8u { return number(word); }
    var value = (word >> channel) & 255u;
    if params.premultiply != 0u && channel != 24u {
        value = (value * (word >> 24u) + 127u) / 255u;
    }
    return number(bitcast<u32>(f32(value)));
}
fn sample(base: u32, x: u32, y: u32, channel: u32) -> F64OrderedState {
    return channel_sample(input[geometry[base + 1u + x] + geometry[base + 5u + y]], channel);
}
fn interpolate(base: u32, channel: u32, dx: F64OrderedState, dy: F64OrderedState) -> F64OrderedState {
    let typed = params.mode == 8u;
    if params.filter_code == 1u {
        let p00 = sample(base, 0u, 0u, channel);
        let p10 = sample(base, 1u, 0u, channel);
        let p01 = sample(base, 0u, 1u, channel);
        let p11 = sample(base, 1u, 1u, channel);
        let top = fused(coefficient_sub(p10, p00, typed), dx, promote(p00));
        let bottom = fused(coefficient_sub(p11, p01, typed), dx, promote(p01));
        return fused(minus(bottom, top), dy, top);
    }
    var rows: array<F64OrderedState, 4>;
    for (var y = 0u; y < 4u; y = y + 1u) {
        rows[y] = cubic(array<F64OrderedState, 4>(sample(base, 0u, y, channel), sample(base, 1u, y, channel),
            sample(base, 2u, y, channel), sample(base, 3u, y, channel)), dx, typed);
    }
    return cubic(rows, dy, false);
}
fn byte_store(value: F64OrderedState) -> u32 {
    if value.negative || is_zero(value) { return 0u; }
    if !value.valid { return select(255u, 0u, is_nan(value)); }
    let length = i32(u128_bit_length(value.magnitude)) + value.exponent;
    if length > 8 { return 255u; }
    if length <= 0 { return 0u; }
    if value.exponent >= 0 { return min(u128_shl(value.magnitude, u32(value.exponent)).a, 255u); }
    return min(u128_shr(value.magnitude, u32(-value.exponent)).a, 255u);
}
@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.dst_w || gid.y >= params.dst_h { return; }
    let index = gid.y * params.dst_w + gid.x;
    let base = index * 13u;
    if geometry[base] == 0u { output[index] = get_fill_pixel(); return; }
    if geometry[base] == 2u { output[index] = 0u; return; }
    if params.filter_code == 0u { output[index] = input[geometry[base + 1u]]; return; }
    let dx = fraction(geometry[base + 9u], geometry[base + 10u]);
    let dy = fraction(geometry[base + 11u], geometry[base + 12u]);
    if params.mode == 8u { output[index] = float_bits(interpolate(base, 0u, dx, dy)); return; }
    var r = byte_store(interpolate(base, 0u, dx, dy));
    var g = 0u;
    var b = 0u;
    var a = 255u;
    if mode_has_g(params.mode) { g = byte_store(interpolate(base, 8u, dx, dy)); }
    if mode_has_b(params.mode) { b = byte_store(interpolate(base, 16u, dx, dy)); }
    if mode_has_a(params.mode) || mode_has_fourth(params.mode) { a = byte_store(interpolate(base, 24u, dx, dy)); }
    if params.premultiply != 0u && a != 0u {
        r = min(r * 255u / a, 255u);
        g = min(g * 255u / a, 255u);
        b = min(b * 255u / a, 255u);
    }
    output[index] = r | (g << 8u) | (b << 16u) | (a << 24u);
}
