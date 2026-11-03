//! Closed operation dispatch over plans already prepared by their family owner.

use super::support::{OperationId, OperationSupport, StrategySelection};
use crate::error::PilError;
use crate::raster::DynamicImage;

/// One validated CPU operation with concrete mode and architecture strategies.
///
/// The registry returns this plan after the operation owner validates inputs
/// and selects its implementation. Execution consumes that decision exactly
/// once; it does not repeat admission or make another routing decision.
pub struct PreparedOperation<'a> {
    implementation: Implementation<'a>,
}

macro_rules! declare_prepared_operations {
    ($($variant:ident, $family:ident $(:: $subfamily:ident)?, $plan:ident, $constructor:ident);+ $(;)?) => {
        enum Implementation<'a> {
            $($variant(crate::compute::pool_cpu::ops::$family $(::$subfamily)?::ops::$plan<'a>),)+
        }

        impl<'a> PreparedOperation<'a> {
            $(pub(crate) fn $constructor(
                plan: crate::compute::pool_cpu::ops::$family $(::$subfamily)?::ops::$plan<'a>,
            ) -> Self {
                Self { implementation: Implementation::$variant(plan) }
            })+

            /// Returns the actual selection, including any declared fallback.
            #[must_use]
            pub fn selection(&self) -> &StrategySelection {
                match &self.implementation {
                    $(Implementation::$variant(plan) => plan.selection(),)+
                }
            }

            /// Returns selections for prepared child operations, if this plan composes any.
            #[must_use]
            pub fn composed_selections(&self) -> &[StrategySelection] {
                match &self.implementation {
                    Implementation::Contain(plan)
                    | Implementation::Cover(plan)
                    | Implementation::Fit(plan)
                    | Implementation::Scale(plan) => plan.composed_selections(),
                    Implementation::Pad(plan) => plan.composed_selections(),
                    Implementation::Thumbnail(plan) => plan.composed_selections(),
                    Implementation::Contrast(plan) => plan.composed_selections(),
                    Implementation::Convert(plan) => plan.composed_selections(),
                    _ => &[],
                }
            }

            /// Executes the original typed algorithm selected during preparation.
            ///
            /// # Errors
            ///
            /// Propagates the operation's existing error contract without retrying
            /// another mode, architecture, or backend.
            pub fn execute(self) -> Result<DynamicImage, PilError> {
                crate::compute::record_pipeline_operation_selection(self.selection());
                match self.implementation {
                    $(Implementation::$variant(plan) => plan.execute(),)+
                }
            }

            /// Executes the prepared ExtractBand plan into a caller-owned byte slice.
            ///
            /// This terminal form preserves the direct GetData allocation boundary.
            ///
            /// # Returns
            ///
            /// Returns `None` when this plan has no caller-owned output strategy.
            ///
            /// # Errors
            ///
            /// Propagates output-shape and kernel errors from a selected ExtractBand strategy.
            pub(crate) fn execute_into(
                self,
                output: &mut [u8],
            ) -> Result<Option<(u64, u64, &'static str)>, PilError> {
                match self.implementation {
                    Implementation::ExtractBand(plan) => plan.execute_into(output),
                    _ => Ok(None),
                }
            }
        }
    };
}

declare_prepared_operations! {
    Draw, draw, PreparedDraw, from_draw;
    Crop, crop, PreparedCrop, from_crop;
    CropBorder, imageops, PreparedCropBorder, from_crop_border;
    Resize, resize, PreparedResize, from_resize;
    Contain, geometry, PreparedContain, from_contain;
    Cover, geometry, PreparedCover, from_cover;
    Fit, geometry, PreparedFit, from_fit;
    Pad, geometry::pad, PreparedPad, from_pad;
    Scale, geometry, PreparedScale, from_scale;
    Thumbnail, thumbnail, PreparedThumbnail, from_thumbnail;
    Flip, transpose, PreparedFlip, from_flip;
    Mirror, transpose, PreparedMirror, from_mirror;
    Transpose, transpose, PreparedTranspose, from_transpose;
    Rotate, rotate, PreparedRotate, from_rotate;
    Transform, transform, PreparedTransform, from_transform;
    Reduce, reduce, PreparedReduce, from_reduce;
    Grayscale, grayscale, PreparedGrayscale, from_grayscale;
    Eval, eval, PreparedEval, from_eval;
    Invert, imageops, PreparedInvert, from_invert;
    Posterize, imageops, PreparedPosterize, from_posterize;
    Solarize, imageops, PreparedSolarize, from_solarize;
    Colorize, imageops, PreparedColorize, from_colorize;
    Autocontrast, imageops, PreparedAutocontrast, from_autocontrast;
    Equalize, imageops, PreparedEqualize, from_equalize;
    Expand, imageops, PreparedExpand, from_expand;
    ExtractBand, put, PreparedExtractBand, from_extract_band;
    PutPixel, put, PreparedPutPixel, from_put_pixel;
    PutPixelBatch, put, PreparedPutPixelBatch, from_put_pixel_batch;
    PutData, put, PreparedPutData, from_put_data;
    PutAlpha, put, PreparedPutAlpha, from_put_alpha;
    PutAlphaData, put, PreparedPutAlphaData, from_put_alpha_data;
    Color3DLut, color3dlut, PreparedColor3DLut, from_color3dlut;
    Convert, color, PreparedConvert, from_convert;
    ConvertLab, color, PreparedConvertLab, from_convert_lab;
    RemapPalette, color, PreparedRemapPalette, from_remap_palette;
    EffectSpread, effects, PreparedEffectSpread, from_effect_spread;
    EffectNoise, effects, PreparedEffectNoise, from_effect_noise;
    Brightness, enhance, PreparedBrightness, from_brightness;
    Contrast, enhance, PreparedContrast, from_contrast;
    ColorSaturation, enhance, PreparedColorSaturation, from_color_saturation;
    Sharpness, enhance, PreparedSharpness, from_sharpness;
    Filter, filter, PreparedFilter, from_filter;
    Chops, chops, PreparedChops, from_chops;
    AlphaComposite, composite, PreparedAlphaComposite, from_alpha_composite;
    Merge, composite, PreparedMerge, from_merge;
    BlendModule, composite, PreparedBlendModule, from_blend_module;
    CompositeModule, composite, PreparedCompositeModule, from_composite_module;
    Paste, paste, PreparedPaste, from_paste;
}

impl PreparedOperation<'_> {
    /// Executes Convert while retaining palette metadata for the Image wrapper.
    pub(crate) fn execute_convert_output(
        self,
    ) -> Result<crate::compute::pool_cpu::ops::color::convert::ops::PreparedConvertOutput, PilError>
    {
        match self.implementation {
            Implementation::Convert(plan) => plan.execute_with_output(),
            _ => Err(PilError::InternalError(
                "palette output requested from a non-Convert prepared operation".into(),
            )),
        }
    }

    /// Returns the filter owner's prepared identity proof when it can share the input Arc.
    pub(crate) fn shared_identity_proof(&self) -> Option<bool> {
        match &self.implementation {
            Implementation::Filter(plan) => plan.shared_identity_proof(),
            _ => None,
        }
    }

    /// Returns the operation identity carried by the selected implementation.
    #[must_use]
    pub fn operation(&self) -> OperationId {
        self.selection().implementation.operation
    }

    /// Returns the operation owner's exhaustive support declaration.
    #[must_use]
    pub fn support(&self) -> &'static OperationSupport {
        self.operation().support()
    }
}

impl std::fmt::Debug for PreparedOperation<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedOperation")
            .field("selection", self.selection())
            .finish_non_exhaustive()
    }
}
