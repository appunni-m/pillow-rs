//! Public box and Gaussian blur orchestration.

use super::{CheckedDims, DynamicImage, PilError, preserve_mode, raw_bytes_to_image};

#[path = "kernels/blur_line_steps.rs"]
mod blur_line_steps;
#[path = "blur/box_rows.rs"]
mod box_rows;
#[path = "blur/line.rs"]
mod line;
#[path = "blur/radius_one.rs"]
mod radius_one;

#[cfg(not(feature = "parallel"))]
use box_rows::blur_box_radius_one_row_ring;
#[cfg(all(test, not(feature = "parallel")))]
use line::blur_line;
use line::{blur_columns, blur_line_step, blur_rows};
#[cfg(feature = "parallel")]
use radius_one::{
    VERTICAL_BLUR_TRANSPOSE_THRESHOLD, blur_columns_radius_one_rows, transpose_interleaved_rows,
};
#[cfg(not(feature = "parallel"))]
use radius_one::{
    blur_interleaved_rows_three_passes, blur_interleaved_vertical_radius_one_fractional_rows,
    blur_la_rows_three_passes_radius_one, blur_luma_rows_three_passes,
    blur_luma_vertical_radius_one_rows,
};
#[cfg(all(test, not(feature = "parallel")))]
use radius_one::{
    blur_la_radius_one_fractional_line, blur_luma_radius_one_fractional_line,
    blur_rgb_radius_one_fractional_line,
};

#[derive(Clone, Copy)]
struct BoxBlurPassContext {
    width: usize,
    height: usize,
    channels: usize,
    passes: u32,
    horizontal_radius: usize,
    horizontal_weight: u32,
    horizontal_fractional_weight: u32,
    vertical_radius: usize,
    vertical_weight: u32,
    vertical_fractional_weight: u32,
    rgb_gaussian_row_fusion: bool,
    luma_gaussian_row_fusion: bool,
    la_gaussian_row_fusion: bool,
    rgba_gaussian_row_fusion: bool,
}

#[inline(always)]
fn blur_horizontal_passes(
    source: &[u8],
    mut work: Vec<u8>,
    mut scratch: Vec<u8>,
    context: BoxBlurPassContext,
) -> (Vec<u8>, Vec<u8>) {
    #[cfg(feature = "parallel")]
    let _ = source;

    let BoxBlurPassContext {
        width,
        height,
        channels,
        passes,
        horizontal_radius,
        horizontal_weight,
        horizontal_fractional_weight,
        rgb_gaussian_row_fusion,
        luma_gaussian_row_fusion,
        la_gaussian_row_fusion,
        rgba_gaussian_row_fusion,
        ..
    } = context;
    // PIL does ALL horizontal passes first (matching ImagingBoxBlur order)
    if rgb_gaussian_row_fusion {
        #[cfg(not(feature = "parallel"))]
        blur_interleaved_rows_three_passes::<3>(
            source,
            &mut work,
            width,
            height,
            horizontal_radius,
            horizontal_weight,
            horizontal_fractional_weight,
        );
    } else if luma_gaussian_row_fusion {
        #[cfg(not(feature = "parallel"))]
        blur_luma_rows_three_passes(
            source,
            &mut work,
            width,
            height,
            horizontal_radius,
            horizontal_weight,
            horizontal_fractional_weight,
        );
    } else if la_gaussian_row_fusion {
        #[cfg(not(feature = "parallel"))]
        if horizontal_radius == 1 && horizontal_fractional_weight != 0 && width >= 5 {
            blur_la_rows_three_passes_radius_one(
                source,
                &mut work,
                width,
                height,
                horizontal_weight,
                horizontal_fractional_weight,
            );
        } else {
            blur_interleaved_rows_three_passes::<2>(
                source,
                &mut work,
                width,
                height,
                horizontal_radius,
                horizontal_weight,
                horizontal_fractional_weight,
            );
        }
    } else if rgba_gaussian_row_fusion {
        #[cfg(not(feature = "parallel"))]
        blur_interleaved_rows_three_passes::<4>(
            source,
            &mut work,
            width,
            height,
            horizontal_radius,
            horizontal_weight,
            horizontal_fractional_weight,
        );
    } else {
        for _ in 0..passes {
            blur_rows(
                &work,
                &mut scratch,
                width,
                height,
                channels,
                horizontal_radius,
                horizontal_weight,
                horizontal_fractional_weight,
            );
            std::mem::swap(&mut work, &mut scratch);
        }
    }

    (work, scratch)
}

