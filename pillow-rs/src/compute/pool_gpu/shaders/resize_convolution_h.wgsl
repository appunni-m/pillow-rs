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
// every selected source word has one sign, every coefficient is a nonnegative
// 22-bit dyadic, and each aligned row sum fits 53 bits. Keeping the product
// and accumulation out of f32 avoids adapter-specific relaxed-f32 rounding.
struct U64 {
    lo: u32,
    hi: u32,
}

fn u64_add(left: U64, right: U64) -> U64 {
    let lo = left.lo + right.lo;
    let carry = select(0u, 1u, lo < left.lo);
    return U64(lo, left.hi + right.hi + carry);
}

fn u64_mul_mantissa_weight(mantissa: u32, weight: u32) -> U64 {
    // Splitting both operands into 16-bit limbs keeps every intermediate
    // product and carry inside u32 while covering the complete 32x32-bit
    // product. The host currently admits only nonnegative 22-bit weights.
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

fn integer_sum_to_f32(sum: U64, minimum_exponent: i32, negative: bool) -> u32 {
    let bit_length = u64_bit_length(sum);
    if bit_length == 0u {
        return 0u;
    }
    var exponent = minimum_exponent - 45 + i32(bit_length) - 1;
    var mantissa: u32;
    if bit_length > 24u {
        let shift = bit_length - 24u;
        mantissa = u64_shr(sum, shift).lo;
        let remainder = u64_low_bits(sum, shift);
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
        mantissa = sum.lo << (24u - bit_length);
    }
    let result = (u32(exponent + 127) << 23u) | (mantissa & 0x7fffffu);
    if negative {
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
    var negative = false;
    var found = false;
    for (var tap = 0u; tap < count; tap = tap + 1u) {
        let weight = coefficients[weight_base + tap];
        if weight == 0i {
            continue;
        }
        let bits = input[source_y * params.width + source_x + tap];
        if ((bits & 0x7fffffffu) == 0u) {
            continue;
        }
        let exponent = i32((bits >> 23u) & 255u) - 127;
        if !found {
            minimum_exponent = exponent;
            negative = (bits & 0x80000000u) != 0u;
            found = true;
        } else {
            minimum_exponent = min(minimum_exponent, exponent);
        }
    }
    if !found {
        return 0u;
    }
    var sum = U64(0u, 0u);
    for (var tap = 0u; tap < count; tap = tap + 1u) {
        let weight = coefficients[weight_base + tap];
        if weight == 0i {
            continue;
        }
        let bits = input[source_y * params.width + source_x + tap];
        if (bits & 0x7fffffffu) == 0u {
            continue;
        }
        let exponent = i32((bits >> 23u) & 255u) - 127;
        let mantissa = (bits & 0x7fffffu) | 0x800000u;
        let product = u64_mul_mantissa_weight(mantissa, u32(weight));
        sum = u64_add(sum, u64_shl(product, u32(exponent - minimum_exponent)));
    }
    return integer_sum_to_f32(sum, minimum_exponent, negative);
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

fn pack_filtered(source_y: u32, output_x: u32) -> u32 {
    if params.mode == 7u {
        // I-mode nearest resize uses the host-generated one-tap table for
        // Pillow's cumulative f64 coordinate walk. Copy the complete signed
        // sample word; treating its bytes as independent channels changes
        // negative values and is not the I-mode contract.
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
            // that source word so the finite f32 values admitted by the host
            // proof retain their exact representation.
            return filtered_typed(source_y, output_x);
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
