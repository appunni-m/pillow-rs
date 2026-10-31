//! Filter support contracts and prepared CPU dispatch.
//!
//! TODO(pillow-rs#3ptb): compare the general blur and BoxBlur mode routes against
//! the archive; include the RGBA radius-seven small-shape architecture fallback
//! in CPU parity; cover the exact-binomial I32 AVX/FMA route and ineligible-
//! request Default fallback; reconcile every public Filter caller before closing
//! this family. Keep Rayon out of this serial owner.

use crate::compute::ExecutionContext;
use crate::compute::registry::support::{
    ImplementationSupport, InputContract, ModeSupport, OperationId, OperationInputs,
    OperationSupport, OutputModeRule, PaletteInput, RejectionKind, StrategySelection,
    SupportAssessment,
};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

const NO_INPUTS: &[crate::compute::registry::support::InputContract] = &[];
const FILTER3X3_INPUTS: &[InputContract] = &[InputContract::Filter3x3Parameters];
const FILTER5X5_INPUTS: &[InputContract] = &[InputContract::Filter5x5Parameters];
const GAUSSIAN_BLUR_INPUTS: &[InputContract] = &[InputContract::GaussianBlurSigma];
const BOX_BLUR_INPUTS: &[InputContract] = &[InputContract::BoxBlurParameters];
const UNSHARP_INPUTS: &[InputContract] = &[InputContract::UnsharpMaskParameters];
const PALETTE_MODE: &str = "cannot filter palette images";
const INTERNAL_MODE: &str = "CPU filter does not support this internal raster mode";
const WRONG_UNSHARP_MODE: &str = "image has wrong mode";
const RANK_WINDOW: &[InputContract] = &[InputContract::OddRankWindow {
    rank_required: false,
}];
const RANK_WINDOW_VALUE: &[InputContract] = &[InputContract::OddRankWindow {
    rank_required: true,
}];

pub(super) const fn inputs_for(operation: OperationId) -> &'static [InputContract] {
    match operation {
        OperationId::Filter3x3 => FILTER3X3_INPUTS,
        OperationId::Filter5x5 => FILTER5X5_INPUTS,
        OperationId::GaussianBlur => GAUSSIAN_BLUR_INPUTS,
        OperationId::BoxBlur => BOX_BLUR_INPUTS,
        OperationId::UnsharpMask => UNSHARP_INPUTS,
        OperationId::MedianFilter | OperationId::MaxFilter | OperationId::MinFilter => RANK_WINDOW,
        OperationId::RankFilter => RANK_WINDOW_VALUE,
        _ => NO_INPUTS,
    }
}

fn core_only_reason(operation: OperationId) -> &'static str {
    match operation {
        OperationId::Filter3x3 | OperationId::Filter5x5 => {
            "native per-mode kernel filtering extends Pillow's documented L/RGB contract"
        }
        OperationId::GaussianBlur | OperationId::BoxBlur => {
            "native per-mode blur extends Pillow's supported uint8 mode set"
        }
        OperationId::MedianFilter
        | OperationId::MaxFilter
        | OperationId::MinFilter
        | OperationId::RankFilter => {
            "native per-channel rank filtering extends Pillow's scalar mode contract"
        }
        _ => "native filter mode extension",
    }
}

pub(super) fn core_only(
    operation: OperationId,
    mode: Mode,
    architectures: &'static [ImplementationSupport],
) -> ModeSupport {
    ModeSupport::cpu_core_only(
        operation,
        mode,
        OutputModeRule::Preserve,
        inputs_for(operation),
        architectures,
        core_only_reason(operation),
    )
}

pub(super) fn core_only_serial(operation: OperationId, mode: Mode) -> ModeSupport {
    core_only(operation, mode, &[])
}

fn rejected(mode: Mode, reason: &'static str) -> ModeSupport {
    ModeSupport::Rejected {
        mode,
        reason,
        kind: RejectionKind::ValueError,
    }
}

fn support_for_mode(operation: OperationId, mode: Mode) -> ModeSupport {
    match mode {
        Mode::One => super::one::ops::support(operation),
        Mode::ImageLuma8 => super::luma8::ops::support(operation),
        Mode::ImageLumaA8 => super::luma_a8::ops::support(operation),
        Mode::ImageLaPremultiplied8 => super::la_premultiplied8::ops::support(operation),
        Mode::ImagePalette8 => rejected(mode, PALETTE_MODE),
        Mode::ImagePaletteAlpha8 => super::palette_alpha8::ops::support(operation),
        Mode::ImageRgb8 => super::rgb8::ops::support(operation),
        Mode::ImageRgba8 => super::rgba8::ops::support(operation),
        Mode::ImageRgbPremultiplied8 => super::rgb_premultiplied8::ops::support(operation),
        Mode::ImageRgbx8 => super::rgbx8::ops::support(operation),
        Mode::ImageCmyk8 => super::cmyk8::ops::support(operation),
        Mode::ImageYCbCr8 => super::ycbcr8::ops::support(operation),
        Mode::ImageLab8 => super::lab8::ops::support(operation),
        Mode::ImageHsv8 => super::hsv8::ops::support(operation),
        Mode::ImageI32 => super::i32::ops::support(operation),
        Mode::ImageF32 => super::f32::ops::support(operation),
        Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N
        | Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => rejected(mode, INTERNAL_MODE),
    }
}

