//! Geometry operations extracted from image.rs execute_op().
//!
//! These functions are standalone implementations of PIL-compatible geometry
//! operations (Resize, Crop, Rotate, Transpose, Thumbnail, Reduce) that operate
//! on DynamicImage and return new DynamicImage instances.

use crate::raster::{DynamicImage, GenericImageView};
use std::f64;

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::image_utils::raw_bytes_to_image;
use crate::ops::pil_resize::{
    pil_resize, pil_resize_boxed, precompute_coeffs_f64, precompute_coeffs_f64_boxed,
    premultiply_alpha, round_up, unpremultiply_alpha,
};
use crate::pipeline::{ResampleFilter, TransposeMethod};

// ── PIL-compatible filter kernels (f64 precision) ──

/// Box / Nearest-neighbor kernel.
fn f_kernel_box(x: f64) -> f64 {
    if x > -0.5 && x <= 0.5 { 1.0 } else { 0.0 }
}

/// Triangle (bilinear) kernel.
fn f_kernel_triangle(x: f64) -> f64 {
    let a = x.abs();
    if a < 1.0 { 1.0 - a } else { 0.0 }
}

/// Catmull-Rom (bicubic) kernel.
fn f_kernel_catrom(x: f64) -> f64 {
    let a = x.abs();
    if a < 1.0 {
        // Match Pillow's Resample.c Horner evaluation exactly.  Expanding
        // this polynomial with powi changes the last f64 bits for some
        // heterogeneous F samples, which becomes a visible f32 ULP after
        // ImagingResample stores the accumulated value.
        let t = 1.5f64.mul_add(a, -2.5);
        a.mul_add(t * a, 1.0)
    } else if a < 2.0 {
        let t = (a - 5.0).mul_add(a, 8.0);
        a.mul_add(t, -4.0) * -0.5
    } else {
        0.0
    }
}

/// Lanczos kernel with window `a`.
fn f_kernel_lanczos(x: f64, a: f64) -> f64 {
    // Resample.c uses `-a <= x && x < a`, so the negative support edge is
    // evaluated (and can produce a tiny signed coefficient) while the
    // positive edge is excluded.
    if x < -a || x >= a {
        return 0.0;
    }
    let sinc = |value: f64| {
        if value == 0.0 {
            1.0
        } else {
            let pix = value * std::f64::consts::PI;
            pix.sin() / pix
        }
    };
    sinc(x) * sinc(x / a)
}

/// Hamming kernel.
fn f_kernel_hamming(x: f64) -> f64 {
    if x.abs() >= 1.0 {
        0.0
    } else if x.abs() < 1e-10 {
        1.0
    } else {
        // Keep the numeric F/I paths aligned with Pillow's Hamming
        // windowed-sinc resampler (Resample.c), including its sinc factor.
        let pix = std::f64::consts::PI * x;
        // Resample.c uses sincos and contracts the `0.46f * cos + 0.54f`
        // window expression; preserving those operations matters for exact
        // f32 cancellation residuals.
        let (sin, cos) = pix.sin_cos();
        (sin / pix) * cos.mul_add(0.46_f32 as f64, 0.54_f32 as f64)
    }
}

fn f_kernel_lanczos3(x: f64) -> f64 {
    f_kernel_lanczos(x, 3.0)
}

/// Returns the scalar kernel and support used to build Pillow-compatible
/// coefficients. SIMD uses this only for its control-plane coefficient table;
/// pixel accumulation remains in the SIMD adapter.
pub(crate) fn resample_kernel(filter: &ResampleFilter) -> (fn(f64) -> f64, f64) {
    match filter {
        ResampleFilter::Nearest => (f_kernel_box, 0.5),
        ResampleFilter::Bilinear => (f_kernel_triangle, 1.0),
        ResampleFilter::Bicubic => (f_kernel_catrom, 2.0),
        ResampleFilter::Lanczos => (f_kernel_lanczos3, 3.0),
        ResampleFilter::Box => (f_kernel_box, 0.5),
        ResampleFilter::Hamming => (f_kernel_hamming, 1.0),
    }
}

// ── Helpers ──

// Pillow 12.2.0's arm64 FLOAT32 horizontal resampler uses scalar FMA for
// rows with at most 15 taps, then switches to complete 16-tap vector
// product/add blocks; any tail remains scalar FMA. The vertical resampler
// remains scalar FMA for every tap count. Keep the same split in the exact CPU
// implementation so heterogeneous wide reductions match the native Pillow
// build rather than the compiler's fused Rust loop.
const F_RESIZE_VECTOR_WIDTH: usize = 16;

fn f_resize_accumulate(
    accumulator: &mut f64,
    weight: f64,
    sample: f32,
    separate_product_add: bool,
) {
    let sample = f64::from(sample);
    if separate_product_add {
        // Keep the product out of the following add. LLVM may otherwise
        // contract this expression back into an FMA, defeating the arm64
        // wide-row contract that Pillow uses after 15 taps.
        let product = std::hint::black_box(weight * sample);
        *accumulator += product;
    } else {
        *accumulator = weight.mul_add(sample, *accumulator);
    }
}

// ── F-mode / I-mode resize ──

