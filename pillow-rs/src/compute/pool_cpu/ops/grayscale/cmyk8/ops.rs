//! CMYK semantics composed with declared concrete architecture strategies.

use crate::compute::{Architecture, ExecutionContext, registry::support::{
    ImplementationSupport, ModeSupport, OperationId, OutputModeRule, PaletteKnowledge,
    StrategyGuard, StrategyInputs, SupportAssessment,
}};
use crate::compute::pool_cpu::helpers::execution::prepared::Prepared;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use super::default::grayscale_impl;

macro_rules! declare_architectures {
    ($($variant:ident, $module:ident, $cfg:meta, $requirements:expr, $prepare:path);+ $(;)?) => {
        pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
            OperationId::Grayscale, Mode::ImageCmyk8,
            OutputModeRule::Fixed(Mode::ImageLuma8), grayscale_impl::INPUTS, &[
                $(ImplementationSupport::cpu(OperationId::Grayscale, Mode::ImageCmyk8,
                    Architecture::$variant, cfg!($cfg), $requirements,
                    StrategyGuard::NativeBytes { channels: 4 }),)+
            ],
        );

        pub(crate) enum ArchitectureStrategy<'a> {
            Default(grayscale_impl::Kernel<'a>),
            $(#[cfg($cfg)] $variant(super::$module::grayscale_impl::Kernel<'a>),)+
        }
        pub(crate) type Strategy<'a> = Prepared<ArchitectureStrategy<'a>>;

        pub(crate) fn assess(input: StrategyInputs, ctx: &ExecutionContext)
            -> Result<SupportAssessment, PilError> {
            SUPPORT.assess(ctx, input)
        }

        pub(crate) fn prepare<'a>(input: &'a grayscale_impl::Input, ctx: &ExecutionContext)
            -> Result<Strategy<'a>, PilError> {
            let facts = StrategyInputs { width: input.width(), height: input.height(),
                byte_len: Some(input.as_raw().len()), palette: PaletteKnowledge::Unknown, request: Default::default() };
            let input = grayscale_impl::prepare(input)?;
            let selection = SUPPORT.select(ctx, facts)?;
            let implementation = match selection.implementation.architecture {
                Architecture::Default => ArchitectureStrategy::Default(input),
                $(#[cfg($cfg)] Architecture::$variant => ArchitectureStrategy::$variant(($prepare)(input)),)+
                _ => return Err(PilError::InternalError("unbound CMYK grayscale strategy".into())),
            };
            Ok(Prepared::new(implementation, selection))
        }

        pub(crate) fn execute(strategy: Strategy<'_>) -> Result<DynamicImage, PilError> {
                let output = match strategy.implementation {
                    ArchitectureStrategy::Default(input) => grayscale_impl::execute(input),
                    $(#[cfg($cfg)] ArchitectureStrategy::$variant(input) => {
                        let (width, height) = super::$module::grayscale_impl::dimensions(&input);
                        super::super::helpers::luma::vector_output(width, height,
                            super::$module::grayscale_impl::execute(input), "vector")
                    },)+
                }?;
                Ok(DynamicImage::ImageLuma8(output))
        }
    };
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
fn prepare_neon(input: grayscale_impl::Kernel<'_>) -> super::neon::grayscale_impl::Kernel<'_> {
    // The target statically guarantees NEON; the context-selected declaration
    // constructs the token once, before the original pixel loops run.
    let neon = fearless_simd::Neon::assume_supported();
    super::neon::grayscale_impl::prepare(input, neon)
}

declare_architectures! {
    Avx2, avx2, all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "avx2"),
        if cfg!(target_feature = "ssse3") { &[Architecture::Sse2, Architecture::Ssse3, Architecture::Avx2] } else { &[Architecture::Sse2, Architecture::Avx2] }, super::avx2::grayscale_impl::prepare;
    Ssse3, ssse3, all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "ssse3", not(target_feature = "avx2")),
        &[Architecture::Sse2, Architecture::Ssse3], super::ssse3::grayscale_impl::prepare;
    Sse2, sse2, all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2", not(target_feature = "ssse3"), not(target_feature = "avx2")),
        &[Architecture::Sse2], super::sse2::grayscale_impl::prepare;
    WasmSimd128, wasm_simd128, all(target_arch = "wasm32", target_feature = "simd128"),
        &[Architecture::WasmSimd128], super::wasm_simd128::grayscale_impl::prepare;
    Neon, neon, all(target_arch = "aarch64", target_feature = "neon"),
        &[Architecture::Neon], prepare_neon;
}