#[cfg(feature = "parallel")]
#[inline(always)]
fn blur_transposed_vertical_passes(
    mut work: Vec<u8>,
    mut scratch: Vec<u8>,
    context: BoxBlurPassContext,
) -> Vec<u8> {
    let BoxBlurPassContext {
        width,
        height,
        channels,
        passes,
        vertical_radius,
        vertical_weight,
        vertical_fractional_weight,
        ..
    } = context;
    transpose_interleaved_rows(&work, &mut scratch, width, height, channels);
    for pass in 0..passes {
        blur_rows(
            &scratch,
            &mut work,
            height,
            width,
            channels,
            vertical_radius,
            vertical_weight,
            vertical_fractional_weight,
        );
        // Keep the final blurred buffer in `work` for the final transpose.
        if pass + 1 < passes {
            std::mem::swap(&mut work, &mut scratch);
        }
    }
    transpose_interleaved_rows(&work, &mut scratch, height, width, channels);
    scratch
}

#[inline(always)]
fn blur_vertical_passes(
    mut work: Vec<u8>,
    mut scratch: Vec<u8>,
    context: BoxBlurPassContext,
) -> Vec<u8> {
    let BoxBlurPassContext {
        width,
        height,
        channels,
        passes,
        vertical_radius,
        vertical_weight,
        vertical_fractional_weight,
        luma_gaussian_row_fusion,
        rgba_gaussian_row_fusion,
        ..
    } = context;
    // Pillow transposes before its vertical passes. For large parallel jobs,
    // keep that layout explicit: one transpose makes every original column a
    // writable row, all vertical passes can then reuse the row-parallel blur,
    // and one final transpose restores the public row-major layout. The
    // serial/small-image path keeps the algebraically equivalent wide-row
    // recurrence and avoids paying for the extra data movement.
    #[cfg(feature = "parallel")]
    let use_parallel_radius_one_vertical = width.saturating_mul(height)
        >= VERTICAL_BLUR_TRANSPOSE_THRESHOLD
        && passes > 0
        && vertical_radius == 1
        && vertical_fractional_weight == 0;
    #[cfg(not(feature = "parallel"))]
    let use_parallel_radius_one_vertical = false;
    #[cfg(feature = "parallel")]
    let use_transposed_vertical = width.saturating_mul(height) >= VERTICAL_BLUR_TRANSPOSE_THRESHOLD
        && passes > 0
        && !use_parallel_radius_one_vertical;
    #[cfg(not(feature = "parallel"))]
    let use_transposed_vertical = false;

    if use_parallel_radius_one_vertical {
        #[cfg(feature = "parallel")]
        for _ in 0..passes {
            blur_columns_radius_one_rows(
                &work,
                &mut scratch,
                width,
                height,
                channels,
                vertical_weight,
            );
            std::mem::swap(&mut work, &mut scratch);
        }
    } else if use_transposed_vertical {
        #[cfg(feature = "parallel")]
        return blur_transposed_vertical_passes(work, scratch, context);
    } else if luma_gaussian_row_fusion && vertical_radius == 1 {
        #[cfg(not(feature = "parallel"))]
        for _ in 0..passes {
            blur_luma_vertical_radius_one_rows(
                &work,
                &mut scratch,
                width,
                height,
                vertical_weight,
                vertical_fractional_weight,
            );
            std::mem::swap(&mut work, &mut scratch);
        }
    } else if rgba_gaussian_row_fusion && vertical_radius == 1 && vertical_fractional_weight != 0 {
        #[cfg(not(feature = "parallel"))]
        for _ in 0..passes {
            blur_interleaved_vertical_radius_one_fractional_rows(
                &work,
                &mut scratch,
                width,
                height,
                channels,
                vertical_weight,
                vertical_fractional_weight,
            );
            std::mem::swap(&mut work, &mut scratch);
        }
    } else {
        let mut vertical_accumulator = vec![0u32; width * channels];
        for _ in 0..passes {
            blur_columns(
                &work,
                &mut scratch,
                width,
                height,
                channels,
                vertical_radius,
                vertical_weight,
                vertical_fractional_weight,
                &mut vertical_accumulator,
            );
            std::mem::swap(&mut work, &mut scratch);
        }
    }

    if use_transposed_vertical {
        scratch
    } else {
        work
    }
}