fn unsharp_support_for_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::ImageRgb8 => super::rgb8::ops::unsharp_mask_support(),
        Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8 => {
            ModeSupport::serial(OperationId::UnsharpMask, mode, UNSHARP_INPUTS)
        }
        _ => rejected(mode, WRONG_UNSHARP_MODE),
    }
}

pub(crate) static FILTER3X3_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::Filter3x3,
    FILTER3X3_INPUTS,
    |mode| support_for_mode(OperationId::Filter3x3, mode),
);
pub(crate) static FILTER5X5_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::Filter5x5,
    FILTER5X5_INPUTS,
    |mode| support_for_mode(OperationId::Filter5x5, mode),
);
pub(crate) static GAUSSIAN_BLUR_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::GaussianBlur,
    GAUSSIAN_BLUR_INPUTS,
    |mode| support_for_mode(OperationId::GaussianBlur, mode),
);
pub(crate) static BOX_BLUR_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::BoxBlur, BOX_BLUR_INPUTS, |mode| {
        support_for_mode(OperationId::BoxBlur, mode)
    });
pub(crate) static UNSHARP_MASK_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::UnsharpMask,
    UNSHARP_INPUTS,
    unsharp_support_for_mode,
);
pub(crate) static MEDIAN_FILTER_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::MedianFilter,
    RANK_WINDOW,
    |mode| support_for_mode(OperationId::MedianFilter, mode),
);
pub(crate) static MAX_FILTER_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::MaxFilter,
    RANK_WINDOW,
    |mode| support_for_mode(OperationId::MaxFilter, mode),
);
pub(crate) static MIN_FILTER_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::MinFilter,
    RANK_WINDOW,
    |mode| support_for_mode(OperationId::MinFilter, mode),
);
pub(crate) static RANK_FILTER_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::RankFilter,
    RANK_WINDOW_VALUE,
    |mode| support_for_mode(OperationId::RankFilter, mode),
);

/// Prepared mode and architecture selection for the direct UnsharpMask blend.
struct PreparedUnsharpBlend {
    mode: Mode,
    percent: i32,
    threshold: i32,
    selection: StrategySelection,
}

impl PreparedUnsharpBlend {
    fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    /// Applies the selected implementation to the original and blur buffers.
    fn execute(
        self,
        original: &[u8],
        blurred_output: &mut [u8],
    ) -> Result<(), PilError> {
        if self.selection.implementation.architecture != crate::compute::Architecture::Default {
            if self.mode != Mode::ImageRgb8 {
                return Err(PilError::InternalError(
                    "UnsharpMask selected an architecture outside RGB mode".into(),
                ));
            }
            if super::rgb8::ops::execute_unsharp_blend(
                &self.selection,
                original,
                blurred_output,
                self.threshold,
            ) {
                return Ok(());
            }
        }
        super::helpers::blend_unsharp_native_bytes(
            original,
            blurred_output,
            self.percent,
            self.threshold,
        )
    }
}

/// Prepared UnsharpMask composition with the Gaussian blur dependency selected
/// from the same injected context as the native-mode blend.
pub(crate) struct PreparedUnsharpMask<'a> {
    original: &'a DynamicImage,
    blur: crate::compute::registry::PreparedOperation<'a>,
    blend: PreparedUnsharpBlend,
}

impl PreparedUnsharpMask<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        self.blend.selection()
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        let Self { original, blur, blend } = self;
        let mut blurred = blur.execute()?;
        let Some(blurred_bytes) = blurred.as_bytes_mut() else {
            return Err(PilError::InternalError(
                "UnsharpMask Gaussian blur has no mutable native byte storage".into(),
            ));
        };
        blend.execute(original.as_bytes(), blurred_bytes)?;
        Ok(blurred)
    }
}

/// Select the direct UnsharpMask blend from its declared exact-mode strategy.
fn prepare_unsharp_blend(
    image: &DynamicImage,
    sigma: f32,
    percent: i32,
    threshold: i32,
    ctx: &ExecutionContext,
) -> Result<PreparedUnsharpBlend, PilError> {
    let inputs = OperationInputs { image, palette: PaletteInput::default() };
    let mode = inputs.mode();
    let facts = inputs.facts().with_unsharp_mask(sigma, percent, threshold);
    let selection = match UNSHARP_MASK_SUPPORT.for_mode(mode).assess(ctx, facts)? {
        SupportAssessment::Ready(selection) => selection,
        SupportAssessment::Rejected { reason, kind, .. } => return Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => {
            return Err(PilError::InternalError(
                "materialized UnsharpMask has unresolved strategy facts".into(),
            ));
        }
    };
    Ok(PreparedUnsharpBlend { mode, percent, threshold, selection })
}

