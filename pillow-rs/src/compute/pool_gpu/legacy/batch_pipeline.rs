use super::*;

mod blur_sources;
use blur_sources::{native_rgb_packed_blur_sources, packed_native_byte_blur_sources};

pub(super) enum ResolvedPipeline {
    Single(Arc<CachedPipeline>),
    /// The second public operation of a private fused dispatch.  It remains
    /// in the prepared operation/resource arrays so the public operation
    /// count, auxiliary ownership, and parameter indexing stay unchanged.
    Skip,
    Blur {
        horizontal: Arc<CachedPipeline>,
        vertical: Arc<CachedPipeline>,
        pass_count: usize,
    },
    /// Non-nearest resize is the same exact two-pass fixed-point algorithm
    /// used by the CPU/SIMD paths.  Coefficients live in an auxiliary storage
    /// range, so the two dispatches stay on-device with no host materialize.
    Resize {
        horizontal: Arc<CachedPipeline>,
        vertical: Arc<CachedPipeline>,
    },
    /// Pad is an exact contain resize followed by a device-side fill/copy
    /// placement.  Keeping both stages inside one compute pass avoids a host
    /// materialization while preserving Pillow's separable resize contract.
    Pad {
        horizontal: Arc<CachedPipeline>,
        vertical: Arc<CachedPipeline>,
        place: Arc<CachedPipeline>,
        /// RGBX resize variant that writes the vertical result into the final
        /// canvas, including its fill rows, instead of materializing a resized
        /// frame for the placement pass.
        vertical_pad_rgbx: Option<Arc<CachedPipeline>>,
    },
    /// Histogram-driven ImageOps keep all control/data passes on the device:
    /// clear the reusable histogram, accumulate the current image, derive a
    /// packed 256-entry LUT, and apply that LUT through the vector remap.
    Histogram {
        clear: Arc<CachedPipeline>,
        histogram: Arc<CachedPipeline>,
        derive: Arc<CachedPipeline>,
        remap: Arc<CachedPipeline>,
    },
}

impl GpuInner {
    pub(super) fn blur_pass_count(op: &PipelineOp) -> Option<usize> {
        match op {
            PipelineOp::BoxBlur { .. } => Some(1),
            PipelineOp::BoxBlurXY { passes, .. } => Some((*passes).max(1) as usize),
            PipelineOp::GaussianBlur { .. } => Some(3),
            _ => None,
        }
    }

    pub(super) fn resolve_batch_pipelines(
        &self,
        ops: &[PipelineOp],
        logical_mode: Option<&str>,
        input_dims: &[(u32, u32)],
        packed_luma_point: bool,
        packed_luma_putdata: bool,
        packed_luma_order_statistic: bool,
        packed_native_byte_filter: bool,
        packed_luma_pad: bool,
        packed_la_pad: bool,
        native_sharpness_l_input: bool,
        native_sharpness_la_input: bool,
        native_sharpness_rgb_input: bool,
        native_cover_packed_output: Option<NativeCoverPackedOutputDispatch>,
        native_luma_transform_output: bool,
        native_rgb_packed_output: bool,
        native_la_transform_output: bool,
    ) -> Result<Vec<ResolvedPipeline>, PilError> {
        BatchPipelineResolver::new(
            self,
            ops,
            logical_mode,
            input_dims,
            BatchPipelineOptions {
                packed_luma_point,
                packed_luma_putdata,
                packed_luma_order_statistic,
                packed_native_byte_filter,
                packed_luma_pad,
                packed_la_pad,
                native_sharpness_l_input,
                native_sharpness_la_input,
                native_sharpness_rgb_input,
                native_cover_packed_output,
                native_luma_transform_output,
                native_rgb_packed_output,
                native_la_transform_output,
            },
        )
        .resolve()
    }
}

