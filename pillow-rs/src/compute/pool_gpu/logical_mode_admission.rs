use super::*;

/// Shared logical-mode admission for queued submission and device execution.
/// Inspecting an already-loaded source never executes an image operation.
pub(super) fn gpu_logical_mode_is_supported(
    ops: &[PipelineOp],
    img: Option<&DynamicImage>,
    mode: Option<&str>,
    f_resize_constant_bits: Option<u32>,
    f_pad_f64_is_exact: bool,
    inspect_auxiliary_modes: bool,
) -> bool {
    mode.is_none_or(|logical_mode| {
        matches!(logical_mode, "L" | "LA" | "RGB" | "RGBA")
                || (logical_mode == "1"
                    && img.is_none_or(|img| matches!(img, DynamicImage::ImageLuma8(_)))
                    && matches!(ops, [PipelineOp::Flip]))
                // YCbCr shares RGB8 transport but grayscale reads only its Y
                // sample; admit exactly the singleton kernel that carries
                // this native-mode interpretation in its fourth uniform.
                || (logical_mode == "YCbCr"
                    && matches!(ops, [PipelineOp::Grayscale])
                    && img.is_none_or(|img| gpu_native_grayscale_rgb_input(ops, img, mode)))
                // On little-endian targets, HSV uses the same packed
                // three-byte storage as RGB. A singleton transpose only
                // relocates complete triplets; it does not interpret or
                // convert H/S/V samples. Keep other targets on the proven
                // semantic-control path until their native upload is covered.
                || (cfg!(target_endian = "little")
                    && logical_mode == "HSV"
                    && matches!(ops, [PipelineOp::Transpose { .. }])
                    && img.is_none_or(|img| matches!(img, DynamicImage::ImageRgb8(_))))
                // LAB shares RGB8 storage. The core has already encoded its
                // public signed A/B samples into the stored +128 byte domain,
                // so the raw three-channel PutPixel shader is exact here.
                || (logical_mode == "LAB"
                    && ops.iter().all(|op| {
                        matches!(
                            op,
                            PipelineOp::PutPixel {
                                palette_index: false,
                                ..
                            }
                        )
                    }))
                || (cfg!(target_endian = "little")
                    && matches!(ops, [PipelineOp::Multiply { .. }])
                    && matches!(logical_mode, "1" | "L" | "P" | "LA" | "La" | "PA" | "RGB" | "HSV" | "YCbCr" | "RGBA" | "RGBa" | "RGBX" | "CMYK")
                    && img.is_none_or(|img| gpu_native_multiply_channels(img, mode).is_some()))
                || (logical_mode == "La" && ops.iter().all(gpu_transform_is_projective))
                // ImageDraw's geometry is scan-converted by the exact host
                // canvas before the packed draw shader copies the complete
                // result.  The data-plane therefore preserves raw indexed,
                // typed-word, and native color bytes just like the ordinary
                // byte modes; no shader arithmetic interprets those samples.
                || (matches!(
                    logical_mode,
                    "1" | "P" | "PA" | "RGBX" | "RGBa" | "CMYK" | "HSV" | "YCbCr" | "I" | "F"
                ) && !ops.is_empty()
                    && ops
                        .iter()
                        .all(crate::compute::pool_cpu::ops::draw::is_draw_op))
                || (matches!(logical_mode, "P" | "PA")
                    && ops.iter().all(|op| {
                        matches!(
                            op,
                                PipelineOp::Add { .. }
                                | PipelineOp::Subtract { .. }
                                | PipelineOp::Multiply { .. }
                                | PipelineOp::Screen { .. }
                                | PipelineOp::Darker { .. }
                                | PipelineOp::Lighter { .. }
                                | PipelineOp::Difference { .. }
                                | PipelineOp::Overlay { .. }
                                | PipelineOp::HardLight { .. }
                                | PipelineOp::SoftLight { .. }
                                | PipelineOp::AddModulo { .. }
                                | PipelineOp::SubtractModulo { .. }
                                | PipelineOp::PutAlpha { .. }
                                | PipelineOp::PutAlphaData { .. }
                                | PipelineOp::PutData { .. }
                                | PipelineOp::ExtractBand { .. }
                                | PipelineOp::Eval { .. }
                                | PipelineOp::PutPixel { .. }
                                | PipelineOp::EffectSpread { .. }
                                | PipelineOp::CompositeModule { .. }
                                | PipelineOp::Filter3x3 { .. }
                                | PipelineOp::Filter5x5 { .. }
                                | PipelineOp::RemapPalette { .. }
                                | PipelineOp::InvertChops
                                | PipelineOp::Offset { .. }
                                | PipelineOp::Mirror
                                | PipelineOp::Transpose { .. }
                                | PipelineOp::Crop { .. }
                                | PipelineOp::CropBorder { .. }
                                | PipelineOp::Expand { .. }
                                | PipelineOp::Duplicate
                                | PipelineOp::Flip
                                | PipelineOp::Reduce { .. }
                                | PipelineOp::Paste { .. }
                                | PipelineOp::Scale { .. }
                                | PipelineOp::Contain { .. }
                                | PipelineOp::Cover { .. }
                                | PipelineOp::Pad { .. }
                                | PipelineOp::Fit {
                                    filter: ResampleFilter::Nearest,
                                    ..
                                }
                                | PipelineOp::Transform { .. }
                                | PipelineOp::Resize {
                                    ..
                                }
                                | PipelineOp::Equalize | PipelineOp::EqualizeMasked { .. }
                        )
                    }))
                || (matches!(ops, [PipelineOp::Merge { mode: ColorMode::RGB, .. }])
                    && logical_mode == "P"
                    && (!inspect_auxiliary_modes || gpu_palette_first_rgb_merge_is_supported(ops, mode)))
                || (logical_mode == "1"
                    && ops
                        .iter()
                        .all(|op| {
                            matches!(
                                op,
                                PipelineOp::PutData { .. }
                                    | PipelineOp::PutPixel { .. }
                                    | PipelineOp::EffectSpread { .. }
                                    | PipelineOp::CompositeModule { .. }
                                    | PipelineOp::Filter3x3 { .. }
                                | PipelineOp::Filter5x5 { .. }
                                    | PipelineOp::Eval { .. }
                                    | PipelineOp::LogicalAnd { .. }
                                    | PipelineOp::LogicalOr { .. }
                                    | PipelineOp::LogicalXor { .. }
                                    | PipelineOp::Offset { .. }
                                    | PipelineOp::Transpose { .. }
                                    | PipelineOp::Paste { .. }
                                    | PipelineOp::Duplicate
                                    | PipelineOp::Contain {
                                        filter: crate::pipeline::ResampleFilter::Nearest,
                                        ..
                                    }
                                    | PipelineOp::Cover {
                                        filter: crate::pipeline::ResampleFilter::Nearest,
                                        ..
                                    }
                                    | PipelineOp::Pad {
                                        filter: crate::pipeline::ResampleFilter::Nearest,
                                        ..
                                    }
                                    | PipelineOp::Transform { .. }
                                    | PipelineOp::Resize {
                                        filter: crate::pipeline::ResampleFilter::Nearest,
                                        ..
                                    }
                            )
                        }))
                // RGBX shares RGBA's four-byte storage, but its fourth byte
                // is padding rather than alpha. Transpose is a pure native
                // byte relocation, so the RGBA transport can preserve all
                // four bytes without applying alpha semantics. PutPixel is
                // likewise a raw four-byte write in this logical mode.
                // Sharpness has a dedicated mode-6 shader branch that filters
                // all four stored bytes, matching Pillow's RGBX behavior.
                || (logical_mode == "RGBX"
                    && ops.iter().all(|op| {
                        matches!(
                            op,
                            PipelineOp::Sharpness { .. }
                                | PipelineOp::PutPixel { .. }
                                | PipelineOp::EffectSpread { .. }
                                | PipelineOp::Transpose { .. }
                                | PipelineOp::Paste { mask: None, .. }
                                | PipelineOp::Scale { .. }
                                | PipelineOp::Contain { .. }
                                | PipelineOp::Cover { .. }
                                | PipelineOp::Pad { .. }
                                | PipelineOp::Transform { .. }
                                | PipelineOp::Resize {
                                    ..
                                }
                                | PipelineOp::Convert {
                                    mode: ColorMode::L
                                        | ColorMode::LA
                                        | ColorMode::RGB
                                        | ColorMode::RGBA
                                        | ColorMode::CMYK
                                        | ColorMode::YCbCr
                                        | ColorMode::HSV
                                        | ColorMode::I
                                        | ColorMode::F,
                                    matrix: None,
                                    dither: None,
                                }
                        )
                    }))
                // RGBa uses the same four-byte storage as RGBA. Pillow's
                // Image.resize leaves RGBa in its already-premultiplied
                // representation (PIL/Image.py resize mode dispatch), so
                // boxed Fit can use the same coefficient kernels with
                // `premultiply = 0`; ImageChops likewise operates on these
                // stored bytes directly. Keep this whitelist limited to
                // raw-channel operations and geometry that preserves that
                // four-byte sample contract.
                || (logical_mode == "RGBa"
                    && ops.iter().all(|op| {
                        matches!(
                            op,
                            PipelineOp::PutPixel { .. }
                                | PipelineOp::EffectSpread { .. }
                                | PipelineOp::Add { .. }
                                | PipelineOp::Subtract { .. }
                                | PipelineOp::Multiply { .. }
                                | PipelineOp::Screen { .. }
                                | PipelineOp::Darker { .. }
                                | PipelineOp::Lighter { .. }
                                | PipelineOp::Difference { .. }
                                | PipelineOp::Overlay { .. }
                                | PipelineOp::HardLight { .. }
                                | PipelineOp::SoftLight { .. }
                                | PipelineOp::AddModulo { .. }
                                | PipelineOp::SubtractModulo { .. }
                                | PipelineOp::InvertChops
                                | PipelineOp::Paste { mask: None, .. }
                                | PipelineOp::Scale { .. }
                                | PipelineOp::Contain { .. }
                                | PipelineOp::Cover { .. }
                                | PipelineOp::Pad { .. }
                                | PipelineOp::Transform { .. }
                                | PipelineOp::Fit { .. }
                                | PipelineOp::Resize {
                                    ..
                                }
                        )
                    }))
                || (matches!(logical_mode, "HSV" | "YCbCr")
                    && ops.iter().all(|op| {
                        matches!(
                            op,
                            PipelineOp::Add { .. }
                                | PipelineOp::Subtract { .. }
                                | PipelineOp::Multiply { .. }
                                | PipelineOp::Screen { .. }
                                | PipelineOp::Darker { .. }
                                | PipelineOp::Lighter { .. }
                                | PipelineOp::Difference { .. }
                                | PipelineOp::Overlay { .. }
                                | PipelineOp::HardLight { .. }
                                | PipelineOp::SoftLight { .. }
                                | PipelineOp::AddModulo { .. }
                                | PipelineOp::SubtractModulo { .. }
                                | PipelineOp::Brightness { .. }
                                | PipelineOp::Filter3x3 { .. }
                                | PipelineOp::Filter5x5 { .. }
                                    | PipelineOp::Reduce { .. }
                                    | PipelineOp::PutData { .. }
                                    | PipelineOp::Eval { .. }
                                | PipelineOp::PutPixel { .. }
                                | PipelineOp::EffectSpread { .. }
                                | PipelineOp::Paste { .. }
                                | PipelineOp::Crop { .. }
                                | PipelineOp::Scale { .. }
                                | PipelineOp::Contain { .. }
                                | PipelineOp::Cover { .. }
                                | PipelineOp::Pad { .. }
                                | PipelineOp::Transform { .. }
                                | PipelineOp::Resize {
                                    ..
                                }
                        )
                    }))
                // Singleton HSV Expand reads the image's three stored
                // samples as-is. Keep this tied to the native-layout gate so
                // the shader cannot admit a logical HSV image with a
                // mismatched physical representation.
                || (logical_mode == "HSV"
                    && matches!(ops, [PipelineOp::Expand { .. }])
                    && img.is_none_or(|img| gpu_native_expand_channels(ops, img, mode).is_some()))
                // CMYK, HSV, and YCbCr retain their native channel order in
                // the packed RGBA/RGB transport.  ExtractBand only copies
                // one requested byte and then publishes an L8 result, so it
                // does not reinterpret the samples as RGB or alpha.  Keep
                // PutPixel here as well: the maintained getchannel batches
                // often write one source pixel before extracting its band.
                || (matches!(logical_mode, "CMYK" | "HSV" | "YCbCr")
                    && ops.iter().all(|op| {
                        matches!(
                            op,
                            PipelineOp::ExtractBand { .. }
                                | PipelineOp::PutPixel { .. }
                                | PipelineOp::EffectSpread { .. }
                        )
                    }))
                // CMYK putalpha is a terminal promotion through the exact
                // integer CMYK->RGB conversion in put_alpha.wgsl.  Keep this
                // whitelist terminal-only: after promotion the public mode
                // is RGBA, and a following operation needs a segmented batch
                // with the updated logical layout rather than CMYK metadata.
                || (logical_mode == "CMYK"
                    && ops.len() == 1
                    && matches!(
                        ops[0],
                        PipelineOp::PutAlpha {
                            mode: PixelMode::CMYK,
                            ..
                        } | PipelineOp::PutAlphaData {
                            mode: PixelMode::CMYK,
                            ..
                        }
                    ))
                // Keep CMYK→RGB as one terminal native conversion. The
                // convert shader interprets the four bytes as C/M/Y/K and
                // applies Pillow's exact integer inverse before RGB readback.
                || (logical_mode == "CMYK"
                    && matches!(ops, [PipelineOp::Convert {
                        mode: ColorMode::RGB,
                        matrix: None,
                        dither: None,
                    }])
                    && img.is_none_or(|img| matches!(img, DynamicImage::ImageRgba8(_))))
                || (matches!(logical_mode, "RGB" | "RGBA" | "CMYK")
                    && ops
                        .iter()
                        .all(|op| matches!(op, PipelineOp::Color3DLut { .. })))
                // F stores one finite f32 sample in each four-byte word.
                // Its order-statistic shaders compare the decoded samples;
                // the ordinary byte filters would sort IEEE-754 bytes and
                // produce a numerically unrelated result. Mirror remains in
                // this clause because it only relocates complete words.
                || (logical_mode == "F" && img.is_none_or(|img| gpu_float_filter_is_supported(ops, img)))
                // A constant F Pad uses the exact scalar resize marker for
                // its contain step and a raw-word placement shader for the
                // final canvas. The source proof is deliberately limited to
                // a single non-nearest Pad; mixed batches still need the
                // host semantic path until their intermediate contract is
                // proven.
                || (logical_mode == "F"
                    && f_resize_constant_bits.is_some()
                    && ops.len() == 1
                    && matches!(ops[0], PipelineOp::Pad { .. }))
                // A heterogeneous F Pad uses marker 9 for its contain
                // resize and then copies complete words through placement.
                // The admission proof is limited to one changed-axis Pad
                // with an optional PutData(F)-only prefix; nearest, same-size,
                // and unrelated prefixes retain their existing paths.
                || (logical_mode == "F"
                    && f_pad_f64_is_exact)
                // A nearest F Fit is a two-axis one-tap word relocation. Its
                // boxed coefficients are generated on the host after the
                // same f32 crop-boundary conversion as Pillow's affine
                // nearest path; keep it separate from the vertical pass so
                // Metal cannot observe a stale horizontal intermediate.
                || (logical_mode == "F"
                    && ops.len() == 1
                    && matches!(
                        ops[0],
                        PipelineOp::Fit {
                            filter: ResampleFilter::Nearest,
                            ..
                        }
                    ))
                // I-mode nearest Pad carries signed int32 words through a
                // nearest contain resize and a raw-word placement pass. A
                // filtered Pad would need the typed INT32 accumulator and is
                // intentionally kept on exact host semantic control.
                || (logical_mode == "I"
                    && ops.iter().all(|op| {
                        matches!(
                            op,
                            PipelineOp::Pad {
                                filter: ResampleFilter::Nearest,
                                ..
                            }
                                | PipelineOp::Resize {
                                    filter: ResampleFilter::Nearest,
                                    ..
                                }
                        )
                    }))
                || (logical_mode == "I"
                    && img.is_none_or(|img| gpu_int_filter_is_supported(ops, img)
                        || gpu_int_filter_resize_chain_is_supported(ops, img)))
                // I/F samples are four raw bytes per pixel at this executor
                // boundary.  These operations only relocate or duplicate
                // the complete sample and therefore do not need to decode it
                // as an integer, float, or color.  Keep arithmetic and fill
                // operations out of this clause: their shader contracts need
                // a native typed buffer rather than packed RGBA semantics.
                || (matches!(logical_mode, "I" | "F")
                    && ops.iter().all(|op| {
                        matches!(
                            op,
                            PipelineOp::PutData { .. }
                                | PipelineOp::Offset { .. }
                                | PipelineOp::EffectSpread { .. }
                                | PipelineOp::Flip
                                | PipelineOp::Mirror
                                | PipelineOp::Transpose { .. }
                                | PipelineOp::Crop { .. }
                                | PipelineOp::CropBorder { .. }
                                | PipelineOp::Duplicate
                                | PipelineOp::Paste { mask: None, .. }
                                | PipelineOp::Scale {
                                    ..
                                }
                                | PipelineOp::Contain {
                                    ..
                                }
                                | PipelineOp::Cover {
                                    ..
                                }
                                | PipelineOp::Resize {
                                    ..
                                }
                                | PipelineOp::Transform { .. }
                        )
                    }))
                // Native I;16* images use one typed u16 sample per pixel.
                // The packed GPU word is only an opaque transport for these
                // relocation operations; it is never narrowed to an 8-bit
                // luma value or interpreted as RGBA.
                || (matches!(logical_mode, "I;16" | "I;16L" | "I;16B" | "I;16N")
                    && img.is_none_or(|img| gpu_luma16_geometry_is_supported(ops, img, Some(logical_mode))
                        || gpu_luma16_convert_is_supported(ops, img)
                        || gpu_luma16_paste_is_supported(ops, img)))
                || (logical_mode == "CMYK"
                    && ops
                        .iter()
                        .all(|op| {
                            matches!(
                                op,
                                PipelineOp::Grayscale
                                    | PipelineOp::Brightness { .. }
                                    | PipelineOp::Contrast { .. }
                                    | PipelineOp::ColorSaturation { .. }
                                    | PipelineOp::Sharpness { .. }
                                    | PipelineOp::InvertChops
                                    | PipelineOp::Add { .. }
                                    | PipelineOp::Subtract { .. }
                                    | PipelineOp::Multiply { .. }
                                    | PipelineOp::Screen { .. }
                                    | PipelineOp::Darker { .. }
                                    | PipelineOp::Lighter { .. }
                                    | PipelineOp::Difference { .. }
                                    | PipelineOp::Overlay { .. }
                                    | PipelineOp::HardLight { .. }
                                    | PipelineOp::SoftLight { .. }
                                    | PipelineOp::AddModulo { .. }
                                    | PipelineOp::SubtractModulo { .. }
                                    | PipelineOp::LogicalAnd { .. }
                                    | PipelineOp::LogicalOr { .. }
                                    | PipelineOp::LogicalXor { .. }
                                    | PipelineOp::Filter3x3 { .. }
                                    | PipelineOp::Filter5x5 { .. }
                                    | PipelineOp::BlendModule { .. }
                                    | PipelineOp::Offset { .. }
                                    | PipelineOp::Mirror
                                    | PipelineOp::Transpose { .. }
                                    | PipelineOp::Crop { .. }
                                    | PipelineOp::CropBorder { .. }
                                    | PipelineOp::Expand { .. }
                                    | PipelineOp::Duplicate
                                    | PipelineOp::Flip
                            | PipelineOp::Reduce { .. }
                                | PipelineOp::CompositeModule { .. }
                                | PipelineOp::PutData { .. }
                                | PipelineOp::Eval { .. }
                                | PipelineOp::PutPixel { .. }
                                | PipelineOp::EffectSpread { .. }
                                | PipelineOp::Paste { .. }
                                | PipelineOp::Scale { .. }
                                | PipelineOp::Contain { .. }
                            | PipelineOp::Cover { .. }
                            | PipelineOp::Pad { .. }
                            | PipelineOp::Transform { .. }
                            | PipelineOp::Resize {
                                    ..
                                }
                            )
                        }))
    })
}
