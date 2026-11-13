//! StrategyGuard evaluation grouped by execution family.

use super::{GuardAssessment, StrategyGuard, StrategyInputs, guards};
use crate::raster::Mode;

impl StrategyGuard {
    pub(in crate::compute::registry::support) fn assess(
        self,
        input: StrategyInputs,
    ) -> GuardAssessment {
        match self {
            Self::Always => GuardAssessment::Satisfied,
            Self::DrawNativeSpans => guards::draw_native_spans_guard(input),
            Self::ResizeFilteredFma
            | Self::ThumbnailI32Stream2x2
            | Self::ResizeLuma16Vector
            | Self::ResizeByteNearestVector { .. }
            | Self::ResizeByteNativeVector { .. }
            | Self::ResizeByteBoxedVector { .. }
            | Self::ResizeByteBoxedNearestVector { .. }
            | Self::ResizeI32Neon => self.assess_resize(input),
            Self::BrightnessVector { .. }
            | Self::ContrastNativeBytesVectorizable { .. }
            | Self::SharpnessNativeBytesVectorizable { .. }
            | Self::ColorSaturationVector { .. }
            | Self::ChopsEqualDimensions { .. }
            | Self::ChopsAddSubtractVector { .. }
            | Self::ChopsOffsetNativeBytes { .. }
            | Self::ChopsOffsetNativeI16
            | Self::Color3DLutVector { .. } => self.assess_pointwise(input),
            Self::Filter3x3NativeBytesVectorizable { .. }
            | Self::Filter5x5NativeBytesVectorizable { .. }
            | Self::Filter5x5I32BinomialFma
            | Self::Filter5x5I32Vectorizable
            | Self::FilterNativeByteBlur { .. }
            | Self::FilterRgbaGaussianBlurRadiusSeven
            | Self::FilterLumaRankWindow
            | Self::FilterNativeByteExtrema { .. }
            | Self::FilterNativeByteRankWindow { .. }
            | Self::FilterF32RankWindow { .. }
            | Self::FilterUnsharpRgb150
            | Self::Filter3x3I32Vectorizable { .. } => self.assess_filter(input),
            Self::ConvertTargetMode(_)
            | Self::ConvertTargetModeNonEmpty(_)
            | Self::PutAlphaMaskBytes { .. }
            | Self::PutAlphaBytes { .. }
            | Self::ExtractBandBytes { .. }
            | Self::RotateVectorBytes { .. }
            | Self::TransformByteKernels { .. }
            | Self::TransformI16Affine { .. }
            | Self::PutPixelSpan { .. }
            | Self::RgbaPixelWriteRun
            | Self::TransposeRgbaAvx2
            | Self::TransposeNativeBytes { .. } => self.assess_mode_and_geometry(input),
            Self::ReduceNativeByteVector { .. }
            | Self::ReduceF32Vector
            | Self::NativeBytePrefix { .. }
            | Self::NativeMirrorRows { .. }
            | Self::NativePointwiseBytes { .. }
            | Self::NativePointwiseBytesIncludingEmpty { .. }
            | Self::NativeCropBytes { .. }
            | Self::NativeCropBorderBytes { .. }
            | Self::ExactNativeRows { .. }
            | Self::PutDataNative { .. }
            | Self::EffectSpreadBytes { .. }
            | Self::EffectNoiseVector
            | Self::EffectMandelbrotVector
            | Self::NativeBytes { .. }
            | Self::ExpandVector { .. }
            | Self::CheckedNativeBytes { .. } => self.assess_native(input),
            Self::PasteLMaskNeon { .. }
            | Self::AlphaCompositeVector { .. }
            | Self::MergeRgbInterleave
            | Self::MergeNativeInterleave { .. }
            | Self::BlendNativeVector { .. }
            | Self::CompositeNativeByteVector { .. }
            | Self::CompositeLMaskNeon { .. } => self.assess_composite(input),
        }
    }

