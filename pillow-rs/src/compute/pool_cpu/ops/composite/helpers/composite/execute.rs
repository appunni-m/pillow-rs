//! Composite dispatch and validated mask sample preparation.

use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use crate::compute::Architecture;
use crate::compute::registry::support::{OperationId, StrategySelection};
use crate::compute::pool_cpu::ops::composite::ops::CompositeLayout;

/// Executes the selected byte-mode architecture through that exact Paste mode strategy.
pub(crate) fn execute_byte_selected<'a, F>(
    source: &'a DynamicImage,
    destination: &'a DynamicImage,
    mask: &'a DynamicImage,
    mask_alpha: bool,
    mode: Mode,
    channels: u8,
    selection: &StrategySelection,
    execute_architecture: F,
) -> Result<DynamicImage, PilError>
where
    F: FnOnce(
        crate::compute::pool_cpu::ops::paste::helpers::plan::BytePlan<
            'a,
            &'a DynamicImage,
        >,
    ) -> Result<DynamicImage, PilError>,
{
    super::super::strategy::require_operation_mode(
        selection,
        OperationId::CompositeModule,
        mode,
    )?;
    if destination.mode() != mode
        || crate::compute::pool_cpu::ops::paste::helpers::layout::byte_carrier_pixel_width(mode)
            != Some(usize::from(channels))
    {
        return Err(PilError::InternalError(
            "Composite byte architecture received a mismatched exact mode".into(),
        ));
    }
    let plan = crate::compute::pool_cpu::ops::paste::helpers::plan::composite_byte_plan(
        destination,
        mode,
        source,
        mask,
        mask_alpha,
        *selection,
    )?;
    crate::compute::pool_cpu::ops::paste::helpers::plan::execute_with_hidden_lanes(
        plan,
        |plan| match selection.implementation.architecture {
            Architecture::Default => execute(
                source,
                destination,
                mask,
                mask_alpha,
                mode,
                CompositeLayout::Byte { channels },
            ),
            Architecture::Sse2 | Architecture::Neon | Architecture::WasmSimd128 => {
                execute_architecture(plan)
            }
            _ => Err(unbound(mode, selection)),
        },
    )
}

fn unbound(mode: Mode, selection: &StrategySelection) -> PilError {
    PilError::InternalError(format!(
        "unbound {:?} Composite strategy for mode {mode:?}",
        selection.implementation.architecture
    ))
}

pub(crate) fn execute(
    source: &DynamicImage,
    destination: &DynamicImage,
    mask: &DynamicImage,
    mask_alpha: bool,
    mode: Mode,
    layout: CompositeLayout,
) -> Result<DynamicImage, PilError> {
    let samples = super::mask::samples(mask, mask_alpha)?;
    match layout {
        CompositeLayout::Byte { channels } => {
            super::bytes::execute(source, destination, samples, mode, channels)
        }
        CompositeLayout::U16 { channels } => {
            super::numeric::execute_u16(source, destination, samples, mode, channels)
        }
        CompositeLayout::F32 { channels } => {
            super::numeric::execute_f32(source, destination, samples, mode, channels)
        }
    }
}
