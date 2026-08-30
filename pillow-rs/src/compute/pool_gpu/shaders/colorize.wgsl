// Colorize: apply Pillow's integer LUT mapping from grayscale to RGB.
// Param[0] = black_color packed as 0xAABBGGRR
// Param[1] = white_color packed as 0xAABBGGRR
// Param[2] = midpoint color (ignored when has_mid is zero)
// Param[3..5] = blackpoint, midpoint, whitepoint
// Param[6] = has_mid
// Packed u32 RGBA: byte0=R, byte1=G, byte2=B, byte3=A

struct Params {
    width: u32,
    height: u32,
    mode: u32,    // 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,
    black_color: u32,
    white_color: u32,
    mid_color: u32,
    blackpoint: u32,
    midpoint: u32,
    whitepoint: u32,
    has_mid: u32,
}

// Pillow's LUT uses floor division for negative color deltas. WGSL integer
// division truncates toward zero, so spell out mathematical floor division.
fn floor_div(n: i32, d: i32) -> i32 {
    if n >= 0i {
        return n / d;
    }
    return -((-n + d - 1i) / d);
}

fn colorize_channel(
    black: u32,
    white: u32,
    mid: u32,
    luma: u32,
    blackpoint: u32,
    midpoint: u32,
    whitepoint: u32,
    has_mid: u32,
) -> u32 {
    let index = i32(luma);
    let bp = i32(blackpoint);
    let mp = i32(midpoint);
    let wp = i32(whitepoint);
    let black_value = i32(black);
    let white_value = i32(white);
    let mid_value = i32(mid);
    var value: i32;

    if index < bp {
        value = black_value;
    } else if has_mid != 0u {
        if index < mp {
            let span = mp - bp;
            var step: i32 = 0i;
            if span != 0i {
                step = floor_div((index - bp) * (mid_value - black_value), span);
            }
            value = black_value + step;
        } else if index < wp {
            let span = wp - mp;
            var step: i32 = 0i;
            if span != 0i {
                step = floor_div((index - mp) * (white_value - mid_value), span);
            }
            value = mid_value + step;
        } else {
            value = white_value;
        }
    } else if index < wp {
        let span = wp - bp;
        var step: i32 = 0i;
        if span != 0i {
            step = floor_div((index - bp) * (white_value - black_value), span);
        }
        value = black_value + step;
    } else {
        value = white_value;
    }

    return u32(clamp(value, 0i, 255i));
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let idx = gid.y * params.width + gid.x;
    let pixel = input[idx];
    let r = pixel & 0xffu;
    let g = (pixel >> 8u) & 0xffu;
    let b = (pixel >> 16u) & 0xffu;
    let a = (pixel >> 24u) & 0xffu;

    // ImageOps.colorize validates an L source. Its packed transport stores
    // the grayscale sample in byte zero, so no RGB conversion is required.
    let luma = r;

    // Extract black/white colors
    let bc = params.black_color;
    let wc = params.white_color;
    let mc = params.mid_color;
    let br = bc & 0xffu;
    let bg = (bc >> 8u) & 0xffu;
    let bb = (bc >> 16u) & 0xffu;
    let wr = wc & 0xffu;
    let wg = (wc >> 8u) & 0xffu;
    let wb = (wc >> 16u) & 0xffu;

    let mr = mc & 0xffu;
    let mg = (mc >> 8u) & 0xffu;
    let mb = (mc >> 16u) & 0xffu;

    let out_r = colorize_channel(
        br, wr, mr, luma, params.blackpoint, params.midpoint,
        params.whitepoint, params.has_mid,
    );
    let out_g = colorize_channel(
        bg, wg, mg, luma, params.blackpoint, params.midpoint,
        params.whitepoint, params.has_mid,
    );
    let out_b = colorize_channel(
        bb, wb, mb, luma, params.blackpoint, params.midpoint,
        params.whitepoint, params.has_mid,
    );
    let out_a = 255u;

    output[idx] = out_r | (out_g << 8u) | (out_b << 16u) | (out_a << 24u);
}
