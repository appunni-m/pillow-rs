// Native-LA Sharpness. Read the interleaved two-byte source without widening
// it to RGBA, filter luminance once, and preserve alpha exactly. The result
// uses the existing packed RGBA device transport; host readback narrows it
// directly to LA without constructing an intermediate RGBA image.

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

fn native_la_pixel(pixel_index: u32) -> vec2<u32> {
    // Two-byte pixels are aligned to the low or high half of one input word.
    let packed = (input[pixel_index >> 1u] >> ((pixel_index & 1u) * 16u)) & 0xffffu;
    return vec2<u32>(packed & 0xffu, (packed >> 8u) & 0xffu);
}

fn la_transport_pixel(luma: u32, alpha: u32) -> u32 {
    return luma | (luma << 8u) | (luma << 16u) | (alpha << 24u);
}

fn sharpen_pixel(x: u32, y: u32) -> u32 {
    let w = params.width;
    let h = params.height;
    let idx = y * w + x;
    let center = native_la_pixel(idx);
    if params.factor == 1000u || w < 3u || h < 3u {
        return la_transport_pixel(center.x, center.y);
    }
    if x == 0u || x >= w - 1u || y == 0u || y >= h - 1u {
        return la_transport_pixel(center.x, center.y);
    }

    let weighted = native_la_pixel((y - 1u) * w + (x - 1u)).x
        + native_la_pixel((y - 1u) * w + x).x
        + native_la_pixel((y - 1u) * w + (x + 1u)).x
        + native_la_pixel(y * w + (x - 1u)).x
        + center.x * 5u
        + native_la_pixel(y * w + (x + 1u)).x
        + native_la_pixel((y + 1u) * w + (x - 1u)).x
        + native_la_pixel((y + 1u) * w + x).x
        + native_la_pixel((y + 1u) * w + (x + 1u)).x;
    // Match Pillow's rounded SMOOTH result before its truncating fixed-point
    // blend: floor((2 * weighted + 13) / 26).
    let blurred = (weighted * 2u + 13u) / 26u;
    let factor = i32(params.factor);
    let blended = (i32(blurred) * (1000i - factor) + i32(center.x) * factor) / 1000i;
    let luma = u32(clamp(blended, 0i, 255i));
    return la_transport_pixel(luma, center.y);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height {
        return;
    }
    output[gid.y * params.width + gid.x] = sharpen_pixel(gid.x, gid.y);
}