/// Resize an F-mode image (32-bit floats stored as RGBA8 bytes).
/// Uses PIL-compatible direct 2D interpolation with f64 precision,
/// so the result matches PIL's Image.resize() on mode F images.
fn resize_f(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    filter: &ResampleFilter,
) -> Result<DynamicImage, PilError> {
    let (sw, sh) = img.dimensions();

    if dst_w == 0 || dst_h == 0 || sw == 0 || sh == 0 {
        return Ok(DynamicImage::new_rgba8(dst_w, dst_h));
    }
    if (dst_w, dst_h) == (sw, sh) {
        return Ok(img.clone());
    }

    // F-mode images arrive here as their native Rgba8 storage.  Inspect that
    // storage directly before the generic `to_rgba8` clone and f32 decode;
    // Pillow's normalized F-mode resampler preserves an ordinary finite
    // constant sample exactly, so the cache-control resize can be filled
    // without either pass.  Keep negative zero out of this fast path: the
    // scalar path below preserves its sign bit when Pillow's convolution
    // arithmetic produces one.
    if let DynamicImage::ImageRgba8(rgba) = img {
        let raw = rgba.as_raw();
        if let Some(first) = raw.get(..4) {
            let bits = u32::from_le_bytes([first[0], first[1], first[2], first[3]]);
            let constant = f32::from_bits(bits);
            if constant.is_finite()
                && constant.to_bits() != (-0.0f32).to_bits()
                && raw.chunks_exact(4).all(|sample| {
                    u32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]) == bits
                })
            {
                let output_len = (dst_w as usize)
                    .saturating_mul(dst_h as usize)
                    .saturating_mul(4);
                let rgba_bytes = constant.to_le_bytes().repeat(output_len / 4);
                let out = crate::raster::RgbaImage::from_raw(dst_w, dst_h, rgba_bytes)
                    .expect("resize_f constant output shape must match its dimensions");
                return Ok(DynamicImage::ImageRgba8(out));
            }
        }
    }

    let rgba = img.to_rgba8();

    // Reinterpret each 4 RGBA bytes as a f32 (little-endian).
    let src_floats: Vec<f32> = rgba
        .pixels()
        .map(|p| f32::from_le_bytes([p[0], p[1], p[2], p[3]]))
        .collect();

    // Pillow's F-mode ImagingResample keeps an ordinary finite constant sample
    // unchanged because each normalized horizontal/vertical coefficient row
    // sums to one.  Avoid decoding the same value through both convolution
    // passes for this common cache-control workload; negative zero, non-finite,
    // and mixed samples retain the exact scalar path below.
    if let Some(&constant) = src_floats.first()
        && constant.is_finite()
        && constant.to_bits() != (-0.0f32).to_bits()
        && src_floats
            .iter()
            .all(|value| value.to_bits() == constant.to_bits())
    {
        let output_len = (dst_w as usize)
            .saturating_mul(dst_h as usize)
            .saturating_mul(4);
        let rgba_bytes = constant.to_le_bytes().repeat(output_len / 4);
        let out = crate::raster::RgbaImage::from_raw(dst_w, dst_h, rgba_bytes)
            .expect("resize_f constant output shape must match its dimensions");
        return Ok(DynamicImage::ImageRgba8(out));
    }

    // Pillow's F-mode NEAREST resize uses the affine point-sampling path,
    // not the BOX convolution path used by the other filters. The source
    // coordinate advances cumulatively from half a destination pixel, which
    // is observable when a narrow impulse falls between sampled rows.
    if matches!(filter, ResampleFilter::Nearest) {
        let scale_x = sw as f64 / dst_w as f64;
        let scale_y = sh as f64 / dst_h as f64;
        let mut xintab = Vec::with_capacity(dst_w as usize);
        let mut source_x = scale_x * 0.5;
        for _ in 0..dst_w {
            let sx = source_x as u32;
            xintab.push(sx.min(sw - 1));
            source_x += scale_x;
        }
        // Pillow's `libImaging/Geometry.c::ImagingScaleAffine` initializes the
        // vertical coordinate once and advances it after each row. Keep that
        // cumulative f64 sequence
        // instead of recomputing `(y + 0.5) * scale_y`: the two expressions
        // differ at exact-integer boundaries (for example, 2 / 7 reaches
        // 1.0 when multiplied directly but remains just below 1.0 after
        // three cumulative additions), changing the selected source row.
        let mut yintab = Vec::with_capacity(dst_h as usize);
        let mut source_y = scale_y * 0.5;
        for _ in 0..dst_h {
            let sy = source_y as u32;
            yintab.push(sy.min(sh - 1));
            source_y += scale_y;
        }
        let mut out_floats = vec![0.0f32; (dst_w * dst_h) as usize];
        #[cfg(feature = "parallel")]
        crate::par_rows_mut_typed!(
            &mut out_floats,
            dst_w as usize,
            dst_h as usize,
            |_row_start, _row_end, y, row| {
                let sy = yintab[y as usize];
                for (out, &sx) in row.iter_mut().zip(&xintab) {
                    *out = src_floats[(sy * sw + sx) as usize];
                }
            }
        );
        #[cfg(not(feature = "parallel"))]
        for (y, row) in out_floats.chunks_mut(dst_w as usize).enumerate() {
            let sy = yintab[y];
            for (out, &sx) in row.iter_mut().zip(&xintab) {
                *out = src_floats[(sy * sw + sx) as usize];
            }
        }
        let rgba_bytes: Vec<u8> = out_floats.iter().flat_map(|f| f.to_le_bytes()).collect();
        // The loop emits exactly four bytes per checked output pixel, so a
        // shape failure here indicates an internal arithmetic regression, not
        // a public input error.
        let out = crate::raster::RgbaImage::from_raw(dst_w, dst_h, rgba_bytes)
            .expect("resize_f nearest output shape must match its dimensions");
        return Ok(DynamicImage::ImageRgba8(out));
    }

    let (kernel, support) = resample_kernel(filter);
    let needs_horizontal = dst_w != sw;
    let needs_vertical = dst_h != sh;

    // Resample.c skips a pass when its destination axis already matches the
    // source axis. When a horizontal pass is needed, its FLOAT32 output is
    // stored in the intermediate image before the vertical pass reads it;
    // retaining f64 values here introduces tiny side lobes that Pillow does
    // not serialize.
    let mut intermediate = vec![0.0f32; (sh * dst_w) as usize];
    if needs_horizontal {
        let h_coeffs = precompute_coeffs_f64(dst_w, sw, kernel, support);
        #[cfg(feature = "parallel")]
        crate::par_rows_mut_typed!(
            &mut intermediate,
            dst_w as usize,
            sh as usize,
            |_row_start, _row_end, sy, row| {
                let src_row_base = (sy * sw) as usize;
                for (dx, output) in row.iter_mut().enumerate() {
                    let x0 = h_coeffs.xmin[dx];
                    let vector_product_count = (h_coeffs.weights[dx].len() / F_RESIZE_VECTOR_WIDTH)
                        * F_RESIZE_VECTOR_WIDTH;
                    let mut acc = 0.0f64;
                    for (offset, &weight) in h_coeffs.weights[dx].iter().enumerate() {
                        let sx = (x0 + offset as i64) as usize;
                        f_resize_accumulate(
                            &mut acc,
                            weight,
                            src_floats[src_row_base + sx],
                            offset < vector_product_count,
                        );
                    }
                    *output = acc as f32;
                }
            }
        );
        #[cfg(not(feature = "parallel"))]
        for (sy, row) in intermediate.chunks_mut(dst_w as usize).enumerate() {
            let src_row_base = sy * sw as usize;
            for (dx, output) in row.iter_mut().enumerate() {
                let x0 = h_coeffs.xmin[dx];
                let vector_product_count =
                    (h_coeffs.weights[dx].len() / F_RESIZE_VECTOR_WIDTH) * F_RESIZE_VECTOR_WIDTH;
                let mut acc = 0.0f64;
                for (offset, &weight) in h_coeffs.weights[dx].iter().enumerate() {
                    let sx = (x0 + offset as i64) as usize;
                    f_resize_accumulate(
                        &mut acc,
                        weight,
                        src_floats[src_row_base + sx],
                        offset < vector_product_count,
                    );
                }
                *output = acc as f32;
            }
        }
    } else {
        #[cfg(feature = "parallel")]
        crate::par_rows_mut_typed!(
            &mut intermediate,
            dst_w as usize,
            sh as usize,
            |_row_start, _row_end, sy, row| {
                let source_start = (sy * sw) as usize;
                row.copy_from_slice(&src_floats[source_start..source_start + sw as usize]);
            }
        );
        #[cfg(not(feature = "parallel"))]
        for (sy, row) in intermediate.chunks_mut(dst_w as usize).enumerate() {
            let source_start = sy * sw as usize;
            row.copy_from_slice(&src_floats[source_start..source_start + sw as usize]);
        }
    }

    let out_floats: Vec<f32> = if needs_vertical {
        let v_coeffs = precompute_coeffs_f64(dst_h, sh, kernel, support);
        let mut output = vec![0.0f32; (dst_w * dst_h) as usize];
        #[cfg(feature = "parallel")]
        crate::par_rows_mut_typed!(
            &mut output,
            dst_w as usize,
            dst_h as usize,
            |_row_start, _row_end, dy, row| {
                let y0 = v_coeffs.xmin[dy as usize];
                for (dx, output) in row.iter_mut().enumerate() {
                    let mut acc = 0.0f64;
                    for (offset, &weight) in v_coeffs.weights[dy as usize].iter().enumerate() {
                        let sy = (y0 + offset as i64) as usize;
                        acc =
                            weight.mul_add(f64::from(intermediate[sy * dst_w as usize + dx]), acc);
                    }
                    // Pillow's `libImaging/Resample.c::ImagingResampleVertical_32bpc`
                    // stores the float32 accumulator directly; do not
                    // canonicalize a negative zero produced by the sum.
                    *output = acc as f32;
                }
            }
        );
        #[cfg(not(feature = "parallel"))]
        for (dy, row) in output.chunks_mut(dst_w as usize).enumerate() {
            let y0 = v_coeffs.xmin[dy];
            for (dx, output) in row.iter_mut().enumerate() {
                let mut acc = 0.0f64;
                for (offset, &weight) in v_coeffs.weights[dy].iter().enumerate() {
                    let sy = (y0 + offset as i64) as usize;
                    acc = weight.mul_add(f64::from(intermediate[sy * dst_w as usize + dx]), acc);
                }
                // Keep the sign of zero, matching the scalar C path.
                *output = acc as f32;
            }
        }
        output
    } else {
        intermediate
    };

    // Re-pack each f32 as 4 RGBA8 bytes (little-endian).
    let rgba_bytes: Vec<u8> = out_floats.iter().flat_map(|f| f.to_le_bytes()).collect();
    let out = crate::raster::RgbaImage::from_raw(dst_w, dst_h, rgba_bytes)
        .expect("resize_f output shape must match its dimensions");
    Ok(DynamicImage::ImageRgba8(out))
}

/// Resize an I-mode image (32-bit signed integers stored as RGBA8 bytes LE).
/// Uses PIL's two-pass separable approach matching ImagingResample.
fn resize_i(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    filter: &ResampleFilter,
) -> Result<DynamicImage, PilError> {
    let rgba = img.to_rgba8();
    let (sw, sh) = rgba.dimensions();

    if dst_w == 0 || dst_h == 0 || sw == 0 || sh == 0 {
        return Ok(DynamicImage::new_rgba8(dst_w, dst_h));
    }
    if (dst_w, dst_h) == (sw, sh) {
        return Ok(img.clone());
    }

    // Reinterpret each 4 RGBA bytes as i32 (little-endian).
    let src_ints: Vec<i32> = rgba
        .pixels()
        .map(|p| i32::from_le_bytes([p[0], p[1], p[2], p[3]]))
        .collect();

    let (kernel, support) = resample_kernel(filter);
    let sw_f = sw as f64;
    let sh_f = sh as f64;
    let dw_f = dst_w as f64;
    let dh_f = dst_h as f64;

    // PIL-compatible scale factor for kernel widening during downscaling
    let _sx_scale = (sw_f / dw_f).max(1.0);
    let _sy_scale = (sh_f / dh_f).max(1.0);

    let n = (dst_w * dst_h) as usize;

    // NEAREST: Pillow's mode-I path uses the same half-destination-pixel
    // point samples as its native point resampler:
    //   sx = (int)((dx + 0.5) * sw/dw)
    //   sy = (int)((dy + 0.5) * sh/dh)
    //
    // The older affine-style formula shifts a downsampled I image by one
    // source row/column at the leading edge.  That only becomes visible when
    // a public pipeline composes a typed filter with a non-integral resize,
    // so keep the correction in the native I branch rather than changing the
    // byte-image transform contract.
    if matches!(filter, ResampleFilter::Nearest) {
        let mut out_ints = vec![0i32; n];
        #[cfg(feature = "parallel")]
        crate::par_rows_mut_typed!(
            &mut out_ints,
            dst_w as usize,
            dst_h as usize,
            |_row_start, _row_end, dy, row| {
                let sy = ((f64::from(dy) + 0.5) * sh_f / dh_f).floor() as i64;
                let sy = sy.clamp(0, sh as i64 - 1) as u32;
                for (dx, output) in row.iter_mut().enumerate() {
                    let sx = ((dx as f64 + 0.5) * sw_f / dw_f).floor() as i64;
                    let sx = sx.clamp(0, sw as i64 - 1) as u32;
                    *output = src_ints[(sy * sw + sx) as usize];
                }
            }
        );
        #[cfg(not(feature = "parallel"))]
        for (dy, row) in out_ints.chunks_mut(dst_w as usize).enumerate() {
            let sy = ((dy as f64 + 0.5) * sh_f / dh_f).floor() as i64;
            let sy = sy.clamp(0, sh as i64 - 1) as u32;
            for (dx, output) in row.iter_mut().enumerate() {
                let sx = ((dx as f64 + 0.5) * sw_f / dw_f).floor() as i64;
                let sx = sx.clamp(0, sw as i64 - 1) as u32;
                *output = src_ints[(sy * sw + sx) as usize];
            }
        }
        let rgba_bytes: Vec<u8> = out_ints.iter().flat_map(|v| v.to_le_bytes()).collect();
        let out = crate::raster::RgbaImage::from_raw(dst_w, dst_h, rgba_bytes)
            .expect("resize_i output shape must match its dimensions");
        return Ok(DynamicImage::ImageRgba8(out));
    }

    // PIL: for INT32 images, coefficients stay as double-precision (not fixed-point).
    // Use f64 accumulation + ROUND_UP matching PIL's ImagingResample for 32-bit types.
    let h_coeffs_f64 = precompute_coeffs_f64(dst_w, sw, kernel, support);
    let v_coeffs_f64 = precompute_coeffs_f64(dst_h, sh, kernel, support);

    // ImagingResample stores the horizontal INT32 pass in an INT32 image
    // before the vertical pass. Keeping this buffer as f64 changes overflow
    // cases (the C cast saturates to INT32_MIN on the supported platforms).
    let mut intermediate: Vec<i32> = vec![0; (sh * dst_w) as usize];

    // Horizontal pass: f64 accumulation, matching PIL's double-precision path
    #[cfg(feature = "parallel")]
    crate::par_rows_mut_typed!(
        &mut intermediate,
        dst_w as usize,
        sh as usize,
        |_row_start, _row_end, sy, row| {
            let src_row_base = (sy * sw) as usize;
            for (dx, output) in row.iter_mut().enumerate() {
                let x0 = h_coeffs_f64.xmin[dx];
                let mut acc: f64 = 0.0;
                for (cix, &weight) in h_coeffs_f64.weights[dx].iter().enumerate() {
                    let sx = (x0 + cix as i64) as usize;
                    acc = weight.mul_add(f64::from(src_ints[src_row_base + sx]), acc);
                }
                *output = round_up(acc) as i32;
            }
        }
    );
    #[cfg(not(feature = "parallel"))]
    for (sy, row) in intermediate.chunks_mut(dst_w as usize).enumerate() {
        let src_row_base = sy * sw as usize;
        for (dx, output) in row.iter_mut().enumerate() {
            let x0 = h_coeffs_f64.xmin[dx];
            let mut acc: f64 = 0.0;
            for (cix, &weight) in h_coeffs_f64.weights[dx].iter().enumerate() {
                let sx = (x0 + cix as i64) as usize;
                acc = weight.mul_add(f64::from(src_ints[src_row_base + sx]), acc);
            }
            *output = round_up(acc) as i32;
        }
    }

    // Vertical pass
    let mut out_ints = vec![0i32; n];
    #[cfg(feature = "parallel")]
    crate::par_rows_mut_typed!(
        &mut out_ints,
        dst_w as usize,
        dst_h as usize,
        |_row_start, _row_end, dy, row| {
            let y0 = v_coeffs_f64.xmin[dy as usize];
            for (dx, output) in row.iter_mut().enumerate() {
                let mut acc: f64 = 0.0;
                for (cix, &weight) in v_coeffs_f64.weights[dy as usize].iter().enumerate() {
                    let sy = (y0 + cix as i64) as usize;
                    acc = weight.mul_add(f64::from(intermediate[(sy * dst_w as usize) + dx]), acc);
                }
                *output = round_up(acc) as i32;
            }
        }
    );
    #[cfg(not(feature = "parallel"))]
    for (dy, row) in out_ints.chunks_mut(dst_w as usize).enumerate() {
        let y0 = v_coeffs_f64.xmin[dy];
        for (dx, output) in row.iter_mut().enumerate() {
            let mut acc: f64 = 0.0;
            for (cix, &weight) in v_coeffs_f64.weights[dy].iter().enumerate() {
                let sy = (y0 + cix as i64) as usize;
                acc = weight.mul_add(f64::from(intermediate[(sy * dst_w as usize) + dx]), acc);
            }
            *output = round_up(acc) as i32;
        }
    }

    // Re-pack each i32 as 4 RGBA8 bytes (little-endian).
    let rgba_bytes: Vec<u8> = out_ints.iter().flat_map(|v| v.to_le_bytes()).collect();
    let out = crate::raster::RgbaImage::from_raw(dst_w, dst_h, rgba_bytes)
        .ok_or_else(|| PilError::ValueError("resize_i: failed to create output buffer".into()))?;
    Ok(DynamicImage::ImageRgba8(out))
}

