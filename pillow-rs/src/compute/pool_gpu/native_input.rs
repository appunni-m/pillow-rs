use super::*;

// ─── Mode helpers ───────────────────────────────────────────────────────────

/// Map a DynamicImage variant to its mode code for the GPU uniform buffer.
/// 0 = L, 1 = LA, 2 = RGB, 3 = RGBA, 4 = CMYK, 5 = native I;16* geometry,
/// 6 = RGBX, and 7 = native I-mode convolution. The non-byte modes use the
/// packed word only where the selected shader explicitly owns that contract.
pub(super) fn mode_code(img: &DynamicImage) -> u32 {
    match img {
        DynamicImage::ImageLuma8(_) => 0,
        DynamicImage::ImageLumaA8(_) => 1,
        DynamicImage::ImageRgb8(_) => 2,
        DynamicImage::ImageRgba8(_) => 3,
        DynamicImage::ImageLuma16(_) => 5,
        _ => 3, // fallback: treat as RGBA
    }
}

pub(super) fn execution_mode_code(img: &DynamicImage, logical_mode: Option<&str>) -> u32 {
    match logical_mode {
        Some("CMYK") => 4,
        Some("I;16" | "I;16L" | "I;16B" | "I;16N") => 5,
        Some("I") => 7,
        // F stores one little-endian f32 sample in the four-byte transport.
        // The order-statistic shaders use this distinct code instead of
        // treating the sample's representation as four independent bytes.
        Some("F") => 8,
        // RGBX shares RGBA storage, but byte three is padding rather than an
        // alpha sample. The Convert shader uses this distinct source code to
        // force opaque output alpha instead of preserving that padding byte.
        Some("RGBX") => 6,
        // P/PA use the L/LA transport respectively. Preserve the physical
        // two-band code for PA so ExtractBand(1) selects byte three (alpha),
        // and so later raw PA writes do not mistake the alpha byte for green.
        Some("P" | "1") => 0,
        Some("PA") => 1,
        Some("HSV" | "YCbCr") => 2,
        _ => mode_code(img),
    }
}

