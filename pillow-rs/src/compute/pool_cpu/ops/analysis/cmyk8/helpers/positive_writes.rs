//! Vectorized CMYK projection for a proven zero source and a small set of writes.

use crate::compute::Architecture;
use crate::compute::registry::support::StrategySelection;
use crate::pipeline::PipelineOp;
use wide::u32x4;

use super::super::super::helpers::projection_vector::VectorProjection;
use crate::compute::pool_cpu::ops::analysis::helpers::projection::ProjectionOutput;

const MAX_WRITES: usize = 8;

/// Build projection axes without scanning the source when all writes are positive.
pub(crate) fn execute(width: u32, height: u32, ops: &[PipelineOp]) -> Option<VectorProjection> {
    if ops.is_empty() || ops.len() > MAX_WRITES {
        return None;
    }

    let (mut x_marks, mut y_marks) = ([0u32; MAX_WRITES], [0u32; MAX_WRITES]);
    for (index, op) in ops.iter().enumerate() {
        let PipelineOp::PutPixel { x, y, color, palette_index, .. } = op else {
            return None;
        };
        if *palette_index
            || *x >= width
            || *y >= height
            || (color.0 == 0 && color.1 == 0 && color.2 == 0 && color.3 == 0)
        {
            // A zero write can erase an earlier nonzero sample; use the full
            // overwrite-aware projection path for that case.
            return None;
        }
        x_marks[index] = *x;
        y_marks[index] = *y;
    }

    let mut horizontal = vec![0u32; width as usize];
    let mut vertical = vec![0u32; height as usize];
    let (horizontal_blocks, horizontal_tail) = fill_axis(&mut horizontal, &x_marks[..ops.len()]);
    let (vertical_blocks, vertical_tail) = fill_axis(&mut vertical, &y_marks[..ops.len()]);
    Some(VectorProjection {
        axes: (horizontal, vertical),
        vector_blocks: horizontal_blocks.saturating_add(vertical_blocks),
        scalar_tail: horizontal_tail.saturating_add(vertical_tail),
    })
}

/// Run the small-write path under the already selected CMYK strategy.
pub(crate) fn execute_selected(
    selection: StrategySelection,
    width: u32,
    height: u32,
    ops: &[PipelineOp],
) -> Option<ProjectionOutput> {
    if selection.implementation.architecture == Architecture::Default {
        return None;
    }
    let result = execute(width, height, ops)?;
    Some(ProjectionOutput::vector(
        result.axes,
        result.vector_blocks,
        result.scalar_tail,
    ))
}

fn fill_axis(output: &mut [u32], marks: &[u32]) -> (u64, u64) {
    let zero = u32x4::splat(0);
    let one = u32x4::splat(1);
    let vector_len = output.len() / 4 * 4;
    let mut vector_blocks = 0u64;
    for start in (0..vector_len).step_by(4) {
        let lanes = u32x4::new([
            start as u32,
            start as u32 + 1,
            start as u32 + 2,
            start as u32 + 3,
        ]);
        let mut projected = zero;
        for &mark in marks {
            projected |= lanes.simd_eq(u32x4::splat(mark)).bitselect(one, zero);
        }
        output[start..start + 4].copy_from_slice(&projected.to_array());
        vector_blocks = vector_blocks.saturating_add(1);
    }
    for (index, value) in output.iter_mut().enumerate().skip(vector_len) {
        *value = u32::from(marks.contains(&(index as u32)));
    }
    (
        vector_blocks,
        output.len().saturating_sub(vector_len) as u64,
    )
}

