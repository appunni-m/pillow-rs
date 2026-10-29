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
        LogicalModeAdmission {
            ops,
            img,
            logical_mode,
            mode,
            f_resize_constant_bits,
            f_pad_f64_is_exact,
            inspect_auxiliary_modes,
        }
        .is_supported()
    })
}

struct LogicalModeAdmission<'a> {
    ops: &'a [PipelineOp],
    img: Option<&'a DynamicImage>,
    logical_mode: &'a str,
    mode: Option<&'a str>,
    f_resize_constant_bits: Option<u32>,
    f_pad_f64_is_exact: bool,
    inspect_auxiliary_modes: bool,
}

impl LogicalModeAdmission<'_> {
    fn is_supported(&self) -> bool {
        gpu_native_logical_mode_is_supported(self.ops, self.img, self.logical_mode)
            || self.supports_simple_routes()
            || self.supports_draw()
            || self.supports_palette_data_ops()
            || self.supports_palette_rgb_merge()
            || self.supports_mode_one_data_ops()
            || self.supports_rgbx()
            || self.supports_rgba_premultiplied()
            || self.supports_hsv_ycbcr_ops()
            || self.supports_hsv_expand()
            || self.supports_native_channel_extract()
            || self.supports_cmyk_alpha_promotion()
            || self.supports_cmyk_rgb_conversion()
            || self.supports_color_lut()
            || self.supports_float_ops()
            || self.supports_int_ops()
            || self.supports_typed_word_relocations()
            || self.supports_luma16_ops()
            || self.supports_cmyk_ops()
    }

    fn supports_simple_routes(&self) -> bool {
        let logical_mode = self.logical_mode;
        let ops = self.ops;
        let img = self.img;
        let mode = self.mode;
        (logical_mode == "1"
            && img.is_none_or(|img| matches!(img, DynamicImage::ImageLuma8(_)))
            && matches!(ops, [PipelineOp::Flip]))
            // YCbCr shares RGB8 transport but grayscale reads only its Y
            // sample; admit exactly the singleton kernel that carries
            // this native-mode interpretation in its fourth uniform.
            || (logical_mode == "YCbCr"
                && matches!(ops, [PipelineOp::Grayscale])
                && img.is_none_or(|img| gpu_native_grayscale_rgb_input(ops, img, mode)))
            // On little-endian targets, HSV uses the same packed three-byte
            // storage as RGB. A singleton transpose only relocates complete
            // triplets; it does not interpret or convert H/S/V samples. Keep
            // other targets on the semantic-control path until their native
            // upload is covered.
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
    }

    fn supports_draw(&self) -> bool {
        // ImageDraw geometry is scan-converted by the exact host canvas
        // before the packed draw shader copies the complete result. The
        // data plane preserves raw indexed, typed-word, and native color
        // bytes without interpreting samples in the shader.
        matches!(
            self.logical_mode,
            "1" | "P" | "PA" | "RGBX" | "RGBa" | "CMYK" | "HSV" | "YCbCr" | "I" | "F"
        ) && !self.ops.is_empty()
            && self
                .ops
                .iter()
                .all(crate::compute::pool_cpu::ops::draw::is_draw_op)
    }

    fn supports_palette_data_ops(&self) -> bool {
        matches!(self.logical_mode, "P" | "PA")
            && self.ops.iter().all(|op| {
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
                        | PipelineOp::Resize { .. }
                        | PipelineOp::Equalize
                        | PipelineOp::EqualizeMasked { .. }
                )
            })
    }

    fn supports_palette_rgb_merge(&self) -> bool {
        matches!(
            self.ops,
            [PipelineOp::Merge {
                mode: ColorMode::RGB,
                ..
            }]
        ) && self.logical_mode == "P"
            && (!self.inspect_auxiliary_modes
                || gpu_palette_first_rgb_merge_is_supported(self.ops, self.mode))
    }

    fn supports_mode_one_data_ops(&self) -> bool {
        self.logical_mode == "1"
            && self.ops.iter().all(|op| {
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
                            filter: ResampleFilter::Nearest,
                            ..
                        }
                        | PipelineOp::Cover {
                            filter: ResampleFilter::Nearest,
                            ..
                        }
                        | PipelineOp::Pad {
                            filter: ResampleFilter::Nearest,
                            ..
                        }
                        | PipelineOp::Transform { .. }
                        | PipelineOp::Resize {
                            filter: ResampleFilter::Nearest,
                            ..
                        }
                )
            })
    }

    // RGBX shares RGBA's four-byte storage, but its fourth byte is padding
    // rather than alpha. Transpose, PutPixel, and the dedicated mode-6
    // Sharpness shader preserve all four stored bytes without alpha logic.
    fn supports_rgbx(&self) -> bool {
        self.logical_mode == "RGBX"
            && self.ops.iter().all(|op| {
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
                        | PipelineOp::Resize { .. }
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
            })
    }

    // RGBa uses RGBA storage. Pillow's Image.resize preserves its already
    // premultiplied representation, so boxed Fit can use the same coefficient
    // kernels with premultiply disabled; ImageChops also operates on these
    // stored bytes directly. Keep this list to raw channels and geometry
    // that preserves the four-byte sample contract.
    fn supports_rgba_premultiplied(&self) -> bool {
        self.logical_mode == "RGBa"
            && self.ops.iter().all(|op| {
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
                        | PipelineOp::Resize { .. }
                )
            })
    }

    fn supports_hsv_ycbcr_ops(&self) -> bool {
        matches!(self.logical_mode, "HSV" | "YCbCr")
            && self.ops.iter().all(|op| {
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
                        | PipelineOp::Resize { .. }
                )
            })
    }

    fn supports_hsv_expand(&self) -> bool {
        // Singleton HSV Expand reads the three stored samples as-is. Keep it
        // tied to the native-layout gate so a logical HSV image with a
        // mismatched physical representation cannot enter this route.
        self.logical_mode == "HSV"
            && matches!(self.ops, [PipelineOp::Expand { .. }])
            && self
                .img
                .is_none_or(|img| gpu_native_expand_channels(self.ops, img, self.mode).is_some())
    }

    fn supports_native_channel_extract(&self) -> bool {
        // CMYK, HSV, and YCbCr retain their native channel order in packed
        // transport. ExtractBand copies one byte and publishes L8 without
        // interpreting samples as RGB or alpha. PutPixel is allowed because
        // getchannel batches can write a source pixel before extracting it.
        matches!(self.logical_mode, "CMYK" | "HSV" | "YCbCr")
            && self.ops.iter().all(|op| {
                matches!(
                    op,
                    PipelineOp::ExtractBand { .. }
                        | PipelineOp::PutPixel { .. }
                        | PipelineOp::EffectSpread { .. }
                )
            })
    }

    fn supports_cmyk_alpha_promotion(&self) -> bool {
        // CMYK PutAlpha is a terminal promotion through the exact integer
        // CMYK-to-RGB conversion. A following operation needs a segmented
        // batch with updated RGBA metadata, so this admission stays terminal.
        self.logical_mode == "CMYK"
            && self.ops.len() == 1
            && matches!(
                self.ops[0],
                PipelineOp::PutAlpha {
                    mode: PixelMode::CMYK,
                    ..
                } | PipelineOp::PutAlphaData {
                    mode: PixelMode::CMYK,
                    ..
                }
            )
    }

    fn supports_cmyk_rgb_conversion(&self) -> bool {
        // Keep CMYK-to-RGB as one terminal native conversion. The shader
        // interprets C/M/Y/K and applies Pillow's exact integer inverse.
        self.logical_mode == "CMYK"
            && matches!(
                self.ops,
                [PipelineOp::Convert {
                    mode: ColorMode::RGB,
                    matrix: None,
                    dither: None,
                }]
            )
            && self
                .img
                .is_none_or(|img| matches!(img, DynamicImage::ImageRgba8(_)))
    }

    fn supports_color_lut(&self) -> bool {
        matches!(self.logical_mode, "RGB" | "RGBA" | "CMYK")
            && self
                .ops
                .iter()
                .all(|op| matches!(op, PipelineOp::Color3DLut { .. }))
    }

    fn supports_float_ops(&self) -> bool {
        // F order-statistic shaders compare decoded f32 samples. Ordinary
        // byte filters would sort IEEE-754 bytes and produce a different
        // result; Mirror is safe because it only relocates complete words.
        let filter_or_mirror = self.logical_mode == "F"
            && self
                .img
                .is_none_or(|img| gpu_float_filter_is_supported(self.ops, img));
        // The exact scalar resize marker plus raw-word canvas placement makes
        // a single constant Pad safe; mixed batches keep host control.
        let constant_pad = self.logical_mode == "F"
            && self.f_resize_constant_bits.is_some()
            && self.ops.len() == 1
            && matches!(self.ops[0], PipelineOp::Pad { .. });
        // A changed-axis F Pad with a PutData(F)-only prefix is admitted only
        // when the host f64 proof establishes the same placement.
        let exact_pad = self.logical_mode == "F" && self.f_pad_f64_is_exact;
        // Nearest F Fit is a one-tap word relocation using Pillow's f32 crop
        // boundary conversion. It is separate from the vertical pass so
        // Metal cannot observe a stale horizontal intermediate.
        let nearest_fit = self.logical_mode == "F"
            && self.ops.len() == 1
            && matches!(
                self.ops[0],
                PipelineOp::Fit {
                    filter: ResampleFilter::Nearest,
                    ..
                }
            );
        filter_or_mirror || constant_pad || exact_pad || nearest_fit
    }

    fn supports_int_ops(&self) -> bool {
        // I Pad and Resize are admitted here only for nearest sampling; a
        // filtered Pad requires the typed INT32 accumulator.
        let nearest_geometry = self.logical_mode == "I"
            && self.ops.iter().all(|op| {
                matches!(
                    op,
                    PipelineOp::Pad {
                        filter: ResampleFilter::Nearest,
                        ..
                    } | PipelineOp::Resize {
                        filter: ResampleFilter::Nearest,
                        ..
                    }
                )
            });
        let filter = self.logical_mode == "I"
            && self.img.is_none_or(|img| {
                gpu_int_filter_is_supported(self.ops, img)
                    || gpu_int_filter_resize_chain_is_supported(self.ops, img)
            });
        nearest_geometry || filter
    }

    fn supports_typed_word_relocations(&self) -> bool {
        // I/F samples are four raw bytes per pixel at this boundary. These
        // operations relocate or duplicate whole samples; arithmetic and
        // fill stay out because they require native typed buffers.
        matches!(self.logical_mode, "I" | "F")
            && self.ops.iter().all(|op| {
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
                        | PipelineOp::Scale { .. }
                        | PipelineOp::Contain { .. }
                        | PipelineOp::Cover { .. }
                        | PipelineOp::Resize { .. }
                        | PipelineOp::Transform { .. }
                )
            })
    }

    fn supports_luma16_ops(&self) -> bool {
        // Native I;16* uses one typed u16 per pixel. Packed GPU words are an
        // opaque transport here and are never narrowed to luma bytes or
        // interpreted as RGBA.
        matches!(self.logical_mode, "I;16" | "I;16L" | "I;16B" | "I;16N")
            && self.img.is_none_or(|img| {
                gpu_luma16_geometry_is_supported(self.ops, img, Some(self.logical_mode))
                    || gpu_luma16_convert_is_supported(self.ops, img)
                    || gpu_luma16_paste_is_supported(self.ops, img)
            })
    }

    fn supports_cmyk_ops(&self) -> bool {
        self.logical_mode == "CMYK"
            && self.ops.iter().all(|op| {
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
                        | PipelineOp::Resize { .. }
                )
            })
    }
}

