//! Public box and Gaussian blur orchestration.

// TODO(pillow-rs#3ptb): freeze BoxBlur, BoxBlurXY, and GaussianBlur output and
// selected/fallback receipts across declared exact modes and architectures in
// CPU parity. Source comparison against the archived implementation is complete.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;

pub(crate) const BOX_BLUR_BIAS: u32 = 1 << 23;

#[path = "kernels/blur_line_steps.rs"]
mod blur_line_steps;
#[path = "blur/box_rows.rs"]
mod box_rows;
#[path = "blur/line.rs"]
mod line;
#[path = "blur/radius_one.rs"]
mod radius_one;
#[path = "blur/vector.rs"]
mod vector;


use box_rows::blur_box_radius_one_row_ring;
#[cfg(test)]
use line::blur_line;
use line::{blur_columns, blur_line_step, blur_rows};


use radius_one::{
    blur_interleaved_rows_three_passes, blur_interleaved_vertical_radius_one_fractional_rows,
    blur_la_rows_three_passes_radius_one, blur_luma_rows_three_passes,
    blur_luma_vertical_radius_one_rows,
};
#[cfg(test)]
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
    fuse_three_horizontal_passes: bool,
}

#[inline(always)]
fn blur_horizontal_passes(
    source: &[u8],
    mut work: Vec<u8>,
    mut scratch: Vec<u8>,
    context: BoxBlurPassContext,
) -> (Vec<u8>, Vec<u8>) {
    

    let BoxBlurPassContext {
        width,
        height,
        channels,
        passes,
        horizontal_radius,
        horizontal_weight,
        horizontal_fractional_weight,
        fuse_three_horizontal_passes,
        ..
    } = context;
    // PIL does ALL horizontal passes first (matching ImagingBoxBlur order)
    if fuse_three_horizontal_passes {
        match channels {
            1 => blur_luma_rows_three_passes(
                source,
                &mut work,
                width,
                height,
                horizontal_radius,
                horizontal_weight,
                horizontal_fractional_weight,
            ),
            2 if horizontal_radius == 1 && horizontal_fractional_weight != 0 && width >= 5 => {
                blur_la_rows_three_passes_radius_one(
                    source,
                    &mut work,
                    width,
                    height,
                    horizontal_weight,
                    horizontal_fractional_weight,
                )
            }
            2 => blur_interleaved_rows_three_passes::<2>(
                source,
                &mut work,
                width,
                height,
                horizontal_radius,
                horizontal_weight,
                horizontal_fractional_weight,
            ),
            3 => blur_interleaved_rows_three_passes::<3>(
                source,
                &mut work,
                width,
                height,
                horizontal_radius,
                horizontal_weight,
                horizontal_fractional_weight,
            ),
            4 => blur_interleaved_rows_three_passes::<4>(
                source,
                &mut work,
                width,
                height,
                horizontal_radius,
                horizontal_weight,
                horizontal_fractional_weight,
            ),
            _ => blur_rows_repeated(
                &mut work,
                &mut scratch,
                width,
                height,
                channels,
                passes,
                horizontal_radius,
                horizontal_weight,
                horizontal_fractional_weight,
            ),
        }
    } else {
        blur_rows_repeated(
            &mut work,
            &mut scratch,
            width,
            height,
            channels,
            passes,
            horizontal_radius,
            horizontal_weight,
            horizontal_fractional_weight,
        );
    }

    (work, scratch)
}

#[inline(always)]
fn blur_rows_repeated(
    work: &mut Vec<u8>,
    scratch: &mut Vec<u8>,
    width: usize,
    height: usize,
    channels: usize,
    passes: u32,
    radius: usize,
    weight: u32,
    fractional_weight: u32,
) {
    for _ in 0..passes {
        blur_rows(
            work,
            scratch,
            width,
            height,
            channels,
            radius,
            weight,
            fractional_weight,
        );
        std::mem::swap(work, scratch);
    }
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
        fuse_three_horizontal_passes,
        ..
    } = context;
    if fuse_three_horizontal_passes && channels == 1 && vertical_radius == 1 {
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
    } else if fuse_three_horizontal_passes
        && channels == 4
        && vertical_radius == 1
        && vertical_fractional_weight != 0
    {
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
    work
}

// ── PIL-style box blur ──

const UNIFORM_BOX_BLUR_MAX_PIXELS: usize = 64 * 64;

/// Check whether a box blur has no image domain or no active radius.
#[inline(always)]
fn box_blur_input_is_noop(width: u32, height: u32, radius_x: f32, radius_y: f32) -> bool {
    width == 0 || height == 0 || (radius_x == 0.0 && radius_y == 0.0)
}