/// Resize an `I` image through a fractional source box.
///
/// Pillow's reducing-gap thumbnail path passes the original source box after
/// the integer reduction. The intermediate image is still an INT32 image, so
/// both separable passes round their f64 sums to i32 before the next pass.
fn resize_i_boxed(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    box_left: f64,
    box_top: f64,
    box_right: f64,
    box_bottom: f64,
    filter: ResampleFilter,
) -> Result<DynamicImage, PilError> {
    let rgba = img.to_rgba8();
    let (source_width, source_height) = rgba.dimensions();
    if dst_w == 0 || dst_h == 0 || source_width == 0 || source_height == 0 {
        return Ok(DynamicImage::new_rgba8(dst_w, dst_h));
    }
    let source: Vec<i32> = rgba
        .pixels()
        .map(|pixel| i32::from_le_bytes([pixel[0], pixel[1], pixel[2], pixel[3]]))
        .collect();
    let horizontal = precompute_coeffs_f64_boxed(dst_w, source_width, box_left, box_right, filter);
    let vertical = precompute_coeffs_f64_boxed(dst_h, source_height, box_top, box_bottom, filter);

    let mut intermediate = vec![0i32; (source_height * dst_w) as usize];
    for source_y in 0..source_height as usize {
        let source_row = source_y * source_width as usize;
        let intermediate_row = source_y * dst_w as usize;
        for output_x in 0..dst_w as usize {
            let x0 = horizontal.xmin[output_x];
            let mut sum = 0.0f64;
            for (tap, &weight) in horizontal.weights[output_x].iter().enumerate() {
                let source_x = (x0 + tap as i64) as usize;
                sum = weight.mul_add(f64::from(source[source_row + source_x]), sum);
            }
            intermediate[intermediate_row + output_x] = round_up(sum) as i32;
        }
    }

    let mut output = vec![0i32; (dst_w * dst_h) as usize];
    for output_y in 0..dst_h as usize {
        let y0 = vertical.xmin[output_y];
        let output_row = output_y * dst_w as usize;
        for output_x in 0..dst_w as usize {
            let mut sum = 0.0f64;
            for (tap, &weight) in vertical.weights[output_y].iter().enumerate() {
                let source_y = (y0 + tap as i64) as usize;
                sum = weight.mul_add(
                    f64::from(intermediate[source_y * dst_w as usize + output_x]),
                    sum,
                );
            }
            output[output_row + output_x] = round_up(sum) as i32;
        }
    }
    let bytes: Vec<u8> = output
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    let out = crate::raster::RgbaImage::from_raw(dst_w, dst_h, bytes).ok_or_else(|| {
        PilError::ValueError("resize_i boxed: failed to create output buffer".into())
    })?;
    Ok(DynamicImage::ImageRgba8(out))
}

// ── Generic rotation & transform helpers (mode-aware) ──

fn affine_nearest_fixed(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    channels: usize,
    affine: [f64; 6],
    fill: (u8, u8, u8, u8),
    output: &mut [u8],
) {
    // Pillow's ImagingTransformAffine nearest path rounds the six affine
    // coefficients to signed 16.16 values once, then advances those integers
    // across each output row. Recomputing the same coordinates as f64 changes
    // which source pixel wins at exact integer boundaries.
    let fixed = |value: f64| (value.mul_add(65_536.0, 0.5).floor()) as i64;
    let [a, b, c, d, e, f] = affine;
    let step_x_x = fixed(a);
    let step_y_x = fixed(b);
    let step_x_y = fixed(d);
    let step_y_y = fixed(e);
    let origin_x = fixed(c + a * 0.5 + b * 0.5);
    let origin_y = fixed(f + d * 0.5 + e * 0.5);
    let (source_width, source_height) = source_size;
    let (destination_width, destination_height) = destination_size;
    if destination_width == 0 || destination_height == 0 {
        return;
    }

    let process_row = |y: u32, row: &mut [u8]| {
        let mut source_x = origin_x + i64::from(y) * step_y_x;
        let mut source_y = origin_y + i64::from(y) * step_y_y;
        for x in 0..destination_width {
            let input_x = source_x >> 16;
            let input_y = source_y >> 16;
            let output_index = x as usize * channels;
            if input_x >= 0
                && input_x < i64::from(source_width)
                && input_y >= 0
                && input_y < i64::from(source_height)
            {
                let input_index =
                    (input_y as u32 * source_width + input_x as u32) as usize * channels;
                row[output_index..output_index + channels]
                    .copy_from_slice(&source[input_index..input_index + channels]);
            } else {
                for channel in 0..channels.min(4) {
                    row[output_index + channel] = if channels == 2 && channel == 1 {
                        // LA/PA normalize their second sample as alpha in
                        // fill.3; fill.1 is only the duplicated luma/index
                        // component used by the host-neutral color record.
                        fill.3
                    } else {
                        match channel {
                            0 => fill.0,
                            1 => fill.1,
                            2 => fill.2,
                            _ => fill.3,
                        }
                    };
                }
            }
            source_x += step_x_x;
            source_y += step_x_y;
        }
    };

    #[cfg(feature = "parallel")]
    const PARALLEL_PIXEL_THRESHOLD: usize = 512 * 512;
    let output_stride = destination_width as usize * channels;
    #[cfg(feature = "parallel")]
    if (destination_width as usize).saturating_mul(destination_height as usize)
        >= PARALLEL_PIXEL_THRESHOLD
    {
        crate::par_rows_mut!(
            output,
            output_stride,
            destination_height as usize,
            |_row_start, _row_end, y, row| {
                process_row(y, row);
            }
        );
    } else {
        for (y, row) in output
            .chunks_exact_mut(output_stride)
            .take(destination_height as usize)
            .enumerate()
        {
            process_row(y as u32, row);
        }
    }
    #[cfg(not(feature = "parallel"))]
    for (y, row) in output
        .chunks_exact_mut(output_stride)
        .take(destination_height as usize)
        .enumerate()
    {
        process_row(y as u32, row);
    }
}

