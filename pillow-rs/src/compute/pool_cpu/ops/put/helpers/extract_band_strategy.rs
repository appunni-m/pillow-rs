//! One declaration binds band support, preparation, and concrete execution.

#[cfg(any(
    target_arch = "x86_64",
    all(target_arch = "x86", any(target_feature = "sse2", target_feature = "ssse3")),
    all(target_arch = "wasm32", target_feature = "simd128"),
    all(target_arch = "aarch64", target_feature = "neon"),
))]
macro_rules! execute_architecture {
    (safe, $execute:expr) => { $execute };
    (unsafe, $execute:expr) => {{
        // SAFETY: preparation selected this implementation from its declared
        // hardware/build requirements and exact native-layout/index guard.
        #[allow(unsafe_code)]
        unsafe { $execute }
    }};
}
#[cfg(any(
    target_arch = "x86_64",
    all(target_arch = "x86", any(target_feature = "sse2", target_feature = "ssse3")),
    all(target_arch = "wasm32", target_feature = "simd128"),
    all(target_arch = "aarch64", target_feature = "neon"),
))]
pub(crate) use execute_architecture;

macro_rules! declare_strategies {
    ($mode:ident, $rule:expr; $(
        $variant:ident, $module:ident, $cfg:meta, $requires:expr, $channels:literal, $safety:ident
    );* $(;)?) => {
        use crate::compute::{Architecture, ExecutionContext, registry::support::{
            InputContract, ModeSupport, OperationId, OutputModeRule, StrategyInputs, SupportAssessment,
        }};
        use crate::compute::pool_cpu::helpers::execution::prepared::Prepared;
        use crate::error::PilError;
        use crate::raster::{DynamicImage, Mode};
        use super::default::extract_band_impl as scalar;

        pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
            OperationId::ExtractBand, Mode::$mode, OutputModeRule::Fixed(Mode::ImageLuma8),
            &[
                InputContract::CheckedByteDimensions { bytes_per_pixel: 1, allow_empty: true },
                InputContract::ExtractBandIndex($rule),
            ],
            &[$(crate::compute::registry::support::ImplementationSupport::cpu(
                OperationId::ExtractBand, Mode::$mode, Architecture::$variant, cfg!($cfg),
                $requires, crate::compute::registry::support::StrategyGuard::ExtractBandBytes { channels: $channels },
            ),)*],
        );

        pub(crate) enum ArchitectureStrategy<'a> {
            Default(scalar::Kernel<'a>),
            $(#[cfg($cfg)] $variant(super::$module::extract_band_impl::Kernel<'a>),)*
        }
        pub(crate) type Strategy<'a> = Prepared<ArchitectureStrategy<'a>>;

        pub(crate) fn assess(input: StrategyInputs, ctx: &ExecutionContext)
            -> Result<SupportAssessment, PilError> {
            SUPPORT.assess(ctx, input)
        }

        #[inline]
        pub(crate) fn prepare<'a>(input: &'a scalar::Input, index: u8, ctx: &ExecutionContext)
            -> Result<Strategy<'a>, PilError> {
            let facts = super::super::helpers::extract_band::facts(input, index);
            let selection = SUPPORT.select(ctx, facts)?;
            let input = scalar::prepare(input, index)?;
            let implementation = match selection.implementation.architecture {
                Architecture::Default => ArchitectureStrategy::Default(input),
                $(#[cfg($cfg)] Architecture::$variant =>
                    ArchitectureStrategy::$variant(super::$module::extract_band_impl::prepare(input)),)*
                _ => return Err(PilError::InternalError("unbound ExtractBand strategy".into())),
            };
            Ok(Prepared::new(implementation, selection))
        }

        #[inline]
        pub(crate) fn execute(strategy: Strategy<'_>) -> Result<DynamicImage, PilError> {
                match strategy.implementation {
                    ArchitectureStrategy::Default(input) => scalar::execute(input),
                    $(#[cfg($cfg)] ArchitectureStrategy::$variant(input) =>
                        $crate::compute::pool_cpu::ops::put::helpers::extract_band_strategy::execute_architecture!(
                            $safety, super::$module::extract_band_impl::execute(input)
                        ),)*
                }
        }
    };
}
pub(crate) use declare_strategies;
