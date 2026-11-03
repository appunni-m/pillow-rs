//! CompositeModule mode support, preparation, and exact-mode execution.

use std::sync::Arc;

use crate::compute::{Architecture, ExecutionContext};
use crate::compute::registry::support::{
    ImplementationSupport, InputContract, ModeSupport, OperationId, OperationInputs,
    OperationSupport, OutputModeRule, StrategyGuard, StrategySelection,
};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

use super::CompositeLayout;
use super::{secondary_facts, select_support};

const COMPOSITE_LUMA_MASKS: &[Mode] = &[Mode::One, Mode::ImageLuma8];
const COMPOSITE_ALPHA_MASKS: &[Mode] = &[
    Mode::ImageLumaA8,
    Mode::ImageRgba8,
    Mode::ImageRgbPremultiplied8,
];
const COMPOSITE_INPUT: InputContract = InputContract::CompositeInputs {
    luma_masks: COMPOSITE_LUMA_MASKS,
    alpha_masks: COMPOSITE_ALPHA_MASKS,
    mode_rejection: "images do not match",
    mask_rejection: "bad transparency mask",
    size_rejection: "images do not match",
};
const COMPOSITE_INPUTS: &[InputContract] = &[COMPOSITE_INPUT];

const COMPOSITE_SSE2: &[Architecture] = &[Architecture::Sse2];
const COMPOSITE_NEON: &[Architecture] = &[Architecture::Neon];
const COMPOSITE_WASM_SIMD128: &[Architecture] = &[Architecture::WasmSimd128];

const fn composite_vector_architectures(
    mode: Mode,
    channels: u8,
) -> [ImplementationSupport; 3] {
    [
        ImplementationSupport::cpu(
            OperationId::CompositeModule,
            mode,
            Architecture::Sse2,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "sse2"
            )),
            COMPOSITE_SSE2,
            StrategyGuard::CompositeNativeByteVector { mode, channels },
        ),
        ImplementationSupport::cpu(
            OperationId::CompositeModule,
            mode,
            Architecture::Neon,
            cfg!(all(target_arch = "aarch64", target_feature = "neon")),
            COMPOSITE_NEON,
            StrategyGuard::CompositeNativeByteVector { mode, channels },
        ),
        ImplementationSupport::cpu(
            OperationId::CompositeModule,
            mode,
            Architecture::WasmSimd128,
            cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
            COMPOSITE_WASM_SIMD128,
            StrategyGuard::CompositeNativeByteVector { mode, channels },
        ),
    ]
}

macro_rules! composite_architectures {
    ($name:ident, $mode:ident, $channels:literal) => {
        static $name: [ImplementationSupport; 3] =
            composite_vector_architectures(Mode::$mode, $channels);
    };
}

macro_rules! composite_l_mask_architectures {
    ($name:ident, $mode:ident, $channels:literal, $minimum_width:literal) => {
        static $name: [ImplementationSupport; 4] = [
            ImplementationSupport::cpu(
                OperationId::CompositeModule,
                Mode::$mode,
                Architecture::Sse2,
                cfg!(all(
                    any(target_arch = "x86", target_arch = "x86_64"),
                    target_feature = "sse2"
                )),
                COMPOSITE_SSE2,
                StrategyGuard::CompositeNativeByteVector {
                    mode: Mode::$mode,
                    channels: $channels,
                },
            ),
            ImplementationSupport::cpu(
                OperationId::CompositeModule,
                Mode::$mode,
                Architecture::Neon,
                cfg!(all(target_arch = "aarch64", target_feature = "neon")),
                COMPOSITE_NEON,
                StrategyGuard::CompositeLMaskNeon {
                    mode: Mode::$mode,
                    channels: $channels,
                    minimum_clipped_width: $minimum_width,
                },
            ),
            ImplementationSupport::cpu(
                OperationId::CompositeModule,
                Mode::$mode,
                Architecture::Neon,
                cfg!(all(target_arch = "aarch64", target_feature = "neon")),
                COMPOSITE_NEON,
                StrategyGuard::CompositeNativeByteVector {
                    mode: Mode::$mode,
                    channels: $channels,
                },
            ),
            ImplementationSupport::cpu(
                OperationId::CompositeModule,
                Mode::$mode,
                Architecture::WasmSimd128,
                cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
                COMPOSITE_WASM_SIMD128,
                StrategyGuard::CompositeNativeByteVector {
                    mode: Mode::$mode,
                    channels: $channels,
                },
            ),
        ];
    };
}

