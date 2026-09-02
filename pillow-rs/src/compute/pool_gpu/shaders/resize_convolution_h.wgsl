// Exact horizontal pass for Pillow's separable byte resize.
//
// The host builds the same 22-bit fixed-point coefficient tables used by the
// CPU and SIMD implementations.  The storage table is laid out as three
// i32/u32 metadata words per output column (xmin, count, weight offset),
// followed by the flattened signed weights.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
    dst_w: u32,
    dst_h: u32,
    channels: u32,
    premultiply: u32,
}

const FIXED_BIAS: i32 = 2097152;

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;
@group(0) @binding(3) var<storage, read> coefficients: array<i32>;

fn pixel_channel(pixel: u32, channel: u32) -> u32 {
    // LA is transported as RGBA: logical alpha is stored in packed byte 3,
    // not byte 1.  Keep the logical channel numbering used by the coefficient
    // path while preserving the native packed upload representation.
    if params.channels == 2u && channel == 1u {
        return (pixel >> 24u) & 255u;
    }
    return (pixel >> (channel * 8u)) & 255u;
}

fn luma16_sample(word: u32) -> u32 {
    let low = word & 255u;
    let high = (word >> 8u) & 255u;
    // The uploader stores the declared byte sequence in the low two bytes of
    // each word. It is already the sequence consumed by Pillow's byte-aware
    // I;16 resampler, so decode those bytes directly. The final store always
    // packs the result as the little-endian transport word.
    return low | (high << 8u);
}

fn f64_sample_bits(word: u32) -> u32 {
    if params.mode == 5u {
        return bitcast<u32>(f32(luma16_sample(word)));
    }
    return word;
}

fn premultiply(value: u32, alpha: u32) -> u32 {
    return (value * alpha + 127u) / 255u;
}

fn fixed_to_byte(sum: i32) -> u32 {
    let value = (sum + FIXED_BIAS) >> 22;
    return u32(clamp(value, 0, 255));
}

fn filtered_channel(source_y: u32, output_x: u32, channel: u32) -> u32 {
    let metadata = output_x * 3u;
    let source_x = u32(coefficients[metadata]);
    let count = u32(coefficients[metadata + 1u]);
    let weight_base = 3u * params.dst_w + u32(coefficients[metadata + 2u]);
    var sum: i32 = 0;
    for (var tap = 0u; tap < count; tap = tap + 1u) {
        let pixel = input[source_y * params.width + source_x + tap];
        var value = pixel_channel(pixel, channel);
        if params.premultiply != 0u && channel + 1u < params.channels {
            value = premultiply(value, pixel_channel(pixel, params.channels - 1u));
        }
        sum = sum + i32(value) * coefficients[weight_base + tap];
    }
    return fixed_to_byte(sum);
}

fn filtered_float(source_y: u32, output_x: u32) -> f32 {
    let metadata = output_x * 3u;
    let source_x = u32(coefficients[metadata]);
    let count = u32(coefficients[metadata + 1u]);
    let weight_base = 3u * params.dst_w + u32(coefficients[metadata + 2u]);
    var sum: f32 = 0.0;
    for (var tap = 0u; tap < count; tap = tap + 1u) {
        let sample = bitcast<f32>(input[source_y * params.width + source_x + tap]);
        sum = sum + sample * f32(coefficients[weight_base + tap]) / 4194304.0;
    }
    return sum;
}

// Marker 6 uses this integer-only reduction after the host has proved that
// every selected source word is a finite normal f32, the fixed coefficients
// equal Pillow's f64 table, and every aligned signed partial sum fits 53 bits.
// Keeping the product and accumulation out of f32 avoids adapter-specific
// relaxed-f32 rounding, including ringing-filter and mixed-sign cancellation.
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

fn u64_shl(value: U64, shift: u32) -> U64 {
    if shift == 0u {
        return value;
    }
    if shift < 32u {
        return U64(value.lo << shift, (value.hi << shift) | (value.lo >> (32u - shift)));
    }
    if shift < 64u {
        return U64(0u, value.lo << (shift - 32u));
    }
    return U64(0u, 0u);
}

