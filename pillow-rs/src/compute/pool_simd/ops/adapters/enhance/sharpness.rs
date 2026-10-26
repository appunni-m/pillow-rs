use super::{
    DynamicImage, PilError, PipelineOp, f32x8, has_vectorized_sharpness_bytes, i16x8,
    native_filter_3x3_channel_block_integer, native_filter_3x3_luma_block_integer,
    native_filter_3x3_pixel_block_integer, native_filter_3x3_rgb_byte_block_integer,
    native_filter_3x3_vector, native_filter_load_byte_block, native_sharpness_layout,
    preserve_mode, simd_duplicate, simd_unsupported,
};
/// Run Sharpness's 3x3 byte filter and Pillow's f32 blend in one output pass.
///
/// Interior filtered bytes are narrowed exactly as Pillow's SMOOTH result is
/// before they enter the blend. Edge pixels use the original byte as their
/// filtered value, preserving the reference's exact-copy border behavior.
/// LA/RGBA alpha stays copied from the source.
#[derive(Clone, Copy)]
struct NativeSharpnessPlan<'a> {
    source: &'a [u8],
    width: usize,
    height: usize,
    channels: usize,
    active_channels: usize,
    row_stride: usize,
    kernel: &'a [f32; 9],
    rounding_bias: f32,
    alpha: f32,
    grouped_rgb_bytes: bool,
    grouped_pixels: bool,
    pixels_per_block: usize,
}

#[inline]
fn native_sharpness_rgb_row(plan: &NativeSharpnessPlan<'_>, y: usize, row: &mut [u8]) {
    // RGB's channel bytes are contiguous, so vectorize them as one native
    // stream instead of gathering one channel across eight pixels at a time.
    let interior_end = (plan.width - 1) * plan.channels;
    let source_row = y * plan.row_stride;
    let mut byte_offset = plan.channels;
    while byte_offset < interior_end {
        let active_bytes = (interior_end - byte_offset).min(8);
        let smooth = native_filter_3x3_rgb_byte_block_integer(
            plan.source,
            plan.width,
            y,
            byte_offset,
            active_bytes,
        );
        let original =
            native_filter_load_byte_block(plan.source, source_row + byte_offset, active_bytes);
        let values = native_sharpness_blend_lanes(original, smooth, plan.alpha).to_array();
        for (lane, value) in values.into_iter().enumerate().take(active_bytes) {
            row[byte_offset + lane] = native_sharpness_blend_result(value);
        }
        byte_offset += active_bytes;
    }
}

#[inline]
fn native_sharpness_rgbx_row(plan: &NativeSharpnessPlan<'_>, y: usize, row: &mut [u8]) {
    // RGBX processes X as stored data; group adjacent pixels so the four
    // strided channel gathers fill one compact eight-lane block.
    let source_row = y * plan.row_stride;
    let mut x = 1usize;
    while x < plan.width - 1 {
        let active_pixels = (plan.width - 1 - x).min(plan.pixels_per_block);
        let smooth_bytes = native_filter_3x3_pixel_block_integer(
            plan.source,
            plan.width,
            plan.channels,
            y,
            x,
            active_pixels * plan.channels,
        );
        let active_bytes = active_pixels * plan.channels;
        let source_start = source_row + x * plan.channels;
        let original_bytes = std::array::from_fn(|lane| {
            if lane < active_bytes {
                plan.source[source_start + lane]
            } else {
                0
            }
        });
        let values = native_sharpness_blend_integer_block(original_bytes, smooth_bytes, plan.alpha);
        for pixel in 0..active_pixels {
            for channel in 0..plan.channels {
                let lane = pixel * plan.channels + channel;
                row[(x + pixel) * plan.channels + channel] = values[lane];
            }
        }
        x += active_pixels;
    }
}

