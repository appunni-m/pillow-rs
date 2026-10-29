//! One declaration binds native writes to concrete mode and architecture strategies.

macro_rules! declare_strategies {
    ($mode:ident, $channels:literal;
        single [$($pixel_variant:ident, $pixel_module:ident, $pixel_cfg:meta, $pixel_requires:expr;)*]
        batch [$($batch_variant:ident, $batch_module:ident, $batch_cfg:meta, $batch_requires:expr;)*]
    ) => {
        use crate::compute::{Architecture as WriteArch, ExecutionContext as WriteContext};
        use crate::compute::registry::support::{
            InputContract as WriteContract,
            ModeSupport as WriteSupport, OperationId as WriteOperation,
            OutputModeRule as WriteOutput,
            StrategyInputs as WriteFacts, SupportAssessment as WriteAssessment,
        };
        use crate::compute::pool_cpu::helpers::execution::prepared::Prepared as PreparedWrite;
        use super::default::put_pixel_impl as pixel_scalar;

        pub(crate) const PUT_PIXEL_SUPPORT: WriteSupport = WriteSupport::cpu(
            WriteOperation::PutPixel, crate::raster::Mode::$mode, WriteOutput::Preserve,
            &[WriteContract::CallerValidatedPixelCoordinates],
            &[$(crate::compute::registry::support::ImplementationSupport::cpu(
                WriteOperation::PutPixel, crate::raster::Mode::$mode,
                WriteArch::$pixel_variant, cfg!($pixel_cfg), $pixel_requires,
                crate::compute::registry::support::StrategyGuard::PutPixelSpan { channels: $channels },
            ),)*],
        );

        pub(crate) const PUT_PIXEL_BATCH_SUPPORT: WriteSupport = WriteSupport::cpu(
            WriteOperation::PutPixelBatch, crate::raster::Mode::$mode, WriteOutput::Preserve,
            &[WriteContract::OrderedPixelWrites],
            &[$(crate::compute::registry::support::ImplementationSupport::cpu(
                WriteOperation::PutPixelBatch, crate::raster::Mode::$mode,
                WriteArch::$batch_variant, cfg!($batch_cfg), $batch_requires,
                crate::compute::registry::support::StrategyGuard::RgbaPixelWriteRun,
            ),)*],
        );

        pub(crate) enum PixelArchitectureStrategy<'a> {
            Default(pixel_scalar::Kernel<'a>),
            $(#[cfg($pixel_cfg)]
                $pixel_variant(super::$pixel_module::put_pixel_impl::Kernel<'a>),)*
        }
        pub(crate) enum BatchArchitectureStrategy<'a> {
            Default(pixel_scalar::BatchKernel<'a>),
            $(#[cfg($batch_cfg)]
                $batch_variant(super::$batch_module::put_pixel_impl::BatchKernel<'a>),)*
        }
        pub(crate) type PixelStrategy<'a> = PreparedWrite<PixelArchitectureStrategy<'a>>;
        pub(crate) type BatchStrategy<'a> = PreparedWrite<BatchArchitectureStrategy<'a>>;

        pub(crate) fn assess_pixel(input: WriteFacts, ctx: &WriteContext)
            -> Result<WriteAssessment, crate::error::PilError> {
            PUT_PIXEL_SUPPORT.assess(ctx, input)
        }

        pub(crate) fn assess_pixel_batch(input: WriteFacts, ctx: &WriteContext)
            -> Result<WriteAssessment, crate::error::PilError> {
            PUT_PIXEL_BATCH_SUPPORT.assess(ctx, input)
        }

        #[inline]
        pub(crate) fn prepare_pixel<'a>(
            input: &'a pixel_scalar::Input,
            write: super::super::helpers::put_pixel::PixelWrite, ctx: &WriteContext,
        ) -> Result<PixelStrategy<'a>, crate::error::PilError> {
            let facts = super::super::helpers::put_pixel::facts(input, write);
            let selection = PUT_PIXEL_SUPPORT.select(ctx, facts)?;
            let input = pixel_scalar::prepare(input, write);
            let implementation = match selection.implementation.architecture {
                WriteArch::Default => PixelArchitectureStrategy::Default(input),
                $(#[cfg($pixel_cfg)] WriteArch::$pixel_variant =>
                    PixelArchitectureStrategy::$pixel_variant(
                        super::$pixel_module::put_pixel_impl::prepare(input)
                    ),)*
                _ => return Err(crate::error::PilError::InternalError(
                    "unbound PutPixel strategy".into(),
                )),
            };
            Ok(PreparedWrite::new(implementation, selection))
        }

        #[inline]
        pub(crate) fn prepare_pixel_batch<'a>(
            input: &'a pixel_scalar::Input, ops: &'a [crate::pipeline::PipelineOp],
            ctx: &WriteContext,
        ) -> Result<BatchStrategy<'a>, crate::error::PilError> {
            let facts = super::super::helpers::put_pixel::batch_facts(input, ops);
            // Inspect the run only for a declared, build/hardware/policy-eligible
            // specialization. Default preparation retains the original single pass.
            $(#[cfg($batch_cfg)]
            let facts = if ctx.supports_architecture(ctx.architecture())
                && ctx.permits_architecture(WriteArch::$batch_variant)
                && ($batch_requires).iter().all(|&arch| ctx.supports_architecture(arch)) {
                WriteFacts {
                    request: crate::compute::registry::support::RequestFacts::PutPixelBatch {
                        count: ops.len(),
                        writes: super::super::helpers::put_pixel::rgba_run_facts(input, ops),
                    },
                    ..facts
                }
            } else { facts };)*
            let selection = PUT_PIXEL_BATCH_SUPPORT.select(ctx, facts)?;
            let input = pixel_scalar::prepare_batch(input, ops);
            let implementation = match selection.implementation.architecture {
                WriteArch::Default => BatchArchitectureStrategy::Default(input),
                $(#[cfg($batch_cfg)] WriteArch::$batch_variant =>
                    BatchArchitectureStrategy::$batch_variant(
                        super::$batch_module::put_pixel_impl::prepare_batch(input)
                    ),)*
                _ => return Err(crate::error::PilError::InternalError(
                    "unbound PutPixel batch strategy".into(),
                )),
            };
            Ok(PreparedWrite::new(implementation, selection))
        }

        impl PixelStrategy<'_> {
            #[inline]
            pub(crate) fn execute(self) -> Result<crate::raster::DynamicImage, crate::error::PilError> {
                match self.implementation {
                    PixelArchitectureStrategy::Default(input) => pixel_scalar::execute(input),
                    $(#[cfg($pixel_cfg)] PixelArchitectureStrategy::$pixel_variant(input) =>
                        super::$pixel_module::put_pixel_impl::execute(input),)*
                }
            }
        }

        impl BatchStrategy<'_> {
            #[inline]
            pub(crate) fn execute(self) -> Result<crate::raster::DynamicImage, crate::error::PilError> {
                match self.implementation {
                    BatchArchitectureStrategy::Default(input) => pixel_scalar::execute_batch(input),
                    $(#[cfg($batch_cfg)] BatchArchitectureStrategy::$batch_variant(input) =>
                        super::$batch_module::put_pixel_impl::execute_batch(input),)*
                }
            }
        }
    };
}
pub(crate) use declare_strategies;
