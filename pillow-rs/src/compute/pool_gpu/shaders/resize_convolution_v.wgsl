// Exact vertical pass for Pillow's separable byte resize.
//
// The input is the packed intermediate produced by resize_convolution_h.  Its
// width is dst_w and its height is the original source height.  The vertical
// table uses the same metadata layout as the horizontal table, with dst_h
// metadata entries.

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

fn fixed_to_byte(sum: i32) -> u32 {
    let value = (sum + FIXED_BIAS) >> 22;
    return u32(clamp(value, 0, 255));
}

fn filtered_channel(output_x: u32, output_y: u32, channel: u32) -> u32 {
    let metadata = output_y * 3u;
    let source_y = u32(coefficients[metadata]);
    let count = u32(coefficients[metadata + 1u]);
    let weight_base = 3u * params.dst_h + u32(coefficients[metadata + 2u]);
    var sum: i32 = 0;
    for (var tap = 0u; tap < count; tap = tap + 1u) {
        let pixel = input[(source_y + tap) * params.dst_w + output_x];
        sum = sum + i32(pixel_channel(pixel, channel)) * coefficients[weight_base + tap];
    }
    return fixed_to_byte(sum);
}

fn filtered_float(output_x: u32, output_y: u32) -> f32 {
    let metadata = output_y * 3u;
    let source_y = u32(coefficients[metadata]);
    let count = u32(coefficients[metadata + 1u]);
    let weight_base = 3u * params.dst_h + u32(coefficients[metadata + 2u]);
    var sum: f32 = 0.0;
    for (var tap = 0u; tap < count; tap = tap + 1u) {
        let sample = bitcast<f32>(input[(source_y + tap) * params.dst_w + output_x]);
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

fn filtered_integer_exact(output_x: u32, output_y: u32) -> u32 {
    let metadata = output_y * 3u;
    let source_y = u32(coefficients[metadata]);
    let count = u32(coefficients[metadata + 1u]);
    let weight_base = 3u * params.dst_h + u32(coefficients[metadata + 2u]);
    var minimum_exponent: i32 = 128;
    var found = false;
    for (var tap = 0u; tap < count; tap = tap + 1u) {
        let weight = coefficients[weight_base + tap];
        if weight == 0i {
            continue;
        }
        let bits = input[(source_y + tap) * params.dst_w + output_x];
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
        let bits = input[(source_y + tap) * params.dst_w + output_x];
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
    let result = (u32(exponent + 127) << 23u) | (mantissa & 0x7fffffu);
    if sum.negative {
        return result | 0x80000000u;
    }
    return result;
}

fn filtered_f64_exact(output_x: u32, output_y: u32) -> u32 {
    let metadata = output_y * 3u;
    let source_y = u32(coefficients[metadata]);
    let count = u32(coefficients[metadata + 1u]);
    let weight_base = 3u * params.dst_h + u32(coefficients[metadata + 2u]);
    var minimum_exponent: i32 = 0;
    var found = false;
    for (var tap = 0u; tap < count; tap = tap + 1u) {
        let coeff = f64_coeff(weight_base + tap * 4u);
        if coeff.mantissa_lo == 0u && coeff.mantissa_hi == 0u {
            continue;
        }
        let bits = input[(source_y + tap) * params.dst_w + output_x];
        if (bits & 0x7fffffffu) == 0u {
            continue;
        }
        let sample_exponent = i32((bits >> 23u) & 255u) - 127 - 23;
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
        let bits = input[(source_y + tap) * params.dst_w + output_x];
        if (bits & 0x7fffffffu) == 0u {
            continue;
        }
        let sample_exponent = i32((bits >> 23u) & 255u) - 127 - 23;
        let product = f64_product((bits & 0x7fffffu) | 0x800000u, coeff);
        let term = u128_shl(product, u32(sample_exponent + coeff.exponent - minimum_exponent));
        let sample_negative = (bits & 0x80000000u) != 0u;
        sum = signed_u128_add(sum, term, sample_negative != coeff.negative);
    }
    return f64_sum_to_f32(sum, minimum_exponent);
}


fn filtered_box_average(output_x: u32, output_y: u32) -> f32 {
    let metadata = output_y * 3u;
    let source_y = u32(coefficients[metadata]);
    let count = u32(coefficients[metadata + 1u]);
    var sum: f32 = 0.0;
    for (var tap = 0u; tap < count; tap = tap + 1u) {
        let sample = bitcast<f32>(input[(source_y + tap) * params.dst_w + output_x]);
        // Divide each sample before adding so the largest finite f32 value
        // cannot overflow an intermediate fixed-point-scale multiply.
        sum = sum + sample * 0.5;
    }
    return sum;
}

fn filtered_typed(output_x: u32, output_y: u32) -> u32 {
    let metadata = output_y * 3u;
    let source_y = u32(coefficients[metadata]);
    return input[source_y * params.dst_w + output_x];
}

fn unpremultiply(value: u32, alpha: u32) -> u32 {
    if alpha == 0u {
        // The scalar path leaves a premultiplied channel unchanged when the
        // filtered alpha is zero; preserve that byte instead of discarding
        // it before the final pack.
        return value;
    }
    // Pillow clips the unpremultiplied channel to one byte after the
    // fixed-point vertical pass.  Without this clamp, ringing filters can
    // produce a quotient above 255; packing that u32 into the RGBA word then
    // wraps the channel instead of matching CLIP8.
    return min((value * 255u) / alpha, 255u);
}

fn pack_filtered(output_x: u32, output_y: u32) -> u32 {
    if params.mode == 7u {
        // I-mode nearest resize uses the host-generated one-tap table for
        // Pillow's cumulative f64 coordinate walk. Copy the complete signed
        // sample word rather than filtering its bytes as color channels.
        return filtered_typed(output_x, output_y);
    }
    if params.mode == 8u {
        if params.premultiply == 7u {
            // F-mode nearest resize uses the host-generated one-tap table for
            // Pillow's cumulative f64 coordinate walk. Copy the complete
            // sample word so NaN, infinity, and signed zero survive exactly.
            return filtered_typed(output_x, output_y);
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
            // that source word so the finite f32 values admitted by the host
            // proof retain their exact representation.
            return filtered_typed(output_x, output_y);
        }
        if params.premultiply == 5u {
            // A same-size filtered F resize is an identity in Pillow's
            // resize_f path. Copy the word so NaN, infinity, and negative
            // zero retain their exact representation.
            return input[output_y * params.dst_w + output_x];
        }
        if params.premultiply == 4u {
            // An exact 2:1 Box downscale has two normalized 0.5 taps. The
            // host proof excludes subnormal inputs, so this f32 reduction has
            // the same final bits as Pillow's f64 sum followed by f32 store.
            let count = u32(coefficients[output_y * 3u + 1u]);
            if count == 2u {
                return bitcast<u32>(filtered_box_average(output_x, output_y));
            }
            // The orthogonal unchanged axis has one unit tap. It shares the
            // proof tag but must copy that sample without applying arithmetic
            // (including fixed-point scaling that could overflow a max-f32).
            return filtered_typed(output_x, output_y);
        }
        if params.premultiply == 6u {
            // The host has proved exact f64/fixed-table agreement and either
            // the historical dyadic domain or the integer-emulation domain.
            // The latter keeps arbitrary f32 significands out of relaxed-f32
            // arithmetic while reproducing Pillow's final RN-even f32 store.
            return filtered_integer_exact(output_x, output_y);
        }
        if params.premultiply == 9u {
            // Pillow skips an unchanged vertical pass; preserve that exact
            // intermediate word rather than evaluating same-size tails.
            if params.height == params.dst_h {
                return input[output_y * params.dst_w + output_x];
            }
            return filtered_f64_exact(output_x, output_y);
        }
        return bitcast<u32>(filtered_float(output_x, output_y));
    }
    let alpha = select(
        255u,
        filtered_channel(output_x, output_y, params.channels - 1u),
        params.channels == 2u || params.channels == 4u,
    );
    var red = filtered_channel(output_x, output_y, 0u);
    if params.premultiply != 0u {
        red = unpremultiply(red, alpha);
    }
    let green = select(
        0u,
        filtered_channel(output_x, output_y, 1u),
        params.channels >= 3u,
    );
    let blue = select(
        0u,
        filtered_channel(output_x, output_y, 2u),
        params.channels >= 3u,
    );
    var result_green = green;
    var result_blue = blue;
    if params.premultiply != 0u && params.channels == 4u {
        result_green = unpremultiply(green, alpha);
        result_blue = unpremultiply(blue, alpha);
    }
    return red | (result_green << 8u) | (result_blue << 16u) | (alpha << 24u);
}

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.dst_w || gid.y >= params.dst_h {
        return;
    }
    output[gid.y * params.dst_w + gid.x] = pack_filtered(gid.x, gid.y);
}