/// Rotate an image by an arbitrary angle, working on the native number of channels.
/// When `nearest` is true, uses nearest-neighbor sampling.
fn rotate_arbitrary_generic(
    img: &DynamicImage,
    angle: f64,
    expand: bool,
    fill: Option<(u8, u8, u8, u8)>,
    nearest: bool,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let channels = img.color().channel_count() as usize;
    let (w, h) = img.dimensions();
    let sw = w as f64;
    let sh = h as f64;
    // Pillow builds the reverse affine transform (destination -> source),
    // rounding the trigonometric coefficients to 15 decimal places before
    // calculating the expanded canvas.
    // Pillow's affine coefficients map destination pixels back into the
    // source image. The inverse mapping uses the negative angle; using the
    // forward sign mirrors the exposed fill region for arbitrary angles.
    let rad = -angle.to_radians();
    let aff_a = crate::ops::rotate::round_rotate_coefficient(rad.cos());
    let aff_b = crate::ops::rotate::round_rotate_coefficient(rad.sin());
    let aff_d = crate::ops::rotate::round_rotate_coefficient(-rad.sin());
    let aff_e = aff_a;
    // Pillow's Image.rotate composes post-translation into the reverse affine
    // matrix before calculating expand bounds; applying it after sampling
    // changes both the canvas size and the selected source pixels.
    let (center_x, center_y) = center.unwrap_or((sw / 2.0, sh / 2.0));
    let (translate_x, translate_y) = translate.unwrap_or((0.0, 0.0));
    let mut aff_c =
        aff_a * (-center_x - translate_x) + aff_b * (-center_y - translate_y) + center_x;
    let mut aff_f =
        aff_d * (-center_x - translate_x) + aff_e * (-center_y - translate_y) + center_y;
    let transform =
        |x: f64, y: f64, c: f64, f: f64| (aff_a * x + aff_b * y + c, aff_d * x + aff_e * y + f);

    // Pillow rounds each outer edge independently. This differs from taking
    // ceil(max - min) whenever the transformed minimum is fractional.
    let corners = [(0.0, 0.0), (sw, 0.0), (sw, sh), (0.0, sh)];
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(cx, cy) in &corners {
        let (rx, ry) = transform(cx, cy, aff_c, aff_f);
        min_x = min_x.min(rx);
        max_x = max_x.max(rx);
        min_y = min_y.min(ry);
        max_y = max_y.max(ry);
    }
    let (dw, dh) = if expand {
        (
            (max_x.ceil() - min_x.floor()) as u32,
            (max_y.ceil() - min_y.floor()) as u32,
        )
    } else {
        (w, h)
    };

    if expand {
        let shift_x = -(dw as f64 - sw) / 2.0;
        let shift_y = -(dh as f64 - sh) / 2.0;
        (aff_c, aff_f) = transform(shift_x, shift_y, aff_c, aff_f);
    }

    let raw = img.as_bytes();
    let fill_color = fill.unwrap_or((0, 0, 0, 0));
    let pa_mode = explicit_mode == Some("PA");

    let mut out = CheckedDims::new(dw, dh, channels as u8)?.alloc_buffer();

    if nearest {
        affine_nearest_fixed(
            raw,
            (w, h),
            (dw, dh),
            channels,
            [aff_a, aff_b, aff_c, aff_d, aff_e, aff_f],
            fill_color,
            &mut out,
        );
    } else if matches!(explicit_mode, Some("F") | Some("I")) && channels == 4 {
        rotate_arbitrary_scalar(
            raw,
            (w, h),
            (dw, dh),
            channels,
            [aff_a, aff_b, aff_c, aff_d, aff_e, aff_f],
            fill_color,
            explicit_mode == Some("F"),
            &mut out,
        );
    } else {
        for dy in 0..dh {
            for dx in 0..dw {
                // Geometry.c first maps the destination pixel center and the
                // bilinear filter then converts that center to the source
                // pixel's corner coordinate by subtracting 0.5. Keep the
                // two stages explicit so arbitrary rotations use the same
                // source coordinates as Pillow's affine kernel.
                let (sx_rel, sy_rel) = transform(dx as f64 + 0.5, dy as f64 + 0.5, aff_c, aff_f);
                let (sx_rel, sy_rel) = if explicit_mode == Some("PA") {
                    // The palette-preserving PA pipeline has already
                    // expressed its index samples in pixel-center space.
                    // Keep that established convention while regular byte
                    // modes follow Geometry.c's filter-side subtraction.
                    (sx_rel, sy_rel)
                } else {
                    (sx_rel - 0.5, sy_rel - 0.5)
                };

                let out_idx = (dy * dw + dx) as usize * channels;

                // Pillow's regular byte bilinear filter has a half-pixel
                // footprint. PA keeps its established index/alpha sampling
                // convention, so retain its integer-domain bounds here.
                let in_filter_support = if pa_mode {
                    sx_rel >= 0.0 && sx_rel < sw && sy_rel >= 0.0 && sy_rel < sh
                } else {
                    sx_rel >= -0.5 && sx_rel < sw - 0.5 && sy_rel >= -0.5 && sy_rel < sh - 0.5
                };
                if in_filter_support {
                    let sx_rel = if pa_mode {
                        sx_rel
                    } else {
                        sx_rel.clamp(0.0, sw - 1.0)
                    };
                    let sy_rel = if pa_mode {
                        sy_rel
                    } else {
                        sy_rel.clamp(0.0, sh - 1.0)
                    };
                    let sx = sx_rel.floor() as u32;
                    let sy = sy_rel.floor() as u32;
                    let fx = sx_rel - sx as f64;
                    let fy = sy_rel - sy as f64;
                    let sx1 = (sx + 1).min(w - 1);
                    let sy1 = (sy + 1).min(h - 1);
                    for c in 0..channels {
                        let p00 = raw[(sy * w + sx) as usize * channels + c] as f64;
                        let p10 = raw[(sy * w + sx1) as usize * channels + c] as f64;
                        let p01 = raw[(sy1 * w + sx) as usize * channels + c] as f64;
                        let p11 = raw[(sy1 * w + sx1) as usize * channels + c] as f64;
                        // Evaluate the two horizontal blends first for
                        // regular byte modes. This preserves an exact
                        // constant edge sample in f64; expanding all four
                        // products can leave a value such as
                        // 14.999999999999998, which C's fixed-point
                        // accumulator reports as 15 after truncation. PA
                        // retains its established arithmetic ordering.
                        let v = if pa_mode {
                            (1.0 - fx) * (1.0 - fy) * p00
                                + fx * (1.0 - fy) * p10
                                + (1.0 - fx) * fy * p01
                                + fx * fy * p11
                        } else {
                            (1.0 - fy) * ((1.0 - fx) * p00 + fx * p10)
                                + fy * ((1.0 - fx) * p01 + fx * p11)
                        };
                        // Geometry.c's UINT8 bilinear filter stores a C cast
                        // of the interpolated value, which truncates toward
                        // zero. This matters for the premultiplied alpha
                        // round trip used by LA/RGBA transforms.
                        out[out_idx + c] = v as u8;
                    }
                } else {
                    for c in 0..channels.min(4) {
                        out[out_idx + c] = match c {
                            0 => fill_color.0,
                            1 => fill_color.1,
                            2 => fill_color.2,
                            _ => fill_color.3,
                        };
                    }
                }
            }
        }
    }

    raw_bytes_to_image(dw, dh, out, channels)
}

/// Apply Pillow's bilinear transform to native four-byte `I`/`F` samples.
///
/// These modes are represented by four bytes in the backend image, but the
/// transform kernel operates on one signed 32-bit or float32 sample. Pillow's
/// scalar affine kernels accumulate the bilinear value in double precision;
/// `F` then stores float32 and `I` truncates the result toward zero.
fn rotate_arbitrary_scalar(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    channels: usize,
    affine: [f64; 6],
    fill: (u8, u8, u8, u8),
    is_float: bool,
    output: &mut [u8],
) {
    let [a, b, c, d, e, f] = affine;
    let (source_width, source_height) = source_size;
    let (destination_width, destination_height) = destination_size;
    let scalar_fill = if is_float {
        f32::from_le_bytes([fill.0, fill.1, fill.2, fill.3]) as f64
    } else {
        i32::from_le_bytes([fill.0, fill.1, fill.2, fill.3]) as f64
    };

    for dy in 0..destination_height {
        for dx in 0..destination_width {
            let sx = a * (dx as f64 + 0.5) + b * (dy as f64 + 0.5) + c - 0.5;
            let sy = d * (dx as f64 + 0.5) + e * (dy as f64 + 0.5) + f - 0.5;
            let output_index = (dy * destination_width + dx) as usize * channels;

            let value = if sx >= 0.0
                && sx < source_width as f64
                && sy >= 0.0
                && sy < source_height as f64
            {
                let x0 = sx.floor() as u32;
                let y0 = sy.floor() as u32;
                let x1 = (x0 + 1).min(source_width - 1);
                let y1 = (y0 + 1).min(source_height - 1);
                let fx = sx - x0 as f64;
                let fy = sy - y0 as f64;
                let read = |x: u32, y: u32| {
                    let index = ((y * source_width + x) as usize) * channels;
                    if is_float {
                        f32::from_le_bytes([
                            source[index],
                            source[index + 1],
                            source[index + 2],
                            source[index + 3],
                        ]) as f64
                    } else {
                        i32::from_le_bytes([
                            source[index],
                            source[index + 1],
                            source[index + 2],
                            source[index + 3],
                        ]) as f64
                    }
                };
                let p00 = read(x0, y0);
                let p10 = read(x1, y0);
                let p01 = read(x0, y1);
                let p11 = read(x1, y1);
                (1.0 - fx) * (1.0 - fy) * p00
                    + fx * (1.0 - fy) * p10
                    + (1.0 - fx) * fy * p01
                    + fx * fy * p11
            } else {
                scalar_fill
            };

            let bytes = if is_float {
                (value as f32).to_le_bytes()
            } else {
                (value as i32).to_le_bytes()
            };
            output[output_index..output_index + 4].copy_from_slice(&bytes);
        }
    }
}

// ── Execute geometry ops ──

/// Execute a Resize operation.
/// F-mode uses float interpolation; I-mode uses int32 interpolation;
/// all other modes use the standard PIL-compatible two-pass resize.
pub fn execute_resize(
    img: &DynamicImage,
    w: u32,
    h: u32,
    filter: &ResampleFilter,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // Only use F/I resize paths when the image is already stored as Rgba8
    // (4 bytes per pixel), meaning it has been converted to F/I mode already.
    // If the image is still RGB (3 bytes per pixel), use normal resize regardless
    // of explicit_mode, because the F/I convert hasn't happened yet in the pipeline.
    if explicit_mode == Some("F") && matches!(img, DynamicImage::ImageRgba8(_)) {
        return resize_f(img, w, h, filter);
    }
    if explicit_mode == Some("I") && matches!(img, DynamicImage::ImageRgba8(_)) {
        return resize_i(img, w, h, filter);
    }
    // Mode "1": convert to L, resize, then convert back to "1" by thresholding at 128.
    // PIL's C extension handles mode "1" internally with bit-unpacking, but our
    // pil_resize works on Luma8 which has equivalent data. The two-pass BOX filter
    // (NEAREST) produces averages; the conversion back to "1" thresholds them.
    if explicit_mode == Some("1") {
        // Image is already Luma8 with {0,255}. Resize via pil_resize (which uses
        // the BOX filter for NEAREST, matching PIL's behavior for mode "1").
        let result = pil_resize(img, w, h, *filter, explicit_mode);
        // After resize, threshold back to binary {0, 255}: pixel >= 128 => 255 else 0
        let gray = result.to_luma8();
        let (rw, rh) = gray.dimensions();
        let mut out = crate::raster::GrayImage::new(rw, rh);
        for (op, ip) in out.pixels_mut().zip(gray.pixels()) {
            op[0] = if ip[0] >= 128 { 255 } else { 0 };
        }
        return Ok(preserve_mode(img, DynamicImage::ImageLuma8(out)));
    }
    let result = pil_resize(img, w, h, *filter, explicit_mode);
    Ok(preserve_mode(img, result))
}

