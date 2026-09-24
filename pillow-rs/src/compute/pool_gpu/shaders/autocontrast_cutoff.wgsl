// Autocontrast LUT derivation.
//
// The host computes percentile ranks with Pillow's truncation. Mapping must
// preserve its binary64 division, separate multiplications and addition before
// truncating: the exact rational formula can differ by one at integral results.
// Integer pairs reproduce that arithmetic without requiring shader f64 support.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
    cutoff_bits: u32,
    cutoff_low: u32,
    cutoff_high: u32,
    selected_pixels: u32,
}

@group(0) @binding(0) var<storage, read> histogram_data: array<u32>;
@group(0) @binding(1) var<storage, read_write> lut: array<u32, 256>;
@group(0) @binding(2) var<uniform> params: Params;

fn histogram_value(base: u32, rank: u32) -> u32 {
    var remaining = rank;
    for (var i = 0u; i < 256u; i = i + 1u) {
        let count = histogram_data[base + i];
        if remaining < count {
            return i;
        }
        remaining = remaining - count;
    }
    return 0u;
}

struct Scale {
    // Low/high words of a 53-bit significand, in units of 2^(exponent - 52).
    significand: vec2<u32>,
    exponent: u32,
}

fn rounded_scale(distance: u32) -> Scale {
    // 255/distance is finite and in [1,255]. Binary long division needs only
    // a small remainder and at most 52 fractional bits; round ties to even.
    let whole = 255u / distance;
    let exponent = 31u - countLeadingZeros(whole);
    var significand = vec2<u32>(whole, 0u);
    var remainder = 255u % distance;
    for (var bit = 0u; bit < 52u - exponent; bit++) {
        remainder *= 2u;
        let digit = select(0u, 1u, remainder >= distance);
        remainder -= digit * distance;
        significand = vec2<u32>((significand.x << 1u) | digit,
                                (significand.y << 1u) | (significand.x >> 31u));
    }
    if remainder * 2u > distance ||
       (remainder * 2u == distance && (significand.x & 1u) != 0u) {
        significand.x += 1u;
        significand.y += select(0u, 1u, significand.x == 0u);
    }
    return Scale(significand, exponent);
}

fn round_significand(value: vec2<u32>) -> vec2<u32> {
    let high_bits = 32u - countLeadingZeros(value.y);
    if high_bits <= 21u {
        return value;
    }
    // A 53-bit scale times a byte fits 61 bits, so at most eight low bits
    // need rounding. Keep the same fixed-point unit through both products.
    let shift = high_bits - 21u;
    let mask = (1u << shift) - 1u;
    let tail = value.x & mask;
    let half = 1u << (shift - 1u);
    let increment = tail > half || (tail == half && ((value.x >> shift) & 1u) != 0u);
    let base = value.x & ~mask;
    let low = base + select(0u, 1u << shift, increment);
    return vec2<u32>(low, value.y + select(0u, 1u, low < base));
}

fn rounded_product(value: u32, significand: vec2<u32>) -> vec2<u32> {
    let low_product = (significand.x & 65535u) * value;
    let middle = (significand.x >> 16u) * value + (low_product >> 16u);
    return round_significand(vec2<u32>(
        (middle << 16u) | (low_product & 65535u),
        significand.y * value + (middle >> 16u)));
}

fn remap(value: u32, lo: u32, hi: u32, scale: Scale) -> u32 {
    if hi <= lo { return value; }
    if value <= lo { return 0u; }
    // At hi itself, Pillow's result can be 254 after rounding. Only values
    // strictly above hi are known to saturate independently of that rounding.
    if value > hi { return 255u; }
    let product = rounded_product(value, scale.significand);
    let offset = rounded_product(lo, scale.significand);
    let difference = round_significand(vec2<u32>(
        product.x - offset.x,
        product.y - offset.y - select(0u, 1u, product.x < offset.x)));
    // The common fixed-point representation has 45..52 fractional bits.
    return min(255u, difference.y >> (20u - scale.exponent));
}

var<workgroup> bounds: array<vec2<u32>, 3>;
var<workgroup> scales: array<Scale, 3>;

@compute @workgroup_size(256)
fn main(@builtin(local_invocation_id) lid: vec3<u32>) {
    let channels = select(1u, 3u, params.mode >= 2u);
    if lid.x < channels {
        var limits = vec2<u32>(0u);
        if params.selected_pixels != 0u {
            limits = vec2<u32>(histogram_value(lid.x * 256u, params.cutoff_low),
                               histogram_value(lid.x * 256u, params.cutoff_high));
        }
        bounds[lid.x] = limits;
        scales[lid.x] = rounded_scale(max(1u, limits.y - min(limits.x, limits.y)));
    }
    workgroupBarrier();

    let value = lid.x;
    var rgb = vec3<u32>(value);
    for (var channel = 0u; channel < channels; channel++) {
        rgb[channel] = remap(value, bounds[channel].x, bounds[channel].y, scales[channel]);
    }
    lut[value] = rgb.x | (rgb.y << 8u) | (rgb.z << 16u) | 0xff000000u;
}