    fn assess_resize(self, input: StrategyInputs) -> GuardAssessment {
        match self {
            Self::ResizeFilteredFma => guards::resize_filtered_fma_guard(input),
            Self::ThumbnailI32Stream2x2 => guards::thumbnail_i32_stream_guard(input),
            Self::ResizeLuma16Vector => guards::resize_luma16_vector_guard(input),
            Self::ResizeByteNearestVector { channels } => {
                guards::resize_byte_nearest_vector_guard(input, channels)
            }
            Self::ResizeByteNativeVector { channels } => {
                guards::resize_byte_native_vector_guard(input, channels)
            }
            Self::ResizeByteBoxedVector {
                channels,
                straight_alpha,
            } => guards::resize_byte_boxed_vector_guard(input, channels, straight_alpha),
            Self::ResizeByteBoxedNearestVector { channels } => {
                guards::resize_byte_boxed_nearest_vector_guard(input, channels)
            }
            Self::ResizeI32Neon => guards::resize_i32_neon_guard(input),
            _ => unreachable!("resize assessment received another guard"),
        }
    }

    fn assess_pointwise(self, input: StrategyInputs) -> GuardAssessment {
        match self {
            Self::BrightnessVector { channels } => guards::brightness_vector_guard(input, channels),
            Self::ContrastNativeBytesVectorizable { channels } => {
                guards::contrast_vector_guard(input, channels)
            }
            Self::SharpnessNativeBytesVectorizable { channels } => {
                guards::sharpness_vector_guard(input, channels)
            }
            Self::ColorSaturationVector { channels } => {
                guards::color_saturation_vector_guard(input, channels)
            }
            Self::ChopsEqualDimensions { channels } => {
                guards::chops_equal_dimensions_guard(input, channels)
            }
            Self::ChopsAddSubtractVector { channels } => {
                guards::chops_add_subtract_guard(input, channels)
            }
            Self::ChopsOffsetNativeBytes { channels } => {
                guards::chops_offset_native_bytes_guard(input, channels)
            }
            Self::ChopsOffsetNativeI16 => guards::chops_offset_native_i16_guard(input),
            Self::Color3DLutVector { source_channels } => {
                guards::color3dlut_guard(input, source_channels)
            }
            _ => unreachable!("pointwise assessment received another guard"),
        }
    }

    fn assess_filter(self, input: StrategyInputs) -> GuardAssessment {
        match self {
            Self::Filter3x3NativeBytesVectorizable { channels } => {
                guards::filter3x3_native_bytes_vectorizable_guard(input, channels)
            }
            Self::Filter5x5NativeBytesVectorizable { channels } => {
                guards::filter5x5_native_bytes_vectorizable_guard(input, channels)
            }
            Self::Filter5x5I32BinomialFma => guards::filter5x5_i32_binomial_fma_guard(input),
            Self::Filter5x5I32Vectorizable => guards::filter5x5_i32_vectorizable_guard(input),
            Self::FilterNativeByteBlur { channels } => {
                guards::filter_native_byte_blur_guard(input, channels)
            }
            Self::FilterRgbaGaussianBlurRadiusSeven => {
                guards::filter_rgba_gaussian_blur_radius_seven_guard(input)
            }
            Self::FilterLumaRankWindow => guards::filter_luma_rank_window_guard(input),
            Self::FilterNativeByteExtrema { channels } => {
                guards::filter_native_byte_extrema_guard(input, channels)
            }
            Self::FilterNativeByteRankWindow { channels } => {
                guards::filter_native_byte_rank_window_guard(input, channels)
            }
            Self::FilterF32RankWindow { max_size } => {
                guards::filter_f32_rank_window_guard(input, max_size)
            }
            Self::FilterUnsharpRgb150 => guards::filter_unsharp_rgb150_guard(input),
            Self::Filter3x3I32Vectorizable { channels } => {
                guards::filter3x3_i32_vectorizable_guard(input, channels)
            }
            _ => unreachable!("filter assessment received another guard"),
        }
    }

