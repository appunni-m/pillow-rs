//! Mode semantics composed with context-selected concrete architecture strategies.

use crate::compute::{Architecture, ExecutionContext, registry::support::{
    ImplementationSupport, ModeSupport, OperationId, OutputModeRule, StrategyGuard,
    StrategyInputs, PaletteKnowledge, SupportAssessment,
}};
use crate::compute::pool_cpu::helpers::execution::prepared::Prepared;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use super::default::grayscale_impl;

macro_rules! declare_architectures {
    ($($variant:ident, $cfg:meta, $requirements:expr, $guard:ident, $run:expr, $path:literal);+ $(;)?) => {
        pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
            OperationId::Grayscale, Mode::ImageYCbCr8,
            OutputModeRule::Fixed(Mode::ImageLuma8), &[], &[
                $(ImplementationSupport::cpu(OperationId::Grayscale, Mode::ImageYCbCr8,
                    Architecture::$variant, cfg!($cfg), $requirements,
                    StrategyGuard::$guard { channels: 3 }),)+
            ],
        );

        pub(crate) enum ArchitectureStrategy<'a> {
            Default(grayscale_impl::Kernel<'a>),
            $(#[cfg($cfg)] $variant(grayscale_impl::Kernel<'a>),)+
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
            let selection = SUPPORT.select(ctx, facts)?;
            let input = grayscale_impl::prepare(input);
            let implementation = match selection.implementation.architecture {
                Architecture::Default => ArchitectureStrategy::Default(input),
                $(#[cfg($cfg)] Architecture::$variant => ArchitectureStrategy::$variant(input),)+
                _ => return Err(PilError::InternalError("unbound grayscale strategy".into())),
            };
            Ok(Prepared::new(implementation, selection))
        }

        pub(crate) fn execute(strategy: Strategy<'_>) -> Result<DynamicImage, PilError> {
                let output = match strategy.implementation {
                    ArchitectureStrategy::Default(input) => Ok(grayscale_impl::execute(input)),
                    $(#[cfg($cfg)] ArchitectureStrategy::$variant(input) => {
                        let width = input.width();
                        let height = input.height();
                        super::super::helpers::luma::vector_output(width, height, ($run)(input), $path)
                    },)+
                }?;
                Ok(DynamicImage::ImageLuma8(output))
        }
    };
}

declare_architectures! {
    Ssse3, any(target_arch = "x86_64", all(target_arch = "x86", target_feature = "ssse3")),
        &[Architecture::Sse2, Architecture::Ssse3], CheckedNativeBytes,
        |input: grayscale_impl::Kernel<'_>| {
            #[cfg(target_arch = "x86_64")]
            let output = {
                // SAFETY: preparation checked SSE2 and SSSE3; the kernel bounds
                // every load by complete source triples or its padded tail.
                #[allow(unsafe_code)]
                unsafe { super::ssse3::grayscale_impl::execute(input.as_raw()) }
            };
            #[cfg(target_arch = "x86")]
            let output = super::ssse3::grayscale_wide_impl::execute(input);
            output
        }, "ssse3-deinterleave";
    Sse2, all(target_arch = "x86", target_feature = "sse2", not(target_feature = "ssse3")),
        &[Architecture::Sse2], CheckedNativeBytes,
        super::sse2::grayscale_impl::execute, "vector-gather";
    WasmSimd128, all(target_arch = "wasm32", target_feature = "simd128"),
        &[Architecture::WasmSimd128], CheckedNativeBytes,
        super::wasm_simd128::grayscale_impl::execute, "vector-gather";
    Neon, all(target_arch = "aarch64", target_feature = "neon"), &[Architecture::Neon], CheckedNativeBytes,
        |input: grayscale_impl::Kernel<'_>| {
                    super::neon::grayscale_impl::execute(input.as_raw())
        }, "neon-deinterleave";
}