/// Selects and composes the mode-specific blend with its GaussianBlur input.
pub(crate) fn prepare_unsharp_mask<'a>(
    image: &'a DynamicImage,
    sigma: f32,
    percent: i32,
    threshold: i32,
    ctx: &ExecutionContext,
) -> Result<PreparedUnsharpMask<'a>, PilError> {
    let blend = prepare_unsharp_blend(image, sigma, percent, threshold, ctx)?;
    let blur_op = PipelineOp::GaussianBlur { sigma };
    let blur_inputs = OperationInputs {
        image,
        palette: PaletteInput::default(),
    };
    let blur = prepare_pipeline(blur_inputs, &blur_op, ctx)?;
    Ok(PreparedUnsharpMask {
        original: image,
        blur,
        blend,
    })
}

#[derive(Clone, Copy)]
pub(super) enum FilterRequest {
    Filter3x3 { kernel: [f32; 9], scale: f32, offset: f32 },
    Filter5x5 { kernel: [f32; 25], scale: f32, offset: f32 },
    GaussianBlur { sigma: f32 },
    BoxBlur { radius: u32 },
    BoxBlurXY { radius_x: f32, radius_y: f32, passes: u32 },
    MedianFilter { size: u32 },
    MaxFilter { size: u32 },
    MinFilter { size: u32 },
    RankFilter { size: u32, rank: u32 },
}

impl FilterRequest {
    fn operation(self) -> OperationId {
        match self {
            Self::Filter3x3 { .. } => OperationId::Filter3x3,
            Self::Filter5x5 { .. } => OperationId::Filter5x5,
            Self::GaussianBlur { .. } => OperationId::GaussianBlur,
            Self::BoxBlur { .. } | Self::BoxBlurXY { .. } => OperationId::BoxBlur,
            Self::MedianFilter { .. } => OperationId::MedianFilter,
            Self::MaxFilter { .. } => OperationId::MaxFilter,
            Self::MinFilter { .. } => OperationId::MinFilter,
            Self::RankFilter { .. } => OperationId::RankFilter,
        }
    }
}

/// Prepared native-mode filter with a concrete CPU architecture selection.
pub(crate) struct PreparedFilter<'a> {
    image: &'a DynamicImage,
    mode: Mode,
    request: FilterRequest,
    selection: StrategySelection,
    context: ExecutionContext,
}

impl PreparedFilter<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn shared_identity_proof(&self) -> Option<bool> {
        let FilterRequest::Filter3x3 { kernel, scale, offset } = self.request else {
            return None;
        };
        (self.mode == Mode::ImageI32).then(|| {
            super::i32::ops::prepared_uniform_filter3x3_identity(
                self.image,
                &kernel,
                scale,
                offset,
                &self.selection,
            )
        })
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        let image = self.image;
        let request = self.request;
        let selection = &self.selection;
        let context = self.context;
        if selection.implementation.architecture != crate::compute::Architecture::Default
            && matches!(
                request,
                FilterRequest::GaussianBlur { .. }
                    | FilterRequest::BoxBlur { .. }
                    | FilterRequest::BoxBlurXY { .. }
            )
        {
            return execute_architecture_blur(image, self.mode, request, selection, &context);
        }
        match self.mode {
            Mode::One => super::one::ops::execute(image, request, selection),
            Mode::ImageLuma8 => super::luma8::ops::execute(image, request, selection),
            Mode::ImageLumaA8 => super::luma_a8::ops::execute(image, request, selection),
            Mode::ImageLaPremultiplied8 => {
                super::la_premultiplied8::ops::execute(image, request, selection)
            }
            Mode::ImagePaletteAlpha8 => {
                super::palette_alpha8::ops::execute(image, request, selection)
            }
            Mode::ImageRgb8 => super::rgb8::ops::execute(image, request, selection),
            Mode::ImageRgba8 => super::rgba8::ops::execute(image, request, selection, &context),
            Mode::ImageRgbPremultiplied8 => {
                super::rgb_premultiplied8::ops::execute(image, request, selection)
            }
            Mode::ImageRgbx8 => super::rgbx8::ops::execute(image, request, selection),
            Mode::ImageCmyk8 => super::cmyk8::ops::execute(image, request, selection),
            Mode::ImageYCbCr8 => super::ycbcr8::ops::execute(image, request, selection),
            Mode::ImageLab8 => super::lab8::ops::execute(image, request, selection),
            Mode::ImageHsv8 => super::hsv8::ops::execute(image, request, selection),
            Mode::ImageI32 => super::i32::ops::execute(image, request, selection),
            Mode::ImageF32 => super::f32::ops::execute(image, request, selection),
            _ => Err(PilError::InternalError(
                "rejected raster mode reached prepared filter execution".into(),
            )),
        }
    }
}