// ── PIL-style box blur ──

const BOX_BLUR_SCALE: u32 = 1 << 24;
const BOX_BLUR_BIAS: u32 = 1 << 23;
const UNIFORM_BOX_BLUR_MAX_PIXELS: usize = 64 * 64;

/// Check whether a box blur has no image domain or no active radius.
#[inline(always)]
fn box_blur_input_is_noop(width: u32, height: u32, radius_x: f32, radius_y: f32) -> bool {
    width == 0 || height == 0 || (radius_x == 0.0 && radius_y == 0.0)
}

/// Check small images for the replicated-edge box-blur identity case.
#[inline(always)]
fn small_uniform_box_blur_is_identity(
    img: &DynamicImage,
    width: usize,
    height: usize,
    channels: usize,
) -> bool {
    if width.saturating_mul(height) > UNIFORM_BOX_BLUR_MAX_PIXELS {
        return false;
    }
    let mut pixels = img.as_bytes().chunks_exact(channels);
    if let Some(first_pixel) = pixels.next() {
        pixels.all(|pixel| pixel == first_pixel) && pixels.remainder().is_empty()
    } else {
        false
    }
}

/// Return whether Pillow's fixed-point box pass leaves one constant byte
/// unchanged for every position in a replicated-edge line.
#[cfg(not(feature = "parallel"))]
#[inline]
fn box_blur_preserves_constant_byte(
    value: u8,
    radius: usize,
    whole_weight: u32,
    fractional_weight: u32,
) -> bool {
    let Some(window_pixels) = radius
        .checked_mul(2)
        .and_then(|diameter| diameter.checked_add(1))
        .and_then(|window| u32::try_from(window).ok())
    else {
        return false;
    };

    let mut weighted = u32::from(value)
        .wrapping_mul(window_pixels)
        .wrapping_mul(whole_weight);
    if fractional_weight != 0 {
        let far_pair = (u32::from(value) + u32::from(value)).wrapping_mul(fractional_weight);
        weighted = weighted.wrapping_add(far_pair);
    }
    (weighted.wrapping_add(BOX_BLUR_BIAS) >> 24) as u8 == value
}

/// Prove that a large RGBA image is uniform and that one horizontal and one
/// vertical fixed-point pass preserve each of its channel values. The full
/// scan is intentional: sampled probes cannot prove that an arbitrary input
/// is uniform.
#[cfg(not(feature = "parallel"))]
#[inline]
fn uniform_rgba_box_blur_is_identity(
    raw: &[u8],
    width: usize,
    height: usize,
    horizontal: (usize, u32, u32),
    vertical: (usize, u32, u32),
) -> bool {
    let Some(expected_len) = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
    else {
        return false;
    };
    if width == 0 || height == 0 || raw.len() != expected_len {
        return false;
    }
    let first_pixel = &raw[..4];
    if !first_pixel.iter().all(|&value| {
        box_blur_preserves_constant_byte(value, horizontal.0, horizontal.1, horizontal.2)
            && box_blur_preserves_constant_byte(value, vertical.0, vertical.1, vertical.2)
    }) {
        return false;
    }

    // Cheaply reject typical nonuniform images before the full proof scan.
    // These points can reject a candidate, but only the complete scan below
    // can prove uniformity.
    let last_x = width - 1;
    let last_y = height - 1;
    for row in 0..=4 {
        let y = (last_y / 4) * row + (last_y % 4) * row / 4;
        for column in 0..=6 {
            let x = (last_x / 6) * column + (last_x % 6) * column / 6;
            let start = (y * width + x) * 4;
            if &raw[start..start + 4] != first_pixel {
                return false;
            }
        }
    }
    raw.chunks_exact(4).all(|pixel| pixel == first_pixel)
}

/// Preserve the exact large-uniform RGBA Gaussian fast path.
#[cfg(not(feature = "parallel"))]
#[inline(always)]
fn large_uniform_rgba_gaussian_blur_is_identity(
    img: &DynamicImage,
    width: usize,
    height: usize,
    passes: u32,
    horizontal: (usize, u32, u32),
    vertical: (usize, u32, u32),
) -> bool {
    passes == 3
        && width.saturating_mul(height) > UNIFORM_BOX_BLUR_MAX_PIXELS
        && matches!(img, DynamicImage::ImageRgba8(_))
        && uniform_rgba_box_blur_is_identity(img.as_bytes(), width, height, horizontal, vertical)
}

