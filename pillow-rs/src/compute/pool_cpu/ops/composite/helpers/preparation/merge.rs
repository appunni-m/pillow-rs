//! Prepared execution boundary for Merge.

use std::sync::Arc;

use crate::compute::ExecutionContext;
use crate::compute::registry::support::{OperationInputs, StrategySelection};
use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

// VERIFY: source=reviewed@2026-11-26; parity=pending; benchmark=pending
pub(crate) struct PreparedMerge<'a> {
    source: &'a DynamicImage,
    remaining: Vec<Arc<DynamicImage>>,
    mode: Mode,
    selection: StrategySelection,
}

impl PreparedMerge<'_> {
    // VERIFY: source=reviewed@2026-11-26; parity=pending; benchmark=pending
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    // VERIFY: source=reviewed@2026-11-26; parity=pending; benchmark=pending
    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        super::super::super::ops::execute_merge_mode(
            self.mode,
            self.source,
            &self.remaining,
            &self.selection,
        )
    }
}

// VERIFY: source=reviewed@2026-11-26; parity=pending; benchmark=pending
fn prepare_merge<'a>(
    input: OperationInputs<'a>,
    target: Mode,
    bands: &'a [Image],
    ctx: &ExecutionContext,
) -> Result<PreparedMerge<'a>, PilError> {
    let selection = super::super::super::ops::select_merge(
        target,
        super::super::super::ops::merge_facts(input, target, bands)?,
        ctx,
    )?;
    let remaining = bands
        .iter()
        .skip(1)
        .map(Image::materialized_shared)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PreparedMerge {
        source: input.image,
        remaining,
        mode: target,
        selection,
    })
}

// VERIFY: source=reviewed@2026-11-26; parity=pending; benchmark=pending
pub(crate) fn prepare_merge_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::Merge { mode, bands } = op else {
        return Err(PilError::ValueError("expected Merge op".into()));
    };
    prepare_merge(input, *mode, bands, ctx)
        .map(crate::compute::registry::PreparedOperation::from_merge)
}