/// Return the native packed-byte channel count for a pure ExtractBand batch.
///
/// ExtractBand's output is already L and compact. Its input reader can consume
/// the source's native 1/2/3/4-byte pixels directly, avoiding the generic
/// L/LA/RGB-to-RGBA staging expansion. Keep typed I/F and I;16 layouts on
/// their established paths, and require the dynamic image's physical storage
/// to match the public mode's byte layout before setting the shader flag.
#[cfg(target_endian = "little")]
pub(super) fn gpu_native_extract_band_channels(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> Option<u8> {
    let [PipelineOp::ExtractBand { index }] = ops else {
        return None;
    };
    let channels = match (execution_mode_code(image, logical_mode), image) {
        (0, DynamicImage::ImageLuma8(_)) => 1u8,
        (1, DynamicImage::ImageLumaA8(_)) => 2,
        (2, DynamicImage::ImageRgb8(_)) => 3,
        (3 | 4 | 6, DynamicImage::ImageRgba8(_)) => 4,
        _ => return None,
    };
    if *index >= channels {
        return None;
    }
    let input_bytes = u64::from(image.width())
        .checked_mul(u64::from(image.height()))?
        .checked_mul(u64::from(channels))?;
    (input_bytes <= u64::from(u32::MAX)).then_some(channels)
}

/// Admit tightly packed RGB or YCbCr triples as the source for a singleton
/// grayscale dispatch. The shader addresses byte triples through u32 words,
/// so all products must fit its u32 indexing and storage must match exactly.
#[cfg(target_endian = "little")]
pub(super) fn gpu_native_grayscale_rgb_input(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if !matches!(ops, [PipelineOp::Grayscale])
        || !matches!(logical_mode, None | Some("RGB" | "YCbCr"))
    {
        return false;
    }
    let DynamicImage::ImageRgb8(rgb) = image else {
        return false;
    };
    let Ok(layout) = CheckedDims::new(image.width(), image.height(), 3) else {
        return false;
    };
    let input_bytes = layout.total_bytes();
    input_bytes > 0 && input_bytes <= u32::MAX as usize && rgb.as_raw().len() == input_bytes
}

/// Admit exact packed CMYK storage for the singleton grayscale shader. CMYK
/// already occupies the shader's ordinary four-byte input layout, so the
/// mapped path can retain Pillow's mode-specific conversion without expanding
/// or rewriting the source pixels on the host.
#[cfg(target_endian = "little")]
pub(super) fn gpu_native_grayscale_cmyk_input(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if !matches!(ops, [PipelineOp::Grayscale]) || logical_mode != Some("CMYK") {
        return false;
    }
    let DynamicImage::ImageRgba8(cmyk) = image else {
        return false;
    };
    let Ok(layout) = CheckedDims::new(image.width(), image.height(), 4) else {
        return false;
    };
    let input_bytes = layout.total_bytes();
    input_bytes > 0 && input_bytes <= u32::MAX as usize && cmyk.as_raw().len() == input_bytes
}

/// Admit a singleton L -> RGBA conversion whose shader can unpack source
/// samples directly from four-byte storage words. Empty images retain the
/// ordinary path because the packed uploader requires a nonzero transfer.
#[cfg(target_endian = "little")]
pub(super) fn gpu_packed_luma_convert_input(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    matches!(
        ops,
        [PipelineOp::Convert {
            mode: ColorMode::RGBA,
            ..
        }]
    ) && matches!(image, DynamicImage::ImageLuma8(_))
        && matches!(logical_mode, None | Some("L"))
        && image.width() != 0
        && image.height() != 0
}

/// Admit a singleton native-L transform that can unpack source bytes from
/// four-sample storage words. Transform retains its existing RGBA output
/// transport; this only removes source widening and upload bytes.
#[cfg(target_endian = "little")]
pub(super) fn gpu_packed_luma_transform_input(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if !matches!(ops, [PipelineOp::Transform { .. }]) || !matches!(logical_mode, None | Some("L")) {
        return false;
    }
    let DynamicImage::ImageLuma8(luma) = image else {
        return false;
    };
    let Ok(layout) = CheckedDims::new(image.width(), image.height(), 1) else {
        return false;
    };
    let pixels = layout.total_pixels();
    pixels > 0 && pixels <= u32::MAX as usize && luma.as_raw().len() == pixels
}

#[cfg(not(target_endian = "little"))]
pub(super) fn gpu_packed_luma_transform_input(
    _ops: &[PipelineOp],
    _image: &DynamicImage,
    _logical_mode: Option<&str>,
) -> bool {
    false
}

/// Keep a singleton L byte-LUT remap in packed one-byte storage. In addition
/// to ordinary Eval runs, the unmasked native-L AutoContrast lowering supplies
/// an exact Eval LUT and can use the same packed shader without RGBA staging.
pub(super) fn gpu_packed_luma_point_input(
    source_is_luma_point_run: bool,
    host_autocontrast_luma_lut: bool,
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    let native_luma_equalize = matches!(ops, [PipelineOp::Equalize]);
    if !matches!(ops, [PipelineOp::Eval { .. } | PipelineOp::Equalize])
        || !matches!(logical_mode, None | Some("L"))
        || (!native_luma_equalize && !(source_is_luma_point_run || host_autocontrast_luma_lut))
    {
        return false;
    }
    let DynamicImage::ImageLuma8(luma) = image else {
        return false;
    };
    let Ok(layout) = CheckedDims::new(image.width(), image.height(), 1) else {
        return false;
    };
    let pixels = layout.total_pixels();
    pixels != 0 && pixels <= u32::MAX as usize && luma.as_raw().len() == pixels
}

/// Admit native-L neighborhood specializations. Their shaders operate on
/// four adjacent samples packed into one word; keep prefixes, palette modes,
/// empty images, unsupported filter sizes/ranks, and malformed backing lengths
/// on the ordinary path.
#[cfg(target_endian = "little")]
pub(super) fn gpu_packed_luma_order_statistic_input(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if !matches!(
        ops,
        [PipelineOp::MedianFilter { size: 3 }]
            | [PipelineOp::MaxFilter { size: 3 }]
            | [PipelineOp::MinFilter { size: 3 }]
            | [PipelineOp::RankFilter { size: 3, rank: 1 }]
            | [PipelineOp::RankFilter { size: 9, .. }]
    ) || !matches!(logical_mode, None | Some("L"))
    {
        return false;
    }
    let DynamicImage::ImageLuma8(luma) = image else {
        return false;
    };
    let Ok(layout) = CheckedDims::new(image.width(), image.height(), 1) else {
        return false;
    };
    let pixel_count = layout.total_pixels();
    pixel_count > 0 && pixel_count <= u32::MAX as usize && luma.as_raw().len() == pixel_count
}

/// Keep supported singleton native-L and LA byte filters in compact storage.
/// LA MedianFilter(3), RankFilter(3), MaxFilter(3), and MinFilter(3) use two
/// adjacent pixels per word, while the existing blur paths preserve their
/// exact channel-wise fixed-point arithmetic.
#[cfg(target_endian = "little")]
pub(super) fn gpu_packed_native_byte_filter_input(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    match image {
        DynamicImage::ImageLuma8(luma) if matches!(logical_mode, None | Some("L")) => {
            let supported = match ops {
                [PipelineOp::GaussianBlur { sigma }] => {
                    registry::separable_gaussian_blur_radius(*sigma)
                        .is_some_and(|radius| radius <= MAX_GPU_BLUR_RADIUS)
                }
                [PipelineOp::BoxBlur { radius: 1 }] => true,
                [
                    PipelineOp::Filter5x5 {
                        kernel,
                        scale,
                        offset,
                    },
                ] => {
                    // The packed shader emits four adjacent samples per word;
                    // row starts must therefore remain word-aligned. It uses
                    // the same ordered f32/FMA contract as the general 5x5
                    // shader, but only when that contract is proven safe.
                    image.width() % 4 == 0
                        && registry::gpu_filter_kernel_is_exact(kernel, *scale, *offset)
                }
                _ => false,
            };
            if !supported {
                return false;
            }
            let Ok(layout) = CheckedDims::new(image.width(), image.height(), 1) else {
                return false;
            };
            let pixels = layout.total_pixels();
            pixels > 0 && pixels <= u32::MAX as usize && luma.as_raw().len() == pixels
        }
        DynamicImage::ImageLumaA8(la) if matches!(logical_mode, Some("LA")) => {
            let supported = match ops {
                [PipelineOp::GaussianBlur { sigma }] => {
                    registry::separable_gaussian_blur_radius(*sigma)
                        .is_some_and(|radius| radius <= MAX_GPU_BLUR_RADIUS)
                }
                [
                    PipelineOp::Filter3x3 {
                        kernel,
                        scale,
                        offset,
                    },
                ] => crate::compute::pool_cpu::ops::filter::binomial3x3_parameters_match(
                    kernel, *scale, *offset,
                ),
                [
                    PipelineOp::Filter5x5 {
                        kernel,
                        scale,
                        offset,
                    },
                ] => registry::gpu_filter_kernel_is_exact(kernel, *scale, *offset),
                [PipelineOp::BoxBlur { radius: 1 }]
                | [PipelineOp::MedianFilter { size: 3 }]
                | [PipelineOp::MaxFilter { size: 3 }]
                | [PipelineOp::MinFilter { size: 3 }] => true,
                [PipelineOp::RankFilter { size: 3, rank }] if *rank < 9 => true,
                _ => false,
            };
            if !supported {
                return false;
            }
            let Ok(layout) = CheckedDims::new(image.width(), image.height(), 2) else {
                return false;
            };
            let pixels = layout.total_pixels();
            pixels > 0 && pixels <= u32::MAX as usize && la.as_raw().len() == layout.total_bytes()
        }
        _ => false,
    }
}

pub(super) fn gpu_packed_la_rank_filter_3_input(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    matches!(ops, [PipelineOp::RankFilter { size: 3, rank }] if *rank < 9)
        && matches!(logical_mode, Some("LA"))
        && gpu_packed_native_byte_filter_input(ops, image, logical_mode)
}

/// Admit compact L transport for Pad. Real resizes are limited to contained
/// widths divisible by four: the generic exact resize kernels then keep each
/// row-packed intermediate byte-aligned for the flattened placement kernel.
#[cfg(target_endian = "little")]
pub(super) fn gpu_packed_luma_pad_input(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    let [
        PipelineOp::Pad {
            w: dst_w, h: dst_h, ..
        },
    ] = ops
    else {
        return false;
    };
    if !matches!(logical_mode, None | Some("L")) {
        return false;
    }
    let DynamicImage::ImageLuma8(luma) = image else {
        return false;
    };
    let Ok(input_layout) = CheckedDims::new(image.width(), image.height(), 1) else {
        return false;
    };
    let Ok(output_layout) = CheckedDims::new(*dst_w, *dst_h, 1) else {
        return false;
    };
    let input_pixels = input_layout.total_pixels();
    let output_pixels = output_layout.total_pixels();
    if input_pixels == 0
        || output_pixels == 0
        || input_pixels > u32::MAX as usize
        || output_pixels > u32::MAX as usize
        || luma.as_raw().len() != input_pixels
    {
        return false;
    }
    gpu_pad_geometry(&ops[0], image.width(), image.height()).is_some_and(
        |(resize_dimensions, _)| {
            resize_dimensions == image.dimensions() || resize_dimensions.0 % 4 == 0
        },
    )
}

#[cfg(not(target_endian = "little"))]
pub(super) fn gpu_packed_luma_pad_input(
    _ops: &[PipelineOp],
    _image: &DynamicImage,
    _logical_mode: Option<&str>,
) -> bool {
    false
}

pub(super) fn gpu_packed_luma_pad_dispatch_supported(
    op: &PipelineOp,
    source_dimensions: (u32, u32),
    max_workgroups_per_dimension: u32,
) -> bool {
    let PipelineOp::Pad { w, h, .. } = op else {
        return false;
    };
    let Some((resize_dimensions, _)) =
        gpu_pad_geometry(op, source_dimensions.0, source_dimensions.1)
    else {
        return false;
    };
    if resize_dimensions != source_dimensions
        && (plan_packed_luma_resize_dispatch(
            resize_dimensions.0,
            source_dimensions.1,
            max_workgroups_per_dimension,
        )
        .is_err()
            || plan_packed_luma_resize_dispatch(
                resize_dimensions.0,
                resize_dimensions.1,
                max_workgroups_per_dimension,
            )
            .is_err())
    {
        return false;
    }
    plan_packed_luma_dispatch(*w, *h, max_workgroups_per_dimension).is_ok()
}

/// Admit native LA transport for identity-contain Pad only. A resize needs its
/// own exact two-channel separable kernels; keeping this path to a same-size
/// contain means the shader only places whole LA samples and fills the border.
#[cfg(target_endian = "little")]
pub(super) fn gpu_packed_la_pad_input(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if !matches!(ops, [PipelineOp::Pad { .. }]) || !matches!(logical_mode, None | Some("LA")) {
        return false;
    }
    let DynamicImage::ImageLumaA8(la) = image else {
        return false;
    };
    let Ok(layout) = CheckedDims::new(image.width(), image.height(), 2) else {
        return false;
    };
    let pixels = layout.total_pixels();
    pixels > 0
        && pixels <= u32::MAX as usize
        && la.as_raw().len() == layout.total_bytes()
        && gpu_pad_geometry(&ops[0], image.width(), image.height())
            .is_some_and(|(resize_dimensions, _)| resize_dimensions == image.dimensions())
}

#[cfg(not(target_endian = "little"))]
pub(super) fn gpu_packed_la_pad_input(
    _ops: &[PipelineOp],
    _image: &DynamicImage,
    _logical_mode: Option<&str>,
) -> bool {
    false
}

pub(super) fn gpu_packed_la_pad_dispatch_supported(
    op: &PipelineOp,
    source_dimensions: (u32, u32),
    max_workgroups_per_dimension: u32,
) -> bool {
    let PipelineOp::Pad { w, h, .. } = op else {
        return false;
    };
    let Some(source_pixels) =
        u64::from(source_dimensions.0).checked_mul(u64::from(source_dimensions.1))
    else {
        return false;
    };
    source_pixels > 0
        && source_pixels <= u64::from(u32::MAX)
        && plan_packed_la_dispatch(*w, *h, max_workgroups_per_dimension).is_ok()
}

pub(super) fn gpu_packed_luma_rank_filter_9_input(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    matches!(ops, [PipelineOp::RankFilter { size: 9, .. }])
        && gpu_packed_luma_order_statistic_input(ops, image, logical_mode)
}

pub(super) fn gpu_packed_luma_rank_filter_9_work_items(output_dimensions: (u32, u32)) -> u64 {
    // Eight binary-search passes inspect each of the 81 window samples once
    // per output lane. Count all four lanes in the final packed word too, even
    // if the image has a partial word, so the watchdog estimate stays conservative.
    let pixels = u64::from(output_dimensions.0).saturating_mul(u64::from(output_dimensions.1));
    pixels.div_ceil(4).saturating_mul(4).saturating_mul(8 * 81)
}

pub(super) fn gpu_packed_la_rank_filter_3_work_items(
    output_dimensions: (u32, u32),
    rank: u32,
) -> u64 {
    // Rank 0/8 reduce directly to a per-channel min/max; rank 1 maintains the
    // two smallest channel values; other ranks use the exact 9-element sorting
    // network. Count scalar lane operations conservatively per output pixel.
    let per_pixel = match rank {
        0 | 8 => 18,
        1 => 46,
        _ => 162,
    };
    u64::from(output_dimensions.0)
        .saturating_mul(u64::from(output_dimensions.1))
        .saturating_mul(per_pixel)
}

#[cfg(not(target_endian = "little"))]
pub(super) fn gpu_packed_luma_order_statistic_input(
    _ops: &[PipelineOp],
    _image: &DynamicImage,
    _logical_mode: Option<&str>,
) -> bool {
    false
}

#[cfg(not(target_endian = "little"))]
pub(super) fn gpu_packed_native_byte_filter_input(
    _ops: &[PipelineOp],
    _image: &DynamicImage,
    _logical_mode: Option<&str>,
) -> bool {
    false
}

#[cfg(not(target_endian = "little"))]
pub(super) fn gpu_packed_luma_convert_input(
    _ops: &[PipelineOp],
    _image: &DynamicImage,
    _logical_mode: Option<&str>,
) -> bool {
    false
}

/// Admit singleton consumers of the compact native RGB upload. Convert widens
/// to RGBA in its shader; Transform samples compact triples; RGB AutoContrast,
/// Equalize, and 3x3 order filters use packed output shaders when their checked
/// output plan permits native transfers.
#[cfg(target_endian = "little")]
pub(super) fn gpu_native_rgb_compact_input(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
    host_autocontrast_rgb_lut: bool,
) -> bool {
    let supported_consumer = match ops {
        [
            PipelineOp::Convert {
                mode: ColorMode::RGBA,
                matrix: None,
                dither: None,
            },
        ] => true,
        [PipelineOp::Transform { .. }] => true,
        [PipelineOp::MedianFilter { size: 3 }] => true,
        [PipelineOp::MinFilter { size: 3 }] => true,
        [PipelineOp::MaxFilter { size: 3 }] => true,
        [PipelineOp::BoxBlur { radius: 1 }] => true,
        [PipelineOp::Eval { .. }] => host_autocontrast_rgb_lut,
        [PipelineOp::Equalize] => true,
        _ => false,
    };
    if !supported_consumer
        || !matches!(logical_mode, None | Some("RGB"))
        || image.width() == 0
        || image.height() == 0
    {
        return false;
    }
    let DynamicImage::ImageRgb8(rgb) = image else {
        return false;
    };
    let Ok(layout) = CheckedDims::new(image.width(), image.height(), 3) else {
        return false;
    };
    let input_bytes = layout.total_bytes();
    input_bytes > 0 && input_bytes <= u32::MAX as usize && rgb.as_raw().len() == input_bytes
}

#[cfg(not(target_endian = "little"))]
pub(super) fn gpu_native_rgb_compact_input(
    _ops: &[PipelineOp],
    _image: &DynamicImage,
    _logical_mode: Option<&str>,
    _host_autocontrast_rgb_lut: bool,
) -> bool {
    false
}

/// Resolve a queued LA PutPixel immediately before Sharpness to the byte
/// patch that must be applied to native input before the GPU dispatch.
#[cfg(target_endian = "little")]
pub(super) fn gpu_native_sharpness_la_putpixel(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> Option<(usize, [u8; 2])> {
    if !matches!(logical_mode, None | Some("LA")) {
        return None;
    }
    let [
        PipelineOp::PutPixel {
            x,
            y,
            color,
            palette_index: false,
        },
        PipelineOp::Sharpness { .. },
    ] = ops
    else {
        return None;
    };
    let DynamicImage::ImageLumaA8(la) = image else {
        return None;
    };
    if *x >= image.width() || *y >= image.height() {
        return None;
    }
    let layout = CheckedDims::new(image.width(), image.height(), 2).ok()?;
    let input_bytes = layout.total_bytes();
    if input_bytes == 0 || input_bytes > u32::MAX as usize || la.as_raw().len() != input_bytes {
        return None;
    }
    let pixel_index = u64::from(*y)
        .checked_mul(u64::from(image.width()))?
        .checked_add(u64::from(*x))?;
    let byte_offset = pixel_index.checked_mul(2)?;
    Some((usize::try_from(byte_offset).ok()?, [color.0, color.3]))
}

/// Admit tightly packed L bytes for singleton Sharpness. The shader consumes
/// and emits four samples per storage word so transfers and working data stay
/// compact throughout the GPU operation.
#[cfg(target_endian = "little")]
pub(super) fn gpu_native_sharpness_l_input(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if !matches!(ops, [PipelineOp::Sharpness { .. }]) || !matches!(logical_mode, None | Some("L")) {
        return false;
    }
    let DynamicImage::ImageLuma8(luma) = image else {
        return false;
    };
    let Ok(layout) = CheckedDims::new(image.width(), image.height(), 1) else {
        return false;
    };
    let input_bytes = layout.total_bytes();
    input_bytes > 0 && input_bytes <= u32::MAX as usize && luma.as_raw().len() == input_bytes
}

/// Admit tightly packed LA bytes for singleton Sharpness, optionally folding
/// one queued LA PutPixel into the upload. The shader reads native two-byte
/// pixels and emits the existing packed RGBA transport, which the readback
/// path narrows directly to LA without constructing an RGBA image.
#[cfg(target_endian = "little")]
pub(super) fn gpu_native_sharpness_la_input(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    let standalone = matches!(ops, [PipelineOp::Sharpness { .. }]);
    if !matches!(logical_mode, None | Some("LA"))
        || (!standalone && gpu_native_sharpness_la_putpixel(ops, image, logical_mode).is_none())
    {
        return false;
    }
    let DynamicImage::ImageLumaA8(la) = image else {
        return false;
    };
    let Ok(layout) = CheckedDims::new(image.width(), image.height(), 2) else {
        return false;
    };
    let input_bytes = layout.total_bytes();
    input_bytes > 0 && input_bytes <= u32::MAX as usize && la.as_raw().len() == input_bytes
}

/// Admit tightly packed RGB bytes for a singleton Sharpness dispatch. The
/// shader gathers each three-byte pixel from the storage words and synthesizes
/// the opaque fourth transport byte in registers.
#[cfg(target_endian = "little")]
pub(super) fn gpu_native_sharpness_rgb_input(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if !matches!(ops, [PipelineOp::Sharpness { .. }]) || !matches!(logical_mode, None | Some("RGB"))
    {
        return false;
    }
    let DynamicImage::ImageRgb8(rgb) = image else {
        return false;
    };
    let Ok(layout) = CheckedDims::new(image.width(), image.height(), 3) else {
        return false;
    };
    let input_bytes = layout.total_bytes();
    input_bytes > 0 && input_bytes <= u32::MAX as usize && rgb.as_raw().len() == input_bytes
}

/// Admit tightly packed RGB bytes for a singleton Reduce dispatch. The
/// shader gathers each three-byte sample from storage words and supplies the
/// opaque alpha byte that the existing RGB averaging path expects.
#[cfg(target_endian = "little")]
pub(super) fn gpu_native_reduce_rgb_input(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if !matches!(ops, [PipelineOp::Reduce { .. }]) || !matches!(logical_mode, None | Some("RGB")) {
        return false;
    }
    let DynamicImage::ImageRgb8(rgb) = image else {
        return false;
    };
    let Ok(layout) = CheckedDims::new(image.width(), image.height(), 3) else {
        return false;
    };
    let input_bytes = layout.total_bytes();
    input_bytes > 0 && input_bytes <= u32::MAX as usize && rgb.as_raw().len() == input_bytes
}

/// Return the packed native-byte channel count for a singleton Expand batch.
/// When the adapter's bounded output-word grid fits, the shader also writes
/// the result in this compact native layout; otherwise the generic RGBA output
/// path remains available.
#[cfg(target_endian = "little")]
pub(super) fn gpu_native_expand_channels(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> Option<u8> {
    let [PipelineOp::Expand { .. }] = ops else {
        return None;
    };
    let channels = match (logical_mode, image) {
        (None | Some("L"), DynamicImage::ImageLuma8(_)) => 1u8,
        (None | Some("LA"), DynamicImage::ImageLumaA8(_)) => 2,
        (None | Some("RGB" | "HSV"), DynamicImage::ImageRgb8(_)) => 3,
        // CMYK's carrier also has four bytes, but keeps C/M/Y/K byte order;
        // singleton Expand copies those samples without color interpretation.
        (None | Some("RGBA" | "CMYK"), DynamicImage::ImageRgba8(_)) => 4,
        _ => return None,
    };
    let input_bytes = u64::from(image.width())
        .checked_mul(u64::from(image.height()))?
        .checked_mul(u64::from(channels))?;
    (input_bytes > 0 && input_bytes <= u64::from(u32::MAX)).then_some(channels)
}

/// Return the native input stride for a singleton Cover whose byte layout
/// would otherwise be widened to the generic RGBA transport. The resize
/// shader still emits its established packed output; this admission only
/// changes how the source samples are read.
#[cfg(target_endian = "little")]
pub(super) fn gpu_native_cover_input_channels(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> Option<u8> {
    let [PipelineOp::Cover { .. }] = ops else {
        return None;
    };
    let channels = match (logical_mode, image) {
        (None | Some("L"), DynamicImage::ImageLuma8(_)) => 1u8,
        (None | Some("LA"), DynamicImage::ImageLumaA8(_)) => 2,
        (None | Some("RGB" | "HSV"), DynamicImage::ImageRgb8(_)) => 3,
        _ => return None,
    };
    let input_bytes = u64::from(image.width())
        .checked_mul(u64::from(image.height()))?
        .checked_mul(u64::from(channels))?;
    (input_bytes > 0
        && input_bytes <= u64::from(u32::MAX)
        && usize::try_from(input_bytes).ok()? == image.as_bytes().len())
    .then_some(channels)
}

pub(super) fn gpu_native_cover_packed_output_dispatch(
    ops: &[PipelineOp],
    native_cover_input_channels: Option<u8>,
    logical_mode: Option<&str>,
    source_height: u32,
    max_workgroups_per_dimension: u32,
    max_storage_buffer_binding_size: u32,
    max_buffer_size: u64,
    buffer_capacity_pixels: u32,
) -> Option<NativeCoverPackedOutputDispatch> {
    let [PipelineOp::Resize { w, h, filter }] = ops else {
        return None;
    };
    let Some(channels @ (1 | 2)) = native_cover_input_channels else {
        return None;
    };
    if !matches!(
        (channels, logical_mode),
        (1, None | Some("L")) | (2, None | Some("LA"))
    ) || (matches!(filter, ResampleFilter::Nearest)
        && !gpu_resize_nearest_uses_coefficients(logical_mode))
    {
        return None;
    }
    let buffer_capacity_bytes = u64::from(buffer_capacity_pixels).checked_mul(4)?;
    plan_native_cover_packed_output_dispatch(
        *w,
        *h,
        source_height,
        channels,
        max_workgroups_per_dimension,
        max_storage_buffer_binding_size,
        max_buffer_size,
        buffer_capacity_bytes,
    )
}

#[cfg(not(target_endian = "little"))]
pub(super) fn gpu_native_cover_input_channels(
    _ops: &[PipelineOp],
    _image: &DynamicImage,
    _logical_mode: Option<&str>,
) -> Option<u8> {
    None
}

#[cfg(not(target_endian = "little"))]
pub(super) fn gpu_native_expand_channels(
    _ops: &[PipelineOp],
    _image: &DynamicImage,
    _logical_mode: Option<&str>,
) -> Option<u8> {
    None
}

#[cfg(not(target_endian = "little"))]
pub(super) fn gpu_native_extract_band_channels(
    _ops: &[PipelineOp],
    _image: &DynamicImage,
    _logical_mode: Option<&str>,
) -> Option<u8> {
    None
}

#[cfg(not(target_endian = "little"))]
pub(super) fn gpu_native_grayscale_rgb_input(
    _ops: &[PipelineOp],
    _image: &DynamicImage,
    _logical_mode: Option<&str>,
) -> bool {
    false
}

#[cfg(not(target_endian = "little"))]
pub(super) fn gpu_native_grayscale_cmyk_input(
    _ops: &[PipelineOp],
    _image: &DynamicImage,
    _logical_mode: Option<&str>,
) -> bool {
    false
}

#[cfg(not(target_endian = "little"))]
pub(super) fn gpu_native_sharpness_la_putpixel(
    _ops: &[PipelineOp],
    _image: &DynamicImage,
    _logical_mode: Option<&str>,
) -> Option<(usize, [u8; 2])> {
    None
}

#[cfg(not(target_endian = "little"))]
pub(super) fn gpu_native_sharpness_l_input(
    _ops: &[PipelineOp],
    _image: &DynamicImage,
    _logical_mode: Option<&str>,
) -> bool {
    false
}

#[cfg(not(target_endian = "little"))]
pub(super) fn gpu_native_sharpness_la_input(
    _ops: &[PipelineOp],
    _image: &DynamicImage,
    _logical_mode: Option<&str>,
) -> bool {
    false
}

#[cfg(not(target_endian = "little"))]
pub(super) fn gpu_native_sharpness_rgb_input(
    _ops: &[PipelineOp],
    _image: &DynamicImage,
    _logical_mode: Option<&str>,
) -> bool {
    false
}

#[cfg(not(target_endian = "little"))]
pub(super) fn gpu_native_reduce_rgb_input(
    _ops: &[PipelineOp],
    _image: &DynamicImage,
    _logical_mode: Option<&str>,
) -> bool {
    false
}
