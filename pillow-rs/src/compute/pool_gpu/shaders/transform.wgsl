// Transform: affine, perspective, quadrilateral, and one-record mesh image
// transforms. The method selector keeps all bounded geometry contracts on
// one packed dispatch; method-specific coordinates are carried in the tail
// of Params.
// Map destination pixels to source coordinates via affine transform:
//   sx = a*dx + b*dy + c
//   sy = d*dx + e*dy + f
// Where (dx, dy) are destination pixel coordinates and (sx, sy) are
// source pixel coordinates (floating-point).
//
// Sampling: nearest-neighbor (filter_code=0) or Pillow's affine bilinear
// path (all other filter codes).  The public affine operation accepts other
// resampling names, but Pillow's affine transform uses the bilinear transform
// kernel for every non-nearest filter.
// Out-of-bounds source coordinates are filled with fill_color (packed u32 RGBA).
//
// Dispatch at dst_w x dst_h (output image dimensions).
//
// 3-binding layout: input (source), output, params.
//
// Mode-aware: channel selection per mode.
// Mode codes: 0=L/P/1, 1=LA/PA, 2=RGB, 3=RGBA, 4=CMYK, 5=I;16*,
// 6=RGBX, 7=I, 8=F.

struct Params {
    width: u32,      // source width
    height: u32,     // source height
    mode: u32,
    _pad: u32,
    dst_w: u32,      // output width
    dst_h: u32,      // output height
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    e: f32,
    f: f32,
    fill_color: u32, // packed RGBA: R|G<<8|B<<16|A<<24
    filter_code: u32, // 0=nearest, 1=bilinear
    premultiply: u32,
    method: u32,      // 0=affine, 1=perspective, 2=quad, 3=mesh
    g: f32,           // perspective denominator / quad x3 / mesh source x1
    h: f32,           // perspective denominator / quad y3 / mesh source y1
    mesh0: f32,       // mesh source x2
    mesh1: f32,       // mesh source y2
    mesh2: f32,       // mesh source x3
    mesh3: f32,       // mesh source y3
}

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
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    return a * (1.0 - t) + b * t;
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

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

fn sample_nearest(sx: f32, sy: f32) -> u32 {
    if params.mode == 5u {
        // Geometry.c's I;16 affine path evaluates integer destination
        // coordinates and rounds the mapped source coordinate with
        // floor(value + 0.5), unlike the byte affine path's truncation.
        let ix_f = floor(sx + 0.5);
        let iy_f = floor(sy + 0.5);
        if ix_f < 0.0 || iy_f < 0.0 {
            return get_fill_pixel();
        }
        let ix = u32(ix_f);
        let iy = u32(iy_f);
        if ix >= params.width || iy >= params.height {
            return get_fill_pixel();
        }
        return input[iy * params.width + ix] & 0xffffu;
    }
    if sx < 0.0 || sy < 0.0 {
        return get_fill_pixel();
    }
    // Geometry.c's affine nearest path truncates non-negative coordinates;
    // it does not round to the nearest source pixel.
    let ix = u32(sx);
    let iy = u32(sy);
    if ix >= params.width || iy >= params.height {
        return get_fill_pixel();
    }
    let idx = iy * params.width + ix;
    let pixel = input[idx];
    if params.mode == 7u || params.mode == 8u {
        // I/F samples are opaque four-byte words in the packed transport.
        return pixel;
    }
    let sr = pixel & 0xffu;
    let sg = (pixel >> 8u) & 0xffu;
    let sb = (pixel >> 16u) & 0xffu;
    let sa = (pixel >> 24u) & 0xffu;

    let out_g = select(0u, sg, mode_has_g(params.mode));
    let out_b = select(0u, sb, mode_has_b(params.mode));
    let out_a = select(255u, sa, mode_has_a(params.mode));
    var out_fourth = out_a;
    if mode_has_fourth(params.mode) {
        out_fourth = sa;
    }

    return sr | (out_g << 8u) | (out_b << 16u) | (out_fourth << 24u);
}

