//! Linked source and mask strategies for the original image-backed alpha operation.

macro_rules! declare_strategies {
    ($mode:ident, $output:ident, $channels:literal, $minimum:literal; $(
        $variant:ident, $module:ident, $cfg:meta, $requires:expr
    );* $(;)?) => {
        use crate::compute::{Architecture as MaskArch, ExecutionContext as MaskContext};
        use crate::compute::registry::support::{
            InputContract as MaskContract, ModeSupport as MaskSupport,
            OperationId as MaskOperation, OutputModeRule as MaskOutput,
            StrategyInputs as MaskFacts, SupportAssessment as MaskAssessment,
        };
        use crate::compute::pool_cpu::helpers::execution::prepared::Prepared as PreparedMask;
        use super::default::put_alpha_data_impl as mask_scalar;

        pub(crate) const PUT_ALPHA_DATA_SUPPORT: MaskSupport = MaskSupport::cpu(
            MaskOperation::PutAlphaData, crate::raster::Mode::$mode,
            MaskOutput::Fixed(crate::raster::Mode::$output),
            &[MaskContract::AlphaMaskLumaPrefix {
                modes: &crate::raster::Mode::ALL, normalized: crate::raster::Mode::ImageLuma8,
            }],
            &[$(crate::compute::registry::support::ImplementationSupport::cpu(
                MaskOperation::PutAlphaData, crate::raster::Mode::$mode,
                MaskArch::$variant, cfg!($cfg), $requires,
                crate::compute::registry::support::StrategyGuard::PutAlphaMaskBytes {
                    channels: $channels, minimum_pixels: $minimum,
                },
            ),)*],
        );

        pub(crate) enum AlphaDataArchitectureStrategy<'a> {
            Default(mask_scalar::Kernel<'a>),
            $(#[cfg($cfg)] $variant(super::$module::put_alpha_data_impl::Kernel<'a>),)*
        }
        pub(crate) type AlphaDataStrategy<'a> = PreparedMask<AlphaDataArchitectureStrategy<'a>>;

        pub(crate) fn assess_alpha_data(input: MaskFacts, ctx: &MaskContext)
            -> Result<MaskAssessment, crate::error::PilError> {
            PUT_ALPHA_DATA_SUPPORT.assess(ctx, input)
        }

        #[inline]
        pub(crate) fn prepare_alpha_data<'a>(
            image: &'a mask_scalar::Input,
            mask: super::super::helpers::alpha_mask::MaskStrategy<'a>, ctx: &MaskContext,
        ) -> Result<AlphaDataStrategy<'a>, crate::error::PilError> {
            let input = mask_scalar::prepare(image, mask);
            let selection = PUT_ALPHA_DATA_SUPPORT.select(
                ctx, super::super::helpers::put_alpha_data::facts(&input),
            )?;
            let implementation = match selection.implementation.architecture {
                MaskArch::Default => AlphaDataArchitectureStrategy::Default(input),
                $(#[cfg($cfg)] MaskArch::$variant => AlphaDataArchitectureStrategy::$variant(
                    super::$module::put_alpha_data_impl::prepare(input)?
                ),)*
                _ => return Err(crate::error::PilError::InternalError(
                    "unbound PutAlphaData strategy".into(),
                )),
            };
            Ok(PreparedMask::new(implementation, selection))
        }

        impl AlphaDataStrategy<'_> {
            #[inline]
            pub(crate) fn execute(self) -> Result<crate::raster::DynamicImage, crate::error::PilError> {
                match self.implementation {
                    AlphaDataArchitectureStrategy::Default(input) => mask_scalar::execute(input),
                    $(#[cfg($cfg)] AlphaDataArchitectureStrategy::$variant(input) =>
                        super::$module::put_alpha_data_impl::execute(input),)*
                }
            }
        }
    };
}
pub(crate) use declare_strategies;