fn u64_shr(value: U64, shift: u32) -> U64 {
    if shift == 0u {
        return value;
    }
    if shift < 32u {
        return U64((value.lo >> shift) | (value.hi << (32u - shift)), value.hi >> shift);
    }
    if shift < 64u {
        return U64(value.hi >> (shift - 32u), 0u);
    }
    return U64(0u, 0u);
}

fn u64_bit_length(value: U64) -> u32 {
    if value.hi != 0u {
        return 64u - countLeadingZeros(value.hi);
    }
    if value.lo != 0u {
        return 32u - countLeadingZeros(value.lo);
    }
    return 0u;
}

fn u64_low_bits(value: U64, bits: u32) -> U64 {
    if bits == 0u {
        return U64(0u, 0u);
    }
    if bits < 32u {
        return U64(value.lo & ((1u << bits) - 1u), 0u);
    }
    if bits < 64u {
        return U64(value.lo, value.hi & ((1u << (bits - 32u)) - 1u));
    }
    return value;
}

struct SignedU64 {
    magnitude: U64,
    negative: bool,
}

fn signed_u64_add(sum: SignedU64, term: U64, term_negative: bool) -> SignedU64 {
    if term.lo == 0u && term.hi == 0u {
        return sum;
    }
    if sum.magnitude.lo == 0u && sum.magnitude.hi == 0u {
        return SignedU64(term, term_negative);
    }
    if sum.negative == term_negative {
        return SignedU64(u64_add(sum.magnitude, term), sum.negative);
    }
    if u64_less(sum.magnitude, term) {
        return SignedU64(u64_sub(term, sum.magnitude), term_negative);
    }
    return SignedU64(u64_sub(sum.magnitude, term), sum.negative);
}

fn integer_sum_to_f32(sum: SignedU64, minimum_exponent: i32) -> u32 {
    let bit_length = u64_bit_length(sum.magnitude);
    if bit_length == 0u {
        return 0u;
    }
    var exponent = minimum_exponent - 45 + i32(bit_length) - 1;
    var mantissa: u32;
    if bit_length > 24u {
        let shift = bit_length - 24u;
        mantissa = u64_shr(sum.magnitude, shift).lo;
        let remainder = u64_low_bits(sum.magnitude, shift);
        let halfway = u64_shl(U64(1u, 0u), shift - 1u);
        let greater = remainder.hi > halfway.hi
            || (remainder.hi == halfway.hi && remainder.lo > halfway.lo);
        let equal = remainder.hi == halfway.hi && remainder.lo == halfway.lo;
        if greater || (equal && (mantissa & 1u) != 0u) {
            mantissa = mantissa + 1u;
            if mantissa == (1u << 24u) {
                mantissa = mantissa >> 1u;
                exponent = exponent + 1;
            }
        }
    } else {
        mantissa = sum.magnitude.lo << (24u - bit_length);
    }
    let result = (u32(exponent + 127) << 23u) | (mantissa & 0x7fffffu);
    if sum.negative {
        return result | 0x80000000u;
    }
    return result;
}

