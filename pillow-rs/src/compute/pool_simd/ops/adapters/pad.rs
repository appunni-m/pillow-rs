//! SIMD Pad adapter and native-layout resize/fill orchestration.

use super::resize::{
    native_pad_channels_for_image, native_pad_channels_for_shape, native_pad_contained_dimensions,
};
use super::{
    SimdImageShape, copy_native_bytes, native_expand_fill_sample, native_fill_row,
    simd_resize_convolution, simd_resize_convolution_into, simd_resize_f, simd_resize_i32,
    simd_resize_nearest, simd_unsupported,
};
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::pipeline::{PipelineOp, ResampleFilter};
use crate::raster::DynamicImage;

fn native_pad_offsets(
    contained_width: u32,
    contained_height: u32,
    target_width: u32,
    target_height: u32,
    centering: (f64, f64),
) -> Option<(usize, usize)> {
    if !centering.0.is_finite() || !centering.1.is_finite() {
        return None;
    }
    let width_gap = target_width.checked_sub(contained_width)?;
    let height_gap = target_height.checked_sub(contained_height)?;
    if width_gap != 0 {
        Some((
            usize::try_from(super::resize::native_pad_round_dimension(
                f64::from(width_gap) * centering.0.clamp(0.0, 1.0),
            )?)
            .ok()?,
            0,
        ))
    } else {
        Some((
            0,
            usize::try_from(super::resize::native_pad_round_dimension(
                f64::from(height_gap) * centering.1.clamp(0.0, 1.0),
            )?)
            .ok()?,
        ))
    }
}

/// Pillow's indexed `P` pad path uses nearest-neighbour sampling regardless
/// of the requested filter. `PA` is different: its index and alpha bytes are
/// filtered independently, so it retains the requested kernel.
fn native_pad_filter(mode: Option<&str>, filter: ResampleFilter) -> ResampleFilter {
    if mode == Some("P") {
        ResampleFilter::Nearest
    } else {
        filter
    }
}

fn native_pad_supported_for_image(
    img: &DynamicImage,
    target_width: u32,
    target_height: u32,
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    let Some(channels) = native_pad_channels_for_image(img, mode) else {
        return false;
    };
    let Some(expected_bytes) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return false;
    };
    if img.as_bytes().len() != expected_bytes {
        return false;
    }
    let filter = native_pad_filter(mode, filter);
    // A zero-width/zero-height source still has a valid Pillow pad contract:
    // the output is a filled canvas. There is no aspect-ratio geometry to
    // compute, so admission is based on the vector fill data plane directly.
    if img.width() == 0 || img.height() == 0 {
        return target_width != 0
            && target_height != 0
            && super::resize::native_resize_supported_for_dimensions(
                img.width(),
                img.height(),
                target_width,
                target_height,
                filter,
                channels,
            );
    }
    let Some((contained_width, contained_height)) = super::resize::native_pad_contained_dimensions(
        img.width(),
        img.height(),
        target_width,
        target_height,
    ) else {
        return false;
    };
    if native_pad_offsets(
        contained_width,
        contained_height,
        target_width,
        target_height,
        (0.5, 0.5),
    )
    .is_none()
    {
        return false;
    }
    (img.width(), img.height()) == (contained_width, contained_height)
        || super::resize::native_resize_supported_for_dimensions(
            img.width(),
            img.height(),
            contained_width,
            contained_height,
            filter,
            channels,
        )
}

fn native_pad_supported_for_shape(
    shape: SimdImageShape,
    target_width: u32,
    target_height: u32,
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    let Some(channels) = native_pad_channels_for_shape(shape, mode) else {
        return false;
    };
    let filter = native_pad_filter(mode, filter);
    if shape.width == 0 || shape.height == 0 {
        return target_width != 0
            && target_height != 0
            && super::resize::native_resize_supported_for_dimensions(
                shape.width,
                shape.height,
                target_width,
                target_height,
                filter,
                channels,
            );
    }
    let Some((contained_width, contained_height)) = super::resize::native_pad_contained_dimensions(
        shape.width,
        shape.height,
        target_width,
        target_height,
    ) else {
        return false;
    };
    native_pad_offsets(
        contained_width,
        contained_height,
        target_width,
        target_height,
        (0.5, 0.5),
    )
    .is_some()
        && ((shape.width, shape.height) == (contained_width, contained_height)
            || super::resize::native_resize_supported_for_dimensions(
                shape.width,
                shape.height,
                contained_width,
                contained_height,
                filter,
                channels,
            ))
}