fn execute_architecture_blur(
    image: &DynamicImage,
    mode: Mode,
    request: FilterRequest,
    selection: &StrategySelection,
    context: &ExecutionContext,
) -> Result<DynamicImage, PilError> {
    match mode {
        Mode::ImageLuma8 => {
            super::luma8::ops::execute_architecture_blur(image, request, selection, context)
        }
        Mode::ImageLumaA8 => {
            super::luma_a8::ops::execute_architecture_blur(image, request, selection, context)
        }
        Mode::ImageRgb8 => {
            super::rgb8::ops::execute_architecture_blur(image, request, selection, context)
        }
        Mode::ImageRgba8 => {
            super::rgba8::ops::execute_architecture_blur(image, request, selection, context)
        }
        Mode::ImageRgbPremultiplied8 => super::rgb_premultiplied8::ops::execute_architecture_blur(
            image, request, selection, context,
        ),
        Mode::ImageRgbx8 => {
            super::rgbx8::ops::execute_architecture_blur(image, request, selection, context)
        }
        _ => Err(PilError::InternalError(
            "native-byte blur selected an architecture outside its exact mode owners".into(),
        )),
    }
}

fn request(op: &PipelineOp) -> Result<FilterRequest, PilError> {
    match op {
        PipelineOp::Filter3x3 { kernel, scale, offset } => Ok(FilterRequest::Filter3x3 {
            kernel: *kernel,
            scale: *scale,
            offset: *offset,
        }),
        PipelineOp::Filter5x5 { kernel, scale, offset } => Ok(FilterRequest::Filter5x5 {
            kernel: *kernel,
            scale: *scale,
            offset: *offset,
        }),
        PipelineOp::GaussianBlur { sigma } => Ok(FilterRequest::GaussianBlur { sigma: *sigma }),
        PipelineOp::BoxBlur { radius } => Ok(FilterRequest::BoxBlur { radius: *radius }),
        PipelineOp::BoxBlurXY { radius_x, radius_y, passes } => Ok(FilterRequest::BoxBlurXY {
            radius_x: *radius_x,
            radius_y: *radius_y,
            passes: *passes,
        }),
        PipelineOp::MedianFilter { size } => Ok(FilterRequest::MedianFilter { size: *size }),
        PipelineOp::MaxFilter { size } => Ok(FilterRequest::MaxFilter { size: *size }),
        PipelineOp::MinFilter { size } => Ok(FilterRequest::MinFilter { size: *size }),
        PipelineOp::RankFilter { size, rank } => Ok(FilterRequest::RankFilter {
            size: *size,
            rank: *rank,
        }),
        _ => Err(PilError::ValueError("expected filter operation".into())),
    }
}

pub(crate) fn prepare_pipeline<'a>(
    inputs: OperationInputs<'a>,
    op: &PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let request = request(op)?;
    let operation = request.operation();
    let mode = inputs.mode();
    let facts = match request {
        FilterRequest::GaussianBlur { sigma } => inputs.facts().with_gaussian_blur(sigma),
        FilterRequest::Filter3x3 { kernel, scale, offset } => {
            inputs.facts().with_filter3x3(&kernel, scale, offset)
        }
        FilterRequest::Filter5x5 { kernel, scale, offset } => {
            inputs.facts().with_filter5x5(&kernel, scale, offset)
        }
        FilterRequest::BoxBlur { radius } => inputs.facts().with_box_blur(radius),
        FilterRequest::BoxBlurXY { radius_x, radius_y, passes } => {
            inputs.facts().with_box_blur_xy(radius_x, radius_y, passes)
        }
        FilterRequest::MedianFilter { size }
        | FilterRequest::MaxFilter { size }
        | FilterRequest::MinFilter { size } => inputs.facts().with_filter_window(size, None),
        FilterRequest::RankFilter { size, rank } => {
            inputs.facts().with_filter_window(size, Some(rank))
        }
        _ => inputs.facts(),
    };
    let selection = match operation.support().for_mode(mode).assess(ctx, facts)? {
        SupportAssessment::Ready(selection) => selection,
        SupportAssessment::Rejected { reason, kind, .. } => return Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => {
            return Err(PilError::InternalError(
                "materialized filter request has unresolved support facts".into(),
            ));
        }
    };
    let plan = PreparedFilter {
        image: inputs.image,
        mode,
        request,
        selection,
        context: *ctx,
    };
    Ok(crate::compute::registry::PreparedOperation::from_filter(plan))
}