type PipelineSource = (&'static str, &'static str, &'static str);

#[derive(Clone, Copy)]
struct BatchPipelineOptions {
    packed_luma_point: bool,
    packed_luma_putdata: bool,
    packed_luma_order_statistic: bool,
    packed_native_byte_filter: bool,
    packed_luma_pad: bool,
    packed_la_pad: bool,
    native_sharpness_l_input: bool,
    native_sharpness_la_input: bool,
    native_sharpness_rgb_input: bool,
    native_cover_packed_output: Option<NativeCoverPackedOutputDispatch>,
    native_luma_transform_output: bool,
    native_rgb_packed_output: bool,
    native_la_transform_output: bool,
}

struct BatchPipelineResolver<'a> {
    gpu: &'a GpuInner,
    ops: &'a [PipelineOp],
    logical_mode: Option<&'a str>,
    input_dims: &'a [(u32, u32)],
    options: BatchPipelineOptions,
}

impl<'a> BatchPipelineResolver<'a> {
    fn new(
        gpu: &'a GpuInner,
        ops: &'a [PipelineOp],
        logical_mode: Option<&'a str>,
        input_dims: &'a [(u32, u32)],
        options: BatchPipelineOptions,
    ) -> Self {
        Self {
            gpu,
            ops,
            logical_mode,
            input_dims,
            options,
        }
    }

    fn resolve(self) -> Result<Vec<ResolvedPipeline>, PilError> {
        let mut resolved = Vec::with_capacity(self.ops.len());
        let mut index = 0;
        while index < self.ops.len() {
            if let Some(consumed) = self.resolve_fused_prefix(index, &mut resolved)? {
                index += consumed;
                continue;
            }
            if let Some(pipeline) = self.resolve_packed_single(index)? {
                resolved.push(ResolvedPipeline::Single(pipeline));
                index += 1;
                continue;
            }
            if let Some(pipeline) = self.resolve_common_median(index)? {
                resolved.push(ResolvedPipeline::Single(pipeline));
            } else {
                resolved.push(self.resolve_general(index)?);
            }
            index += 1;
        }
        Ok(resolved)
    }

    fn resolve_fused_prefix(
        &self,
        index: usize,
        resolved: &mut Vec<ResolvedPipeline>,
    ) -> Result<Option<usize>, PilError> {
        let source = if index + 1 < self.ops.len() && can_fuse_gpu_multiply_screen(self.ops, index)
        {
            Some((
                "__internal_multiply_screen",
                "multiply_screen.wgsl",
                include_str!("shaders/multiply_screen.wgsl"),
            ))
        } else if can_fuse_gpu_alpha_composite_mirror(
            self.ops,
            index,
            self.logical_mode,
            self.input_dims[index],
        ) {
            Some((
                "__internal_alpha_composite_mirror",
                "alpha_composite_mirror.wgsl",
                include_str!("shaders/alpha_composite_mirror.wgsl"),
            ))
        } else {
            None
        };
        let Some(source) = source else {
            return Ok(None);
        };
        let fused = self.resolve_source(source)?;
        resolved.push(ResolvedPipeline::Single(fused));
        resolved.push(ResolvedPipeline::Skip);
        Ok(Some(2))
    }

    fn resolve_packed_single(&self, index: usize) -> Result<Option<Arc<CachedPipeline>>, PilError> {
        let sources = [
            self.packed_point_source(index),
            self.packed_rgb_order_source(index),
            self.packed_luma_order_source(index),
            self.packed_native_filter_source(index),
            self.packed_luma_rank9_source(index),
            self.native_sharpness_source(index),
            self.packed_radius_one_blur_source(index),
        ];
        let Some(source) = sources.into_iter().flatten().next() else {
            return Ok(None);
        };
        self.resolve_source(source).map(Some)
    }

    fn packed_point_source(&self, index: usize) -> Option<PipelineSource> {
        let op = &self.ops[index];
        if self.options.packed_luma_putdata && matches!(op, PipelineOp::PutData { .. }) {
            Some((
                "__internal_put_data_luma_packed",
                "put_data_luma_packed.wgsl",
                include_str!("shaders/put_data_luma_packed.wgsl"),
            ))
        } else if self.options.packed_luma_point && matches!(op, PipelineOp::Eval { .. }) {
            Some((
                "__internal_point_luma_packed",
                "point_luma_packed.wgsl",
                include_str!("shaders/point_luma_packed.wgsl"),
            ))
        } else if self.options.native_rgb_packed_output && matches!(op, PipelineOp::Eval { .. }) {
            Some((
                "__internal_point_rgb_packed",
                "point_rgb_packed.wgsl",
                include_str!("shaders/point_rgb_packed.wgsl"),
            ))
        } else {
            None
        }
    }