// The native affine-nearest kernel consumes a signed 16.16 plan.  Its six
// affine slots are transported as integer bit patterns in the f32 fields:
//   a = dx step, b = dy step, c = row origin (x),
//   d = dx step, e = dy step, f = row origin (y).
// Keeping the coordinate walk integer-for-integer avoids f32 boundary drift
// for expanded rotations whose edge pixels sit exactly on the fill boundary.
fn sample_nearest_fixed(sx_fixed: i32, sy_fixed: i32) -> u32 {
    if sx_fixed < 0 || sy_fixed < 0 {
        return get_fill_pixel();
    }
    let ix = sx_fixed >> 16;
    let iy = sy_fixed >> 16;
    if ix < 0 || iy < 0 || ix >= i32(params.width) || iy >= i32(params.height) {
        return get_fill_pixel();
    }
    let pixel = input[u32(iy) * params.width + u32(ix)];
    if params.mode == 7u || params.mode == 8u {
        return pixel;
    }
    let sr = pixel & 0xffu;
    let sg = (pixel >> 8u) & 0xffu;
    let sb = (pixel >> 16u) & 0xffu;
    let sa = (pixel >> 24u) & 0xffu;
    let out_g = select(0u, sg, mode_has_g(params.mode));
    let out_b = select(0u, sb, mode_has_b(params.mode));
    let out_a = select(255u, sa, mode_has_a(params.mode));
    var out_fourth = out_a;
    if mode_has_fourth(params.mode) {
        out_fourth = sa;
    }
    return sr | (out_g << 8u) | (out_b << 16u) | (out_fourth << 24u);
}

fn sample_nearest_projective(sx: f32, sy: f32) -> u32 {
    if params.mode == 5u {
        let ix_f = floor(sx + 0.5);
        let iy_f = floor(sy + 0.5);
        if ix_f < 0.0 || iy_f < 0.0 {
            return get_fill_pixel();
        }
        let ix = u32(ix_f);
        let iy = u32(iy_f);
        if ix >= params.width || iy >= params.height {
            return get_fill_pixel();
        }
        return input[iy * params.width + ix] & 0xffffu;
    }
    let ix_f = floor(sx + 0.5);
    let iy_f = floor(sy + 0.5);
    if ix_f < 0.0 || iy_f < 0.0 {
        return get_fill_pixel();
    }
    let ix = u32(ix_f);
    let iy = u32(iy_f);
    if ix >= params.width || iy >= params.height {
        return get_fill_pixel();
    }
    let pixel = input[iy * params.width + ix];
    if params.mode == 7u || params.mode == 8u {
        return pixel;
    }
    let sr = pixel & 0xffu;
    let sg = (pixel >> 8u) & 0xffu;
    let sb = (pixel >> 16u) & 0xffu;
    let sa = (pixel >> 24u) & 0xffu;
    let out_g = select(0u, sg, mode_has_g(params.mode));
    let out_b = select(0u, sb, mode_has_b(params.mode));
    let out_a = select(255u, sa, mode_has_a(params.mode));
    let out_fourth = select(out_a, sa, mode_has_fourth(params.mode));
    return sr | (out_g << 8u) | (out_b << 16u) | (out_fourth << 24u);
}

fn bilinear_channel(
    p00: u32,
    p10: u32,
    p01: u32,
    p11: u32,
    shift: u32,
    fx: f32,
    fy: f32,
) -> u32 {
    let c00 = f32((p00 >> shift) & 0xffu);
    let c10 = f32((p10 >> shift) & 0xffu);
    let c01 = f32((p01 >> shift) & 0xffu);
    let c11 = f32((p11 >> shift) & 0xffu);
    let top = lerp(c00, c10, fx);
    let bottom = lerp(c01, c11, fx);
    return u32(clamp(round(lerp(top, bottom, fy)), 0.0, 255.0));
}