/// Execute a Crop operation.
///
/// `Image::crop` owns Pillow's signed/out-of-bounds normalization before it
/// queues `PipelineOp::Crop`. The compute operation therefore receives only a
/// positive, in-bounds box; keeping padding logic here would duplicate that
/// public contract and leave an unreachable second crop implementation.
pub fn execute_crop(
    img: &DynamicImage,
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
) -> Result<DynamicImage, PilError> {
    let (iw, ih) = (img.width(), img.height());
    debug_assert!(
        left < iw && top < ih && right <= iw && bottom <= ih,
        "crop pipeline coordinates must be normalized before execution"
    );
    let width = right.checked_sub(left).ok_or_else(|| {
        PilError::InternalError("crop pipeline width underflow after normalization".into())
    })?;
    let height = bottom.checked_sub(top).ok_or_else(|| {
        PilError::InternalError("crop pipeline height underflow after normalization".into())
    })?;

    // Native byte layouts can copy complete rows directly.  Keep the image
    // crate path for typed samples such as I;16, whose byte stride is wider
    // than its logical channel count and therefore needs its own layout
    // handling.
    let channels = img.color().channel_count() as usize;
    if matches!(
        img.color(),
        crate::raster::ColorType::L8
            | crate::raster::ColorType::La8
            | crate::raster::ColorType::Rgb8
            | crate::raster::ColorType::Rgba8
    ) {
        let source = img.as_bytes();
        let source_stride = iw as usize * channels;
        let output_stride = width as usize * channels;
        let mut output = CheckedDims::new(width, height, channels as u8)?.alloc_buffer();
        #[cfg(feature = "parallel")]
        if output.len() >= 4 * 1024 * 1024 && output_stride != 0 {
            crate::par_rows_mut!(
                &mut output,
                output_stride,
                height as usize,
                |_row_start, _row_end, y, row| {
                    let source_start =
                        (top as usize + y as usize) * source_stride + left as usize * channels;
                    row.copy_from_slice(&source[source_start..source_start + output_stride]);
                }
            );
        } else {
            for y in 0..height as usize {
                let source_start = (top as usize + y) * source_stride + left as usize * channels;
                let output_start = y * output_stride;
                output[output_start..output_start + output_stride]
                    .copy_from_slice(&source[source_start..source_start + output_stride]);
            }
        }
        #[cfg(not(feature = "parallel"))]
        for y in 0..height as usize {
            let source_start = (top as usize + y) * source_stride + left as usize * channels;
            let output_start = y * output_stride;
            output[output_start..output_start + output_stride]
                .copy_from_slice(&source[source_start..source_start + output_stride]);
        }
        return raw_bytes_to_image(width, height, output, channels);
    }
    Ok(img.crop_imm(left, top, width, height))
}

/// Execute a Rotate operation.
/// Fast-path for 90-degree multiples; otherwise uses arbitrary rotation.
/// Rotate 90 or 270 degrees without expanding (clip to original size).
/// Matches PIL's behavior: rotate and center the result in the original canvas,
/// filling exposed areas with the fill color.
fn rotate_90_non_expand(
    img: &DynamicImage,
    clockwise_270: bool,
    fill: Option<(u8, u8, u8, u8)>,
) -> Result<DynamicImage, PilError> {
    let channels = img.color().channel_count() as usize;
    let (w, h) = img.dimensions();
    let raw = img.as_bytes();
    let fill_color = fill.unwrap_or((0, 0, 0, 0));

    // Initialize output with fill color
    let fill_pixel: Vec<u8> = match channels {
        1 => vec![fill_color.0],
        2 => vec![fill_color.0, fill_color.3],
        3 => vec![fill_color.0, fill_color.1, fill_color.2],
        _ => vec![fill_color.0, fill_color.1, fill_color.2, fill_color.3],
    };
    let dims = CheckedDims::new(w, h, channels as u8)?;
    let mut out = fill_pixel.repeat(dims.total_pixels());

    // Pillow centers the expanded 90-degree result with independent edge
    // rounding. Rust integer division truncates negative halves toward zero,
    // so calculate the floor/ceil offsets explicitly for odd dimension gaps.
    let width_gap = w as f64 - h as f64;
    let height_gap = h as f64 - w as f64;
    let dx_off_90 = (width_gap / 2.0).floor() as i32;
    let dy_off_90 = (height_gap / 2.0).ceil() as i32;
    let dx_off_270 = (width_gap / 2.0).ceil() as i32;
    let dy_off_270 = (height_gap / 2.0).floor() as i32;

    for dy in 0..h {
        for dx in 0..w {
            let (sx, sy) = if clockwise_270 {
                // 270° CCW = 90° CW: expand_true uses input(dy, h-1-dx)
                // dx_true = dx - dx_off, dy_true = dy - dy_off
                // sx = dy_true = dy - dy_off, sy = h - 1 - dx_true
                //     = h - 1 - dx + dx_off
                (
                    dy as i32 - dy_off_270,
                    h as i32 - 1 - dx as i32 + dx_off_270,
                )
            } else {
                // 90° CCW: expand_true uses input(w-1-dy, dx)
                // dx_true = dx - dx_off, dy_true = dy - dy_off
                // sx = w - 1 - dy_true = w - 1 - dy + dy_off, sy = dx_true = dx - dx_off
                (w as i32 - 1 - dy as i32 + dy_off_90, dx as i32 - dx_off_90)
            };
            if sx >= 0 && sx < w as i32 && sy >= 0 && sy < h as i32 {
                let in_idx = (sy as u32 * w + sx as u32) as usize * channels;
                let out_idx = (dy * w + dx) as usize * channels;
                out[out_idx..out_idx + channels].copy_from_slice(&raw[in_idx..in_idx + channels]);
            }
        }
    }

    // Create output dynamic image
    raw_bytes_to_image(w, h, out, channels)
}

pub fn execute_rotate(
    img: &DynamicImage,
    angle: f64,
    expand: bool,
    fill: Option<(u8, u8, u8, u8)>,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
    requested_nearest: bool,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let has_custom_transform = center.is_some() || translate.is_some();
    let nearest = requested_nearest || explicit_mode == Some("P") || explicit_mode == Some("1");
    // Pillow's PA transform path samples the raw index/alpha bands directly;
    // unlike LA and RGBA, it does not use a premultiplied intermediate. The
    // public fillcolor arrives as (index, index, index, alpha) so the generic
    // four-component record can represent it; normalize that record to the
    // native two-band layout before either nearest or interpolated sampling.
    let fill = if explicit_mode == Some("PA") {
        fill.map(|(index, _, _, alpha)| (index, alpha, 0, alpha))
    } else {
        fill
    };
    // Fast path: exact 90-degree multiples
    // PIL rotates counterclockwise; image crate rotates clockwise.
    // PIL 90° CCW = image crate 270° CW, PIL 270° CCW = image crate 90° CW.
    // For 90/270 with expand=False, compute the clipped result directly
    // by pasting the expanded result centered in the original-sized canvas.
    let normalized_angle = angle.rem_euclid(360.0);
    let result = if !has_custom_transform && normalized_angle.abs() <= f64::EPSILON {
        // Pillow's public rotate() returns an exact copy at angle 0 (and
        // every multiple of 360) before considering the requested filter.
        // Keep this fast path ahead of filtered affine sampling so LA/RGBA
        // bytes and alpha channels are not rounded needlessly.
        img.clone()
    } else if !has_custom_transform && normalized_angle == 90.0 {
        if expand {
            img.rotate270() // 270° CW = 90° CCW (PIL)
        } else {
            rotate_90_non_expand(img, false, fill)?
        }
    } else if !has_custom_transform && normalized_angle == 180.0 {
        img.rotate180()
    } else if !has_custom_transform && normalized_angle == 270.0 {
        if expand {
            img.rotate90() // 90° CW = 270° CCW (PIL)
        } else {
            rotate_90_non_expand(img, true, fill)?
        }
    } else {
        // Pillow routes non-nearest LA/RGBA transforms through their
        // premultiplied modes (La/RGBa), then converts the interpolated
        // samples back. RGBa is already premultiplied and must remain a direct
        // native-channel path, just as in pil_resize.
        let needs_alpha_roundtrip = !nearest
            && !matches!(
                explicit_mode,
                Some("PA") | Some("RGBa") | Some("F") | Some("I")
            )
            && matches!(
                img.color(),
                crate::raster::ColorType::La8 | crate::raster::ColorType::Rgba8
            );
        let work = if needs_alpha_roundtrip {
            premultiply_alpha(img)
        } else {
            img.clone()
        };
        let rotated = rotate_arbitrary_generic(
            &work,
            angle,
            expand,
            fill,
            nearest,
            center,
            translate,
            explicit_mode,
        )?;
        if needs_alpha_roundtrip {
            unpremultiply_alpha(&rotated)
        } else {
            rotated
        }
    };
    Ok(preserve_mode(img, result))
}

/// Execute a Transpose operation.
const TRANSPOSE_TILE_SIZE: u32 = 32;
const TRANSPOSE_TILE_THRESHOLD_PIXELS: usize = 256 * 1024;

#[inline]
fn should_tile_transpose(width: u32, height: u32) -> bool {
    width >= TRANSPOSE_TILE_SIZE
        && height >= TRANSPOSE_TILE_SIZE
        && (width as usize).saturating_mul(height as usize) >= TRANSPOSE_TILE_THRESHOLD_PIXELS
}