    fn packed_rgb_order_source(&self, index: usize) -> Option<PipelineSource> {
        if !self.options.native_rgb_packed_output {
            return None;
        }
        match &self.ops[index] {
            PipelineOp::MedianFilter { size: 3 } => Some((
                "__internal_median_filter_3x3_rgb_packed",
                "median_filter_3x3_rgb_packed.wgsl",
                include_str!("shaders/median_filter_3x3_rgb_packed.wgsl"),
            )),
            PipelineOp::MaxFilter { size: 3 } => Some((
                "__internal_max_filter_3x3_rgb_packed",
                "max_filter_3x3_rgb_packed.wgsl",
                include_str!("shaders/max_filter_3x3_rgb_packed.wgsl"),
            )),
            PipelineOp::MinFilter { size: 3 } => Some((
                "__internal_min_filter_3x3_rgb_packed",
                "min_filter_3x3_rgb_packed.wgsl",
                include_str!("shaders/min_filter_3x3_rgb_packed.wgsl"),
            )),
            _ => None,
        }
    }

    fn packed_luma_order_source(&self, index: usize) -> Option<PipelineSource> {
        if !self.options.packed_luma_order_statistic {
            return None;
        }
        match &self.ops[index] {
            PipelineOp::MedianFilter { size: 3 } => Some((
                "__internal_median_filter_3x3_luma_packed",
                "median_filter_3x3_luma_packed.wgsl",
                include_str!("shaders/median_filter_3x3_luma_packed.wgsl"),
            )),
            PipelineOp::MaxFilter { size: 3 } => Some((
                "__internal_max_filter_3x3_luma_packed",
                "max_filter_3x3_luma_packed.wgsl",
                include_str!("shaders/max_filter_3x3_luma_packed.wgsl"),
            )),
            PipelineOp::MinFilter { size: 3 } => Some((
                "__internal_min_filter_3x3_luma_packed",
                "min_filter_3x3_luma_packed.wgsl",
                include_str!("shaders/min_filter_3x3_luma_packed.wgsl"),
            )),
            PipelineOp::RankFilter { size: 3, rank: 1 } => Some((
                "__internal_rank_filter_3x3_luma_packed",
                "rank_filter_3x3_luma_packed.wgsl",
                include_str!("shaders/rank_filter_3x3_luma_packed.wgsl"),
            )),
            _ => None,
        }
    }

    fn packed_native_filter_source(&self, index: usize) -> Option<PipelineSource> {
        if !self.options.packed_native_byte_filter {
            return None;
        }
        let op = &self.ops[index];
        if matches!(self.logical_mode, None | Some("L"))
            && matches!(op, PipelineOp::Filter5x5 { .. })
        {
            return Some((
                "__internal_filter_5x5_luma_packed",
                "filter_5x5_luma_packed.wgsl",
                include_str!("shaders/filter_5x5_luma_packed.wgsl"),
            ));
        }
        if !matches!(self.logical_mode, Some("LA")) {
            return None;
        }
        match op {
            PipelineOp::Filter3x3 { .. } => Some((
                "__internal_filter_3x3_la_binomial_packed",
                "filter_3x3_la_binomial_packed.wgsl",
                include_str!("shaders/filter_3x3_la_binomial_packed.wgsl"),
            )),
            PipelineOp::Filter5x5 { .. } => Some((
                "__internal_filter_5x5_la_packed",
                "filter_5x5_la_packed.wgsl",
                include_str!("shaders/filter_5x5_la_packed.wgsl"),
            )),
            PipelineOp::MedianFilter { size: 3 } => Some((
                "__internal_median_filter_3x3_la_packed",
                "median_filter_3x3_la_packed.wgsl",
                include_str!("shaders/median_filter_3x3_la_packed.wgsl"),
            )),
            PipelineOp::RankFilter { size: 3, rank } if *rank < 9 => Some((
                "__internal_rank_filter_3x3_la_packed",
                "rank_filter_3x3_la_packed.wgsl",
                include_str!("shaders/rank_filter_3x3_la_packed.wgsl"),
            )),
            PipelineOp::MaxFilter { size: 3 } => Some((
                "__internal_max_filter_3x3_la_packed",
                "max_filter_3x3_la_packed.wgsl",
                include_str!("shaders/max_filter_3x3_la_packed.wgsl"),
            )),
            PipelineOp::MinFilter { size: 3 } => Some((
                "__internal_min_filter_3x3_la_packed",
                "min_filter_3x3_la_packed.wgsl",
                include_str!("shaders/min_filter_3x3_la_packed.wgsl"),
            )),
            _ => None,
        }
    }