    fn assess_mode_and_geometry(self, input: StrategyInputs) -> GuardAssessment {
        match self {
            Self::ConvertTargetMode(target) => guards::convert_target_guard(input, target, false),
            Self::ConvertTargetModeNonEmpty(target) => {
                guards::convert_target_guard(input, target, true)
            }
            Self::PutAlphaMaskBytes {
                channels,
                minimum_pixels,
            } => guards::alpha_mask_guard(input, channels, minimum_pixels),
            Self::PutAlphaBytes {
                channels,
                minimum_pixels,
            } => guards::put_alpha_bytes_guard(input, channels, minimum_pixels),
            Self::ExtractBandBytes { channels } => {
                guards::extract_band_bytes_guard(input, channels)
            }
            Self::RotateVectorBytes {
                channels,
                force_nearest,
                bilinear,
            } => guards::rotate_vector_bytes_guard(input, channels, force_nearest, bilinear),
            Self::TransformByteKernels {
                channels,
                affine_nearest,
                affine_bilinear,
                projective_nearest,
            } => guards::transform_byte_kernels_guard(
                input,
                channels,
                affine_nearest,
                affine_bilinear,
                projective_nearest,
            ),
            Self::TransformI16Affine { nearest, bilinear } => {
                guards::transform_i16_affine_guard(input, nearest, bilinear)
            }
            Self::PutPixelSpan { channels } => guards::pixel_span_guard(input, channels),
            Self::RgbaPixelWriteRun => guards::pixel_write_run_guard(input),
            Self::TransposeRgbaAvx2 => guards::rgba_transpose_avx2_guard(input),
            Self::TransposeNativeBytes {
                channels,
                allow_empty,
            } => guards::native_transpose_bytes_guard(input, channels, allow_empty),
            _ => unreachable!("mode or geometry assessment received another guard"),
        }
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn assess_native(self, input: StrategyInputs) -> GuardAssessment {
        match self {
            Self::ReduceNativeByteVector { channels } => {
                guards::reduce_native_byte_vector_guard(input, channels)
            }
            Self::ReduceF32Vector => guards::reduce_f32_vector_guard(input),
            Self::NativeBytePrefix { channels } => {
                guards::native_byte_prefix_guard(input, channels)
            }
            Self::NativeMirrorRows { channels } => {
                guards::native_mirror_rows_guard(input, channels)
            }
            Self::NativePointwiseBytes { channels } => {
                guards::native_pointwise_bytes_guard(input, channels)
            }
            Self::NativePointwiseBytesIncludingEmpty { channels } => {
                guards::native_pointwise_bytes_including_empty_guard(input, channels)
            }
            Self::NativeCropBytes { channels } => guards::native_crop_bytes_guard(input, channels),
            Self::NativeCropBorderBytes { channels } => {
                guards::native_crop_border_bytes_guard(input, channels)
            }
            Self::ExactNativeRows { channels } => guards::exact_native_rows_guard(input, channels),
            Self::PutDataNative { channels, payloads } => {
                guards::put_data_native_guard(input, channels, payloads)
            }
            Self::EffectSpreadBytes { channels } => {
                guards::effect_spread_bytes_guard(input, channels)
            }
            Self::EffectNoiseVector => guards::effect_noise_vector_guard(input),
            Self::EffectMandelbrotVector => guards::effect_mandelbrot_vector_guard(input),
            Self::NativeBytes { channels } => guards::native_bytes_guard(input, channels),
            Self::ExpandVector { channels } => guards::expand_vector_guard(input, channels),
            Self::CheckedNativeBytes { channels } => {
                guards::checked_native_bytes_guard(input, channels)
            }
            _ => unreachable!("native assessment received another guard"),
        }
    }

    fn assess_composite(self, input: StrategyInputs) -> GuardAssessment {
        match self {
            Self::PasteLMaskNeon {
                mode,
                channels,
                minimum_clipped_width,
            } => guards::paste_l_mask_neon_guard(input, mode, channels, minimum_clipped_width),
            Self::AlphaCompositeVector { channels } => {
                guards::alpha_composite_vector_guard(input, channels)
            }
            Self::MergeRgbInterleave => guards::merge_rgb_interleave_guard(input),
            Self::MergeNativeInterleave {
                target_mode,
                channels,
            } => guards::merge_native_interleave_guard(input, target_mode, channels),
            Self::BlendNativeVector {
                channels,
                compatible_modes,
            } => guards::blend_native_vector_guard(input, channels, compatible_modes),
            Self::CompositeNativeByteVector { mode, channels } => {
                guards::composite_native_byte_vector_guard(input, mode, channels)
            }
            Self::CompositeLMaskNeon {
                mode,
                channels,
                minimum_clipped_width,
            } => guards::composite_l_mask_neon_guard(input, mode, channels, minimum_clipped_width),
            _ => unreachable!("composition assessment received another guard"),
        }
    }

    pub(in crate::compute::registry::support) fn assess_for_mode(
        self,
        input: StrategyInputs,
        source_mode: Mode,
    ) -> GuardAssessment {
        // Prepared Convert returns the source identity before reaching a
        // target kernel, so same-mode requests must select the Default path.
        if matches!(
            self,
            Self::ConvertTargetMode(target) | Self::ConvertTargetModeNonEmpty(target)
                if target == source_mode
        ) {
            return GuardAssessment::Ineligible;
        }
        self.assess(input)
    }
}