fn sample_projective_bilinear(sx: f32, sy: f32) -> u32 {
    let src_w_f = f32(params.width);
    let src_h_f = f32(params.height);
    if sx < 0.0 || sx >= src_w_f || sy < 0.0 || sy >= src_h_f {
        return get_fill_pixel();
    }
    let x0_f = floor(sx);
    let y0_f = floor(sy);
    let fx = sx - x0_f;
    let fy = sy - y0_f;
    let x0 = u32(clamp(x0_f, 0.0, src_w_f - 1.0));
    let y0 = u32(clamp(y0_f, 0.0, src_h_f - 1.0));
    let x1 = u32(clamp(x0_f + 1.0, 0.0, src_w_f - 1.0));
    let y1 = u32(clamp(y0_f + 1.0, 0.0, src_h_f - 1.0));
    let p00 = input[y0 * params.width + x0];
    let p10 = input[y0 * params.width + x1];
    let p01 = input[y1 * params.width + x0];
    let p11 = input[y1 * params.width + x1];
    let r = bilinear_channel(p00, p10, p01, p11, 0u, fx, fy);
    let g = select(0u, bilinear_channel(p00, p10, p01, p11, 8u, fx, fy), mode_has_g(params.mode));
    let b = select(0u, bilinear_channel(p00, p10, p01, p11, 16u, fx, fy), mode_has_b(params.mode));
    let alpha_sample = bilinear_channel(p00, p10, p01, p11, 24u, fx, fy);
    let alpha = select(255u, alpha_sample, mode_has_a(params.mode));
    let fourth = select(alpha, alpha_sample, mode_has_fourth(params.mode));
    return r | (g << 8u) | (b << 16u) | (fourth << 24u);
}

fn source_coordinates(dx: f32, dy: f32) -> vec2<f32> {
    if params.method == 0u {
        var ax = dx + 0.5;
        var ay = dy + 0.5;
        if params.mode == 5u {
            ax = dx;
            ay = dy;
        }
        return vec2<f32>(
            params.a * ax + params.b * ay + params.c,
            params.d * ax + params.e * ay + params.f,
        );
    }
    if params.method == 1u {
        let denominator = params.g * dx + params.h * dy + 1.0;
        if denominator == 0.0 {
            return vec2<f32>(-1.0, -1.0);
        }
        return vec2<f32>(
            (params.a * dx + params.b * dy + params.c) / denominator,
            (params.d * dx + params.e * dy + params.f) / denominator,
        );
    }
    if params.method == 2u {
        let width = max(f32(params.dst_w), 1.0);
        let height = max(f32(params.dst_h), 1.0);
        let x0 = params.a;
        let y0 = params.b;
        let x1 = params.c;
        let y1 = params.d;
        let x2 = params.e;
        let y2 = params.f;
        let x3 = params.g;
        let y3 = params.h;
        let u = dx / width;
        let v = dy / height;
        return vec2<f32>(
            x0 + (x3 - x0) * u + (x1 - x0) * v
                + (x2 - x1 - x3 + x0) * u * v,
            y0 + (y3 - y0) * u + (y1 - y0) * v
                + (y2 - y1 - y3 + y0) * u * v,
        );
    }
    // Mesh: one bounded record. The first four values are the destination
    // bbox, followed by the four source corners in PIL's order.
    let bx0 = params.a;
    let by0 = params.b;
    let bx1 = params.c;
    let by1 = params.d;
    if dx < bx0 || dx >= bx1 || dy < by0 || dy >= by1 {
        return vec2<f32>(-1.0, -1.0);
    }
    let bw = max(bx1 - bx0, 1.0);
    let bh = max(by1 - by0, 1.0);
    let u = (dx - bx0) / bw;
    let v = (dy - by0) / bh;
    let x0 = params.e;
    let y0 = params.f;
    let x1 = params.g;
    let y1 = params.h;
    let x2 = params.mesh0;
    let y2 = params.mesh1;
    let x3 = params.mesh2;
    let y3 = params.mesh3;
    return vec2<f32>(
        (1.0 - u) * (1.0 - v) * x0 + u * (1.0 - v) * x3
            + u * v * x2 + (1.0 - u) * v * x1,
        (1.0 - u) * (1.0 - v) * y0 + u * (1.0 - v) * y3
            + u * v * y2 + (1.0 - u) * v * y1,
    );
}

