//! Admission planning for bounded lazy GPU operation graphs.

use super::super::{
    GPU_BUFFER_CAPACITY, GpuInner, GpuPool, MAX_GPU_OPS_PER_SUBMISSION, MAX_GPU_SHADER_WORK_ITEMS,
    aligned_bytes, gpu_buffer_capacity_exceeds_limits, gpu_dispatch_dimensions_require_cpu,
    gpu_f_resize_ordered_stage_resource_bytes, gpu_operation_is_safe, gpu_shader_work_items,
    gpu_working_set_bytes, op_has_explicit_output_dimensions, op_output_dims,
};
use super::native_channels;
use crate::Image;
use crate::checked_dims::CheckedDims;
use crate::compute::registry;
use crate::error::PilError;
use crate::image::GpuPendingGraph;
use crate::pipeline::{ColorMode, PipelineOp, ResampleFilter};

pub(super) fn planned_channels(mode: &str) -> Result<u8, PilError> {
    native_channels(mode).ok_or_else(|| {
        PilError::InternalError(format!("GPU plan has no native channel layout for {mode}"))
    })
}

// Encoding plans refer to the canonical operations. Layout flags select existing
// native shaders; no second public operation class or host image evaluator exists.
#[derive(Default, Clone, Copy)]
pub(super) struct Flags {
    pub(super) extract: bool,
    pub(super) luma_point: bool,
    pub(super) luma_order: bool,
    pub(super) byte_filter: bool,
    pub(super) rgb: bool,
    pub(super) rgb_blur: bool,
    pub(super) grayscale_rgb: bool,
    pub(super) luma_convert: bool,
    pub(super) luma_colorize: bool,
}
pub(super) struct Stage<'a> {
    pub(super) op: &'a PipelineOp,
    pub(super) secondary: Option<Box<Plan<'a>>>,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) mode: String,
    pub(super) flags: Flags,
    pub(super) byte: bool,
    pub(super) relocation: bool,
    pub(super) ordered_f_resize: bool,
    pub(super) arena_bytes: u64,
}
pub(super) struct Plan<'a> {
    pub(super) source: GpuPendingGraph<'a>,
    pub(super) stages: Vec<Stage<'a>>,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) mode: String,
    pub(super) capacity: u32,
    pub(super) gpu_bytes: u64,
    pub(super) host_bytes: u64,
    pub(super) metadata_bytes: u64,
    pub(super) shader_work: u64,
    pub(super) op_count: usize,
}

#[derive(Default)]
struct GraphBounds {
    work: u64,
    ops: usize,
}
impl GraphBounds {
    fn add(&mut self, work: u64) -> Result<(), PilError> {
        let total = self
            .work
            .checked_add(work)
            .ok_or_else(|| PilError::ValueError("GPU work arithmetic overflow".into()))?;
        if total > MAX_GPU_SHADER_WORK_ITEMS || self.ops >= MAX_GPU_OPS_PER_SUBMISSION {
            return Err(PilError::ValueError(
                "GPU graph exceeds cumulative submission work/operation bounds".into(),
            ));
        }
        self.work = total;
        self.ops += 1;
        Ok(())
    }
}
pub(super) static LUMA_BAND: PipelineOp = PipelineOp::ExtractBand { index: 0 };

pub(super) fn plan_graph<'a>(image: &'a Image, gpu: &GpuInner) -> Result<Plan<'a>, PilError> {
    let mut bounds = GraphBounds::default();
    plan_graph_inner(image, gpu, 0, &mut bounds)
}

