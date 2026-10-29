use super::alpha::premultiply_channel;
use super::coefficients::BOXED_ZERO_FAST_PATH_MAX_PIXELS;
use super::float::{
    f_resize_boxed_horizontal_pass, f_resize_boxed_vertical_pass, typed_color_resize_bytes,
};
use super::horizontal_passes::horizontal_pass_boxed_rows;
use super::luma16::pil_resize_luma16;
use super::vertical_passes::{
    vertical_pass_rows, vertical_pass_rows_alpha, vertical_pass_rows_sparse_rgb,
};
use super::*;
use crate::compute::{Backend, ExecutionContext};
use crate::pipeline::ResampleFilter;
use crate::raster::DynamicImage;

fn f_boxed_nearest_axis_maps_identity(
    source_size: u32,
    output_size: u32,
    box_start: f64,
    box_end: f64,
) -> bool {
    if source_size == 0
        || source_size != output_size
        || !box_start.is_finite()
        || !box_end.is_finite()
    {
        return false;
    }
    let scale = (box_end as f32 - box_start as f32) as f64 / f64::from(output_size);
    if !scale.is_finite() {
        return false;
    }
    let last = i64::from(source_size - 1);
    let mut coordinate = box_start + scale * 0.5;
    for expected in 0..output_size {
        let selected = (coordinate.floor() as i64).clamp(0, last) as u32;
        if selected != expected {
            return false;
        }
        coordinate += scale;
    }
    true
}

/// Resize an F-mode image through a fractional source box.
///
/// F samples are IEEE-754 values packed four bytes at a time. They must be
/// decoded before filtered resampling; treating the bytes as four independent
/// image channels is not Pillow-compatible. The intermediate remains f32,
/// while each separable accumulation follows Pillow's f64-kernel/f32-store
/// order.
fn pil_resize_f_boxed(
    context: &ExecutionContext,
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    box_left: f64,
    box_top: f64,
    box_right: f64,
    box_bottom: f64,
    filter: ResampleFilter,
) -> DynamicImage {
    #[cfg(target_arch = "x86_64")]
    if let Some(fma) = X86FmaToken::from_context(context) {
        return pil_resize_f_boxed_with_fma::<_, true>(
            img, dst_w, dst_h, box_left, box_top, box_right, box_bottom, filter, &fma,
        );
    }
    #[cfg(not(target_arch = "x86_64"))]
    let _ = context;
    pil_resize_f_boxed_with_fma::<_, true>(
        img,
        dst_w,
        dst_h,
        box_left,
        box_top,
        box_right,
        box_bottom,
        filter,
        &PortableFma,
    )
}

#[inline]
fn pil_resize_f_boxed_nearest(
    img: &DynamicImage,
    source_bytes: &[u8],
    source_width: u32,
    source_height: u32,
    dst_w: u32,
    dst_h: u32,
    box_left: f64,
    box_top: f64,
    box_right: f64,
    box_bottom: f64,
    output_len: usize,
) -> DynamicImage {
    if source_width == dst_w
        && source_height == dst_h
        && f_boxed_nearest_axis_maps_identity(source_width, dst_w, box_left, box_right)
        && f_boxed_nearest_axis_maps_identity(source_height, dst_h, box_top, box_bottom)
    {
        // The fractional box can still select the same scalar sample at
        // every destination coordinate. A single owned copy then has the
        // same bytes as the general gather path and avoids its per-pixel
        // coordinate and output loops.
        return img.clone();
    }
    let scale_x = (box_right as f32 - box_left as f32) as f64 / f64::from(dst_w);
    let scale_y = (box_bottom as f32 - box_top as f32) as f64 / f64::from(dst_h);
    let last_x = i64::from(source_width - 1);
    let last_y = i64::from(source_height - 1);
    let mut output = Vec::with_capacity(output_len);
    // Pillow's nearest boxed F resize also reaches ImagingScaleAffine,
    // whose coordinates advance cumulatively from one pixel to the next.
    // Keep that recurrence so exact upsample boundaries select the same
    // source word as the native path.
    let mut source_y = box_top + scale_y * 0.5;
    for _ in 0..dst_h {
        let source_y_index = source_y.floor() as i64;
        let source_y_index = source_y_index.clamp(0, last_y) as usize;
        let mut source_x = box_left + scale_x * 0.5;
        for _ in 0..dst_w {
            let source_x_index = source_x.floor() as i64;
            let source_x_index = source_x_index.clamp(0, last_x) as usize;
            let source_start = (source_y_index * source_width as usize + source_x_index) * 4;
            output.extend_from_slice(&source_bytes[source_start..source_start + 4]);
            source_x += scale_x;
        }
        source_y += scale_y;
    }
    raw_to_dynamic_owned(output, dst_w, dst_h, 4)
}