    fn packed_luma_rank9_source(&self, index: usize) -> Option<PipelineSource> {
        if !self.options.packed_luma_order_statistic
            || !matches!(self.ops[index], PipelineOp::RankFilter { size: 9, .. })
        {
            return None;
        }
        if self.input_dims[index].0.is_multiple_of(4) {
            Some((
                "__internal_rank_filter_9x9_luma_row_packed",
                "rank_filter_9x9_luma_row_packed.wgsl",
                include_str!("shaders/rank_filter_9x9_luma_row_packed.wgsl"),
            ))
        } else {
            Some((
                "__internal_rank_filter_9x9_luma_packed",
                "rank_filter_9x9_luma_packed.wgsl",
                include_str!("shaders/rank_filter_9x9_luma_packed.wgsl"),
            ))
        }
    }

    fn native_sharpness_source(&self, index: usize) -> Option<PipelineSource> {
        if !matches!(self.ops[index], PipelineOp::Sharpness { .. }) {
            return None;
        }
        if self.options.native_sharpness_l_input {
            Some((
                "__internal_sharpness_l_native",
                "sharpness_l_native.wgsl",
                include_str!("shaders/sharpness_l_native.wgsl"),
            ))
        } else if self.options.native_sharpness_la_input {
            Some((
                "__internal_sharpness_la_native",
                "sharpness_la_native.wgsl",
                include_str!("shaders/sharpness_la_native.wgsl"),
            ))
        } else if self.options.native_sharpness_rgb_input {
            Some((
                "__internal_sharpness_rgb_native",
                "sharpness_rgb_native.wgsl",
                include_str!("shaders/sharpness_rgb_native.wgsl"),
            ))
        } else {
            None
        }
    }

    fn packed_radius_one_blur_source(&self, index: usize) -> Option<PipelineSource> {
        let radius_one = matches!(self.ops[index], PipelineOp::BoxBlur { radius: 1 });
        if radius_one
            && self.options.native_rgb_packed_output
            && matches!(self.logical_mode, None | Some("RGB"))
        {
            Some((
                "__internal_blur_hv_rgb_radius_one_packed",
                "box_blur_hv_rgb_radius_one_packed.wgsl",
                include_str!("shaders/box_blur_hv_rgb_radius_one_packed.wgsl"),
            ))
        } else if radius_one
            && self.options.packed_native_byte_filter
            && matches!(self.logical_mode, None | Some("L"))
        {
            Some((
                "__internal_blur_hv_luma_radius_one_packed",
                "box_blur_hv_luma_radius_one_packed.wgsl",
                include_str!("shaders/box_blur_hv_luma_radius_one_packed.wgsl"),
            ))
        } else {
            None
        }
    }

    fn resolve_common_median(&self, index: usize) -> Result<Option<Arc<CachedPipeline>>, PilError> {
        let op = &self.ops[index];
        if !matches!(op, PipelineOp::MedianFilter { size: 3 })
            || matches!(self.logical_mode, Some("F" | "I"))
        {
            return Ok(None);
        }
        self.resolve_source((
            "__internal_median_filter_3x3",
            "median_filter_3x3.wgsl",
            include_str!("shaders/median_filter_3x3.wgsl"),
        ))
        .map(Some)
    }