/// Check small images for the replicated-edge box-blur identity case.
#[inline(always)]
fn small_uniform_box_blur_is_identity(
    raw: &[u8],
    width: usize,
    height: usize,
    channels: usize,
) -> bool {
    if width.saturating_mul(height) > UNIFORM_BOX_BLUR_MAX_PIXELS {
        return false;
    }
    let mut pixels = raw.chunks_exact(channels);
    if let Some(first_pixel) = pixels.next() {
        pixels.all(|pixel| pixel == first_pixel) && pixels.remainder().is_empty()
    } else {
        false
    }
}

/// Return whether Pillow's fixed-point box pass leaves one constant byte
/// unchanged for every position in a replicated-edge line.

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

/// Prove that a large four-channel image is uniform and one horizontal and one
/// vertical fixed-point pass preserve each of its channel values. The full
/// scan is intentional: sampled probes cannot prove that an arbitrary input
/// is uniform.

#[inline]
fn uniform_four_channel_box_blur_is_identity(
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

/// Preserve the exact large-uniform four-channel Gaussian fast path.

#[inline(always)]
fn large_uniform_four_channel_gaussian_blur_is_identity(
    raw: &[u8],
    width: usize,
    height: usize,
    passes: u32,
    horizontal: (usize, u32, u32),
    vertical: (usize, u32, u32),
) -> bool {
    passes == 3
        && width.saturating_mul(height) > UNIFORM_BOX_BLUR_MAX_PIXELS
        && uniform_four_channel_box_blur_is_identity(raw, width, height, horizontal, vertical)
}

#[inline]
fn blur_parameters(radius: f32) -> (usize, u32, u32) {
    // Share Pillow's exact f32 weighting arithmetic with strategy guards.
    crate::compute::helpers::filters::box_blur_parameters(radius)
}

/// PIL-style box blur with fractional radius support.
/// Uses sliding-window accumulator with fixed-point (24-bit) arithmetic.
/// Matches PIL order: ALL horizontal passes first, then ALL vertical passes.
fn pil_box_blur(
    raw: &[u8],
    width: u32,
    height: u32,
    channels: usize,
    radius: f32,
    passes: u32,
) -> Result<Option<Vec<u8>>, PilError> {
    pil_box_blur_xy(raw, width, height, channels, radius, radius, passes)
}

/// PIL-style box blur with independent horizontal and vertical radii.
///
/// Pillow's `ImagingBoxBlur` applies every horizontal pass with `xradius`,
/// then every vertical pass with `yradius`. Keeping the fixed-point weights
/// separate is required for `BoxBlur((xradius, yradius))`.
fn pil_box_blur_xy(
    raw: &[u8],
    width: u32,
    height: u32,
    channels: usize,
    radius_x: f32,
    radius_y: f32,
    passes: u32,
) -> Result<Option<Vec<u8>>, PilError> {
    pil_box_blur_xy_impl(
        raw, width, height, channels, radius_x, radius_y, passes,
    )
}

fn pil_box_blur_xy_impl(
    raw: &[u8],
    w_u32: u32,
    h_u32: u32,
    channels: usize,
    radius_x: f32,
    radius_y: f32,
    passes: u32,
) -> Result<Option<Vec<u8>>, PilError> {
    if box_blur_input_is_noop(w_u32, h_u32, radius_x, radius_y) {
        return Ok(None);
    }

    let (width, height) = (w_u32 as usize, h_u32 as usize);

    // This replicated-edge identity check avoids allocating work buffers.
    if small_uniform_box_blur_is_identity(raw, width, height, channels) || passes == 0 {
        return Ok(None);
    }

    let (horizontal_radius, horizontal_weight, horizontal_fractional_weight) =
        blur_parameters(radius_x);
    let (vertical_radius, vertical_weight, vertical_fractional_weight) = blur_parameters(radius_y);

    // The fixed-point proof protects the large four-channel fast path.
    
    if large_uniform_four_channel_gaussian_blur_is_identity(
        raw,
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
        return Ok(None);
    }

    
    if passes == 1
        && horizontal_radius == 1
        && vertical_radius == 1
        && horizontal_fractional_weight == 0
        && vertical_fractional_weight == 0
        && (1..=4).contains(&channels)
    {
        let output = blur_box_radius_one_row_ring(
            raw,
            w_u32,
            h_u32,
            channels,
            horizontal_weight,
            vertical_weight,
        )?;
        return Ok(Some(output));
    }

    let fuse_three_horizontal_passes = passes == 3 && (1..=4).contains(&channels);

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
        fuse_three_horizontal_passes,
    };
    let work = if fuse_three_horizontal_passes {
        CheckedDims::new(w_u32, h_u32, channels as u8)?.alloc_buffer()
    } else {
        raw.to_vec()
    };
    let scratch = CheckedDims::new(w_u32, h_u32, channels as u8)?.alloc_buffer();
    let (work, scratch) = blur_horizontal_passes(raw, work, scratch, pass_context);
    let output = blur_vertical_passes(work, scratch, pass_context);
    Ok(Some(output))
}

