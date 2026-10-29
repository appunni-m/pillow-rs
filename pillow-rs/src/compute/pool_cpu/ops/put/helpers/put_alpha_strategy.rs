//! One declaration binds scalar alpha semantics to concrete architecture owners.

macro_rules! declare_strategies {
    ($mode:ident, $output:ident, $channels:literal, $minimum:literal; $(
        $variant:ident, $module:ident, $cfg:meta, $requires:expr
    );* $(;)?) => {
        use crate::compute::{Architecture as AlphaArch, ExecutionContext as AlphaContext};
        use crate::compute::registry::support::{
            ModeSupport as AlphaSupport, OperationId as AlphaOperation,
            OutputModeRule as AlphaOutput, StrategyInputs as AlphaFacts,
            SupportAssessment as AlphaAssessment,
        };
        use crate::compute::pool_cpu::helpers::execution::prepared::Prepared as PreparedAlpha;
        use super::default::put_alpha_impl as alpha_scalar;

        pub(crate) const PUT_ALPHA_SUPPORT: AlphaSupport = AlphaSupport::cpu(
            AlphaOperation::PutAlpha, crate::raster::Mode::$mode,
            AlphaOutput::Fixed(crate::raster::Mode::$output), &[],
            &[$(crate::compute::registry::support::ImplementationSupport::cpu(
                AlphaOperation::PutAlpha, crate::raster::Mode::$mode,
                AlphaArch::$variant, cfg!($cfg), $requires,
                crate::compute::registry::support::StrategyGuard::PutAlphaBytes {
                    channels: $channels, minimum_pixels: $minimum,
                },
            ),)*],
        );

        pub(crate) enum AlphaArchitectureStrategy<'a> {
            Default(alpha_scalar::Kernel<'a>),
            $(#[cfg($cfg)] $variant(super::$module::put_alpha_impl::Kernel<'a>),)*
        }
        pub(crate) type AlphaStrategy<'a> = PreparedAlpha<AlphaArchitectureStrategy<'a>>;

        pub(crate) fn assess_alpha(input: AlphaFacts, ctx: &AlphaContext)
            -> Result<AlphaAssessment, crate::error::PilError> {
            PUT_ALPHA_SUPPORT.assess(ctx, input)
        }

        #[inline]
        pub(crate) fn prepare_alpha<'a>(
            image: &'a alpha_scalar::Input, alpha: u8, ctx: &AlphaContext,
        ) -> Result<AlphaStrategy<'a>, crate::error::PilError> {
            let input = alpha_scalar::prepare(image, alpha);
            let selection = PUT_ALPHA_SUPPORT.select(
                ctx, super::super::helpers::put_alpha::facts(&input),
            )?;
            let implementation = match selection.implementation.architecture {
                AlphaArch::Default => AlphaArchitectureStrategy::Default(input),
                $(#[cfg($cfg)] AlphaArch::$variant => AlphaArchitectureStrategy::$variant(
                    super::$module::put_alpha_impl::prepare(input)
                ),)*
                _ => return Err(crate::error::PilError::InternalError(
                    "unbound PutAlpha strategy".into(),
                )),
            };
            Ok(PreparedAlpha::new(implementation, selection))
        }

        impl AlphaStrategy<'_> {
            #[inline]
            pub(crate) fn execute(self) -> Result<crate::raster::DynamicImage, crate::error::PilError> {
                match self.implementation {
                    AlphaArchitectureStrategy::Default(input) => alpha_scalar::execute(input),
                    $(#[cfg($cfg)] AlphaArchitectureStrategy::$variant(input) =>
                        super::$module::put_alpha_impl::execute(input),)*
                }
            }
        }
    };
}
pub(crate) use declare_strategies;