fn filtered_integer_exact(source_y: u32, output_x: u32) -> u32 {
    let metadata = output_x * 3u;
    let source_x = u32(coefficients[metadata]);
    let count = u32(coefficients[metadata + 1u]);
    let weight_base = 3u * params.dst_w + u32(coefficients[metadata + 2u]);
    var minimum_exponent: i32 = 128;
    var found = false;
    for (var tap = 0u; tap < count; tap = tap + 1u) {
        let weight = coefficients[weight_base + tap];
        if weight == 0i {
            continue;
        }
        let bits = f64_sample_bits(input[source_y * params.width + source_x + tap]);
        if ((bits & 0x7fffffffu) == 0u) {
            continue;
        }
        let exponent = i32((bits >> 23u) & 255u) - 127;
        if !found {
            minimum_exponent = exponent;
            found = true;
        } else {
            minimum_exponent = min(minimum_exponent, exponent);
        }
    }
    if !found {
        return 0u;
    }
    var sum = SignedU64(U64(0u, 0u), false);
    for (var tap = 0u; tap < count; tap = tap + 1u) {
        let signed_weight = coefficients[weight_base + tap];
        if signed_weight == 0i {
            continue;
        }
        let bits = f64_sample_bits(input[source_y * params.width + source_x + tap]);
        if (bits & 0x7fffffffu) == 0u {
            continue;
        }
        let exponent = i32((bits >> 23u) & 255u) - 127;
        let mantissa = (bits & 0x7fffffu) | 0x800000u;
        let weight_bits = bitcast<u32>(signed_weight);
        let weight_negative = signed_weight < 0i;
        let weight = select(weight_bits, 0u - weight_bits, weight_negative);
        let product = u64_mul_mantissa_weight(mantissa, weight);
        let term = u64_shl(product, u32(exponent - minimum_exponent));
        let sample_negative = (bits & 0x80000000u) != 0u;
        sum = signed_u64_add(sum, term, sample_negative != weight_negative);
    }
    return integer_sum_to_f32(sum, minimum_exponent);
}

// Marker 9 carries Pillow's f64 coefficient table as dyadic integer parts.
// The reducer keeps the complete exact product/sum in four u32 limbs and
// rounds only once at the observable f32 store.  The host admission proof
// rejects rows whose ordered f64 FMA rounding would change that final word.
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

struct F64Coeff {
    mantissa_lo: u32,
    mantissa_hi: u32,
    exponent: i32,
    negative: bool,
}

fn f64_coeff(index: u32) -> F64Coeff {
    // Callers pass the word offset of the four-word coefficient group.
    let base = index;
    return F64Coeff(
        bitcast<u32>(coefficients[base]),
        bitcast<u32>(coefficients[base + 1u]),
        coefficients[base + 2u],
        coefficients[base + 3u] != 0i,
    );
}