#[inline]
fn blur_parameters(radius: f32) -> (usize, u32, u32) {
    // Integer part of radius (PIL: (int)floatRadius).
    let integer_radius = radius as i32 as usize;
    // Number of pixels in the integer window.
    let window_pixels = (2 * integer_radius + 1) as u32;
    // Fixed-point weight: PIL uses f32 precision for ww computation
    // (UINT32)((1 << 24) / (floatRadius * 2 + 1)) — all in f32.
    let whole_weight = (BOX_BLUR_SCALE as f32 / (radius * 2.0 + 1.0)) as u32;
    // Fractional edge weight (PIL: fw = ((1 << 24) - window_pixels * ww) / 2).
    let fractional_weight =
        BOX_BLUR_SCALE.wrapping_sub(window_pixels.wrapping_mul(whole_weight)) / 2;
    (integer_radius, whole_weight, fractional_weight)
}

/// PIL-style box blur with fractional radius support.
/// Uses sliding-window accumulator with fixed-point (24-bit) arithmetic.
/// Matches PIL order: ALL horizontal passes first, then ALL vertical passes.
fn pil_box_blur(img: &DynamicImage, radius: f32, passes: u32) -> Result<DynamicImage, PilError> {
    pil_box_blur_xy(img, radius, radius, passes)
}

/// PIL-style box blur with independent horizontal and vertical radii.
///
/// Pillow's `ImagingBoxBlur` applies every horizontal pass with `xradius`,
/// then every vertical pass with `yradius`. Keeping the fixed-point weights
/// separate is required for `BoxBlur((xradius, yradius))`.
fn pil_box_blur_xy(
    img: &DynamicImage,
    radius_x: f32,
    radius_y: f32,
    passes: u32,
) -> Result<DynamicImage, PilError> {
    pil_box_blur_xy_impl(img, radius_x, radius_y, passes)
}

fn pil_box_blur_xy_impl(
    img: &DynamicImage,
    radius_x: f32,
    radius_y: f32,
    passes: u32,
) -> Result<DynamicImage, PilError> {
    let channels = img.color().channel_count() as usize;
    let (w_u32, h_u32) = (img.width(), img.height());
    if box_blur_input_is_noop(w_u32, h_u32, radius_x, radius_y) {
        return Ok(img.clone());
    }

    let (width, height) = (w_u32 as usize, h_u32 as usize);

    // This replicated-edge identity check avoids allocating work buffers.
    if small_uniform_box_blur_is_identity(img, width, height, channels) || passes == 0 {
        return Ok(img.clone());
    }

    let (horizontal_radius, horizontal_weight, horizontal_fractional_weight) =
        blur_parameters(radius_x);
    let (vertical_radius, vertical_weight, vertical_fractional_weight) = blur_parameters(radius_y);

    // The exact rounded fixed-point proof protects the large RGBA fast path.
    #[cfg(not(feature = "parallel"))]
    if large_uniform_rgba_gaussian_blur_is_identity(
        img,
        width,
        height,
        passes,
        (
            horizontal_radius,
            horizontal_weight,
            horizontal_fractional_weight,
        ),
        (vertical_radius, vertical_weight, vertical_fractional_weight),
    ) {
        return Ok(img.clone());
    }

    #[cfg(not(feature = "parallel"))]
    if passes == 1
        && horizontal_radius == 1
        && vertical_radius == 1
        && horizontal_fractional_weight == 0
        && vertical_fractional_weight == 0
        && matches!(
            img,
            DynamicImage::ImageLuma8(_)
                | DynamicImage::ImageLumaA8(_)
                | DynamicImage::ImageRgb8(_)
                | DynamicImage::ImageRgba8(_)
        )
    {
        let output = blur_box_radius_one_row_ring(
            img.as_bytes(),
            w_u32,
            h_u32,
            channels,
            horizontal_weight,
            vertical_weight,
        )?;
        let result = raw_bytes_to_image(w_u32, h_u32, output, channels)?;
        return Ok(preserve_mode(img, result));
    }

    #[cfg(not(feature = "parallel"))]
    let rgb_gaussian_row_fusion = passes == 3 && matches!(img, DynamicImage::ImageRgb8(_));
    #[cfg(feature = "parallel")]
    let rgb_gaussian_row_fusion = false;
    #[cfg(not(feature = "parallel"))]
    let luma_gaussian_row_fusion = passes == 3 && matches!(img, DynamicImage::ImageLuma8(_));
    #[cfg(feature = "parallel")]
    let luma_gaussian_row_fusion = false;
    #[cfg(not(feature = "parallel"))]
    let la_gaussian_row_fusion = passes == 3 && matches!(img, DynamicImage::ImageLumaA8(_));
    #[cfg(feature = "parallel")]
    let la_gaussian_row_fusion = false;
    #[cfg(not(feature = "parallel"))]
    let rgba_gaussian_row_fusion = passes == 3 && matches!(img, DynamicImage::ImageRgba8(_));
    #[cfg(feature = "parallel")]
    let rgba_gaussian_row_fusion = false;

    let pass_context = BoxBlurPassContext {
        width,
        height,
        channels,
        passes,
        horizontal_radius,
        horizontal_weight,
        horizontal_fractional_weight,
        vertical_radius,
        vertical_weight,
        vertical_fractional_weight,
        rgb_gaussian_row_fusion,
        luma_gaussian_row_fusion,
        la_gaussian_row_fusion,
        rgba_gaussian_row_fusion,
    };
    let work = if rgb_gaussian_row_fusion
        || luma_gaussian_row_fusion
        || la_gaussian_row_fusion
        || rgba_gaussian_row_fusion
    {
        CheckedDims::new(w_u32, h_u32, channels as u8)?.alloc_buffer()
    } else {
        img.as_bytes().to_vec()
    };
    let scratch = CheckedDims::new(w_u32, h_u32, channels as u8)?.alloc_buffer();
    let (work, scratch) = blur_horizontal_passes(img.as_bytes(), work, scratch, pass_context);
    let output = blur_vertical_passes(work, scratch, pass_context);
    let result = raw_bytes_to_image(w_u32, h_u32, output, channels)?;
    Ok(preserve_mode(img, result))
}