#[inline]
fn native_sharpness_channels_row(plan: &NativeSharpnessPlan<'_>, y: usize, row: &mut [u8]) {
    let source_row = y * plan.row_stride;
    for channel in 0..plan.active_channels {
        let first = source_row + channel;
        row[channel] =
            native_sharpness_blend_value(plan.source[first], plan.source[first], plan.alpha);
        let mut x = 1usize;
        while x < plan.width - 1 {
            let active = (plan.width - 1 - x).min(8);
            let smooth = if plan.alpha.is_finite() {
                if plan.channels == 1 && active == 8 {
                    native_filter_3x3_luma_block_integer(plan.source, plan.width, y, x)
                        .unwrap_or_else(|| {
                            native_filter_3x3_channel_block_integer(
                                plan.source,
                                plan.width,
                                plan.channels,
                                channel,
                                y,
                                x,
                                active,
                            )
                        })
                } else {
                    native_filter_3x3_channel_block_integer(
                        plan.source,
                        plan.width,
                        plan.channels,
                        channel,
                        y,
                        x,
                        active,
                    )
                }
            } else {
                // Preserve the floating gather when a finite f64 factor
                // overflows as f32; it also defines NaN border behavior.
                let smooth = native_filter_3x3_vector(
                    plan.source,
                    plan.width,
                    plan.channels,
                    channel,
                    y,
                    x,
                    plan.kernel,
                    plan.rounding_bias,
                );
                f32x8::from(smooth.map(f32::from))
            };
            let original: [f32; 8] = std::array::from_fn(|lane| {
                let pixel_x = (x + lane).min(plan.width - 2);
                f32::from(plan.source[source_row + pixel_x * plan.channels + channel])
            });
            let values =
                native_sharpness_blend_lanes(f32x8::from(original), smooth, plan.alpha).to_array();
            for (lane, value) in values.into_iter().enumerate().take(active) {
                row[(x + lane) * plan.channels + channel] = native_sharpness_blend_result(value);
            }
            x += active;
        }
        let last = source_row + (plan.width - 1) * plan.channels + channel;
        row[(plan.width - 1) * plan.channels + channel] =
            native_sharpness_blend_value(plan.source[last], plan.source[last], plan.alpha);
    }
}

#[inline]
fn native_sharpness_blend_row(plan: &NativeSharpnessPlan<'_>, y: usize, row: &mut [u8]) {
    if !(1..plan.height - 1).contains(&y) {
        let source_row = y * plan.row_stride;
        for x in 0..plan.width {
            for channel in 0..plan.active_channels {
                let index = source_row + x * plan.channels + channel;
                row[x * plan.channels + channel] = native_sharpness_blend_value(
                    plan.source[index],
                    plan.source[index],
                    plan.alpha,
                );
            }
        }
    } else if plan.grouped_rgb_bytes {
        native_sharpness_rgb_row(plan, y, row);
    } else if plan.grouped_pixels {
        native_sharpness_rgbx_row(plan, y, row);
    } else {
        native_sharpness_channels_row(plan, y, row);
    }
}

fn native_sharpness_filter_blend(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    active_channels: usize,
    mode: Option<&str>,
    kernel: &[f32; 9],
    rounding_bias: f32,
    factor: f64,
) -> Option<(u64, u64)> {
    let row_stride = width.checked_mul(channels)?;
    if source.len() != output.len()
        || source.len() != row_stride.checked_mul(height)?
        || !(1..=4).contains(&channels)
        || active_channels == 0
        || active_channels > channels
        || width < 3
        || height < 3
    {
        return None;
    }
    let alpha = factor as f32;
    let interior_width = width - 2;
    let interior_height = height - 2;
    // A finite f64 that overflows when narrowed to f32 uses the scalar-gather
    // path, which also preserves NaN border behavior.
    let grouped_rgb_bytes = channels == 3 && active_channels == 3 && alpha.is_finite();
    let grouped_pixels =
        mode == Some("RGBX") && active_channels == channels && channels == 4 && alpha.is_finite();
    let pixels_per_block = (8 / channels).max(1);
    let vector_blocks = if grouped_rgb_bytes {
        (interior_width * channels).div_ceil(8) * interior_height
    } else if grouped_pixels {
        interior_width
            .div_ceil(pixels_per_block)
            .saturating_mul(interior_height)
    } else {
        interior_width
            .div_ceil(8)
            .saturating_mul(interior_height)
            .saturating_mul(active_channels)
    };
    if vector_blocks == 0 {
        return None;
    }
    let scalar_tail = (width * 2 + height * 2 - 4).saturating_mul(active_channels);
    let plan = NativeSharpnessPlan {
        source,
        width,
        height,
        channels,
        active_channels,
        row_stride,
        kernel,
        rounding_bias,
        alpha,
        grouped_rgb_bytes,
        grouped_pixels,
        pixels_per_block,
    };

    #[cfg(feature = "parallel")]
    super::super::simd_rows_mut_serial!(
        output,
        row_stride,
        height,
        |_row_start, _row_end, y, row| {
            native_sharpness_blend_row(&plan, y as usize, row);
        }
    );
    #[cfg(not(feature = "parallel"))]
    for y in 0..height {
        let row_start = y * row_stride;
        native_sharpness_blend_row(&plan, y, &mut output[row_start..row_start + row_stride]);
    }
    Some((vector_blocks as u64, scalar_tail as u64))
}