fn f64_product(sample_mantissa: u32, coeff: F64Coeff) -> U128 {
    let low = u64_mul_mantissa_weight(sample_mantissa, coeff.mantissa_lo);
    let high = u64_mul_mantissa_weight(sample_mantissa, coeff.mantissa_hi);
    return u128_add(U128(low.lo, low.hi, 0u, 0u), U128(0u, high.lo, high.hi, 0u));
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

// Marker 12 carries a bounded ordered-f64 reducer.  Products are formed as
// exact integer mantissa/exponent pairs, then the accumulator is rounded to a
// normal binary64 value after every tap to match Pillow's `f64::mul_add`
// sequence.  The host admission proof limits this path to rows with at most
// two taps and finite normal intermediates; all wider or exceptional rows use
// marker 9 or exact host semantic control.
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

fn f64_ordered_add_product(
    state: F64OrderedState,
    product: U128,
    product_exp: i32,
    product_negative: bool,
) -> F64OrderedState {
    if !state.valid {
        return state;
    }
    if product.a == 0u && product.b == 0u && product.c == 0u && product.d == 0u {
        return state;
    }
    if state.magnitude.a == 0u && state.magnitude.b == 0u
        && state.magnitude.c == 0u && state.magnitude.d == 0u {
        return f64_ordered_round(SignedU128(product, product_negative), product_exp);
    }
    let minimum_exponent = min(state.exponent, product_exp);
    let state_shift = u32(state.exponent - minimum_exponent);
    let product_shift = u32(product_exp - minimum_exponent);
    if u128_bit_length(state.magnitude) + state_shift > 128u
        || u128_bit_length(product) + product_shift > 128u {
        return F64OrderedState(U128(0u, 0u, 0u, 0u), 0, false, false);
    }
    var sum = SignedU128(
        u128_shl(state.magnitude, state_shift),
        state.negative,
    );
    sum = signed_u128_add(sum, u128_shl(product, product_shift), product_negative);
    return f64_ordered_round(sum, minimum_exponent);
}

fn filtered_f64_ordered_2tap(source_y: u32, output_x: u32) -> u32 {
    let metadata = output_x * 3u;
    let source_x = u32(coefficients[metadata]);
    let count = u32(coefficients[metadata + 1u]);
    let weight_base = 3u * params.dst_w + u32(coefficients[metadata + 2u]);
    if count > 2u {
        return 0u;
    }
    var state = F64OrderedState(U128(0u, 0u, 0u, 0u), 0, false, true);
    for (var tap = 0u; tap < count; tap = tap + 1u) {
        let coeff = f64_coeff(weight_base + tap * 4u);
        let bits = f64_sample_bits(input[source_y * params.width + source_x + tap]);
        let exponent_bits = (bits >> 23u) & 255u;
        if exponent_bits == 255u {
            return 0u;
        }
        if exponent_bits == 0u {
            if (bits & 0x7fffffu) != 0u {
                return 0u;
            }
            continue;
        }
        let sample_mantissa = (bits & 0x7fffffu) | 0x800000u;
        let product = f64_product(sample_mantissa, coeff);
        let sample_exp = i32(exponent_bits) - 127 - 23;
        let product_exp = sample_exp + coeff.exponent;
        let sample_negative = (bits & 0x80000000u) != 0u;
        state = f64_ordered_add_product(
            state,
            product,
            product_exp,
            sample_negative != coeff.negative,
        );
    }
    if !state.valid {
        return 0u;
    }
    return f64_sum_to_f32(SignedU128(state.magnitude, state.negative), state.exponent);
}

fn filtered_f64_exact(source_y: u32, output_x: u32) -> u32 {
    let metadata = output_x * 3u;
    let source_x = u32(coefficients[metadata]);
    let count = u32(coefficients[metadata + 1u]);
    let weight_base = 3u * params.dst_w + u32(coefficients[metadata + 2u]);

    // Handle IEEE special products without going through relaxed f32
    // arithmetic.  Pillow's ordered f64 path preserves the first NaN payload,
    // turns zero*infinity and opposite infinities into a quiet NaN, and keeps
    // the sign of a lone infinity.  The host admission proof compares these
    // bits with Pillow before selecting this marker.
    var first_nan: u32 = 0u;
    var has_nan = false;
    var positive_infinity = false;
    var negative_infinity = false;
    for (var tap = 0u; tap < count; tap = tap + 1u) {
        let coeff = f64_coeff(weight_base + tap * 4u);
        let bits = f64_sample_bits(input[source_y * params.width + source_x + tap]);
        let exponent_bits = (bits >> 23u) & 255u;
        if exponent_bits != 255u {
            continue;
        }
        let fraction = bits & 0x7fffffu;
        if fraction != 0u {
            if !has_nan {
                first_nan = bits | 0x00400000u;
                has_nan = true;
            }
        } else if coeff.mantissa_lo == 0u && coeff.mantissa_hi == 0u {
            // 0 * infinity is invalid and Pillow stores the canonical quiet
            // f32 NaN for this operation.
            if !has_nan {
                first_nan = 0x7fc00000u;
                has_nan = true;
            }
        } else if (bits & 0x80000000u) != 0u {
            if coeff.negative {
                positive_infinity = true;
            } else {
                negative_infinity = true;
            }
        } else if coeff.negative {
            negative_infinity = true;
        } else {
            positive_infinity = true;
        }
    }
    if has_nan {
        return first_nan;
    }
    if positive_infinity && negative_infinity {
        return 0x7fc00000u;
    }
    if positive_infinity {
        return 0x7f800000u;
    }
    if negative_infinity {
        return 0xff800000u;
    }

    var minimum_exponent: i32 = 0;
    var found = false;
    for (var tap = 0u; tap < count; tap = tap + 1u) {
        let coeff = f64_coeff(weight_base + tap * 4u);
        if coeff.mantissa_lo == 0u && coeff.mantissa_hi == 0u {
            continue;
        }
        let bits = f64_sample_bits(input[source_y * params.width + source_x + tap]);
        if (bits & 0x7fffffffu) == 0u {
            continue;
        }
        let exponent_bits = (bits >> 23u) & 255u;
        let sample_exponent = select(
            i32(exponent_bits) - 127 - 23,
            -149,
            exponent_bits == 0u,
        );
        let exponent = sample_exponent + coeff.exponent;
        if !found {
            minimum_exponent = exponent;
            found = true;
        } else {
            minimum_exponent = min(minimum_exponent, exponent);
        }
    }
    if !found {
        return 0u;
    }
    var sum = SignedU128(U128(0u, 0u, 0u, 0u), false);
    for (var tap = 0u; tap < count; tap = tap + 1u) {
        let coeff = f64_coeff(weight_base + tap * 4u);
        if coeff.mantissa_lo == 0u && coeff.mantissa_hi == 0u {
            continue;
        }
        let bits = f64_sample_bits(input[source_y * params.width + source_x + tap]);
        if (bits & 0x7fffffffu) == 0u {
            continue;
        }
        let exponent_bits = (bits >> 23u) & 255u;
        let sample_exponent = select(
            i32(exponent_bits) - 127 - 23,
            -149,
            exponent_bits == 0u,
        );
        let sample_mantissa = select(
            (bits & 0x7fffffu) | 0x800000u,
            bits & 0x7fffffu,
            exponent_bits == 0u,
        );
        let product = f64_product(sample_mantissa, coeff);
        let term = u128_shl(product, u32(sample_exponent + coeff.exponent - minimum_exponent));
        let sample_negative = (bits & 0x80000000u) != 0u;
        sum = signed_u128_add(sum, term, sample_negative != coeff.negative);
    }
    return f64_sum_to_f32(sum, minimum_exponent);
}

// Marker 11 is the typed INT32 resize path. Each source word is a signed
// i32, while the coefficient table carries exact binary f64 mantissas. Keep
// the complete weighted sum in the existing four-limb reducer and round once
// away from zero at Pillow's INT32 storage boundary.
fn integer_sum_to_i32(sum: SignedU128, minimum_exponent: i32) -> u32 {
    if sum.magnitude.a == 0u && sum.magnitude.b == 0u
        && sum.magnitude.c == 0u && sum.magnitude.d == 0u {
        return 0u;
    }
    var rounded: U128;
    if minimum_exponent >= 0 {
        rounded = u128_shl(sum.magnitude, u32(minimum_exponent));
    } else {
        let shift = u32(-minimum_exponent);
        if shift >= 128u {
            rounded = U128(0u, 0u, 0u, 0u);
        } else if shift == 0u {
            rounded = sum.magnitude;
        } else {
            rounded = u128_shr(sum.magnitude, shift);
            let remainder = u128_low_bits(sum.magnitude, shift);
            let halfway = u128_shl(U128(1u, 0u, 0u, 0u), shift - 1u);
            // INT32 ROUND_UP is away from zero, including exact ties.
            if !u128_less(remainder, halfway) {
                rounded = u128_add(rounded, U128(1u, 0u, 0u, 0u));
            }
        }
    }
    // The host proof rejects rows outside the signed i32 range. A low-word
    // conversion is therefore sufficient, including i32::MIN as 0x80000000.
    return select(rounded.a, 0u - rounded.a, sum.negative);
}

fn filtered_i32_exact(source_y: u32, output_x: u32) -> u32 {
    let metadata = output_x * 3u;
    let source_x = u32(coefficients[metadata]);
    let count = u32(coefficients[metadata + 1u]);
    let weight_base = 3u * params.dst_w + u32(coefficients[metadata + 2u]);
    var minimum_exponent: i32 = 0;
    var found = false;
    for (var tap = 0u; tap < count; tap = tap + 1u) {
        let coeff = f64_coeff(weight_base + tap * 4u);
        let word = input[source_y * params.width + source_x + tap];
        let negative = (word & 0x80000000u) != 0u;
        let magnitude = select(word, 0u - word, negative);
        if coeff.mantissa_lo == 0u && coeff.mantissa_hi == 0u || magnitude == 0u {
            continue;
        }
        if !found {
            minimum_exponent = coeff.exponent;
            found = true;
        } else {
            minimum_exponent = min(minimum_exponent, coeff.exponent);
        }
    }
    if !found {
        return 0u;
    }
    var sum = SignedU128(U128(0u, 0u, 0u, 0u), false);
    for (var tap = 0u; tap < count; tap = tap + 1u) {
        let coeff = f64_coeff(weight_base + tap * 4u);
        let word = input[source_y * params.width + source_x + tap];
        let sample_negative = (word & 0x80000000u) != 0u;
        let sample_magnitude = select(word, 0u - word, sample_negative);
        if coeff.mantissa_lo == 0u && coeff.mantissa_hi == 0u || sample_magnitude == 0u {
            continue;
        }
        let product = f64_product(sample_magnitude, coeff);
        let shift = u32(coeff.exponent - minimum_exponent);
        let term = u128_shl(product, shift);
        sum = signed_u128_add(sum, term, sample_negative != coeff.negative);
    }
    return integer_sum_to_i32(sum, minimum_exponent);
}

fn filtered_box_average(source_y: u32, output_x: u32) -> f32 {
    let metadata = output_x * 3u;
    let source_x = u32(coefficients[metadata]);
    let count = u32(coefficients[metadata + 1u]);
    var sum: f32 = 0.0;
    for (var tap = 0u; tap < count; tap = tap + 1u) {
        let sample = bitcast<f32>(input[source_y * params.width + source_x + tap]);
        // Divide each sample before adding so the largest finite f32 value
        // cannot overflow an intermediate fixed-point-scale multiply.
        sum = sum + sample * 0.5;
    }
    return sum;
}

fn filtered_typed(source_y: u32, output_x: u32) -> u32 {
    let metadata = output_x * 3u;
    let source_x = u32(coefficients[metadata]);
    return input[source_y * params.width + source_x];
}

fn luma16_store_from_f32(bits: u32) -> u32 {
    let value = bitcast<f32>(bits);
    let rounded = i32(trunc(select(value - 0.5, value + 0.5, value >= 0.0)));
    let low = u32(clamp(rounded % 256, 0, 255));
    let high = u32(clamp(rounded / 256, 0, 255));
    return low | (high << 8u);
}

fn filtered_box_copy(source_y: u32, output_x: u32) -> u32 {
    let word = filtered_typed(source_y, output_x);
    // Pillow's f64 accumulator starts at +0.0, so a one-tap Box copy
    // canonicalizes an input negative zero at the final f32 store. A
    // signaling NaN is quieted by the f32->f64 product before the store;
    // preserve its payload and sign while setting the quiet bit.
    let exponent = (word >> 23u) & 255u;
    let fraction = word & 0x7fffffu;
    if exponent == 255u && fraction != 0u {
        return word | 0x00400000u;
    }
    return select(word, 0u, word == 0x80000000u);
}

fn pack_filtered(source_y: u32, output_x: u32) -> u32 {
    if params.mode == 5u {
        if params.premultiply == 10u {
            // Marker 10 reuses the exact f64 coefficient reducer, but rounds
            // its f32 result at Pillow's native I;16 byte-level boundary.
            // A same-size horizontal pass is an identity after Pillow's
            // byte-level round/clip. Copying the packed word also avoids
            // evaluating the tiny Lanczos/Bicubic tail coefficients that
            // exist at an unchanged boundary.
            if params.width == params.dst_w {
                return input[source_y * params.width + output_x];
            }
            return luma16_store_from_f32(filtered_f64_exact(source_y, output_x));
        }
        return filtered_typed(source_y, output_x);
    }
    if params.mode == 7u {
        // I-mode nearest resize uses the host-generated one-tap table for
        // Pillow's cumulative f64 coordinate walk. Copy the complete signed
        // sample word; treating its bytes as independent channels changes
        // negative values and is not the I-mode contract.
        if params.premultiply == 11u {
            if params.width == params.dst_w {
                return input[source_y * params.width + output_x];
            }
            return filtered_i32_exact(source_y, output_x);
        }
        return filtered_typed(source_y, output_x);
    }
    if params.mode == 8u {
        if params.premultiply == 7u {
            // F-mode nearest resize uses the host-generated one-tap table for
            // Pillow's cumulative f64 coordinate walk. Copy the complete
            // sample word so NaN, infinity, and signed zero survive exactly.
            return filtered_typed(source_y, output_x);
        }
        // F-mode constant resize uses the unused channel/premultiply slots
        // as an exact sample bit-pattern. Pillow's normalized filter rows
        // preserve finite constants, while the ordinary f32 accumulation
        // below can introduce a byte-visible rounding error.
        if params.premultiply == 2u {
            return params.channels;
        }
        if params.premultiply == 3u {
            // F-mode Box upscales have one normalized unit-weight tap. Copy
            // that source word so all special values retain their Pillow
            // representation; the helper canonicalizes negative zero at the
            // same final f32 store boundary as the host implementation.
            return filtered_box_copy(source_y, output_x);
        }
        if params.premultiply == 5u {
            // A same-size filtered F resize is an identity in Pillow's
            // resize_f path. Copy the word so NaN, infinity, and negative
            // zero retain their exact representation.
            return input[source_y * params.width + output_x];
        }
        if params.premultiply == 4u {
            // An exact 2:1 Box downscale has two normalized 0.5 taps. The
            // host proof excludes subnormal inputs, so this f32 reduction has
            // the same final bits as Pillow's f64 sum followed by f32 store.
            let count = u32(coefficients[output_x * 3u + 1u]);
            if count == 2u {
                return bitcast<u32>(filtered_box_average(source_y, output_x));
            }
            // The orthogonal unchanged axis has one unit tap. It shares the
            // proof tag but must copy that sample without applying arithmetic
            // (including fixed-point scaling that could overflow a max-f32).
            return filtered_typed(source_y, output_x);
        }
        if params.premultiply == 6u {
            // The host has proved exact f64/fixed-table agreement and either
            // the historical dyadic domain or the integer-emulation domain.
            // The latter keeps arbitrary f32 significands out of relaxed-f32
            // arithmetic while reproducing Pillow's final RN-even f32 store.
            return filtered_integer_exact(source_y, output_x);
        }
        if params.premultiply == 9u {
            // Pillow skips an unchanged horizontal pass.  Copy the complete
            // f32 word instead of filtering same-size rows whose kernel tails
            // may be tiny but still observable in the float bit pattern.
            if params.width == params.dst_w {
                return input[source_y * params.width + output_x];
            }
            return filtered_f64_exact(source_y, output_x);
        }
        if params.premultiply == 12u {
            if params.width == params.dst_w {
                return input[source_y * params.width + output_x];
            }
            return filtered_f64_ordered_2tap(source_y, output_x);
        }
        return bitcast<u32>(filtered_float(source_y, output_x));
    }
    let red = filtered_channel(source_y, output_x, 0u);
    let green = select(0u, filtered_channel(source_y, output_x, 1u), params.channels >= 3u);
    let blue = select(0u, filtered_channel(source_y, output_x, 2u), params.channels >= 3u);
    let alpha = select(
        255u,
        filtered_channel(source_y, output_x, params.channels - 1u),
        params.channels == 2u || params.channels == 4u,
    );
    return red | (green << 8u) | (blue << 16u) | (alpha << 24u);
}

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.dst_w || gid.y >= params.height {
        return;
    }
    output[gid.y * params.dst_w + gid.x] = pack_filtered(gid.y, gid.x);
}