    fn resolve_general(&self, index: usize) -> Result<ResolvedPipeline, PilError> {
        let op = &self.ops[index];
        if Self::is_histogram(op) {
            self.resolve_histogram(op).map(Into::into)
        } else if let Some(pass_count) = GpuInner::blur_pass_count(op) {
            self.resolve_blur(op)
                .map(|(horizontal, vertical)| ResolvedPipeline::Blur {
                    horizontal,
                    vertical,
                    pass_count,
                })
        } else if matches!(op, PipelineOp::Pad { .. }) {
            self.resolve_pad(op, index).map(Into::into)
        } else if matches!(op, PipelineOp::Fit { .. } | PipelineOp::ResizeBoxed { .. }) {
            self.resolve_resize_pair(false)
                .map(|(horizontal, vertical)| ResolvedPipeline::Resize {
                    horizontal,
                    vertical,
                })
        } else if Self::is_nearest_resize(op, self.logical_mode) {
            self.resolve_registered(op).map(ResolvedPipeline::Single)
        } else if matches!(op, PipelineOp::Resize { .. }) {
            self.resolve_resize_pair(self.options.native_cover_packed_output.is_some())
                .map(|(horizontal, vertical)| ResolvedPipeline::Resize {
                    horizontal,
                    vertical,
                })
        } else {
            self.resolve_transform_or_registered(op, index)
        }
    }

    fn is_histogram(op: &PipelineOp) -> bool {
        matches!(
            op,
            PipelineOp::Autocontrast { .. }
                | PipelineOp::Equalize
                | PipelineOp::EqualizeMasked { .. }
        )
    }

    fn is_nearest_resize(op: &PipelineOp, logical_mode: Option<&str>) -> bool {
        matches!(op, PipelineOp::Resize { filter: ResampleFilter::Nearest, .. }
            if !gpu_resize_nearest_uses_coefficients(logical_mode))
    }

    fn resolve_histogram(&self, op: &PipelineOp) -> Result<HistogramPipelines, PilError> {
        let clear = self.resolve_source((
            "__internal_histogram_clear",
            "histogram_clear.wgsl",
            include_str!("shaders/histogram_clear.wgsl"),
        ))?;
        let native_rgb =
            self.options.native_rgb_packed_output && matches!(op, PipelineOp::Equalize);
        let native_luma = self.options.packed_luma_point && matches!(op, PipelineOp::Equalize);
        let histogram = if native_rgb {
            self.resolve_source((
                "__internal_equalize_histogram_rgb_packed",
                "equalize_histogram_rgb_packed.wgsl",
                include_str!("shaders/equalize_histogram_rgb_packed.wgsl"),
            ))?
        } else if native_luma {
            self.resolve_source((
                "__internal_equalize_histogram_l_packed",
                "equalize_histogram_l_packed.wgsl",
                include_str!("shaders/equalize_histogram_l_packed.wgsl"),
            ))?
        } else {
            self.resolve_source((
                "__internal_equalize_histogram",
                "equalize_histogram.wgsl",
                include_str!("shaders/equalize_histogram.wgsl"),
            ))?
        };
        let derive = self.resolve_histogram_lut(op)?;
        let remap = self.resolve_histogram_remap(native_rgb, native_luma)?;
        Ok(HistogramPipelines {
            clear,
            histogram,
            derive,
            remap,
        })
    }

    fn resolve_histogram_lut(&self, op: &PipelineOp) -> Result<Arc<CachedPipeline>, PilError> {
        match op {
            PipelineOp::Autocontrast { .. } => self.resolve_source((
                "__internal_autocontrast_lut",
                "autocontrast_cutoff.wgsl",
                include_str!("shaders/autocontrast_cutoff.wgsl"),
            )),
            PipelineOp::Equalize | PipelineOp::EqualizeMasked { .. } => self.resolve_source((
                "__internal_equalize_lut",
                "equalize_cdf.wgsl",
                include_str!("shaders/equalize_cdf.wgsl"),
            )),
            _ => unreachable!("histogram pipeline branch changed"),
        }
    }