/// Transpose a large byte image in bounded output-row tiles.
///
/// A full output row fixes one source column, so the old row-oriented loop
/// reads the source with a `width * channels` stride for its entire lifetime.
/// Grouping output rows into small tiles lets the inner loop visit a compact
/// source-row span before moving to the next tile.  Each Rayon chunk owns a
/// complete group of output rows, so the write proof remains the same as
/// `par_rows_mut!`; the small-image path deliberately keeps its old order.
fn transpose_bytes_tiled(
    source: &[u8],
    output: &mut [u8],
    width: u32,
    height: u32,
    channels: usize,
    method: &TransposeMethod,
) {
    let output_stride = height as usize * channels;
    #[cfg(feature = "parallel")]
    let tile_stride = output_stride * TRANSPOSE_TILE_SIZE as usize;
    let tile_rows = (width as usize).div_ceil(TRANSPOSE_TILE_SIZE as usize);

    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        output,
        tile_stride,
        tile_rows,
        |_row_start, _row_end, tile_index, rows| {
            let output_y_start = tile_index as usize * TRANSPOSE_TILE_SIZE as usize;
            let output_y_end = (output_y_start + TRANSPOSE_TILE_SIZE as usize).min(width as usize);
            for output_x in 0..height as usize {
                for output_y in output_y_start..output_y_end {
                    let (source_x, source_y) = match method {
                        TransposeMethod::Transpose => (output_y, output_x),
                        TransposeMethod::Transverse => (
                            width as usize - 1 - output_y,
                            height as usize - 1 - output_x,
                        ),
                        TransposeMethod::Rotate90 => (width as usize - 1 - output_y, output_x),
                        TransposeMethod::Rotate270 => (output_y, height as usize - 1 - output_x),
                        _ => unreachable!("unsupported tiled transpose method"),
                    };
                    let source_index = (source_y * width as usize + source_x) * channels;
                    let output_index =
                        (output_y - output_y_start) * output_stride + output_x * channels;
                    rows[output_index..output_index + channels]
                        .copy_from_slice(&source[source_index..source_index + channels]);
                }
            }
        }
    );

    #[cfg(not(feature = "parallel"))]
    for tile_index in 0..tile_rows {
        let output_y_start = tile_index * TRANSPOSE_TILE_SIZE as usize;
        let output_y_end = (output_y_start + TRANSPOSE_TILE_SIZE as usize).min(width as usize);
        let row_start = output_y_start * output_stride;
        let row_end = output_y_end * output_stride;
        let rows = &mut output[row_start..row_end];
        for output_x in 0..height as usize {
            for output_y in output_y_start..output_y_end {
                let (source_x, source_y) = match method {
                    TransposeMethod::Transpose => (output_y, output_x),
                    TransposeMethod::Transverse => (
                        width as usize - 1 - output_y,
                        height as usize - 1 - output_x,
                    ),
                    TransposeMethod::Rotate90 => (width as usize - 1 - output_y, output_x),
                    TransposeMethod::Rotate270 => (output_y, height as usize - 1 - output_x),
                    _ => unreachable!("unsupported tiled transpose method"),
                };
                let source_index = (source_y * width as usize + source_x) * channels;
                let output_index =
                    (output_y - output_y_start) * output_stride + output_x * channels;
                rows[output_index..output_index + channels]
                    .copy_from_slice(&source[source_index..source_index + channels]);
            }
        }
    }
}

pub fn execute_transpose(
    img: &DynamicImage,
    method: &TransposeMethod,
) -> Result<DynamicImage, PilError> {
    match method {
        TransposeMethod::FlipLeftRight => Ok(img.fliph()),
        TransposeMethod::FlipTopBottom => Ok(img.flipv()),
        // PIL rotates counter-clockwise; image crate rotates clockwise.
        // PIL ROTATE_90 (CCW) = image crate rotate270 (CW)
        // PIL ROTATE_270 (CCW) = image crate rotate90 (CW)
        TransposeMethod::Rotate90 => {
            if matches!(
                img.color(),
                crate::raster::ColorType::L8
                    | crate::raster::ColorType::La8
                    | crate::raster::ColorType::Rgb8
                    | crate::raster::ColorType::Rgba8
            ) {
                let (width, height) = img.dimensions();
                if should_tile_transpose(width, height) {
                    let channels = img.color().channel_count() as usize;
                    let mut output =
                        CheckedDims::new(height, width, channels as u8)?.alloc_buffer();
                    transpose_bytes_tiled(
                        img.as_bytes(),
                        &mut output,
                        width,
                        height,
                        channels,
                        method,
                    );
                    return raw_bytes_to_image(height, width, output, channels);
                }
            }
            Ok(img.rotate270())
        }
        TransposeMethod::Rotate180 => Ok(img.rotate180()),
        TransposeMethod::Rotate270 => {
            if matches!(
                img.color(),
                crate::raster::ColorType::L8
                    | crate::raster::ColorType::La8
                    | crate::raster::ColorType::Rgb8
                    | crate::raster::ColorType::Rgba8
            ) {
                let (width, height) = img.dimensions();
                if should_tile_transpose(width, height) {
                    let channels = img.color().channel_count() as usize;
                    let mut output =
                        CheckedDims::new(height, width, channels as u8)?.alloc_buffer();
                    transpose_bytes_tiled(
                        img.as_bytes(),
                        &mut output,
                        width,
                        height,
                        channels,
                        method,
                    );
                    return raw_bytes_to_image(height, width, output, channels);
                }
            }
            Ok(img.rotate90())
        }
        TransposeMethod::Transpose | TransposeMethod::Transverse => {
            Ok(img.transpose_diagonal(matches!(method, TransposeMethod::Transverse)))
        }
    }
}

/// Execute a Thumbnail operation.
/// Computes the scale factor to fit within the given box, preserving aspect ratio.
/// Matches PIL's thumbnail behavior including the reducing_gap optimization
/// (default reducing_gap=2.0) for non-NEAREST filters.
pub fn execute_thumbnail(
    img: &DynamicImage,
    w: u32,
    h: u32,
    filter: &ResampleFilter,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (cur_w, cur_h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return Err(PilError::ValueError("thumbnail size must be > 0".into()));
    }
    // `Image::thumbnail` performs Pillow's aspect-preserving `round_aspect`
    // calculation before queuing this operation so lazy shape metadata and
    // the eventual pixels agree. The operation therefore carries the final
    // dimensions; do not apply the aspect calculation a second time here.
    let new_w = w.max(1).min(cur_w);
    let new_h = h.max(1).min(cur_h);
    // PIL forces NEAREST for mode "1" and "P" to avoid non-binary/interpolated values
    let effective_filter = match explicit_mode {
        Some("1") | Some("P") => ResampleFilter::Nearest,
        _ => *filter,
    };
    // PIL's thumbnail uses reducing_gap=2.0 by default: first integer-reduce
    // by up to scale/reducing_gap, then resize the rest.
    // This matches PIL's ImagingReduce then ImagingResample two-step.
    // Skip reducing_gap for modes with alpha (LA, RGBA) to avoid premultiply
    // issues. F/I use RGBA storage internally but are scalar modes, and CMYK
    // uses the same four-byte storage without an alpha channel.
    let has_alpha = matches!(
        img.color(),
        crate::raster::ColorType::La8 | crate::raster::ColorType::Rgba8
    ) && !matches!(explicit_mode, Some("F" | "I" | "CMYK" | "RGBa" | "RGBX"));
    let needs_reduce = !matches!(effective_filter, ResampleFilter::Nearest) && !has_alpha;
    let mut work_img = img.clone();
    let mut resize_box = None;
    if needs_reduce {
        let scale_x = cur_w as f64 / new_w as f64;
        let scale_y = cur_h as f64 / new_h as f64;
        // Image.resize computes these independently for the two axes before
        // calling Image.reduce(factor=(factor_x, factor_y)).
        let factor_x = ((scale_x / 2.0) as u32).max(1);
        let factor_y = ((scale_y / 2.0) as u32).max(1);
        if factor_x > 1 || factor_y > 1 {
            let (rw, rh) = (cur_w.div_ceil(factor_x), cur_h.div_ceil(factor_y));
            // Image.resize keeps the original full-image box after reduce and
            // scales its right/bottom edges by the integer factors. When the
            // source dimensions are not divisible by a factor, that box ends
            // inside the ceil-sized reduced image; using the whole reduced
            // image changes boundary pixels (Pillow's _get_safe_box path).
            resize_box = Some((
                0.0,
                0.0,
                cur_w as f64 / factor_x as f64,
                cur_h as f64 / factor_y as f64,
            ));
            work_img = match explicit_mode {
                // Pillow keeps F/I samples in their native scalar domain for
                // the reducing_gap pass. Averaging encoded RGBA bytes would
                // corrupt the representation before resize_f/resize_i runs.
                Some("F") if matches!(work_img, DynamicImage::ImageRgba8(_)) => {
                    reduce_f_thumbnail(&work_img, rw, rh, factor_x, factor_y)?
                }
                Some("I") if matches!(work_img, DynamicImage::ImageRgba8(_)) => {
                    reduce_i_thumbnail(&work_img, rw, rh, factor_x, factor_y)?
                }
                _ => execute_reduce(&work_img, factor_x, factor_y, explicit_mode)?,
            };
        }
    }
    // Only use F/I thumbnail paths when the image is already stored as Rgba8
    // (4 bytes per pixel), meaning it has been converted to F/I mode already.
    // If the image is still RGB or other format, use normal thumbnail regardless
    // of explicit_mode, because the F/I convert hasn't happened yet in the pipeline.
    let result = match (explicit_mode, &work_img, resize_box) {
        (Some("F"), DynamicImage::ImageRgba8(_), Some((left, top, right, bottom))) => {
            // Image.resize adjusts the source box after its reducing-gap pass.
            // Keep that fractional box for F as well; resizing the complete
            // ceil-sized reduction includes partial edge samples that Pillow
            // deliberately excludes from the final convolution.
            pil_resize_boxed(
                &work_img,
                new_w,
                new_h,
                left,
                top,
                right,
                bottom,
                effective_filter,
                explicit_mode,
            )
        }
        (Some("F"), DynamicImage::ImageRgba8(_), None) => {
            resize_f(&work_img, new_w, new_h, &effective_filter)?
        }
        (Some("I"), DynamicImage::ImageRgba8(_), Some((left, top, right, bottom))) => {
            resize_i_boxed(
                &work_img,
                new_w,
                new_h,
                left,
                top,
                right,
                bottom,
                effective_filter,
            )?
        }
        (Some("I"), DynamicImage::ImageRgba8(_), None) => {
            resize_i(&work_img, new_w, new_h, &effective_filter)?
        }
        (_, _, Some((left, top, right, bottom))) => pil_resize_boxed(
            &work_img,
            new_w,
            new_h,
            left,
            top,
            right,
            bottom,
            effective_filter,
            explicit_mode,
        ),
        _ => pil_resize(&work_img, new_w, new_h, effective_filter, explicit_mode),
    };
    Ok(preserve_mode(img, result))
}