composite_architectures!(COMPOSITE_ONE_ARCHITECTURES, One, 1);
composite_l_mask_architectures!(COMPOSITE_LUMA_ARCHITECTURES, ImageLuma8, 1, 16);
composite_architectures!(COMPOSITE_PALETTE_ARCHITECTURES, ImagePalette8, 1);
composite_l_mask_architectures!(COMPOSITE_LUMA_ALPHA_ARCHITECTURES, ImageLumaA8, 2, 8);
composite_l_mask_architectures!(COMPOSITE_PALETTE_ALPHA_ARCHITECTURES, ImagePaletteAlpha8, 2, 1);
composite_l_mask_architectures!(COMPOSITE_RGB_ARCHITECTURES, ImageRgb8, 3, 16);
composite_l_mask_architectures!(COMPOSITE_HSV_ARCHITECTURES, ImageHsv8, 3, 16);
composite_architectures!(COMPOSITE_YCBCR_ARCHITECTURES, ImageYCbCr8, 3);
composite_l_mask_architectures!(COMPOSITE_RGBA_ARCHITECTURES, ImageRgba8, 4, 4);
composite_l_mask_architectures!(COMPOSITE_CMYK_ARCHITECTURES, ImageCmyk8, 4, 16);
composite_architectures!(COMPOSITE_RGBX_ARCHITECTURES, ImageRgbx8, 4);
composite_architectures!(COMPOSITE_RGB_PREMULTIPLIED_ARCHITECTURES, ImageRgbPremultiplied8, 4);
composite_architectures!(COMPOSITE_I32_ARCHITECTURES, ImageI32, 4);
composite_architectures!(COMPOSITE_F32_ARCHITECTURES, ImageF32, 4);

fn composite_support(mode: Mode) -> ModeSupport {
    let architectures = match mode {
        Mode::One => &COMPOSITE_ONE_ARCHITECTURES[..],
        Mode::ImageLuma8 => &COMPOSITE_LUMA_ARCHITECTURES[..],
        Mode::ImagePalette8 => &COMPOSITE_PALETTE_ARCHITECTURES[..],
        Mode::ImageLumaA8 => &COMPOSITE_LUMA_ALPHA_ARCHITECTURES[..],
        Mode::ImagePaletteAlpha8 => &COMPOSITE_PALETTE_ALPHA_ARCHITECTURES[..],
        Mode::ImageRgb8 => &COMPOSITE_RGB_ARCHITECTURES[..],
        Mode::ImageHsv8 => &COMPOSITE_HSV_ARCHITECTURES[..],
        Mode::ImageYCbCr8 => &COMPOSITE_YCBCR_ARCHITECTURES[..],
        Mode::ImageRgba8 => &COMPOSITE_RGBA_ARCHITECTURES[..],
        Mode::ImageCmyk8 => &COMPOSITE_CMYK_ARCHITECTURES[..],
        Mode::ImageRgbx8 => &COMPOSITE_RGBX_ARCHITECTURES[..],
        Mode::ImageRgbPremultiplied8 => &COMPOSITE_RGB_PREMULTIPLIED_ARCHITECTURES[..],
        Mode::ImageI32 => &COMPOSITE_I32_ARCHITECTURES[..],
        Mode::ImageF32 => &COMPOSITE_F32_ARCHITECTURES[..],
        Mode::ImageLaPremultiplied8
        | Mode::ImageLab8
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N => &[],
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => {
            // NO_PILLOW_SUPPORT: internal high-depth raster representations
            // use native samples and have no Pillow mode identity.
            return ModeSupport::cpu_core_only(
                OperationId::CompositeModule,
                mode,
                OutputModeRule::Preserve,
                COMPOSITE_INPUTS,
                &[],
                "this exact high-depth raster identity is a core extension",
            );
        }
    };
    ModeSupport::cpu(
        OperationId::CompositeModule,
        mode,
        OutputModeRule::Preserve,
        COMPOSITE_INPUTS,
        architectures,
    )
}

/// Complete exact-mode support contract for Image.composite.
pub(crate) static COMPOSITE_MODULE_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::CompositeModule, COMPOSITE_INPUTS, composite_support);

/// Validates the eager wrapper's size check from the same contract used by
/// prepared Composite admission.
pub(crate) fn validate_composite_wrapper_dimensions(
    source_size: (u32, u32),
    mask_size: (u32, u32),
) -> Result<(), PilError> {
    let InputContract::CompositeInputs { size_rejection, .. } = COMPOSITE_INPUT else {
        return Err(PilError::InternalError(
            "Composite support omitted its input contract".into(),
        ));
    };
    if source_size != mask_size {
        return Err(PilError::ValueError(size_rejection.into()));
    }
    Ok(())
}

/// Selects luma or alpha mask samples from Composite's declared mode sets.
pub(crate) fn composite_wrapper_mask_alpha(mask_mode: Mode) -> Result<bool, PilError> {
    let InputContract::CompositeInputs { luma_masks, alpha_masks, mask_rejection, .. } =
        COMPOSITE_INPUT
    else {
        return Err(PilError::InternalError(
            "Composite support omitted its mask contract".into(),
        ));
    };
    if luma_masks.contains(&mask_mode) {
        return Ok(false);
    }
    if alpha_masks.contains(&mask_mode) {
        return Ok(true);
    }
    Err(PilError::ValueError(mask_rejection.into()))
}

