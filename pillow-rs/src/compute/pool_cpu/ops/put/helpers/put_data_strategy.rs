//! Linked source-mode, payload-layout, and concrete native-copy strategies.

macro_rules! declare_strategies {
    ($mode:ident, $channels:literal, $payloads:expr; $(
        $variant:ident, $module:ident, $cfg:meta, $requires:expr
    );* $(;)?) => {
        use crate::compute::{Architecture as DataArch, ExecutionContext as DataContext};
        use crate::compute::registry::support::{
            InputContract as DataContract, ModeSupport as DataSupport,
            OperationId as DataOperation, OutputModeRule as DataOutput,
            StrategyInputs as DataFacts, SupportAssessment as DataAssessment,
        };
        use crate::compute::pool_cpu::helpers::execution::prepared::Prepared as PreparedData;
        use super::default::put_data_impl as data_scalar;

        pub(crate) const PUT_DATA_SUPPORT: DataSupport = DataSupport::cpu(
            DataOperation::PutData, crate::raster::Mode::$mode,
            DataOutput::ByPayload(&super::super::helpers::put_data::outputs(
                crate::raster::Mode::$mode, $channels,
            )),
            &[DataContract::ByteDataPayload {
                modes: super::super::helpers::put_data::PAYLOAD_MODES,
                rejection_reason: super::super::helpers::put_data::UNSUPPORTED_PAYLOAD,
            }],
            &[$(crate::compute::registry::support::ImplementationSupport::cpu(
                DataOperation::PutData, crate::raster::Mode::$mode,
                DataArch::$variant, cfg!($cfg), $requires,
                crate::compute::registry::support::StrategyGuard::PutDataNative {
                    channels: $channels, payloads: $payloads,
                },
            ),)*],
        );

        pub(crate) enum DataArchitectureStrategy<'a> {
            Default(data_scalar::Kernel<'a>),
            $(#[cfg($cfg)] $variant(super::$module::put_data_impl::Kernel<'a>),)*
        }
        pub(crate) type DataStrategy<'a> = PreparedData<DataArchitectureStrategy<'a>>;

        pub(crate) fn assess_data(input: DataFacts, ctx: &DataContext)
            -> Result<DataAssessment, crate::error::PilError> {
            PUT_DATA_SUPPORT.assess(ctx, input)
        }

        #[inline]
        pub(crate) fn prepare_data<'a>(
            input: &'a data_scalar::Input, data: &'a [u8], mode: crate::raster::Mode,
            ctx: &DataContext,
        ) -> Result<DataStrategy<'a>, crate::error::PilError> {
            let payload = super::super::helpers::put_data::PayloadLayout::new(mode)?;
            let facts = super::super::helpers::put_data::facts(input, data, mode);
            let selection = PUT_DATA_SUPPORT.select(ctx, facts)?;
            let input = data_scalar::prepare(input, data, payload);
            let implementation = match selection.implementation.architecture {
                DataArch::Default => DataArchitectureStrategy::Default(input),
                $(#[cfg($cfg)] DataArch::$variant => DataArchitectureStrategy::$variant(
                    super::$module::put_data_impl::prepare(input)
                ),)*
                _ => return Err(crate::error::PilError::InternalError(
                    "unbound PutData strategy".into(),
                )),
            };
            Ok(PreparedData::new(implementation, selection))
        }

        impl DataStrategy<'_> {
            #[inline]
            pub(crate) fn execute(self) -> Result<crate::raster::DynamicImage, crate::error::PilError> {
                match self.implementation {
                    DataArchitectureStrategy::Default(input) => data_scalar::execute(input),
                    $(#[cfg($cfg)] DataArchitectureStrategy::$variant(input) =>
                        super::$module::put_data_impl::execute(input),)*
                }
            }
        }
    };
}
pub(crate) use declare_strategies;