fn sample_bilinear(sx: f32, sy: f32) -> u32 {
    if params.method != 0u {
        return sample_projective_bilinear(sx, sy);
    }
    let src_w_f = f32(params.width);
    let src_h_f = f32(params.height);

    // Geometry.c checks destination pixel centers before moving the
    // bilinear kernel into source-corner space.
    if sx < 0.0 || sx >= src_w_f || sy < 0.0 || sy >= src_h_f {
        return get_fill_pixel();
    }

    // BILINEAR_HEAD subtracts the half-pixel center offset before flooring.
    let x_sample = sx - 0.5;
    let y_sample = sy - 0.5;
    let x0_f = floor(x_sample);
    let y0_f = floor(y_sample);
    let fx = x_sample - x0_f;
    let fy = y_sample - y0_f;

    // Clamp to valid source pixel indices
    let x0 = u32(clamp(x0_f, 0.0, src_w_f - 1.0));
    let y0 = u32(clamp(y0_f, 0.0, src_h_f - 1.0));
    let x1 = u32(clamp(x0_f + 1.0, 0.0, src_w_f - 1.0));
    let y1 = u32(clamp(y0_f + 1.0, 0.0, src_h_f - 1.0));

    // Load 4 neighboring pixels
    let p00 = input[y0 * params.width + x0];
    let p10 = input[y0 * params.width + x1];
    let p01 = input[y1 * params.width + x0];
    let p11 = input[y1 * params.width + x1];

    // Keep the four channel values explicit so the compiler can inline this
    // small kernel.  The packed transport always carries the logical sample
    // in byte zero, optional color bands in bytes one/two, and an optional
    // alpha/padding byte in byte three.
    let r00 = f32(p00 & 0xffu);
    let r10 = f32(p10 & 0xffu);
    let r01 = f32(p01 & 0xffu);
    let r11 = f32(p11 & 0xffu);
    let g00 = f32((p00 >> 8u) & 0xffu);
    let g10 = f32((p10 >> 8u) & 0xffu);
    let g01 = f32((p01 >> 8u) & 0xffu);
    let g11 = f32((p11 >> 8u) & 0xffu);
    let b00 = f32((p00 >> 16u) & 0xffu);
    let b10 = f32((p10 >> 16u) & 0xffu);
    let b01 = f32((p01 >> 16u) & 0xffu);
    let b11 = f32((p11 >> 16u) & 0xffu);
    let a00 = f32((p00 >> 24u) & 0xffu);
    let a10 = f32((p10 >> 24u) & 0xffu);
    let a01 = f32((p01 >> 24u) & 0xffu);
    let a11 = f32((p11 >> 24u) & 0xffu);
    let fourth00 = a00;
    let fourth10 = a10;
    let fourth01 = a01;
    let fourth11 = a11;

    let alpha00 = select(255.0, a00, mode_has_a(params.mode));
    let alpha10 = select(255.0, a10, mode_has_a(params.mode));
    let alpha01 = select(255.0, a01, mode_has_a(params.mode));
    let alpha11 = select(255.0, a11, mode_has_a(params.mode));
    let premul_r00 = select(r00, floor((r00 * alpha00 + 127.5) / 255.0), params.premultiply != 0u);
    let premul_r10 = select(r10, floor((r10 * alpha10 + 127.5) / 255.0), params.premultiply != 0u);
    let premul_r01 = select(r01, floor((r01 * alpha01 + 127.5) / 255.0), params.premultiply != 0u);
    let premul_r11 = select(r11, floor((r11 * alpha11 + 127.5) / 255.0), params.premultiply != 0u);
    let premul_g00 = select(g00, floor((g00 * alpha00 + 127.5) / 255.0), params.premultiply != 0u);
    let premul_g10 = select(g10, floor((g10 * alpha10 + 127.5) / 255.0), params.premultiply != 0u);
    let premul_g01 = select(g01, floor((g01 * alpha01 + 127.5) / 255.0), params.premultiply != 0u);
    let premul_g11 = select(g11, floor((g11 * alpha11 + 127.5) / 255.0), params.premultiply != 0u);
    let premul_b00 = select(b00, floor((b00 * alpha00 + 127.5) / 255.0), params.premultiply != 0u);
    let premul_b10 = select(b10, floor((b10 * alpha10 + 127.5) / 255.0), params.premultiply != 0u);
    let premul_b01 = select(b01, floor((b01 * alpha01 + 127.5) / 255.0), params.premultiply != 0u);
    let premul_b11 = select(b11, floor((b11 * alpha11 + 127.5) / 255.0), params.premultiply != 0u);

    let top_r = lerp(premul_r00, premul_r10, fx);
    let top_g = lerp(premul_g00, premul_g10, fx);
    let top_b = lerp(premul_b00, premul_b10, fx);
    let top_a = lerp(alpha00, alpha10, fx);
    let top_fourth = lerp(fourth00, fourth10, fx);
    let bot_r = lerp(premul_r01, premul_r11, fx);
    let bot_g = lerp(premul_g01, premul_g11, fx);
    let bot_b = lerp(premul_b01, premul_b11, fx);
    let bot_a = lerp(alpha01, alpha11, fx);
    let bot_fourth = lerp(fourth01, fourth11, fx);
    let out_r_f = lerp(top_r, bot_r, fy);
    let out_g_f = lerp(top_g, bot_g, fy);
    let out_b_f = lerp(top_b, bot_b, fy);
    let out_a_f = lerp(top_a, bot_a, fy);
    let out_fourth_f = lerp(top_fourth, bot_fourth, fy);
    let out_a = select(255u, u32(clamp(out_a_f, 0.0, 255.0)), mode_has_a(params.mode));
    // The native byte transform rounds/truncates each premultiplied channel
    // into the intermediate image before the final alpha unpremultiplication
    // pass.  Do that quantization explicitly; unpremultiplying the f32 value
    // directly changes pixels at low alpha.
    let premul_r = u32(clamp(out_r_f, 0.0, 255.0));
    let premul_g = u32(clamp(out_g_f, 0.0, 255.0));
    let premul_b = u32(clamp(out_b_f, 0.0, 255.0));
    var out_r: u32;
    if params.premultiply != 0u && out_a > 0u {
        out_r = min(premul_r * 255u / out_a, 255u);
    } else {
        out_r = premul_r;
    }
    var out_g: u32;
    if mode_has_g(params.mode) {
        if params.premultiply != 0u && out_a > 0u {
            out_g = min(premul_g * 255u / out_a, 255u);
        } else {
            out_g = premul_g;
        }
    } else {
        out_g = 0u;
    }
    var out_b: u32;
    if mode_has_b(params.mode) {
        if params.premultiply != 0u && out_a > 0u {
            out_b = min(premul_b * 255u / out_a, 255u);
        } else {
            out_b = premul_b;
        }
    } else {
        out_b = 0u;
    }
    var out_fourth: u32;
    if mode_has_fourth(params.mode) {
        out_fourth = u32(clamp(out_fourth_f, 0.0, 255.0));
    } else {
        out_fourth = out_a;
    }

    return out_r | (out_g << 8u) | (out_b << 16u) | (out_fourth << 24u);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.dst_w || gid.y >= params.dst_h { return; }

    if params.filter_code == 0u && params.method == 0u {
        let step_x_x = bitcast<i32>(params.a);
        let step_y_x = bitcast<i32>(params.b);
        let origin_x = bitcast<i32>(params.c);
        let step_x_y = bitcast<i32>(params.d);
        let step_y_y = bitcast<i32>(params.e);
        let origin_y = bitcast<i32>(params.f);
        let sx_fixed = origin_x + i32(gid.x) * step_x_x + i32(gid.y) * step_y_x;
        let sy_fixed = origin_y + i32(gid.x) * step_x_y + i32(gid.y) * step_y_y;
        output[gid.y * params.dst_w + gid.x] = sample_nearest_fixed(sx_fixed, sy_fixed);
        return;
    }

    let coordinates = source_coordinates(f32(gid.x), f32(gid.y));
    let sx = coordinates.x;
    let sy = coordinates.y;

    let idx = gid.y * params.dst_w + gid.x;

    if params.filter_code == 0u {
        output[idx] = select(sample_nearest(sx, sy), sample_nearest_projective(sx, sy), params.method != 0u);
    } else {
        output[idx] = sample_bilinear(sx, sy);
    }
}