pub(crate) struct PreparedCompositeModule<'a> {
    source: &'a DynamicImage,
    destination: Arc<DynamicImage>,
    mask: Arc<DynamicImage>,
    mask_alpha: bool,
    mode: Mode,
    selection: StrategySelection,
}

impl PreparedCompositeModule<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        execute_composite_mode(
            self.mode,
            self.source,
            &self.destination,
            &self.mask,
            self.mask_alpha,
            &self.selection,
        )
    }
}

pub(crate) fn prepare_composite_module<'a>(
    input: OperationInputs<'a>,
    destination: Arc<DynamicImage>,
    mask: Arc<DynamicImage>,
    mask_alpha: bool,
    ctx: &ExecutionContext,
) -> Result<PreparedCompositeModule<'a>, PilError> {
    let facts = input.facts().with_composite_inputs(
        input.mode(),
        secondary_facts(&destination),
        secondary_facts(&mask),
        mask_alpha,
    );
    let selection = select_support(composite_support(input.mode()), facts, ctx, "Composite")?;
    Ok(PreparedCompositeModule {
        source: input.image,
        destination,
        mask,
        mask_alpha,
        mode: input.mode(),
        selection,
    })
}

pub(crate) fn prepare_composite_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::CompositeModule { other, mask, mask_alpha } = op else {
        return Err(PilError::ValueError("expected CompositeModule op".into()));
    };
    // The archived executor resolves the mask before the destination image;
    // keep that ordering when either lazy input fails to materialize.
    let mask = mask.materialized_shared()?;
    let destination = other.materialized_shared()?;
    prepare_composite_module(
        input,
        destination,
        mask,
        *mask_alpha,
        ctx,
    )
    .map(crate::compute::registry::PreparedOperation::from_composite_module)
}

fn execute_composite_mode(
    mode: Mode,
    source: &DynamicImage,
    destination: &DynamicImage,
    mask: &DynamicImage,
    mask_alpha: bool,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    match mode {
        Mode::One => super::super::one::ops::composite(source, destination, mask, mask_alpha, selection),
        Mode::ImageLuma8 => super::super::luma8::ops::composite(source, destination, mask, mask_alpha, selection),
        Mode::ImageLumaA8 => super::super::luma_a8::ops::composite(source, destination, mask, mask_alpha, selection),
        Mode::ImageLaPremultiplied8 => super::super::la_premultiplied8::ops::composite(source, destination, mask, mask_alpha, selection),
        Mode::ImagePalette8 => super::super::palette8::ops::composite(source, destination, mask, mask_alpha, selection),
        Mode::ImagePaletteAlpha8 => super::super::palette_alpha8::ops::composite(source, destination, mask, mask_alpha, selection),
        Mode::ImageRgb8 => super::super::rgb8::ops::composite(source, destination, mask, mask_alpha, selection),
        Mode::ImageRgba8 => super::super::rgba8::ops::composite(source, destination, mask, mask_alpha, selection),
        Mode::ImageRgbPremultiplied8 => super::super::rgb_premultiplied8::ops::composite(source, destination, mask, mask_alpha, selection),
        Mode::ImageRgbx8 => super::super::rgbx8::ops::composite(source, destination, mask, mask_alpha, selection),
        Mode::ImageCmyk8 => super::super::cmyk8::ops::composite(source, destination, mask, mask_alpha, selection),
        Mode::ImageYCbCr8 => super::super::ycbcr8::ops::composite(source, destination, mask, mask_alpha, selection),
        Mode::ImageLab8 => super::super::lab8::ops::composite(source, destination, mask, mask_alpha, selection),
        Mode::ImageHsv8 => super::super::hsv8::ops::composite(source, destination, mask, mask_alpha, selection),
        Mode::ImageI32 => super::super::i32::ops::composite(source, destination, mask, mask_alpha, selection),
        Mode::ImageF32 => super::super::f32::ops::composite(source, destination, mask, mask_alpha, selection),
        Mode::ImageI16 => super::super::i16::ops::composite(source, destination, mask, mask_alpha, selection),
        Mode::ImageI16L => super::super::i16l::ops::composite(source, destination, mask, mask_alpha, selection),
        Mode::ImageI16B => super::super::i16b::ops::composite(source, destination, mask, mask_alpha, selection),
        Mode::ImageI16N => super::super::i16n::ops::composite(source, destination, mask, mask_alpha, selection),
        Mode::ImageLumaA16 => super::super::luma_a16::ops::composite(source, destination, mask, mask_alpha, selection),
        Mode::ImageRgb16 => super::super::rgb16::ops::composite(source, destination, mask, mask_alpha, selection),
        Mode::ImageRgba16 => super::super::rgba16::ops::composite(source, destination, mask, mask_alpha, selection),
        Mode::ImageRgb32F => super::super::rgb32f::ops::composite(source, destination, mask, mask_alpha, selection),
        Mode::ImageRgba32F => super::super::rgba32f::ops::composite(source, destination, mask, mask_alpha, selection),
    }
}