fn pil_resize_f_boxed_with_fma<F: F64MulAdd, const UNROLL_EIGHT: bool>(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    box_left: f64,
    box_top: f64,
    box_right: f64,
    box_bottom: f64,
    filter: ResampleFilter,
    fma: &F,
) -> DynamicImage {
    // F-mode bytes are stored in the four-byte carrier used by ImageRgba8,
    // but they are scalar float samples, not RGBA channels. Borrow those
    // sample words directly; converting through `to_rgba8()` clones the
    // entire source before the float working set is built.
    let source_bytes = img.as_bytes();
    let (source_width, source_height) = (img.width(), img.height());
    let output_len = (dst_w as usize)
        .checked_mul(dst_h as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .unwrap_or(0);
    if dst_w == 0 || dst_h == 0 || source_width == 0 || source_height == 0 {
        return DynamicImage::ImageRgba8(
            crate::raster::RgbaImage::from_raw(dst_w, dst_h, vec![0; output_len])
                .unwrap_or_else(|| crate::raster::RgbaImage::new(dst_w, dst_h)),
        );
    }
    let box_left = box_left as f32 as f64;
    let box_top = box_top as f32 as f64;
    let box_right = box_right as f32 as f64;
    let box_bottom = box_bottom as f32 as f64;
    let need_horizontal = dst_w != source_width || box_left != 0.0 || box_right != dst_w as f64;
    let need_vertical = dst_h != source_height || box_top != 0.0 || box_bottom != dst_h as f64;

    if matches!(filter, ResampleFilter::Nearest) {
        return pil_resize_f_boxed_nearest(
            img,
            source_bytes,
            source_width,
            source_height,
            dst_w,
            dst_h,
            box_left,
            box_top,
            box_right,
            box_bottom,
            output_len,
        );
    }

    // Filtered resampling requires numeric samples and keeps Pillow's
    // f64-accumulate/f32-store order. Decode only for this branch; nearest
    // resampling copies the original four-byte F sample words above.
    let source_samples = f32_samples_from_le_bytes(source_bytes, source_bytes.len() / 4);
    let source: &[f32] = source_samples.as_ref();
    let horizontal = precompute_coeffs_f64_boxed(dst_w, source_width, box_left, box_right, filter);
    let vertical = precompute_coeffs_f64_boxed(dst_h, source_height, box_top, box_bottom, filter);
    let mut intermediate = vec![0.0f32; source_height as usize * dst_w as usize];
    let finite_source_samples = f_resize_samples_allow_finite_fma(fma, source);
    if need_horizontal {
        if finite_source_samples && f_resize_coefficients_allow_finite_fma(&horizontal) {
            f_resize_boxed_horizontal_pass::<true, _, UNROLL_EIGHT>(
                source,
                source_width as usize,
                source_height as usize,
                dst_w as usize,
                &horizontal,
                &mut intermediate,
                fma,
            );
        } else {
            f_resize_boxed_horizontal_pass::<false, _, UNROLL_EIGHT>(
                source,
                source_width as usize,
                source_height as usize,
                dst_w as usize,
                &horizontal,
                &mut intermediate,
                fma,
            );
        }
    } else {
        intermediate.copy_from_slice(source);
    }

    let output = if need_vertical {
        let finite_intermediate_samples = f_resize_samples_allow_finite_fma(fma, &intermediate);
        if finite_intermediate_samples && f_resize_coefficients_allow_finite_fma(&vertical) {
            f_resize_boxed_vertical_pass::<true, _>(
                &intermediate,
                dst_w as usize,
                dst_h as usize,
                output_len,
                &vertical,
                fma,
            )
        } else {
            f_resize_boxed_vertical_pass::<false, _>(
                &intermediate,
                dst_w as usize,
                dst_h as usize,
                output_len,
                &vertical,
                fma,
            )
        }
    } else {
        intermediate
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect()
    };
    raw_to_dynamic_owned(output, dst_w, dst_h, 4)
}

#[cfg(test)]
mod boxed_f_resize_eight_tap_tests;

#[cfg(all(test, target_arch = "x86_64"))]
mod boxed_f_resize_fma_tests;

/// Box-based resize: maps source region [box_left, box_right] × [box_top, box_bottom]
/// to the output (dst_w, dst_h). All box coordinates are in source pixel coordinates.
pub fn pil_resize_boxed(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    box_left: f64,
    box_top: f64,
    box_right: f64,
    box_bottom: f64,
    filter: ResampleFilter,
    explicit_mode: Option<&str>,
) -> DynamicImage {
    pil_resize_boxed_in_context(
        &ExecutionContext::for_backend(Backend::Cpu),
        img,
        dst_w,
        dst_h,
        box_left,
        box_top,
        box_right,
        box_bottom,
        filter,
        explicit_mode,
    )
}

pub(crate) fn pil_resize_boxed_in_context(
    context: &ExecutionContext,
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    box_left: f64,
    box_top: f64,
    box_right: f64,
    box_bottom: f64,
    filter: ResampleFilter,
    explicit_mode: Option<&str>,
) -> DynamicImage {
    pil_resize_boxed_with_hints(
        context,
        img,
        dst_w,
        dst_h,
        box_left,
        box_top,
        box_right,
        box_bottom,
        filter,
        explicit_mode,
        0,
        None,
    )
}

/// Boxed RGB resize for a sparse 2×2 thumbnail reduction. `nonzero_source_rows`
/// is produced by the reducer while it visits active blocks, so the resize can
/// skip zero rows without an additional whole-image scan.
pub(crate) fn pil_resize_boxed_with_sparse_rgb_rows(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    box_left: f64,
    box_top: f64,
    box_right: f64,
    box_bottom: f64,
    filter: ResampleFilter,
    explicit_mode: Option<&str>,
    nonzero_source_rows: &[bool],
) -> DynamicImage {
    pil_resize_boxed_with_hints(
        &ExecutionContext::for_backend(Backend::Cpu),
        img,
        dst_w,
        dst_h,
        box_left,
        box_top,
        box_right,
        box_bottom,
        filter,
        explicit_mode,
        0,
        Some(nonzero_source_rows),
    )
}

#[inline(always)]
fn pil_resize_boxed_early_paths(
    context: &ExecutionContext,
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    box_left: f64,
    box_top: f64,
    box_right: f64,
    box_bottom: f64,
    filter: ResampleFilter,
    explicit_mode: Option<&str>,
) -> (Option<DynamicImage>, bool) {
    // Pillow narrows the source box to float32 before `_resize` sees it. Keep
    // those values for the crop and pass-elision decisions below so integer
    // boundaries do not drift in Rust's f64 control plane.
    let box_left_f32 = box_left as f32;
    let box_top_f32 = box_top as f32;
    let box_right_f32 = box_right as f32;
    let box_bottom_f32 = box_bottom as f32;
    let finite_box = box_left_f32.is_finite()
        && box_top_f32.is_finite()
        && box_right_f32.is_finite()
        && box_bottom_f32.is_finite();
    let is_cmyk = explicit_mode == Some("CMYK");
    let is_fi = explicit_mode == Some("F") || explicit_mode == Some("I");
    let needs_alpha = !matches!(filter, ResampleFilter::Nearest)
        && !is_cmyk
        && !is_fi
        && !matches!(explicit_mode, Some("RGBa" | "RGBX" | "La" | "PA"))
        && matches!(
            img.color(),
            crate::raster::ColorType::Rgba8 | crate::raster::ColorType::La8
        );
    if finite_box
        && dst_w == img.width()
        && dst_h == img.height()
        && box_left_f32 == 0.0
        && box_top_f32 == 0.0
        && box_right_f32 == img.width() as f32
        && box_bottom_f32 == img.height() as f32
    {
        // Image.resize returns self.copy() before mode conversion for a
        // complete source at its original size, including F and alpha modes.
        return (Some(img.clone()), needs_alpha);
    }

    // `_imaging.c::_resize` crops an integer-aligned box whose dimensions
    // already equal the target before selecting the resampler. This ordering
    // also matters for F: sending an exact crop through the f64 filter tails
    // would let NaN/Inf values leak into taps that Pillow never evaluates.
    let integer_crop = finite_box
        && box_left_f32.fract() == 0.0
        && box_top_f32.fract() == 0.0
        && box_right_f32 - box_left_f32 == dst_w as f32
        && box_bottom_f32 - box_top_f32 == dst_h as f32;
    if integer_crop && !needs_alpha {
        let left = box_left_f32 as u32;
        let top = box_top_f32 as u32;
        return (Some(img.crop_imm(left, top, dst_w, dst_h)), needs_alpha);
    }

    if let DynamicImage::ImageLuma16(luma) = img {
        // A two-byte typed sample cannot enter the generic four-byte pixel
        // transport. Reuse the native u16 passes with boxed coefficients.
        return (
            Some(pil_resize_luma16(
                luma,
                dst_w,
                dst_h,
                filter,
                explicit_mode,
                Some((box_left, box_top, box_right, box_bottom)),
            )),
            needs_alpha,
        );
    }

    if explicit_mode == Some("F") && matches!(img, DynamicImage::ImageRgba8(_)) {
        return (
            Some(pil_resize_f_boxed(
                context, img, dst_w, dst_h, box_left, box_top, box_right, box_bottom, filter,
            )),
            needs_alpha,
        );
    }
    if let Some(byte_mode) = typed_color_resize_bytes(img) {
        return (
            Some(pil_resize_boxed(
                &byte_mode, dst_w, dst_h, box_left, box_top, box_right, box_bottom, filter, None,
            )),
            needs_alpha,
        );
    }
    (None, needs_alpha)
}

#[inline(always)]
fn pil_resize_boxed_zero_fast_path(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    channels: usize,
) -> Option<DynamicImage> {
    let (source_width, source_height) = (img.width(), img.height());
    let native_byte_image = matches!(
        img,
        DynamicImage::ImageLuma8(_)
            | DynamicImage::ImageLumaA8(_)
            | DynamicImage::ImageRgb8(_)
            | DynamicImage::ImageRgba8(_)
    );
    let source_pixels = (source_width as usize).checked_mul(source_height as usize);
    if native_byte_image
        && source_pixels.is_some_and(|pixels| {
            pixels != 0
                && pixels <= BOXED_ZERO_FAST_PATH_MAX_PIXELS
                && img.as_bytes().iter().all(|&value| value == 0)
        })
        && let Some(output_len) = (dst_w as usize)
            .checked_mul(dst_h as usize)
            .and_then(|pixels| pixels.checked_mul(channels))
    {
        return Some(pil_preserve_mode(
            img,
            raw_to_dynamic_owned(vec![0; output_len], dst_w, dst_h, channels),
        ));
    }
    None
}

#[inline(always)]
fn pil_resize_boxed_nearest(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    box_left: f64,
    box_top: f64,
    box_right: f64,
    box_bottom: f64,
    channels: usize,
) -> DynamicImage {
    let (sw, sh) = (img.width(), img.height());
    if sw == 0 || sh == 0 {
        return pil_preserve_mode(
            img,
            raw_to_dynamic_owned(
                vec![0; (dst_w as usize) * (dst_h as usize) * channels],
                dst_w,
                dst_h,
                channels,
            ),
        );
    }
    // The native transform path receives the same float box record as the
    // resampler; keep its scale calculation at that boundary too.
    let box_left_f32 = box_left as f32;
    let box_top_f32 = box_top as f32;
    let box_right_f32 = box_right as f32;
    let box_bottom_f32 = box_bottom as f32;
    let box_left = box_left_f32 as f64;
    let box_top = box_top_f32 as f64;
    let scale_x = ((box_right_f32 - box_left_f32) as f64) / dst_w as f64;
    let scale_y = ((box_bottom_f32 - box_top_f32) as f64) / dst_h as f64;
    let source = img.as_bytes();
    let mut out_bytes = Vec::with_capacity((dst_w * dst_h) as usize * channels);
    // `_resize` takes this path through ImagingScaleAffine. Its source
    // coordinate is initialized once and advanced with `xo += a[0]` /
    // `yo += a[4]` for every output pixel/row. Preserve that cumulative
    // floating-point rounding instead of recomputing `(index + 0.5) *
    // scale`; the difference is observable exactly at upsample boundaries
    // such as a 2x1 box expanded to seven pixels.
    let mut source_y = box_top + scale_y * 0.5;
    for _ in 0..dst_h {
        let sy = source_y.floor().clamp(0.0, (sh - 1) as f64) as usize;
        let mut source_x = box_left + scale_x * 0.5;
        for _ in 0..dst_w {
            let sx = source_x.floor().clamp(0.0, (sw - 1) as f64) as usize;
            let start = (sy * sw as usize + sx) * channels;
            out_bytes.extend_from_slice(&source[start..start + channels]);
            source_x += scale_x;
        }
        source_y += scale_y;
    }
    pil_preserve_mode(img, raw_to_dynamic_owned(out_bytes, dst_w, dst_h, channels))
}

#[inline(always)]
fn pil_resize_boxed_horizontal_pass(
    img: &DynamicImage,
    source_width: u32,
    first_source_row: u32,
    intermediate_rows: u32,
    channels: usize,
    h_coeffs: &FilterCoeffs,
    dst_w: u32,
    intermediate: &mut [u8],
    needs_alpha: bool,
    parallel_pixel_threshold: usize,
    sparse_rgb_nonzero_rows: Option<&[bool]>,
    need_horizontal: bool,
) {
    if !need_horizontal {
        let source_stride = source_width as usize * channels;
        let source_start = first_source_row as usize * source_stride;
        let source_end = source_start + intermediate.len();
        let source_rows = &img.as_bytes()[source_start..source_end];
        if needs_alpha {
            for (source_pixel, intermediate_pixel) in source_rows
                .chunks_exact(channels)
                .zip(intermediate.chunks_exact_mut(channels))
            {
                let alpha = source_pixel[channels - 1];
                for channel in 0..channels {
                    intermediate_pixel[channel] = if channel == channels - 1 {
                        alpha
                    } else {
                        premultiply_channel(source_pixel[channel], alpha)
                    };
                }
            }
        } else {
            intermediate.copy_from_slice(source_rows);
        }
    } else {
        horizontal_pass_boxed_rows(
            img.as_bytes(),
            source_width,
            first_source_row,
            intermediate_rows,
            channels,
            h_coeffs,
            dst_w,
            intermediate,
            needs_alpha,
            parallel_pixel_threshold,
            sparse_rgb_nonzero_rows,
        );
    }
}

#[inline(always)]
fn pil_resize_boxed_vertical_pass(
    intermediate: &[u8],
    first_source_row: u32,
    intermediate_rows: u32,
    dst_w: u32,
    dst_h: u32,
    channels: usize,
    v_coeffs: &FilterCoeffs,
    needs_alpha: bool,
    sparse_rgb_nonzero_rows: Option<&[bool]>,
    parallel_pixel_threshold: usize,
    need_vertical: bool,
    out_bytes: &mut [u8],
) {
    if !need_vertical {
        if needs_alpha {
            for (intermediate_pixel, output_pixel) in intermediate
                .chunks_exact(channels)
                .zip(out_bytes.chunks_exact_mut(channels))
            {
                let alpha = intermediate_pixel[channels - 1];
                for channel in 0..channels {
                    output_pixel[channel] = if channel == channels - 1 {
                        alpha
                    } else {
                        unpremultiply_channel(intermediate_pixel[channel], alpha)
                    };
                }
            }
        } else {
            let output_len = out_bytes.len();
            out_bytes.copy_from_slice(&intermediate[..output_len]);
        }
    } else if let Some(nonzero_rows) = sparse_rgb_nonzero_rows {
        vertical_pass_rows_sparse_rgb(
            intermediate,
            dst_w,
            dst_h,
            v_coeffs,
            first_source_row,
            nonzero_rows,
            out_bytes,
            parallel_pixel_threshold,
        );
    } else if needs_alpha {
        vertical_pass_rows_alpha(
            intermediate,
            intermediate_rows,
            dst_w,
            dst_h,
            channels,
            v_coeffs,
            out_bytes,
            parallel_pixel_threshold,
        );
    } else {
        vertical_pass_rows(
            intermediate,
            intermediate_rows,
            dst_w,
            dst_h,
            channels,
            v_coeffs,
            out_bytes,
            parallel_pixel_threshold,
        );
    }
}

fn pil_resize_boxed_with_hints(
    context: &ExecutionContext,
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    box_left: f64,
    box_top: f64,
    box_right: f64,
    box_bottom: f64,
    filter: ResampleFilter,
    explicit_mode: Option<&str>,
    parallel_pixel_threshold: usize,
    sparse_rgb_nonzero_rows: Option<&[bool]>,
) -> DynamicImage {
    let (early_output, needs_alpha) = pil_resize_boxed_early_paths(
        context,
        img,
        dst_w,
        dst_h,
        box_left,
        box_top,
        box_right,
        box_bottom,
        filter,
        explicit_mode,
    );
    if let Some(output) = early_output {
        return output;
    }

    let (kernel_fn, support) = filter_from_resample(filter);
    let (sw, sh) = (img.width(), img.height());
    let channels = match img.color() {
        crate::raster::ColorType::L8 => 1usize,
        crate::raster::ColorType::La8 => 2usize,
        crate::raster::ColorType::Rgb8 => 3usize,
        _ => 4usize,
    };
    let sparse_rgb_nonzero_rows = sparse_rgb_nonzero_rows.filter(|rows| {
        matches!(img, DynamicImage::ImageRgb8(_))
            && matches!(explicit_mode, None | Some("RGB"))
            && !needs_alpha
            && rows.len() == sh as usize
    });

    // A bounded all-zero byte source remains zero through every normalized
    // Pillow resampling filter, including alpha premultiplication.
    if let Some(output) = pil_resize_boxed_zero_fast_path(img, dst_w, dst_h, channels) {
        return output;
    }

    // Pillow's boxed nearest path is an affine sample, not a one-tap box
    // convolution. The convolution-style coefficient builder can include
    // adjacent samples at a boundary and produce a value that Pillow never
    // emits (most visibly for indexed ImageOps.fit, but the same source
    // selection applies to every byte layout).
    if matches!(filter, ResampleFilter::Nearest) {
        return pil_resize_boxed_nearest(
            img, dst_w, dst_h, box_left, box_top, box_right, box_bottom, channels,
        );
    }

    // Use box-parameter coefficients for both passes.
    let h_coeffs = precompute_coeffs_boxed(dst_w, sw, box_left, box_right, kernel_fn, support);
    let mut v_coeffs = precompute_coeffs_boxed(dst_h, sh, box_top, box_bottom, kernel_fn, support);

    // Resample.c skips a pass when that axis already has the requested size
    // and covers the complete source extent. A boxed one-axis crop therefore
    // must resample only the changed axis; the unchanged axis retains source
    // samples (or premultiplied samples for LA/RGBA).
    let need_horizontal = dst_w != sw || box_left as f32 != 0.0 || box_right as f32 != dst_w as f32;
    let need_vertical = dst_h != sh || box_top as f32 != 0.0 || box_bottom as f32 != dst_h as f32;

    // The horizontal pass is independent per source row. Avoid processing
    // vertical rows that the rebased output table cannot read, which is a
    // substantial win when a small fractional box is enlarged.
    let (first_source_row, intermediate_rows) = if need_vertical {
        compact_resize_coeffs_to_source_span(&mut v_coeffs, sh).unwrap_or((0, sh))
    } else {
        (0, sh)
    };

    // Allocate only rows referenced by the vertical table.
    let mut intermediate = vec![0u8; (intermediate_rows * dst_w) as usize * channels];
    pil_resize_boxed_horizontal_pass(
        img,
        sw,
        first_source_row,
        intermediate_rows,
        channels,
        &h_coeffs,
        dst_w,
        &mut intermediate,
        needs_alpha,
        parallel_pixel_threshold,
        sparse_rgb_nonzero_rows,
        need_horizontal,
    );

    // Allocate output image.
    let mut out_bytes = vec![0u8; (dst_w * dst_h) as usize * channels];
    pil_resize_boxed_vertical_pass(
        &intermediate,
        first_source_row,
        intermediate_rows,
        dst_w,
        dst_h,
        channels,
        &v_coeffs,
        needs_alpha,
        sparse_rgb_nonzero_rows,
        parallel_pixel_threshold,
        need_vertical,
        &mut out_bytes,
    );

    let result = raw_to_dynamic_owned(out_bytes, dst_w, dst_h, channels);
    pil_preserve_mode(img, result)
}

/// Move a completed native-byte result into its image without allocating
/// and copying the entire output a second time.
pub(super) fn raw_to_dynamic_owned(
    bytes: Vec<u8>,
    w: u32,
    h: u32,
    channels: usize,
) -> DynamicImage {
    match channels {
        1 => DynamicImage::ImageLuma8(
            crate::raster::GrayImage::from_raw(w, h, bytes)
                .unwrap_or_else(|| crate::raster::GrayImage::new(w, h)),
        ),
        2 => DynamicImage::ImageLumaA8(
            crate::raster::GrayAlphaImage::from_raw(w, h, bytes)
                .unwrap_or_else(|| crate::raster::GrayAlphaImage::new(w, h)),
        ),
        3 => DynamicImage::ImageRgb8(
            crate::raster::RgbImage::from_raw(w, h, bytes)
                .unwrap_or_else(|| crate::raster::RgbImage::new(w, h)),
        ),
        _ => DynamicImage::ImageRgba8(
            crate::raster::RgbaImage::from_raw(w, h, bytes)
                .unwrap_or_else(|| crate::raster::RgbaImage::new(w, h)),
        ),
    }
}