#[inline]
fn native_sharpness_blend_value(original: u8, smooth: u8, alpha: f32) -> u8 {
    native_sharpness_blend_result(
        alpha.mul_add(f32::from(original) - f32::from(smooth), f32::from(smooth)),
    )
}

#[inline]
fn native_sharpness_blend_lanes(original: f32x8, smooth: f32x8, alpha: f32) -> f32x8 {
    let delta = original - smooth;
    let alpha = f32x8::splat(alpha);
    #[cfg(any(
        target_feature = "fma",
        all(target_arch = "aarch64", target_feature = "neon")
    ))]
    {
        alpha.mul_add(delta, smooth)
    }
    #[cfg(not(any(
        target_feature = "fma",
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    {
        let alpha = alpha.to_array();
        let delta = delta.to_array();
        let smooth = smooth.to_array();
        f32x8::new(std::array::from_fn(|lane| {
            alpha[lane].mul_add(delta[lane], smooth[lane])
        }))
    }
}

/// Blend grouped byte lanes with exact integer forms for common binary factors.
/// For 0.5, 1.5, and 2.0 the reference FMA is exact on byte-valued inputs; the
/// integer forms preserve its clamp and truncation while avoiding float lanes.
#[inline]
fn native_sharpness_blend_integer_block(original: [u8; 8], smooth: [u8; 8], alpha: f32) -> [u8; 8] {
    if alpha == 0.0 {
        return smooth;
    }
    let original = i16x8::new(original.map(i16::from));
    let smooth_lanes = i16x8::new(smooth.map(i16::from));
    let blended = if alpha == 0.5 {
        (original + smooth_lanes) >> 1u32
    } else if alpha == 1.5 {
        ((original * i16x8::splat(3) - smooth_lanes)
            .max(i16x8::splat(0))
            .min(i16x8::splat(510)))
            >> 1u32
    } else if alpha == 2.0 {
        (original * i16x8::splat(2) - smooth_lanes)
            .max(i16x8::splat(0))
            .min(i16x8::splat(255))
    } else {
        return native_sharpness_blend_lanes(
            f32x8::from(original.to_array().map(f32::from)),
            f32x8::from(smooth_lanes.to_array().map(f32::from)),
            alpha,
        )
        .to_array()
        .map(native_sharpness_blend_result);
    };
    blended
        .to_array()
        .map(|value| u8::try_from(value).unwrap_or_default())
}

#[inline]
fn native_sharpness_blend_result(value: f32) -> u8 {
    if value <= 0.0 {
        0
    } else if value >= 255.0 {
        255
    } else {
        value as u8
    }
}

/// Apply ImageEnhance.Sharpness using the native interleaved layout.
///
/// Pillow first applies the 3x3 SMOOTH kernel, then blends that result with
/// the original image. The existing exact 3x3 SIMD kernel supplies the
/// neighborhood pass; this adapter keeps alpha out of that pass and uses
/// f32x8 fused multiply-add lanes to match Image.blend. No CPU pixel adapter
/// or packed RGBA conversion is used.
pub(crate) fn simd_sharpness(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Sharpness { factor } = op else {
        return Err(PilError::ValueError("expected Sharpness op".into()));
    };
    let Some((channels, active_channels)) = native_sharpness_layout(img, mode) else {
        return Err(simd_unsupported("Sharpness"));
    };
    if !factor.is_finite() || !has_vectorized_sharpness_bytes(img, channels) {
        return Err(simd_unsupported("Sharpness"));
    }
    if *factor == 1.0 {
        // Reuse the native copy kernel: the exact identity needs neither the
        // 3x3 temporary nor a blend output pass.
        return simd_duplicate(img, &PipelineOp::Duplicate, mode);
    }
    let width = img.width() as usize;
    let height = img.height() as usize;
    let source = img.as_bytes();
    let kernel = [
        1.0f32 / 13.0,
        1.0f32 / 13.0,
        1.0f32 / 13.0,
        1.0f32 / 13.0,
        5.0f32 / 13.0,
        1.0f32 / 13.0,
        1.0f32 / 13.0,
        1.0f32 / 13.0,
        1.0f32 / 13.0,
    ];
    let mut output = source.to_vec();
    let (vector_blocks, scalar_tail) = native_sharpness_filter_blend(
        source,
        &mut output,
        width,
        height,
        channels,
        active_channels,
        mode,
        &kernel,
        0.5,
        *factor,
    )
    .ok_or_else(|| simd_unsupported("Sharpness"))?;
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::compute::record_pipeline_operation_path("vector");
    let result =
        crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels)?;
    Ok(preserve_mode(img, result))
}
