//! Lazy pipeline construction methods for [`Image`].

use std::sync::Arc;

use crate::pipeline::PipelineOp;

use super::{
    Image, PipelineOps, PipelinePrefixCache, materialization_cache, op_preserves_mode,
    pipeline_mode_cache, pipeline_shape_cache,
};

impl Image {
    // ── Pipeline ops ──

    /// Returns a lazy image with `op` appended to its pipeline.
    ///
    /// Existing pipelines are extended in order. Non-pipeline images become the
    /// source of a new pipeline. Mode metadata and palettes are carried forward
    /// when the operation can preserve them; operations that fundamentally
    /// change mode clear or replace the explicit mode tag.
    pub fn push_op(source: &Image, op: PipelineOp) -> Image {
        let source_is_paletted = source.has_palette_samples();
        let palette_safe = source_is_paletted && Self::is_palette_safe_op_for_source(source, &op);
        let indexed_chops_result_without_palette =
            source_is_paletted && Self::indexed_chops_result_without_palette(&op);
        let putalpha_mode = match &op {
            PipelineOp::PutAlpha { mode, .. } | PipelineOp::PutAlphaData { mode, .. } => {
                Some(*mode)
            }
            _ => None,
        };
        let promotes_p_to_pa = source.has_palette_mode()
            && matches!(
                &op,
                PipelineOp::PutAlpha {
                    mode: crate::pipeline::PixelMode::P,
                    ..
                } | PipelineOp::PutAlphaData {
                    mode: crate::pipeline::PixelMode::P,
                    ..
                }
            );
        let explicit_mode = if matches!(
            &op,
            PipelineOp::ExtractBand { .. } | PipelineOp::Merge { .. }
        ) {
            None
        } else if source_is_paletted {
            if source.explicit_mode() == Some("PA") && matches!(&op, PipelineOp::PutPixel { .. }) {
                // PA writes keep the raw index/alpha sample layout even when
                // no RGB palette is attached, so later conversion still sees
                // the source as PA rather than generic LA.
                Some("PA".to_owned())
            } else {
                palette_safe.then(|| {
                    if source.explicit_mode() == Some("PA") {
                        "PA".to_owned()
                    } else {
                        "P".to_owned()
                    }
                })
            }
        } else {
            match &op {
                // Pillow's libImaging/Chops.c constant constructor always
                // allocates an L image, even when the source has an explicit
                // CMYK, YCbCr, HSV, I, F, or 1 mode tag. Do not carry that
                // source tag across the mode-changing operation.
                PipelineOp::Grayscale
                | PipelineOp::Constant { .. }
                | PipelineOp::Convert { .. }
                | PipelineOp::ConvertLab { .. }
                | PipelineOp::ExtractBand { .. } => None,
                _ => source.explicit_mode().map(str::to_owned),
            }
        };
        // `push_op` always constructs an Image::Pipeline. Keep putalpha's
        // output-mode promotion here so P becomes PA without expanding its
        // indices, while CMYK becomes ordinary RGBA without a CMYK tag;
        // callers do not need an impossible fallback match after queuing it.
        let explicit_mode = match putalpha_mode {
            Some(crate::pipeline::PixelMode::P | crate::pipeline::PixelMode::PA) => {
                Some("PA".to_owned())
            }
            Some(crate::pipeline::PixelMode::CMYK) => None,
            _ => explicit_mode,
        };
        let preserve_palette = palette_safe
            && !matches!(
                &op,
                PipelineOp::ExtractBand { .. } | PipelineOp::Merge { .. }
            );
        let source_palette = if indexed_chops_result_without_palette {
            Some(Vec::new())
        } else if preserve_palette {
            source.extract_palette()
        } else {
            None
        };
        let source_palette_alpha = if indexed_chops_result_without_palette {
            None
        } else if preserve_palette {
            source.palette_alpha()
        } else {
            None
        };
        match source {
            // Keep a concrete sample-layout boundary around palette expansion
            // and P↔PA transitions. A flattened batch has only one external
            // mode tag, so it cannot correctly run P operations before
            // putalpha or PA operations after it (especially on the GPU).
            Image::Pipeline { .. }
                if source_is_paletted
                    && (!palette_safe
                        || promotes_p_to_pa
                        || source.explicit_mode() == Some("PA")) =>
            {
                Image::Pipeline {
                    source: Arc::new(source.clone()),
                    ops: PipelineOps::one(op),
                    // Pillow derived images (resize, crop, getchannel,
                    // convert, rotate, ...) report format None; only the
                    // originally opened image carries the container format.
                    format: None,
                    explicit_mode,
                    backend: source.backend(),
                    palette: source_palette,
                    palette_alpha: source_palette_alpha,
                    materialized: materialization_cache(),
                    shape: pipeline_shape_cache(),
                    mode: pipeline_mode_cache(),
                }
            }
            Image::Pipeline {
                source: pipeline_source,
                ops,
                backend,
                materialized: pipeline_materialized,
                ..
            } => {
                let can_reuse_prefix = !source_is_paletted
                    && ops.len() > 0
                    && ops.mode_preserving()
                    && op_preserves_mode(&op);
                let ancestor_cache = if can_reuse_prefix {
                    if pipeline_materialized.get().is_some() {
                        Some(PipelinePrefixCache {
                            ops_len: ops.len(),
                            materialized: Arc::clone(pipeline_materialized),
                        })
                    } else {
                        // An unmaterialized prefix must stay part of the
                        // flattened batch. Marking its empty cache here
                        // makes evaluate_pipeline split every lazy chain
                        // into a prefix and suffix submission, defeating
                        // GPU batching and adding an avoidable frame copy.
                        ops.prefix_cache().cloned()
                    }
                } else {
                    None
                };
                let new_ops = ops.append(op).with_prefix_cache(ancestor_cache);
                Image::Pipeline {
                    source: Arc::clone(pipeline_source),
                    ops: new_ops,
                    format: None,
                    explicit_mode,
                    backend: *backend,
                    palette: source_palette,
                    palette_alpha: source_palette_alpha,
                    materialized: materialization_cache(),
                    shape: pipeline_shape_cache(),
                    mode: pipeline_mode_cache(),
                }
            }
            other => Image::Pipeline {
                source: Arc::new(other.clone()),
                ops: PipelineOps::one(op),
                format: None,
                explicit_mode,
                backend: other.backend(),
                palette: source_palette,
                palette_alpha: source_palette_alpha,
                materialized: materialization_cache(),
                shape: pipeline_shape_cache(),
                mode: pipeline_mode_cache(),
            },
        }
    }

    /// Returns a lazy one-operation pipeline with an explicit output mode.
    ///
    /// A separate node is required for mode-changing operations: flattening
    /// into an existing pipeline would pass the final mode tag to operations
    /// that execute before the mode transition.
    pub(crate) fn push_mode_changing_op(
        source: &Image,
        op: PipelineOp,
        output_mode: &str,
    ) -> Image {
        Image::Pipeline {
            source: Arc::new(source.clone()),
            ops: PipelineOps::one(op),
            format: None,
            explicit_mode: Some(output_mode.to_owned()),
            backend: source.backend(),
            palette: None,
            palette_alpha: None,
            materialized: materialization_cache(),
            shape: pipeline_shape_cache(),
            mode: pipeline_mode_cache(),
        }
    }
}