fn plan_graph_inner<'a>(
    image: &'a Image,
    gpu: &GpuInner,
    depth: usize,
    bounds: &mut GraphBounds,
) -> Result<Plan<'a>, PilError> {
    if depth > 64 {
        return Err(PilError::ValueError(
            "GPU auxiliary graph exceeds 64 levels".into(),
        ));
    }
    let starting_work = bounds.work;
    let starting_ops = bounds.ops;
    if !cfg!(target_endian = "little") {
        return Err(PilError::ValueError(
            "native GPU batching requires little-endian samples".into(),
        ));
    }
    let mut source = image.gpu_pending_graph()?;
    let mut mode = source.mode.clone();
    let mut width = source.width;
    let mut height = source.height;
    let channels = native_channels(&mode)
        .ok_or_else(|| PilError::ValueError(format!("no native GPU graph layout for {mode}")))?;
    let source_bytes = CheckedDims::new(width, height, channels)?.total_bytes();
    let source_pixels = CheckedDims::new(width, height, 1)?.total_pixels();
    if width == 0 || height == 0 || source_pixels > GPU_BUFFER_CAPACITY as usize {
        return Err(PilError::ValueError(
            "GPU graph dimensions exceed native bounds".into(),
        ));
    }
    let ops = std::mem::take(&mut source.ops);
    let limits = gpu.device.limits();
    let mut capacity = u32::try_from(source_pixels)
        .map_err(|_| PilError::ValueError("GPU shader index overflow".into()))?;
    let mut stages = Vec::new();
    let mut ops = ops.into_iter().peekable();
    while let Some(op) = ops.next() {
        let followed_by_mirror = matches!(ops.peek().copied(), Some(PipelineOp::Mirror));
        if !GpuPool::descriptor_supports(op)? || !gpu_operation_is_safe(op) {
            return Err(PilError::ValueError(format!(
                "operation {} has no bounded exact GPU descriptor",
                registry::variant_key(op)
            )));
        }
        if matches!(op, PipelineOp::RankFilter { size, rank } if *rank >= size.saturating_mul(*size))
        {
            return Err(PilError::ValueError(
                "GPU rank lies outside its kernel".into(),
            ));
        }
        // The pixel shader silently ignores invalid coordinates; core/Pillow
        // reject them. A direct Rust descriptor must fail before upload too.
        if matches!(op, PipelineOp::PutPixel { x, y, .. } if *x >= width || *y >= height) {
            return Err(PilError::ValueError(
                "GPU pixel coordinate lies outside the current image".into(),
            ));
        }
        let mut flags = Flags::default();
        let mut byte = false;
        let mut relocation = false;
        let mut ordered_f_resize = false;
        let mut arena_bytes = 64 << 10;
        let mut secondary = None;
        let channels = native_channels(&mode)
            .ok_or_else(|| PilError::ValueError("unproven mode transition".into()))?;
        match op {
            PipelineOp::Paste {
                source,
                w,
                h,
                mask: None,
                ..
            } => {
                let child = plan_graph_inner(source, gpu, depth + 1, bounds)?;
                if child.mode != mode
                    || u32::try_from(*w).ok() != Some(child.width)
                    || u32::try_from(*h).ok() != Some(child.height)
                {
                    return Err(PilError::ValueError(
                        "GPU paste requires matching native modes and source region".into(),
                    ));
                }
                secondary = Some(Box::new(child));
            }
            PipelineOp::AlphaComposite {
                source,
                dest: (0, 0),
                src: (0, 0),
            } if mode == "RGBA" && followed_by_mirror => {
                let child = plan_graph_inner(source, gpu, depth + 1, bounds)?;
                if child.mode != "RGBA"
                    || (child.width, child.height) != (width, height)
                    || source.size().ok() != Some((width, height))
                {
                    return Err(PilError::ValueError(
                        "GPU alpha-composite mirror requires matching full-frame RGBA inputs"
                            .into(),
                    ));
                }
                secondary = Some(Box::new(child));
            }
            PipelineOp::Multiply { other }
            | PipelineOp::Screen { other }
            | PipelineOp::Overlay { other }
            | PipelineOp::HardLight { other }
            | PipelineOp::SoftLight { other }
            | PipelineOp::Difference { other }
            | PipelineOp::Darker { other }
            | PipelineOp::Lighter { other }
            | PipelineOp::AddModulo { other }
            | PipelineOp::SubtractModulo { other }
            | PipelineOp::BlendModule { other, .. } => {
                let child = plan_graph_inner(other, gpu, depth + 1, bounds)?;
                if child.mode != mode || (child.width, child.height) != (width, height) {
                    return Err(PilError::ValueError(
                        "GPU byte composition requires identical native modes and dimensions"
                            .into(),
                    ));
                }
                secondary = Some(Box::new(child));
                byte = true;
            }
            PipelineOp::Transpose { .. }
            | PipelineOp::Flip
            | PipelineOp::Mirror
            | PipelineOp::Crop { .. }
            | PipelineOp::CropBorder { .. }
            | PipelineOp::Expand { .. } => relocation = true,
            PipelineOp::Duplicate | PipelineOp::Invert | PipelineOp::InvertChops => byte = true,
            PipelineOp::Solarize { .. } | PipelineOp::Posterize { bits: 1..=8 }
                if matches!(mode.as_str(), "L" | "RGB") =>
            {
                byte = true
            }
            PipelineOp::ExtractBand { index } if *index < channels => flags.extract = true,
            PipelineOp::Eval { lut } if lut.len() == usize::from(channels) * 256 && mode == "L" => {
                flags.luma_point = true
            }
            PipelineOp::Filter3x3 {
                kernel,
                scale,
                offset,
            } if mode == "LA"
                && crate::compute::pool_cpu::ops::filter::binomial3x3_parameters_match(
                    kernel, *scale, *offset,
                ) =>
            {
                // The packed-LA shader preserves the exact native two-band
                // binomial filter; other LA kernels stay outside composed
                // GPU batch admission.
                flags.byte_filter = true
            }
            PipelineOp::Eval { lut }
                if lut.len() == usize::from(channels) * 256 && mode == "RGB" =>
            {
                flags.rgb = true
            }
            PipelineOp::MedianFilter { size: 3 }
            | PipelineOp::MinFilter { size: 3 }
            | PipelineOp::MaxFilter { size: 3 } => match mode.as_str() {
                "L" => flags.luma_order = true,
                "LA" => flags.byte_filter = true,
                "RGB" => flags.rgb = true,
                "RGBA" => {}
                _ => {
                    return Err(PilError::ValueError(
                        "order filter layout unsupported".into(),
                    ));
                }
            },
            PipelineOp::RankFilter { size: 3, rank: 1 } if mode == "L" => flags.luma_order = true,
            PipelineOp::RankFilter { size: 3, rank } if *rank < 9 && mode == "LA" => {
                flags.byte_filter = true
            }
            PipelineOp::RankFilter { size: 9, rank } if *rank < 81 && mode == "L" => {
                flags.luma_order = true
            }
            PipelineOp::BoxBlur { .. }
            | PipelineOp::BoxBlurXY { .. }
            | PipelineOp::GaussianBlur { .. }
                if mode == "RGB" =>
            {
                flags.rgb_blur = true
            }
            PipelineOp::Resize {
                filter: ResampleFilter::Nearest,
                w,
                h,
            } if mode == "RGBA" && u64::from(*w) + u64::from(*h) <= 2048 => {}
            PipelineOp::Resize { w, h, filter } if mode == "F" => {
                let resource_bytes = gpu_f_resize_ordered_stage_resource_bytes(
                    (width, height),
                    (*w, *h),
                    *filter,
                    limits.min_uniform_buffer_offset_alignment as usize,
                    limits.min_storage_buffer_offset_alignment as usize,
                )
                .ok_or_else(|| {
                    PilError::ValueError(
                        "GPU F resize lacks a bounded ordered-f64 shader/resource contract".into(),
                    )
                })?;
                ordered_f_resize = true;
                arena_bytes = u64::try_from(resource_bytes)
                    .map_err(|_| PilError::ValueError("GPU stage resource size overflow".into()))?;
            }
            PipelineOp::BoxBlur { radius: 1 } | PipelineOp::GaussianBlur { .. }
                if matches!(mode.as_str(), "L" | "LA") =>
            {
                flags.byte_filter = true
            }
            PipelineOp::Constant { .. } => {}
            PipelineOp::Grayscale if matches!(mode.as_str(), "L" | "LA") => flags.extract = true,
            PipelineOp::Grayscale if mode == "RGB" => flags.grayscale_rgb = true,
            PipelineOp::Grayscale if matches!(mode.as_str(), "RGBA" | "CMYK" | "RGBX") => {}
            PipelineOp::Convert {
                mode: ColorMode::RGBA,
                matrix: None,
                dither: None,
            } if mode == "L" => flags.luma_convert = true,
            PipelineOp::Convert {
                mode: ColorMode::RGBA,
                matrix: None,
                dither: None,
            } if mode == "RGB" => flags.rgb = true,
            // Generic shaders are already native for an RGBA image. Restrict
            // host-controlled/auxiliary graphs until their native plans exist.
            _ if mode == "RGBA" && rgba_standalone_native(op) => {}
            _ => {
                return Err(PilError::ValueError(format!(
                    "operation {} has no composed native {mode} layout",
                    registry::variant_key(op)
                )));
            }
        }
        let output = match op_output_dims(op, width, height) {
            Some(output) => output,
            None if op_has_explicit_output_dimensions(op) => {
                return Err(PilError::ValueError(
                    "GPU graph has an unresolved output shape".into(),
                ));
            }
            None => (width, height),
        };
        let mut pixels = CheckedDims::new(width, height, 1)?
            .total_pixels()
            .max(CheckedDims::new(output.0, output.1, 1)?.total_pixels());
        if matches!(op, PipelineOp::Resize { .. }) {
            // The horizontal resize pass has destination width and SOURCE
            // height. It can exceed both the input and final pixel counts.
            pixels = pixels.max(CheckedDims::new(output.0, height, 1)?.total_pixels());
        }
        if width == 0 || height == 0 || pixels > GPU_BUFFER_CAPACITY as usize {
            return Err(PilError::ValueError(
                "GPU graph dimensions exceed native bounds".into(),
            ));
        }
        capacity = capacity.max(
            u32::try_from(pixels)
                .map_err(|_| PilError::ValueError("shader index overflow".into()))?,
        );
        if gpu_dispatch_dimensions_require_cpu(
            std::slice::from_ref(op),
            (width, height),
            limits.max_compute_workgroups_per_dimension,
            Some(&mode),
            false,
        ) {
            return Err(PilError::ValueError(
                "GPU dispatch dimensions exceed device limits".into(),
            ));
        }
        let work = gpu_shader_work_items(op, (width, height), output, Some(&mode))
            .ok_or_else(|| PilError::ValueError("GPU work arithmetic overflow".into()))?;
        bounds.add(work)?;
        let next_mode = if secondary.is_some() {
            mode.clone()
        } else {
            crate::image::known_pipeline_op_mode(op, &mode).ok_or_else(|| {
                PilError::ValueError(
                    "GPU mode transition cannot be determined without host execution".into(),
                )
            })?
        };
        stages.push(Stage {
            op,
            secondary,
            width,
            height,
            mode: mode.clone(),
            flags,
            byte,
            relocation,
            ordered_f_resize,
            arena_bytes,
        });
        mode = next_mode;
        (width, height) = output;
    }
    if gpu_buffer_capacity_exceeds_limits(
        capacity,
        limits.max_storage_buffer_binding_size,
        limits.max_buffer_size,
    ) {
        return Err(PilError::ValueError(
            "GPU graph storage exceeds device binding limit".into(),
        ));
    }
    // Exact main workspace + conservative per-stage arenas. Existing parameter
    // encoders can generate coefficient/LUT tables; all reservations precede
    // allocation, and actual retained bytes are checked during compilation.
    let resource_bytes = stages.iter().try_fold(0u64, |total, stage| {
        total
            .checked_add(stage.arena_bytes)
            .ok_or_else(|| PilError::ValueError("GPU graph resource byte count overflow".into()))
    })?;
    let length = CheckedDims::new(width, height, planned_channels(&mode)?)?.total_bytes();
    let staging = aligned_bytes(length, 8) as u64;
    let mut gpu_bytes =
        gpu_working_set_bytes(capacity) + source_bytes as u64 + resource_bytes * 2 + staging;
    let metadata_bytes = image.gpu_metadata_bytes();
    let mut host_bytes = source_bytes as u64 * 3
        + source.encoded_bytes
        + staging * 2
        + resource_bytes
        + metadata_bytes
        + stages.len() as u64 * std::mem::size_of::<Stage<'_>>() as u64;
    for child in stages.iter().filter_map(|s| s.secondary.as_deref()) {
        gpu_bytes = gpu_bytes
            .checked_add(child.gpu_bytes)
            .ok_or_else(|| PilError::ValueError("GPU graph byte count overflow".into()))?;
        host_bytes = host_bytes
            .checked_add(child.host_bytes)
            .ok_or_else(|| PilError::ValueError("GPU graph byte count overflow".into()))?;
    }
    Ok(Plan {
        source,
        stages,
        width,
        height,
        mode,
        capacity,
        gpu_bytes,
        host_bytes,
        metadata_bytes,
        shader_work: bounds.work - starting_work,
        op_count: bounds.ops - starting_ops,
    })
}