    fn resolve_histogram_remap(
        &self,
        native_rgb: bool,
        native_luma: bool,
    ) -> Result<Arc<CachedPipeline>, PilError> {
        let source = if native_rgb {
            (
                "__internal_histogram_remap_rgb_packed",
                "point_rgb_packed.wgsl",
                include_str!("shaders/point_rgb_packed.wgsl"),
            )
        } else if native_luma {
            (
                "__internal_equalize_remap_l_packed",
                "equalize_remap_l_packed.wgsl",
                include_str!("shaders/equalize_remap_l_packed.wgsl"),
            )
        } else {
            (
                "__internal_histogram_remap",
                "point_op.wgsl",
                include_str!("shaders/point_op.wgsl"),
            )
        };
        self.resolve_source(source)
    }

    fn resolve_blur(
        &self,
        op: &PipelineOp,
    ) -> Result<(Arc<CachedPipeline>, Arc<CachedPipeline>), PilError> {
        let sources = native_rgb_packed_blur_sources(self.options, self.logical_mode, op)
            .or_else(|| packed_native_byte_blur_sources(self.options, self.logical_mode, op))
            .unwrap_or({
                (
                    (
                        "__internal_blur_h",
                        "box_blur_h.wgsl",
                        include_str!("shaders/box_blur_h.wgsl"),
                    ),
                    (
                        "__internal_blur_v",
                        "box_blur_v.wgsl",
                        include_str!("shaders/box_blur_v.wgsl"),
                    ),
                )
            });
        let (horizontal, vertical) = sources;
        Ok((
            self.resolve_source(horizontal)?,
            self.resolve_source(vertical)?,
        ))
    }

    fn resolve_pad(&self, op: &PipelineOp, index: usize) -> Result<PadPipelines, PilError> {
        let (horizontal, vertical) = if self.options.packed_luma_pad {
            (
                (
                    "__internal_resize_h_pad_luma_packed",
                    "resize_convolution_h.wgsl",
                    include_str!("shaders/resize_convolution_h.wgsl"),
                ),
                (
                    "__internal_resize_v_pad_luma_packed",
                    "resize_convolution_v.wgsl",
                    include_str!("shaders/resize_convolution_v.wgsl"),
                ),
            )
        } else {
            Self::resize_sources(false)
        };
        let place = if self.options.packed_luma_pad {
            (
                "__internal_pad_luma_packed",
                "pad_luma_packed.wgsl",
                include_str!("shaders/pad_luma_packed.wgsl"),
            )
        } else if self.options.packed_la_pad {
            (
                "__internal_pad_la_packed",
                "pad_la_packed.wgsl",
                include_str!("shaders/pad_la_packed.wgsl"),
            )
        } else {
            (
                "__internal_pad_place",
                "pad.wgsl",
                include_str!("shaders/pad.wgsl"),
            )
        };
        let vertical_pad_rgbx =
            if gpu_pad_rgbx_fused_vertical_geometry(op, self.input_dims[index], self.logical_mode)
                .is_some()
            {
                Some(self.resolve_source((
                    "__internal_resize_v_pad_rgbx",
                    "resize_convolution_v_pad_rgbx.wgsl",
                    include_str!("shaders/resize_convolution_v_pad_rgbx.wgsl"),
                ))?)
            } else {
                None
            };
        Ok(PadPipelines {
            horizontal: self.resolve_source(horizontal)?,
            vertical: self.resolve_source(vertical)?,
            place: self.resolve_source(place)?,
            vertical_pad_rgbx,
        })
    }

    fn resolve_resize_pair(
        &self,
        cover_packed: bool,
    ) -> Result<(Arc<CachedPipeline>, Arc<CachedPipeline>), PilError> {
        let (horizontal, vertical) = Self::resize_sources(cover_packed);
        Ok((
            self.resolve_source(horizontal)?,
            self.resolve_source(vertical)?,
        ))
    }