// ── Rank filter ──

/// Execute Gaussian blur over native interleaved byte samples.
pub(crate) fn execute_gaussian_blur_native_bytes(
    raw: &[u8],
    width: u32,
    height: u32,
    channels: usize,
    sigma: f32,
) -> Result<Option<Vec<u8>>, PilError> {
    let sigma = sigma.abs();
    if sigma == 0.0 {
        return Ok(None);
    }
    pil_box_blur(
        raw,
        width,
        height,
        channels,
        gaussian_blur_box_radius(sigma),
        3,
    )
}

/// Copy an identity blur result in native byte layout and publish its path.
pub(in crate::compute::pool_cpu::ops::filter) fn copy_native_byte_blur_input(
    image: &crate::raster::DynamicImage,
    mode: crate::raster::Mode,
    channels: usize,
) -> Result<crate::raster::DynamicImage, PilError> {
    if image.mode() != mode || !(1..=4).contains(&channels) {
        return Err(PilError::InternalError(
            "native-byte blur identity received a mismatched mode or layout".into(),
        ));
    }
    let dimensions = CheckedDims::new_allow_empty(
        image.width(),
        image.height(),
        channels as u8,
    )?;
    if image.as_bytes().len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(
            "native-byte blur identity input does not match its dimensions".into(),
        ));
    }
    let mut output = dimensions.alloc_buffer();
    let (vector_blocks, scalar_tail) =
        crate::compute::pool_cpu::helpers::bytes::vector_copy::copy_native_bytes(
            image.as_bytes(),
            &mut output,
        )
        .ok_or_else(|| PilError::InternalError("native-byte blur copy shape mismatch".into()))?;
    crate::compute::record_pipeline_operation_path("native-copy");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::compute::pool_cpu::helpers::dynamic_image::from_raw_bytes(
        mode,
        image.width(),
        image.height(),
        output,
    )
    .ok_or_else(|| PilError::InternalError("native-byte blur identity output mode mismatch".into()))
}

/// Match Pillow's three-pass Gaussian-to-box radius calculation.
fn gaussian_blur_box_radius(sigma: f32) -> f32 {
    crate::compute::helpers::filters::gaussian_blur_three_pass_radius(sigma)
}

/// Return the exact native-byte box parameters used by Pillow's Gaussian blur.
pub(crate) fn gaussian_blur_box_parameters(sigma: f32) -> (usize, u32, u32) {
    crate::compute::helpers::filters::gaussian_blur_box_parameters(sigma)
}

pub(in crate::compute::pool_cpu::ops::filter) fn execute_native_byte_blur(
    image: &crate::raster::DynamicImage,
    mode: crate::raster::Mode,
    channels: usize,
    request: super::super::ops::FilterRequest,
    architecture: crate::compute::Architecture,
    context: &crate::compute::ExecutionContext,
) -> Result<crate::raster::DynamicImage, PilError> {
    vector::execute(image, mode, channels, request, architecture, context)
}

pub(in crate::compute::pool_cpu::ops::filter) fn validate_native_byte_blur_selection(
    selection: &crate::compute::registry::support::StrategySelection,
    context: &crate::compute::ExecutionContext,
) -> Result<(), PilError> {
    let architecture = selection.implementation.architecture;
    if selection.requested_backend != context.backend()
        || selection.requested_architecture != context.architecture()
        || context.backend() != crate::compute::Backend::Cpu
        || !context.permits_architecture(architecture)
        || !matches!(
            architecture,
            crate::compute::Architecture::Avx2
                | crate::compute::Architecture::Avx
                | crate::compute::Architecture::Sse2
                | crate::compute::Architecture::Neon
                | crate::compute::Architecture::WasmSimd128
        )
    {
        return Err(PilError::InternalError(
            "native-byte blur strategy is outside the injected backend or architecture".into(),
        ));
    }
    Ok(())
}

/// Execute a box blur over native interleaved byte samples.
pub(crate) fn execute_box_blur_native_bytes(
    raw: &[u8],
    width: u32,
    height: u32,
    channels: usize,
    radius: u32,
) -> Result<Option<Vec<u8>>, PilError> {
    if radius == 0 {
        return Ok(None);
    }
    pil_box_blur(raw, width, height, channels, radius as f32, 1)
}

/// Execute BoxBlurXY over native interleaved byte samples.
pub(crate) fn execute_box_blur_xy_native_bytes(
    raw: &[u8],
    width: u32,
    height: u32,
    channels: usize,
    radius_x: f32,
    radius_y: f32,
    passes: u32,
) -> Result<Option<Vec<u8>>, PilError> {
    if radius_x == 0.0 && radius_y == 0.0 {
        return Ok(None);
    }
    pil_box_blur_xy(raw, width, height, channels, radius_x, radius_y, passes)
}

#[cfg(test)]
#[path = "blur/tests.rs"]
mod tests;
