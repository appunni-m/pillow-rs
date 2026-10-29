//! Exact i16l crop semantics and its declared architecture strategy.

use crate::compute::{Architecture, ExecutionContext, registry::support::{
    ImplementationSupport, ModeSupport, OperationId, PaletteKnowledge, RequestFacts,
    StrategyInputs, StrategyGuard, SupportAssessment, OutputModeRule,
}};
use crate::error::PilError;
use crate::raster::{DynamicImage, GenericImageView, Mode};
use crate::compute::pool_cpu::helpers::execution::prepared::Prepared;

use super::{default, neon, sse2, wasm_simd128};
use super::super::helpers::{crop::CropRegion, strategy::luma16_copy_architectures};

static ARCHITECTURES: [ImplementationSupport; 3] =
    luma16_copy_architectures(
        OperationId::Crop,
        Mode::ImageI16L,
        StrategyGuard::NativeCropBytes { channels: 2 },
    );

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Crop,
    Mode::ImageI16L,
    OutputModeRule::Preserve,
    default::crop_impl::INPUTS,
    &ARCHITECTURES,
);

pub(crate) enum ArchitectureStrategy<'a> {
    Default(default::crop_impl::Crop<'a>),
    Sse2 { image: &'a default::crop_impl::Input, region: CropRegion },
    Neon { image: &'a default::crop_impl::Input, region: CropRegion },
    WasmSimd128 { image: &'a default::crop_impl::Input, region: CropRegion },
}

pub(crate) type Strategy<'a> = Prepared<ArchitectureStrategy<'a>>;

pub(crate) fn assess(region: CropRegion, ctx: &ExecutionContext) -> Result<SupportAssessment, PilError> {
    SUPPORT.assess(ctx, StrategyInputs {
        width: region.source_width,
        height: region.source_height,
        byte_len: None,
        palette: PaletteKnowledge::Unknown,
        request: RequestFacts::Unspecified,
    }
    .with_crop_region(
        region.left,
        region.top,
        region.width,
        region.height,
        region.source_width,
        region.source_height,
    ))
}

pub(crate) fn prepare<'a>(
    image: &'a default::crop_impl::Input,
    region: CropRegion,
    ctx: &ExecutionContext,
) -> Result<Strategy<'a>, PilError> {
    let inputs = StrategyInputs {
        width: image.width(),
        height: image.height(),
        byte_len: Some(std::mem::size_of_val(image.as_raw().as_slice())),
        palette: PaletteKnowledge::Unknown,
        request: RequestFacts::Unspecified,
    }
    .with_crop_region(
        region.left,
        region.top,
        region.width,
        region.height,
        image.width(),
        image.height(),
    );
    let selection = SUPPORT.select(ctx, inputs)?;
    let implementation = match selection.implementation.architecture {
        Architecture::Default => ArchitectureStrategy::Default(
            default::crop_impl::prepare(image, region),
        ),
        Architecture::Sse2 => ArchitectureStrategy::Sse2 { image, region },
        Architecture::Neon => ArchitectureStrategy::Neon { image, region },
        Architecture::WasmSimd128 => ArchitectureStrategy::WasmSimd128 { image, region },
        architecture => return Err(PilError::InternalError(format!(
            "unexpected {architecture:?} architecture selected for I;16L crop"
        ))),
    };
    Ok(Prepared::new(implementation, selection))
}

pub(crate) fn execute(strategy: Strategy<'_>) -> Result<DynamicImage, PilError> {
    match strategy.implementation {
        ArchitectureStrategy::Default(implementation) => {
            Ok(default::crop_impl::execute(implementation))
        }
        ArchitectureStrategy::Sse2 { image, region } => {
            sse2::crop_impl::execute(image, region).map(DynamicImage::ImageI16L)
        }
        ArchitectureStrategy::Neon { image, region } => {
            neon::crop_impl::execute(image, region).map(DynamicImage::ImageI16L)
        }
        ArchitectureStrategy::WasmSimd128 { image, region } => {
            wasm_simd128::crop_impl::execute(image, region).map(DynamicImage::ImageI16L)
        }
    }
}