// ── Rank filter ──

/// Execute a Gaussian blur via 3 passes of PIL-style box blur.
pub fn execute_gaussian_blur(img: &DynamicImage, sigma: f32) -> Result<DynamicImage, PilError> {
    let sigma = sigma.abs();
    if sigma == 0.0 {
        return Ok(img.clone());
    }
    pil_box_blur(img, gaussian_blur_box_radius(sigma), 3)
}

/// Match Pillow's three-pass Gaussian-to-box radius calculation.
fn gaussian_blur_box_radius(sigma: f32) -> f32 {
    // Uses the "From Box Blur to Gaussian Blur" algorithm (Gwosdek et al. 2011).
    // Pillow's ImagingGaussianBlur uses f32 parameters but f64 in sqrt/promotion.
    let passes = 3.0f64;
    let sigma2 = sigma as f64 * sigma as f64 / passes;
    let l_val = ((12.0 * sigma2 + 1.0).sqrt() - 1.0) / 2.0;
    let l = l_val.floor();
    let l1 = l + 1.0;
    let a_num = (2.0 * l + 1.0) * (l * l1 - 3.0 * sigma2);
    // For each interval selected by `l = floor(l_val)`, sigma2 is strictly
    // below `(l + 1)^2`, so this denominator cannot be zero for sigma > 0.
    let a_den = 6.0 * (sigma2 - l1 * l1);
    let a = a_num / a_den;
    // Assign back to f32 (PIL: result is float)
    (l + a) as f32
}

/// Execute a box blur with integer radius.
pub fn execute_box_blur(img: &DynamicImage, radius: u32) -> Result<DynamicImage, PilError> {
    if radius == 0 {
        return Ok(img.clone());
    }
    pil_box_blur(img, radius as f32, 1)
}

/// Execute a box blur with independent radii and a fixed number of passes.
pub fn execute_box_blur_xy_with_passes(
    img: &DynamicImage,
    radius_x: f32,
    radius_y: f32,
    passes: u32,
) -> Result<DynamicImage, PilError> {
    if radius_x == 0.0 && radius_y == 0.0 {
        return Ok(img.clone());
    }
    pil_box_blur_xy(img, radius_x, radius_y, passes)
}

#[cfg(test)]
#[path = "blur/tests.rs"]
mod tests;