fn reduce_f_thumbnail(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    factor_x: u32,
    factor_y: u32,
) -> Result<DynamicImage, PilError> {
    let rgba = img.to_rgba8();
    let (src_w, src_h) = rgba.dimensions();
    let mut out = Vec::with_capacity((dst_w * dst_h * 4) as usize);
    let main_width = src_w / factor_x;
    let main_height = src_h / factor_y;
    for y in 0..dst_h {
        let source_y = y * factor_y;
        let block_h = factor_y.min(src_h - source_y);
        for x in 0..dst_w {
            let source_x = x * factor_x;
            let block_w = factor_x.min(src_w - source_x);
            // `ImagingReduceNxN_32bpc` uses a double accumulator, but its
            // interior 2x2 groups are formed by float additions before being
            // promoted to double. The corner helper handles partial right,
            // bottom, and bottom-right blocks as scalar float values. Keep
            // those two paths distinct: a flat f32 sum drifts on constants,
            // while a flat f64 sum differs on heterogeneous blocks.
            let mut sum = 0.0f64;
            if x < main_width && y < main_height {
                let mut dy = 0;
                while dy + 1 < block_h {
                    let mut dx = 0;
                    while dx + 1 < block_w {
                        let value = |offset_x: u32, offset_y: u32| {
                            let pixel = rgba.get_pixel(source_x + offset_x, source_y + offset_y);
                            f32::from_le_bytes([pixel[0], pixel[1], pixel[2], pixel[3]])
                        };
                        let top_left = value(dx, dy);
                        let top_right = value(dx + 1, dy);
                        let bottom_left = value(dx, dy + 1);
                        let bottom_right = value(dx + 1, dy + 1);
                        let quartet = ((top_left + top_right) + bottom_left) + bottom_right;
                        sum += f64::from(quartet);
                        dx += 2;
                    }
                    if dx < block_w {
                        let value = |offset_y: u32| {
                            let pixel = rgba.get_pixel(source_x + dx, source_y + offset_y);
                            f32::from_le_bytes([pixel[0], pixel[1], pixel[2], pixel[3]])
                        };
                        sum += f64::from(value(dy) + value(dy + 1));
                    }
                    dy += 2;
                }
                if dy < block_h {
                    let mut dx = 0;
                    while dx + 1 < block_w {
                        let value = |offset_x: u32| {
                            let pixel = rgba.get_pixel(source_x + offset_x, source_y + dy);
                            f32::from_le_bytes([pixel[0], pixel[1], pixel[2], pixel[3]])
                        };
                        sum += f64::from(value(dx) + value(dx + 1));
                        dx += 2;
                    }
                    if dx < block_w {
                        let pixel = rgba.get_pixel(source_x + dx, source_y + dy);
                        sum +=
                            f64::from(f32::from_le_bytes([pixel[0], pixel[1], pixel[2], pixel[3]]));
                    }
                }
            } else {
                for dy in 0..block_h {
                    for dx in 0..block_w {
                        let pixel = rgba.get_pixel(source_x + dx, source_y + dy);
                        sum +=
                            f64::from(f32::from_le_bytes([pixel[0], pixel[1], pixel[2], pixel[3]]));
                    }
                }
            }
            let value = (sum * (1.0 / f64::from(block_w * block_h))) as f32;
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    raw_bytes_to_image(dst_w, dst_h, out, 4)
}

fn reduce_i_thumbnail(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    factor_x: u32,
    factor_y: u32,
) -> Result<DynamicImage, PilError> {
    let rgba = img.to_rgba8();
    let (src_w, src_h) = rgba.dimensions();
    let mut out = Vec::with_capacity((dst_w * dst_h * 4) as usize);
    let main_width = src_w / factor_x;
    let main_height = src_h / factor_y;
    for y in 0..dst_h {
        let source_y = y * factor_y;
        let block_h = factor_y.min(src_h - source_y);
        for x in 0..dst_w {
            let source_x = x * factor_x;
            let block_w = factor_x.min(src_w - source_x);
            // ImagingReduceNxN_32bpc adds pairs/quartets while the samples
            // are still INT32, so each intermediate addition wraps at 32
            // bits before the result is promoted to the double accumulator.
            // Its corner helper instead adds each partial-edge sample
            // directly to the double accumulator. Preserve those two paths
            // separately; summing every sample as i64 changes overflow cases.
            let mut sum = 0.0f64;
            if x < main_width && y < main_height {
                let value = |offset_x: u32, offset_y: u32| {
                    let pixel = rgba.get_pixel(source_x + offset_x, source_y + offset_y);
                    i32::from_le_bytes([pixel[0], pixel[1], pixel[2], pixel[3]])
                };
                let mut dy = 0;
                while dy + 1 < block_h {
                    let mut dx = 0;
                    while dx + 1 < block_w {
                        let quartet = value(dx, dy)
                            .wrapping_add(value(dx + 1, dy))
                            .wrapping_add(value(dx, dy + 1))
                            .wrapping_add(value(dx + 1, dy + 1));
                        sum += f64::from(quartet);
                        dx += 2;
                    }
                    if dx < block_w {
                        let pair = value(dx, dy).wrapping_add(value(dx, dy + 1));
                        sum += f64::from(pair);
                    }
                    dy += 2;
                }
                if dy < block_h {
                    let mut dx = 0;
                    while dx + 1 < block_w {
                        let pair = value(dx, dy).wrapping_add(value(dx + 1, dy));
                        sum += f64::from(pair);
                        dx += 2;
                    }
                    if dx < block_w {
                        sum += f64::from(value(dx, dy));
                    }
                }
            } else {
                for dy in 0..block_h {
                    for dx in 0..block_w {
                        let pixel = rgba.get_pixel(source_x + dx, source_y + dy);
                        let value = i32::from_le_bytes([pixel[0], pixel[1], pixel[2], pixel[3]]);
                        sum += f64::from(value);
                    }
                }
            }
            let value = round_up(sum / f64::from(block_w * block_h)) as i32;
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    raw_bytes_to_image(dst_w, dst_h, out, 4)
}

/// Execute a Reduce operation matching Pillow's `Reduce.c`.
///
/// Pillow computes ceil(w/xscale) x ceil(h/yscale) output pixels, averages
/// complete xscale×yscale blocks for the inner region, then fills the right
/// column, bottom row, and bottom-right corner from partial blocks using the
/// same multiplier/amend rounding (`(sum + amend) * multiplier >> 24`).
pub fn execute_reduce(
    img: &DynamicImage,
    x_factor: u32,
    y_factor: u32,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if x_factor < 2 && y_factor < 2 {
        return Ok(img.clone());
    }
    let fx = x_factor.max(1);
    let fy = y_factor.max(1);
    let (w, h) = (img.width(), img.height());
    if matches!(img, DynamicImage::ImageRgba8(_)) && explicit_mode == Some("I") {
        return reduce_i_thumbnail(img, w.div_ceil(fx), h.div_ceil(fy), fx, fy);
    }
    if matches!(img, DynamicImage::ImageRgba8(_)) && explicit_mode == Some("F") {
        return reduce_f_thumbnail(img, w.div_ceil(fx), h.div_ceil(fy), fx, fy);
    }
    let channels = img.color().channel_count() as usize;
    let new_w = w.div_ceil(fx);
    let new_h = h.div_ceil(fy);
    let raw = img.as_bytes();
    let premultiplied_alpha =
        matches!(
            img.color(),
            crate::raster::ColorType::La8 | crate::raster::ColorType::Rgba8
        ) && !matches!(explicit_mode, Some("CMYK" | "RGBa" | "RGBX" | "F" | "I"));
    let mut out = CheckedDims::new(new_w, new_h, channels as u8)?.alloc_buffer();
    if new_w == 0 || new_h == 0 {
        return raw_bytes_to_image(new_w, new_h, out, channels);
    }

    let division_multiplier = |divider: u32| -> u64 {
        // division_UINT32(divider, 8): 2^32 / (256 * divider), truncated.
        ((1u128 << 32) / (u128::from(divider) * 256)) as u64
    };

    // Every reduced output row reads immutable source pixels and owns a
    // disjoint destination slice. Keep the partial right/bottom blocks in the
    // same row function as the full blocks so the parallel and serial lanes
    // share one exact rounding order.
    let main_w = w / fx;
    let main_h = h / fy;
    let right_width = w % fx;
    let bottom_height = h % fy;
    let full_divider = fx * fy;
    let full_multiplier = division_multiplier(full_divider);
    let full_amend = full_divider / 2;
    let right_divider = right_width * fy;
    let right_multiplier = division_multiplier(right_divider.max(1));
    let right_amend = right_divider / 2;
    let bottom_divider = fx * bottom_height;
    let bottom_multiplier = division_multiplier(bottom_divider.max(1));
    let bottom_amend = bottom_divider / 2;
    let corner_divider = right_width * bottom_height;
    let corner_multiplier = division_multiplier(corner_divider.max(1));
    let corner_amend = corner_divider / 2;
    let process_row = |y: u32, row: &mut [u8]| {
        let full_y = y < main_h;
        let y_count = if full_y { fy } else { bottom_height };
        let source_y = if y < main_h { y * fy } else { main_h * fy };
        for x in 0..new_w {
            let full_x = x < main_w;
            let x_count = if full_x { fx } else { right_width };
            let source_x = if x < main_w { x * fx } else { main_w * fx };
            let (multiplier, amend) = match (full_x, full_y) {
                (true, true) => (full_multiplier, full_amend),
                (false, true) => (right_multiplier, right_amend),
                (true, false) => (bottom_multiplier, bottom_amend),
                (false, false) => (corner_multiplier, corner_amend),
            };
            let mut sums = [0u64; 4];
            for dy in 0..y_count {
                for dx in 0..x_count {
                    let src_idx = ((source_y + dy) * w + source_x + dx) as usize * channels;
                    for c in 0..channels {
                        let sample = if premultiplied_alpha && c + 1 < channels {
                            ((u16::from(raw[src_idx + c]) * u16::from(raw[src_idx + channels - 1])
                                + 127)
                                / 255) as u8
                        } else {
                            raw[src_idx + c]
                        };
                        sums[c] += u64::from(sample);
                    }
                }
            }
            let dst_idx = x as usize * channels;
            for c in 0..channels {
                let mut value = (((sums[c] + u64::from(amend)) * multiplier) >> 24) as u8;
                if premultiplied_alpha && c + 1 < channels {
                    let alpha =
                        (((sums[channels - 1] + u64::from(amend)) * multiplier) >> 24) as u8;
                    if alpha != 0 {
                        value = (u16::from(value) * 255 / u16::from(alpha)) as u8;
                    }
                }
                row[dst_idx + c] = value;
            }
        }
    };

    let output_stride = new_w as usize * channels;
    #[cfg(feature = "parallel")]
    {
        // Pillow's Reduce.c has no row-task boundary; each destination row is
        // independent.  Match the other CPU geometry kernels by keeping tiny
        // reductions serial: Rayon setup costs more than the complete 32x24
        // benchmark reduction, while large images still use row-level
        // parallelism.  Use source pixels for the guard so a large source
        // remains parallel even when its reduced output is smaller than 512².
        const REDUCE_PARALLEL_PIXEL_THRESHOLD: usize = 512 * 512;
        let input_pixels = (w as usize).saturating_mul(h as usize);
        if input_pixels >= REDUCE_PARALLEL_PIXEL_THRESHOLD {
            crate::par_rows_mut!(
                &mut out,
                output_stride,
                new_h as usize,
                |_row_start, _row_end, y, row| {
                    process_row(y, row);
                }
            );
        } else {
            for y in 0..new_h {
                let start = y as usize * output_stride;
                process_row(y, &mut out[start..start + output_stride]);
            }
        }
    }

    #[cfg(not(feature = "parallel"))]
    for y in 0..new_h {
        let start = y as usize * output_stride;
        process_row(y, &mut out[start..start + output_stride]);
    }

    let result = raw_bytes_to_image(new_w, new_h, out, channels)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{reduce_f_thumbnail, reduce_i_thumbnail, resize_f};
    use crate::pipeline::ResampleFilter;
    use crate::raster::{DynamicImage, GenericImageView, RgbImage, RgbaImage};

    #[test]
    fn rotate_near_right_angle_uses_affine_sampling() {
        let rgb = DynamicImage::ImageRgb8(
            RgbImage::from_raw(2, 1, vec![10, 20, 30, 40, 50, 60])
                .expect("RGB source shape must be valid"),
        );
        let rgba = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(2, 1, vec![10, 20, 30, 40, 50, 60, 70, 80])
                .expect("RGBA source shape must be valid"),
        );

        // Pillow only selects its transpose fast path for an exact 90°
        // multiple.  Rounding 89.9° or 90.1° into that path moves the source
        // pixel selected at the edge and diverges from Geometry.c's affine
        // nearest sampler.
        let rgb_output =
            super::execute_rotate(&rgb, 89.9, false, None, None, None, true, Some("RGB"))
                .expect("near-right RGB rotation");
        assert_eq!(rgb_output.dimensions(), (2, 1));
        assert_eq!(rgb_output.as_bytes(), &[10, 20, 30, 0, 0, 0]);

        let rgba_output =
            super::execute_rotate(&rgba, 90.1, false, None, None, None, true, Some("RGBA"))
                .expect("near-right RGBA rotation");
        assert_eq!(rgba_output.dimensions(), (2, 1));
        assert_eq!(rgba_output.as_bytes(), &[50, 60, 70, 80, 0, 0, 0, 0]);
    }

    #[cfg(target_endian = "little")]
    #[test]
    fn f_bicubic_preserves_pillow_f64_rounding() {
        let source_words = [0xd9f6def9, 0x3210f3c9];
        let source_bytes = source_words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        let source = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(1, 2, source_bytes).expect("source shape must be valid"),
        );

        let output = resize_f(&source, 2, 6, &ResampleFilter::Bicubic)
            .expect("finite F-mode bicubic resize must succeed");
        let DynamicImage::ImageRgba8(output) = output else {
            panic!("F-mode resize must retain packed float storage");
        };
        let expected_words = [
            0xda086dc0, 0xda086dc0, 0xd9f6def9, 0xd9f6def9, 0xd9accf48, 0xd9accf48, 0xd9141f62,
            0xd9141f62, 0x3210f3c9, 0x3210f3c9, 0x584fe430, 0x584fe430,
        ];
        let expected_bytes: Vec<u8> = expected_words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        assert_eq!(output.as_raw(), &expected_bytes);
    }

    #[cfg(target_endian = "little")]
    #[test]
    fn f_nearest_uses_pillow_cumulative_row_mapping() {
        let source_words = [1.0f32.to_bits(), 2.0f32.to_bits()];
        let source_bytes = source_words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        let source = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(1, 2, source_bytes).expect("source shape must be valid"),
        );

        let output = resize_f(&source, 1, 7, &ResampleFilter::Nearest)
            .expect("finite F-mode nearest resize must succeed");
        let DynamicImage::ImageRgba8(output) = output else {
            panic!("F-mode resize must retain packed float storage");
        };
        let expected_words = [
            1.0f32.to_bits(),
            1.0f32.to_bits(),
            1.0f32.to_bits(),
            1.0f32.to_bits(),
            2.0f32.to_bits(),
            2.0f32.to_bits(),
            2.0f32.to_bits(),
        ];
        let expected_bytes: Vec<u8> = expected_words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        assert_eq!(output.as_raw(), &expected_bytes);
    }

    #[cfg(target_endian = "little")]
    #[test]
    fn f_convolution_preserves_pillow_signed_zero() {
        let source_words = [0x8000_0000, 0x0000_0001];
        let source_bytes = source_words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        let source = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(1, 2, source_bytes).expect("source shape must be valid"),
        );

        let output = resize_f(&source, 1, 3, &ResampleFilter::Bicubic)
            .expect("mixed signed-zero F-mode bicubic resize must succeed");
        let DynamicImage::ImageRgba8(output) = output else {
            panic!("F-mode resize must retain packed float storage");
        };
        let expected_words = [0x8000_0000, 0x0000_0000, 0x0000_0001];
        let expected_bytes: Vec<u8> = expected_words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        assert_eq!(output.as_raw(), &expected_bytes);
    }

    #[cfg(target_endian = "little")]
    #[test]
    fn f_thumbnail_reduce_matches_pillow_32bpc_grouping() {
        let source_words = [
            0x0e65_a54au32,
            0x0e65_a54a,
            0x0e65_a54a,
            0x0e65_a54a,
            0x0e65_a54a,
            0x0e65_a54a,
            0x0e65_a54a,
            0x0e65_a54a,
            0x0e65_a54a,
            0x0e65_a54a,
            0x0e65_a54a,
            0x0e65_a54a,
            0x0e65_a54a,
            0x0e65_a54a,
            0x0e65_a54a,
            0x0e65_a54a,
        ];
        let source_bytes = source_words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        let source = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(4, 4, source_bytes).expect("source shape must be valid"),
        );
        let output = reduce_f_thumbnail(&source, 1, 1, 4, 4)
            .expect("finite F-mode thumbnail reduction must succeed");
        let DynamicImage::ImageRgba8(output) = output else {
            panic!("F-mode thumbnail reduction must retain packed float storage");
        };
        assert_eq!(output.as_raw(), &0x0e65_a54au32.to_le_bytes());
    }

    #[cfg(target_endian = "little")]
    #[test]
    fn f_thumbnail_reduce_matches_pillow_float_group_order() {
        let source_words = [
            0xc42a_2b37,
            0xc3a2_c889,
            0xc3a6_341f,
            0xc432_e997,
            0x4411_0932,
            0xc46f_61cc,
        ];
        let source_bytes = source_words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        let source = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(3, 2, source_bytes).expect("source shape must be valid"),
        );
        let output = reduce_f_thumbnail(&source, 1, 1, 3, 2)
            .expect("finite F-mode thumbnail reduction must succeed");
        let DynamicImage::ImageRgba8(output) = output else {
            panic!("F-mode thumbnail reduction must retain packed float storage");
        };
        assert_eq!(output.as_raw(), &0xc3ca_a3eau32.to_le_bytes());
    }

    #[cfg(target_endian = "little")]
    #[test]
    fn i_thumbnail_reduce_matches_pillow_int32_grouping() {
        let source_words = [i32::MAX; 4];
        let source_bytes: Vec<u8> = source_words
            .into_iter()
            .flat_map(i32::to_le_bytes)
            .collect();
        let source = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(2, 2, source_bytes).expect("source shape must be valid"),
        );
        let output = reduce_i_thumbnail(&source, 1, 1, 2, 2)
            .expect("I-mode thumbnail reduction must succeed");
        let DynamicImage::ImageRgba8(output) = output else {
            panic!("I-mode thumbnail reduction must retain packed integer storage");
        };
        // Reduce.c forms the interior quartet while still INT32: four
        // INT32_MAX values wrap to -4 before promotion and averaging.
        assert_eq!(output.as_raw(), &(-1i32).to_le_bytes());
    }

    #[cfg(target_endian = "little")]
    #[test]
    fn f_hamming_matches_pillow_fma_window_on_cancellation() {
        let source_words = [
            1.0f32.to_bits(),
            (-1.0f32).to_bits(),
            3.0f32.to_bits(),
            (-3.0f32).to_bits(),
            1.0f32.to_bits(),
            (-1.0f32).to_bits(),
            3.0f32.to_bits(),
            (-3.0f32).to_bits(),
        ];
        let source_bytes: Vec<u8> = source_words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        let source = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(8, 1, source_bytes).expect("source shape must be valid"),
        );

        let output = resize_f(&source, 4, 1, &ResampleFilter::Hamming)
            .expect("finite F-mode Hamming resize must succeed");
        let DynamicImage::ImageRgba8(output) = output else {
            panic!("F-mode resize must retain packed float storage");
        };
        // Pillow's Resample.c Hamming window contracts its `0.46f * cos +
        // 0.54f` expression before the sinc product.  The exact cancellation
        // residuals here catch either a separated window expression or a
        // changed accumulation order.
        let expected_words = [0x3df4_077e, 0xa3c0_0000, 0xa300_0000, 0xbd22_afaa];
        let expected_bytes: Vec<u8> = expected_words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        assert_eq!(output.as_raw(), &expected_bytes);
    }
}