fn rgba_standalone_native(op: &PipelineOp) -> bool {
    // Reject incomplete channel tables before source decode/upload. The
    // ordinary encoder also validates LUTs, but that is too late for admission.
    if let PipelineOp::Eval { lut } = op {
        return lut.len() == 4 * 256;
    }
    matches!(
        op,
        PipelineOp::Brightness { .. }
            | PipelineOp::ColorSaturation { .. }
            | PipelineOp::Filter3x3 { .. }
            | PipelineOp::Filter5x5 { .. }
            | PipelineOp::BoxBlur { .. }
            | PipelineOp::BoxBlurXY { .. }
            | PipelineOp::GaussianBlur { .. }
            | PipelineOp::MedianFilter { .. }
            | PipelineOp::MinFilter { .. }
            | PipelineOp::MaxFilter { .. }
            | PipelineOp::RankFilter { .. }
            | PipelineOp::Crop { .. }
            | PipelineOp::Transpose { .. }
            | PipelineOp::Flip
            | PipelineOp::Mirror
            | PipelineOp::Expand { .. }
            | PipelineOp::PutPixel {
                palette_index: false,
                ..
            }
            | PipelineOp::ExtractBand { .. }
            | PipelineOp::Grayscale
            | PipelineOp::Constant { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::{
        GraphBounds, MAX_GPU_OPS_PER_SUBMISSION, MAX_GPU_SHADER_WORK_ITEMS, rgba_standalone_native,
    };
    use crate::pipeline::PipelineOp;
    use std::sync::Arc;

    #[test]
    fn rgba_point_admission_requires_all_four_channel_tables() {
        for length in [0, 256, 768, 1023, 1025] {
            let op = PipelineOp::Eval {
                lut: Arc::from(vec![0; length]),
            };
            assert!(!rgba_standalone_native(&op), "invalid LUT length {length}");
        }
        let op = PipelineOp::Eval {
            lut: Arc::from((0..4).flat_map(|_| 0u8..=255).collect::<Vec<_>>()),
        };
        assert!(rgba_standalone_native(&op));
    }

    #[test]
    fn graph_budget_checks_aggregate_boundaries_without_allocations() {
        let mut bounds = GraphBounds::default();
        bounds.add(MAX_GPU_SHADER_WORK_ITEMS - 1).unwrap();
        bounds.add(1).unwrap();
        assert!(bounds.add(1).is_err());
        assert_eq!(bounds.work, MAX_GPU_SHADER_WORK_ITEMS);
        let mut bounds = GraphBounds::default();
        for _ in 0..MAX_GPU_OPS_PER_SUBMISSION {
            bounds.add(0).unwrap();
        }
        assert!(bounds.add(0).is_err());
        let mut overflow = GraphBounds {
            work: u64::MAX,
            ops: 0,
        };
        assert!(overflow.add(1).is_err());
    }
}