pub(super) fn supports_for_image(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> bool {
    let PipelineOp::Pad { w, h, filter, .. } = op else {
        return false;
    };
    native_pad_supported_for_image(img, *w, *h, *filter, mode)
}

pub(super) fn supports_for_shape(
    shape: SimdImageShape,
    op: &PipelineOp,
    mode: Option<&str>,
) -> bool {
    let PipelineOp::Pad { w, h, filter, .. } = op else {
        return false;
    };
    native_pad_supported_for_shape(shape, *w, *h, *filter, mode)
}

fn native_pad_bytes(
    img: &DynamicImage,
    target_width: u32,
    target_height: u32,
    filter: ResampleFilter,
    color: Option<(u8, u8, u8, u8)>,
    centering: (f64, f64),
    mode: Option<&str>,
) -> Result<Option<(DynamicImage, u64, u64)>, PilError> {
    let Some(channels) = native_pad_channels_for_image(img, mode) else {
        return Ok(None);
    };
    let filter = native_pad_filter(mode, filter);
    if !native_pad_supported_for_image(img, target_width, target_height, filter, mode) {
        return Ok(None);
    }

    // `ImageOps.pad` has no contained image when a source dimension is zero;
    // Pillow returns the requested filled canvas. Fill it as a native byte
    // plane so this remains a real SIMD data path for P/PA/F/I and color-space
    // modes as well as ordinary byte images.
    if img.width() == 0 || img.height() == 0 {
        let target_width_usize =
            usize::try_from(target_width).map_err(|_| simd_unsupported("Pad"))?;
        let target_height_usize =
            usize::try_from(target_height).map_err(|_| simd_unsupported("Pad"))?;
        let target_stride = target_width_usize
            .checked_mul(channels)
            .ok_or_else(|| simd_unsupported("Pad"))?;
        let output_len = target_stride
            .checked_mul(target_height_usize)
            .ok_or_else(|| simd_unsupported("Pad"))?;
        let default_fill = if channels == 2 || channels == 4 {
            (0, 0, 0, 0)
        } else {
            (0, 0, 0, u8::MAX)
        };
        let fill = color.unwrap_or(default_fill);
        let mut output = vec![0u8; output_len];
        let mut vector_blocks = 0u64;
        let mut scalar_tail = 0u64;
        for row in output.chunks_exact_mut(target_stride) {
            let (blocks, tail) = native_fill_row(row, fill, channels)
                .ok_or_else(|| PilError::InternalError("SIMD pad fill shape mismatch".into()))?;
            vector_blocks = vector_blocks.saturating_add(blocks);
            scalar_tail = scalar_tail.saturating_add(tail);
        }
        let result =
            crate::image_utils::raw_bytes_to_image(target_width, target_height, output, channels)?;
        return Ok(Some((
            preserve_mode(img, result),
            vector_blocks,
            scalar_tail,
        )));
    }
    let Some((contained_width, contained_height)) =
        native_pad_contained_dimensions(img.width(), img.height(), target_width, target_height)
    else {
        return Ok(None);
    };
    let Some((offset_x, offset_y)) = native_pad_offsets(
        contained_width,
        contained_height,
        target_width,
        target_height,
        centering,
    ) else {
        return Ok(None);
    };
    let source_width = usize::try_from(contained_width).map_err(|_| simd_unsupported("Pad"))?;
    let source_height = usize::try_from(contained_height).map_err(|_| simd_unsupported("Pad"))?;
    let target_width_usize = usize::try_from(target_width).map_err(|_| simd_unsupported("Pad"))?;
    let target_height_usize =
        usize::try_from(target_height).map_err(|_| simd_unsupported("Pad"))?;
    let source_stride = source_width
        .checked_mul(channels)
        .ok_or_else(|| simd_unsupported("Pad"))?;
    let target_stride = target_width_usize
        .checked_mul(channels)
        .ok_or_else(|| simd_unsupported("Pad"))?;
    let output_len = target_stride
        .checked_mul(target_height_usize)
        .ok_or_else(|| simd_unsupported("Pad"))?;
    let default_fill = if channels == 2 || channels == 4 {
        (0, 0, 0, 0)
    } else {
        (0, 0, 0, u8::MAX)
    };
    let fill = color.unwrap_or(default_fill);

    // For the common vertical-pad case, contain did no resampling and the
    // source spans the full output width. Build one native fill row and repeat
    // it, then copy the original bytes into one contiguous output range. This
    // avoids both an intermediate source clone and per-byte fill lane
    // construction for every output row (especially costly for 3-byte HSV and
    // YCbCr pixels).
    if (img.width(), img.height()) == (contained_width, contained_height)
        && offset_x == 0
        && source_width == target_width_usize
    {
        let fill_pixel = (0..channels)
            .map(|channel| native_expand_fill_sample(fill, channels, channel))
            .collect::<Vec<_>>();
        let source = img.as_bytes();
        let expected_source = source_stride
            .checked_mul(source_height)
            .ok_or_else(|| simd_unsupported("Pad"))?;
        if source.len() != expected_source {
            return Ok(None);
        }
        let source_end = offset_y
            .checked_add(source_height)
            .filter(|end| *end <= target_height_usize)
            .ok_or_else(|| simd_unsupported("Pad"))?;
        let needs_fill_rows = offset_y != 0 || source_end != target_height_usize;
        let (fill_row, vector_blocks, scalar_tail) = if mode == Some("RGBA") && needs_fill_rows {
            // RGBA padding builds its reusable alpha-aware fill row with SIMD
            // lanes, then appends it around the original source bytes.
            let mut row = vec![0u8; target_stride];
            let (vector_blocks, scalar_tail) = native_fill_row(&mut row, fill, channels)
                .ok_or_else(|| {
                    PilError::InternalError("SIMD RGBA Pad fill shape mismatch".into())
                })?;
            (row, vector_blocks, scalar_tail)
        } else {
            (fill_pixel.repeat(target_width_usize), 0, 0)
        };
        if fill_row.len() != target_stride {
            return Ok(None);
        }
        let append_vertical_rows = matches!(mode, Some("RGB" | "RGBA" | "CMYK" | "RGBX"))
            || (channels == 1 && matches!(mode, None | Some("L")));
        let mut output = if append_vertical_rows {
            Vec::with_capacity(output_len)
        } else {
            fill_row.repeat(target_height_usize)
        };
        if append_vertical_rows {
            // Avoid initializing source rows and overwriting them: append
            // native source bytes between the top and bottom fill rows.
            for _ in 0..offset_y {
                output.extend_from_slice(&fill_row);
            }
            output.extend_from_slice(source);
            for _ in source_end..target_height_usize {
                output.extend_from_slice(&fill_row);
            }
            if output.len() != output_len {
                return Ok(None);
            }
        } else {
            let destination_start = offset_y
                .checked_mul(target_stride)
                .ok_or_else(|| simd_unsupported("Pad"))?;
            let destination_end = destination_start
                .checked_add(source.len())
                .filter(|end| *end <= output.len())
                .ok_or_else(|| simd_unsupported("Pad"))?;
            output[destination_start..destination_end].copy_from_slice(source);
        }
        let result =
            crate::image_utils::raw_bytes_to_image(target_width, target_height, output, channels)?;
        return Ok(Some((
            preserve_mode(img, result),
            vector_blocks,
            scalar_tail,
        )));
    }

    let luma_byte_i32 = matches!(filter, ResampleFilter::Bicubic)
        && matches!(mode, None | Some("L"))
        && matches!(img, DynamicImage::ImageLuma8(_));

    // Keep the exact separable resize arithmetic, but let the vertical pass
    // store directly into the full-width content window of the final padded
    // canvas. The previous route materialized a contained image, then
    // initialized a second canvas and copied every resized row into it.
    if (mode == Some("RGBX")
        || matches!(mode, None | Some("L")) && matches!(img, DynamicImage::ImageLuma8(_)))
        && !matches!(filter, ResampleFilter::Nearest)
        && (img.width(), img.height()) != (contained_width, contained_height)
        && offset_x == 0
        && source_width == target_width_usize
    {
        let fill_pixel = (0..channels)
            .map(|channel| native_expand_fill_sample(fill, channels, channel))
            .collect::<Vec<_>>();
        let fill_row = fill_pixel.repeat(target_width_usize);
        if fill_row.len() != target_stride {
            return Ok(None);
        }
        let mut output = fill_row.repeat(target_height_usize);
        let content_len = source_stride
            .checked_mul(source_height)
            .ok_or_else(|| simd_unsupported("Pad"))?;
        let window_start = offset_y
            .checked_mul(target_stride)
            .ok_or_else(|| simd_unsupported("Pad"))?;
        let window_end = window_start
            .checked_add(content_len)
            .filter(|end| *end <= output.len())
            .ok_or_else(|| simd_unsupported("Pad"))?;
        simd_resize_convolution_into(
            img,
            contained_width,
            contained_height,
            filter,
            channels,
            false,
            luma_byte_i32,
            0,
            false,
            &mut output[window_start..window_end],
        )?;
        let result =
            crate::image_utils::raw_bytes_to_image(target_width, target_height, output, channels)?;
        return Ok(Some((preserve_mode(img, result), 0, 0)));
    }

    // If the contain step leaves the source dimensions unchanged, Pillow
    // copies the source directly. Resampling an equal-sized image changes
    // edge pixels for convolution filters, which caused the large RGBA pad
    // cases to diverge even though no resize was required.
    let resize_vector_blocks = 0u64;
    let resize_scalar_tail = 0u64;
    let identity_contain = (img.width(), img.height()) == (contained_width, contained_height);
    let resized = if identity_contain {
        None
    } else if mode == Some("F") {
        Some(simd_resize_f(
            img,
            contained_width,
            contained_height,
            &filter,
        )?)
    } else if mode == Some("I") {
        Some(simd_resize_i32(
            img,
            contained_width,
            contained_height,
            &filter,
        )?)
    } else {
        Some(match filter {
            ResampleFilter::Nearest => {
                simd_resize_nearest(img, contained_width, contained_height, channels)?
            }
            _ => simd_resize_convolution(
                img,
                contained_width,
                contained_height,
                filter,
                channels,
                match channels {
                    2 => matches!(mode, None | Some("LA")),
                    4 => matches!(mode, None | Some("RGBA")),
                    _ => false,
                },
                luma_byte_i32,
            )?,
        })
    };
    let target_width = target_width_usize;
    let target_height = target_height_usize;
    let source = resized
        .as_ref()
        .map_or_else(|| img.as_bytes(), DynamicImage::as_bytes);
    if source.len() != source_stride.saturating_mul(source_height)
        || offset_x.saturating_add(source_width) > target_width
        || offset_y.saturating_add(source_height) > target_height
    {
        return Ok(None);
    }

    let mut output = vec![0u8; output_len];
    let mut vector_blocks = resize_vector_blocks;
    let mut scalar_tail = resize_scalar_tail;
    let fill_is_zero =
        (0..channels).all(|sample| native_expand_fill_sample(fill, channels, sample) == 0);
    let content_start = offset_x
        .checked_mul(channels)
        .ok_or_else(|| simd_unsupported("Pad"))?;
    let content_end = content_start
        .checked_add(source_stride)
        .ok_or_else(|| simd_unsupported("Pad"))?;
    for (output_y, row) in output.chunks_exact_mut(target_stride).enumerate() {
        if output_y < offset_y || output_y - offset_y >= source_height {
            if !fill_is_zero {
                let (blocks, tail) = native_fill_row(row, fill, channels).ok_or_else(|| {
                    PilError::InternalError("SIMD pad fill shape mismatch".into())
                })?;
                vector_blocks = vector_blocks.saturating_add(blocks);
                scalar_tail = scalar_tail.saturating_add(tail);
            }
            continue;
        }

        let source_y = output_y - offset_y;
        if !fill_is_zero {
            let (blocks, tail) = native_fill_row(&mut row[..content_start], fill, channels)
                .ok_or_else(|| PilError::InternalError("SIMD pad fill shape mismatch".into()))?;
            vector_blocks = vector_blocks.saturating_add(blocks);
            scalar_tail = scalar_tail.saturating_add(tail);
        }
        let source_start = source_y
            .checked_mul(source_stride)
            .ok_or_else(|| simd_unsupported("Pad"))?;
        let (blocks, tail) = copy_native_bytes(
            &source[source_start..source_start + source_stride],
            &mut row[content_start..content_end],
        )
        .ok_or_else(|| PilError::InternalError("SIMD pad copy shape mismatch".into()))?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
        if !fill_is_zero {
            let (blocks, tail) = native_fill_row(&mut row[content_end..], fill, channels)
                .ok_or_else(|| PilError::InternalError("SIMD pad fill shape mismatch".into()))?;
            vector_blocks = vector_blocks.saturating_add(blocks);
            scalar_tail = scalar_tail.saturating_add(tail);
        }
    }
    let result = crate::image_utils::raw_bytes_to_image(
        target_width as u32,
        target_height as u32,
        output,
        channels,
    )?;
    Ok(Some((
        preserve_mode(img, result),
        vector_blocks,
        scalar_tail,
    )))
}

/// Execute `ImageOps.pad` with a scalar contain/centering control plane and
/// native-layout SIMD resize, fill, and row-copy data planes. Explicit SIMD
/// never retries the CPU pad implementation when this contract is not met.
pub(crate) fn simd_pad(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Pad {
        w,
        h,
        filter,
        color,
        centering,
    } = op
    else {
        return Err(PilError::ValueError("expected Pad op".into()));
    };
    let Some((result, vector_blocks, scalar_tail)) =
        native_pad_bytes(img, *w, *h, *filter, *color, *centering, mode)?
    else {
        return Err(simd_unsupported("Pad"));
    };
    if vector_blocks != 0 || scalar_tail != 0 {
        crate::compute::record_pipeline_operation_path("vector");
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }
    Ok(result)
}
