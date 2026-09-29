// Native RGB Sharpness avoids widening each host pixel to the generic
// four-byte RGBA transport. The final result remains packed RGB + opaque A so
// the established readback path can narrow it to native RGB.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
    factor: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn native_rgb_pixel(pixel_index: u32) -> vec3<u32> {
    // A three-byte pixel can cross a u32 boundary. Upload size is rounded up
    // to complete words, so both reads stay within the checked input buffer.
    let byte_offset = pixel_index * 3u;
    let word_index = byte_offset >> 2u;
    let shift = (byte_offset & 3u) * 8u;
    var packed = input[word_index] >> shift;
    if shift > 8u {
        packed |= input[word_index + 1u] << (32u - shift);
    }
    return vec3<u32>(packed & 0xffu, (packed >> 8u) & 0xffu, (packed >> 16u) & 0xffu);
}

fn output_pixel(rgb: vec3<u32>) -> u32 {
    return rgb.x | (rgb.y << 8u) | (rgb.z << 16u) | 0xff000000u;
}

fn sharpen_pixel(x: u32, y: u32) -> u32 {
    let w = params.width;
    let h = params.height;
    let idx = y * w + x;
    if params.factor == 1000u || w < 3u || h < 3u {
        return output_pixel(native_rgb_pixel(idx));
    }
    if x == 0u || x >= w - 1u || y == 0u || y >= h - 1u {
        return output_pixel(native_rgb_pixel(idx));
    }

    let p00 = native_rgb_pixel((y - 1u) * w + (x - 1u));
    let p01 = native_rgb_pixel((y - 1u) * w + x);
    let p02 = native_rgb_pixel((y - 1u) * w + (x + 1u));
    let p10 = native_rgb_pixel(y * w + (x - 1u));
    let center = native_rgb_pixel(idx);
    let p12 = native_rgb_pixel(y * w + (x + 1u));
    let p20 = native_rgb_pixel((y + 1u) * w + (x - 1u));
    let p21 = native_rgb_pixel((y + 1u) * w + x);
    let p22 = native_rgb_pixel((y + 1u) * w + (x + 1u));

    let weighted = p00 + p01 + p02 + p10 + center * vec3<u32>(5u) + p12 + p20 + p21 + p22;
    // Pillow narrows the SMOOTH result before blending. This integer form is
    // exact for non-negative samples and matches sharpness.wgsl's rounding.
    let blurred = (weighted * vec3<u32>(2u) + vec3<u32>(13u)) / vec3<u32>(26u);
    let factor = i32(params.factor);
    let original_i = vec3<i32>(center);
    let blurred_i = vec3<i32>(blurred);
    let blended = (blurred_i * vec3<i32>(1000i - factor) + original_i * vec3<i32>(factor))
        / vec3<i32>(1000i);
    let clamped = vec3<u32>(clamp(blended, vec3<i32>(0i), vec3<i32>(255i)));
    return output_pixel(clamped);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height {
        return;
    }
    let idx = gid.y * params.width + gid.x;
    output[idx] = sharpen_pixel(gid.x, gid.y);
}
