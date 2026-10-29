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
    ($($variant:ident, $family:ident, $plan:ident, $constructor:ident);+ $(;)?) => {
        enum Implementation<'a> {
            $($variant(crate::compute::pool_cpu::ops::$family::ops::$plan<'a>),)+
        }

        impl<'a> PreparedOperation<'a> {
            $(pub(crate) fn $constructor(
                plan: crate::compute::pool_cpu::ops::$family::ops::$plan<'a>,
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

            /// Executes the original typed algorithm selected during preparation.
            ///
            /// # Errors
            ///
            /// Propagates the operation's existing error contract without retrying
            /// another mode, architecture, or backend.
            pub fn execute(self) -> Result<DynamicImage, PilError> {
                match self.implementation {
                    $(Implementation::$variant(plan) => plan.execute(),)+
                }
            }
        }
    };
}

declare_prepared_operations! {
    Crop, crop, PreparedCrop, from_crop;
    CropBorder, imageops, PreparedCropBorder, from_crop_border;
    Flip, transpose, PreparedFlip, from_flip;
    Mirror, transpose, PreparedMirror, from_mirror;
    Transpose, transpose, PreparedTranspose, from_transpose;
    Rotate, rotate, PreparedRotate, from_rotate;
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
    EffectSpread, effects, PreparedEffectSpread, from_effect_spread;
    EffectNoise, effects, PreparedEffectNoise, from_effect_noise;
}

impl PreparedOperation<'_> {
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
