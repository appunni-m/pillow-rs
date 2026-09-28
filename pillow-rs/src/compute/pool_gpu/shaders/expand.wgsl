// Expand: add a border of `fill` color around the source image.
// Output dimensions = (w + 2*border) x (h + 2*border)
// Border pixels = fill_color; inner region = source image.
// Mode-aware: fill color respects image mode channels.
// Mode codes: 0=L, 1=LA, 2=RGB/HSV, 3=RGBA
// Generic output is packed u32 RGBA. The bounded native-output route packs
// each four-byte word from the requested 1/2/3/4-byte channel layout.
//
// Source dimensions come from header width/height (= cur_w/cur_h).
// Output dimensions computed as (width + 2*border, height + 2*border).

struct Params {
    width: u32,    // source width (from header = cur_w)
    height: u32,   // source height (from header = cur_h)
    mode: u32,     // 0=L, 1=LA, 2=RGB/HSV, 3=RGBA
    native_input: u32, // bit 0: packed native input; bit 1: compact native output
    border: u32,   // border width in pixels
    fill: u32,     // packed fill color (0xAABBGGRR)
    dst_w: u32,    // generic output width or native output words per dispatch row
    dst_h: u32,    // generic output height or native output word count
}

// ── Mode helpers ──

fn mode_has_g(m: u32) -> bool { return m >= 2u; }
fn mode_has_b(m: u32) -> bool { return m >= 2u; }
fn mode_has_a(m: u32) -> bool { return m == 1u || m == 3u || m == 4u || m == 5u || m == 8u; }

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn read_native_byte(pixel_index: u32, channel: u32, channels: u32) -> u32 {
    let byte_index = pixel_index * channels + channel;
    let word = input[byte_index / 4u];
    return (word >> ((byte_index % 4u) * 8u)) & 0xffu;
}

fn native_expand_byte(byte_index: u32, out_w: u32, border: u32, channels: u32) -> u32 {
    let pixel_index = byte_index / channels;
    let channel = byte_index % channels;
    let x = pixel_index % out_w;
    let y = pixel_index / out_w;
    if x < border || x >= border + params.width || y < border || y >= border + params.height {
        let fill_channel = select(channel, 3u, params.mode == 1u && channel == 1u);
        return (params.fill >> (fill_channel * 8u)) & 0xffu;
    }
    let source_x = x - border;
    let source_y = y - border;
    let source_pixel = source_y * params.width + source_x;
    return read_native_byte(source_pixel, channel, channels);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let b = params.border;
    // Keep the dimension arithmetic total even if a malformed uniform is
    // submitted without the host's checked output-dimension preflight.
    if b > (0xffffffffu - params.width) / 2u
        || b > (0xffffffffu - params.height) / 2u {
        return;
    }
    let out_w = params.width + 2u * b;
    let out_h = params.height + 2u * b;
    if (params.native_input & 2u) != 0u {
        let word_index = gid.y * params.dst_w + gid.x;
        if gid.x >= params.dst_w || word_index >= params.dst_h { return; }
        let channels = params.mode + 1u;
        let output_byte_count = out_w * out_h * channels;
        var packed = 0u;
        for (var lane = 0u; lane < 4u; lane = lane + 1u) {
            let byte_index = word_index * 4u + lane;
            if byte_index < output_byte_count {
                packed |= native_expand_byte(byte_index, out_w, b, channels) << (lane * 8u);
            }
        }
        output[word_index] = packed;
        return;
    }
    if gid.x >= out_w || gid.y >= out_h { return; }

    let idx = gid.y * out_w + gid.x;

    // Check if pixel is in the border region
    if gid.x < b || gid.x >= b + params.width || gid.y < b || gid.y >= b + params.height {
        // Border pixel: use fill color
        let f = params.fill;
        let fr = f & 0xffu;
        let fg = (f >> 8u) & 0xffu;
        let fb = (f >> 16u) & 0xffu;
        let fa = (f >> 24u) & 0xffu;

        let out_r = fr;
        let out_g = select(0u, fg, mode_has_g(params.mode));
        let out_b = select(0u, fb, mode_has_b(params.mode));
        let out_a = select(255u, fa, mode_has_a(params.mode));

        output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
    } else {
        // Inner pixel: copy from source
        let src_x = gid.x - b;
        let src_y = gid.y - b;
        let pixel_index = src_y * params.width + src_x;
        var r = 0u;
        var g = 0u;
        var b2 = 0u;
        var a = 255u;
        if (params.native_input & 1u) != 0u {
            let channels = params.mode + 1u;
            r = read_native_byte(pixel_index, 0u, channels);
            if params.mode == 1u {
                // LA's native alpha is byte one; the packed shader contract
                // stores it in the RGBA alpha byte.
                a = read_native_byte(pixel_index, 1u, channels);
            } else if params.mode >= 2u {
                g = read_native_byte(pixel_index, 1u, channels);
                b2 = read_native_byte(pixel_index, 2u, channels);
                if params.mode == 3u {
                    a = read_native_byte(pixel_index, 3u, channels);
                }
            }
        } else {
            let pixel = input[pixel_index];
            r = pixel & 0xffu;
            g = (pixel >> 8u) & 0xffu;
            b2 = (pixel >> 16u) & 0xffu;
            a = (pixel >> 24u) & 0xffu;
        }

        let out_r = r;
        let out_g = select(0u, g, mode_has_g(params.mode));
        let out_b = select(0u, b2, mode_has_b(params.mode));
        let out_a = select(255u, a, mode_has_a(params.mode));

        output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
    }
}