    fn resize_sources(cover_packed: bool) -> (PipelineSource, PipelineSource) {
        let horizontal_key = if cover_packed {
            "__internal_resize_h_cover_native_packed"
        } else {
            "__internal_resize_h"
        };
        let vertical_key = if cover_packed {
            "__internal_resize_v_cover_native_packed"
        } else {
            "__internal_resize_v"
        };
        (
            (
                horizontal_key,
                "resize_convolution_h.wgsl",
                include_str!("shaders/resize_convolution_h.wgsl"),
            ),
            (
                vertical_key,
                "resize_convolution_v.wgsl",
                include_str!("shaders/resize_convolution_v.wgsl"),
            ),
        )
    }

    fn resolve_transform_or_registered(
        &self,
        op: &PipelineOp,
        index: usize,
    ) -> Result<ResolvedPipeline, PilError> {
        if self.options.native_rgb_packed_output && matches!(op, PipelineOp::Transform { .. }) {
            self.resolve_source((
                "__internal_transform_rgb_nearest_packed",
                "transform_rgb_nearest_packed.wgsl",
                include_str!("shaders/transform_rgb_nearest_packed.wgsl"),
            ))
            .map(ResolvedPipeline::Single)
        } else if self.options.native_la_transform_output
            && matches!(op, PipelineOp::Transform { .. })
        {
            self.resolve_source((
                "__internal_transform_la_nearest_packed",
                "transform_la_nearest_packed.wgsl",
                include_str!("shaders/transform_la_nearest_packed.wgsl"),
            ))
            .map(ResolvedPipeline::Single)
        } else if self.options.native_luma_transform_output
            && matches!(op, PipelineOp::Transform { .. })
        {
            self.resolve_source((
                "__internal_transform_l_nearest_packed",
                "transform_l_nearest_packed.wgsl",
                include_str!("shaders/transform_l_nearest_packed.wgsl"),
            ))
            .map(ResolvedPipeline::Single)
        } else if gpu_transform_uses_geometry_table(op, self.logical_mode, self.input_dims[index]) {
            self.resolve_source((
                "__internal_transform_geometry",
                "transform_geometry.wgsl",
                include_str!("shaders/transform_geometry.wgsl"),
            ))
            .map(ResolvedPipeline::Single)
        } else {
            self.resolve_registered(op).map(ResolvedPipeline::Single)
        }
    }

    fn resolve_registered(&self, op: &PipelineOp) -> Result<Arc<CachedPipeline>, PilError> {
        let key = registry::variant_key(op);
        let entry = registry::registry()?.get(key);
        let source = entry.and_then(|entry| entry.gpu_source).ok_or_else(|| {
            PilError::ValueError(format!(
                "GPU operation '{key}' has no registered shader source"
            ))
        })?;
        let shader_file = entry.and_then(|entry| entry.gpu_shader).ok_or_else(|| {
            PilError::ValueError(format!(
                "GPU operation '{key}' has no registered shader name"
            ))
        })?;
        self.gpu.resolve_pipeline(key, shader_file, source)
    }

    fn resolve_source(&self, source: PipelineSource) -> Result<Arc<CachedPipeline>, PilError> {
        self.gpu.resolve_pipeline(source.0, source.1, source.2)
    }
}

struct HistogramPipelines {
    clear: Arc<CachedPipeline>,
    histogram: Arc<CachedPipeline>,
    derive: Arc<CachedPipeline>,
    remap: Arc<CachedPipeline>,
}

impl From<HistogramPipelines> for ResolvedPipeline {
    fn from(pipelines: HistogramPipelines) -> Self {
        Self::Histogram {
            clear: pipelines.clear,
            histogram: pipelines.histogram,
            derive: pipelines.derive,
            remap: pipelines.remap,
        }
    }
}

struct PadPipelines {
    horizontal: Arc<CachedPipeline>,
    vertical: Arc<CachedPipeline>,
    place: Arc<CachedPipeline>,
    vertical_pad_rgbx: Option<Arc<CachedPipeline>>,
}

impl From<PadPipelines> for ResolvedPipeline {
    fn from(pipelines: PadPipelines) -> Self {
        Self::Pad {
            horizontal: pipelines.horizontal,
            vertical: pipelines.vertical,
            place: pipelines.place,
            vertical_pad_rgbx: pipelines.vertical_pad_rgbx,
        }
    }
}