fn gpu_native_logical_mode_is_supported(
    ops: &[PipelineOp],
    img: Option<&DynamicImage>,
    logical_mode: &str,
) -> bool {
    if matches!(logical_mode, "L" | "LA" | "RGB" | "RGBA") {
        return true;
    }
    match logical_mode {
        "1" => {
            img.is_some_and(|image| matches!(image, DynamicImage::ImageLuma8(_)))
                && matches!(
                    ops,
                    [PipelineOp::Convert {
                        mode: ColorMode::I16,
                        ..
                    }]
                )
        }
        "La" => {
            img.is_none_or(|image| matches!(image, DynamicImage::ImageLumaA8(_)))
                && matches!(
                    ops,
                    [PipelineOp::Convert {
                        mode: ColorMode::LA,
                        matrix: None,
                        dither: None,
                    }]
                )
        }
        "RGBa" => {
            img.is_none_or(|image| matches!(image, DynamicImage::ImageRgba8(_)))
                && matches!(
                    ops,
                    [PipelineOp::Convert {
                        mode: ColorMode::LaPremultiplied
                            | ColorMode::RGBA
                            | ColorMode::RGB
                            | ColorMode::Rgbx,
                        matrix: None,
                        dither: None,
                    }]
                )
        }
        _ => false,
    }
}
