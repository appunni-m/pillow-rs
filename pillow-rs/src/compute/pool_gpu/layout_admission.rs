use super::*;

/// The ordinary GPU transport is packed RGBA8. Do not let
/// `DynamicImage::to_rgba8` silently narrow a 16-bit or floating-point source
/// before a shader runs: those formats have different Pillow sample semantics
/// and must stay on the CPU path until a native GPU representation exists.
pub(super) fn gpu_image_layout_is_supported(image: &DynamicImage) -> bool {
    matches!(
        image,
        DynamicImage::ImageLuma8(_)
            | DynamicImage::ImageLumaA8(_)
            | DynamicImage::ImageRgb8(_)
            | DynamicImage::ImageRgba8(_)
    )
}

/// `I;16*` geometry can use the packed storage buffers without narrowing a
/// sample to the ordinary byte layout. Relocation and nearest paths copy the
/// packed word; a single filtered resize is admitted only when its typed
/// f64-coefficient proof covers both device passes. Other arithmetic, fills,
/// and mixed-input operations remain on exact host semantic control until
/// their native typed-sample contracts are proven.
pub(super) fn gpu_luma16_geometry_is_supported(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if !matches!(image, DynamicImage::ImageI16(_)) || ops.is_empty() {
        return false;
    }
    // A filtered resize is admitted only as a pure, single operation whose
    // f64 coefficient products and both native u16 pass boundaries were
    // proven by the host reducer.  Keep nearest/relocation geometry on the
    // established opaque-word path, but do not mix it with the new arithmetic
    // marker until a chained typed contract exists.
    if ops.iter().any(|op| {
        matches!(
            op,
            PipelineOp::Resize {
                filter,
                ..
            } if !matches!(filter, crate::pipeline::ResampleFilter::Nearest)
        )
    }) {
        return gpu_luma16_resize_f64_is_exact(ops, image, logical_mode);
    }
    ops.iter().all(|op| {
        matches!(
            op,
            PipelineOp::Offset { .. }
                | PipelineOp::Flip
                | PipelineOp::Mirror
                | PipelineOp::Transpose { .. }
                | PipelineOp::Crop { .. }
                | PipelineOp::CropBorder { .. }
                | PipelineOp::Resize {
                    filter: crate::pipeline::ResampleFilter::Nearest,
                    ..
                }
                | PipelineOp::Contain {
                    filter: crate::pipeline::ResampleFilter::Nearest,
                    ..
                }
                | PipelineOp::Cover {
                    filter: crate::pipeline::ResampleFilter::Nearest,
                    ..
                }
                | PipelineOp::Transform { .. }
                | PipelineOp::Duplicate
        )
    })
}

/// `I;16*` conversion uses the same one-word-per-sample transport as native
/// geometry, but the words are uploaded in numeric little-endian form so the
/// Convert shader can clamp the unsigned sample before producing byte output.
/// Keep the conversion terminal: once it emits L/LA/RGB/RGBA bytes, the batch
/// mode changes and the typed source contract no longer applies.
pub(super) fn gpu_luma16_convert_is_supported(ops: &[PipelineOp], image: &DynamicImage) -> bool {
    if ops.is_empty() {
        return false;
    }
    if matches!(image, DynamicImage::ImageLuma8(_)) {
        return matches!(
            ops,
            [PipelineOp::Convert {
                mode: ColorMode::I16,
                matrix: None,
                dither: None,
            }]
        );
    }
    if !matches!(image, DynamicImage::ImageI16(_)) {
        return false;
    }
    let direct = matches!(ops, [PipelineOp::Convert { mode, matrix: None, dither: None }]
        if gpu_standard_color_mode_code(mode).is_some()
            || matches!(mode, ColorMode::I | ColorMode::F));
    // I;16N historically converts through clipped L before producing I/F.
    // Keep that two-step contract on-device instead of materializing the
    // intermediate on the host.
    let clipped_scalar = matches!(
        ops,
        [
            PipelineOp::Convert {
                mode: ColorMode::L,
                matrix: None,
                dither: None,
            },
            PipelineOp::Convert {
                mode: ColorMode::I | ColorMode::F,
                matrix: None,
                dither: None,
            }
        ]
    );
    direct || clipped_scalar
}

/// The GPU Paste shader has a separate numeric-u16 branch for one typed
/// `I;16*` destination/source pair. Keep the first contract deliberately
/// narrow: one Paste operation, with no preceding byte-oriented operation
/// changing the representation. Mask validation is handled after auxiliary
/// images are materialized, where the concrete L/RGBA mask layout is known.
pub(super) fn gpu_luma16_paste_is_supported(ops: &[PipelineOp], image: &DynamicImage) -> bool {
    matches!(image, DynamicImage::ImageI16(_)) && matches!(ops, [PipelineOp::Paste { .. }])
}

pub(super) fn gpu_luma16_paste_source(op: &PipelineOp, mode: u32, image: &DynamicImage) -> bool {
    mode == 5
        && matches!(op, PipelineOp::Paste { .. })
        && matches!(image, DynamicImage::ImageI16(_))
}
