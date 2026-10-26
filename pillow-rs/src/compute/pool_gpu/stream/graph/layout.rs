use super::super::super::{
    GPU_BUFFER_CAPACITY, GpuInner, GpuPool, gpu_dispatch_dimensions_require_cpu,
    gpu_f_resize_ordered_stage_resource_bytes, gpu_operation_is_safe, gpu_shader_work_items,
    op_has_explicit_output_dimensions, op_output_dims,
};
use super::{Flags, GraphBounds, Plan, Stage, plan_graph_inner, rgba_standalone_native};
use crate::checked_dims::CheckedDims;
use crate::compute::registry;
use crate::error::PilError;
use crate::pipeline::{ColorMode, PipelineOp, ResampleFilter};

pub(super) struct PlannedStage<'a> {
    pub(super) stage: Stage<'a>,
    pub(super) output: (u32, u32),
    pub(super) capacity: u32,
    pub(super) next_mode: String,
}

struct Layout {
    flags: Flags,
    byte: bool,
    relocation: bool,
    ordered_f_resize: bool,
    arena_bytes: u64,
}

#[derive(Default)]
struct PlannedAuxiliaries<'a> {
    secondary: Option<Box<Plan<'a>>>,
    tertiary: Option<Box<Plan<'a>>>,
    composed_byte: bool,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            flags: Flags::default(),
            byte: false,
            relocation: false,
            ordered_f_resize: false,
            arena_bytes: 64 << 10,
        }
    }
}

pub(super) fn plan_stage<'a>(
    op: &'a PipelineOp,
    followed_by_mirror: bool,
    mode: &str,
    dimensions: (u32, u32),
    depth: usize,
    gpu: &GpuInner,
    limits: &wgpu::Limits,
    bounds: &mut GraphBounds,
) -> Result<PlannedStage<'a>, PilError> {
    let (width, height) = dimensions;
    validate_operation(op, dimensions)?;
    let channels = super::super::native_channels(mode)
        .ok_or_else(|| PilError::ValueError("unproven mode transition".into()))?;
    let auxiliaries =
        plan_auxiliaries(op, mode, dimensions, followed_by_mirror, depth, gpu, bounds)?;
    let has_auxiliaries = auxiliaries.secondary.is_some() || auxiliaries.tertiary.is_some();
    let mut layout = if has_auxiliaries {
        Layout::default()
    } else {
        select_layout(op, mode, channels, dimensions, limits)?
    };
    layout.byte |= auxiliaries.composed_byte;
    let output = output_dimensions(op, dimensions)?;
    let pixels = stage_capacity_pixels(op, dimensions, output)?;
    let capacity =
        u32::try_from(pixels).map_err(|_| PilError::ValueError("shader index overflow".into()))?;
    validate_dispatch(op, mode, dimensions, limits)?;
    let work = gpu_shader_work_items(op, dimensions, output, Some(mode))
        .ok_or_else(|| PilError::ValueError("GPU work arithmetic overflow".into()))?;
    bounds.add(work)?;
    let next_mode = if has_auxiliaries {
        mode.to_owned()
    } else {
        crate::image::known_pipeline_op_mode(op, mode).ok_or_else(|| {
            PilError::ValueError(
                "GPU mode transition cannot be determined without host execution".into(),
            )
        })?
    };
    Ok(PlannedStage {
        stage: Stage {
            op,
            secondary: auxiliaries.secondary,
            tertiary: auxiliaries.tertiary,
            width,
            height,
            mode: mode.to_owned(),
            flags: layout.flags,
            byte: layout.byte,
            relocation: layout.relocation,
            ordered_f_resize: layout.ordered_f_resize,
            arena_bytes: layout.arena_bytes,
        },
        output,
        capacity,
        next_mode,
    })
}

fn validate_operation(op: &PipelineOp, dimensions: (u32, u32)) -> Result<(), PilError> {
    if !GpuPool::descriptor_supports(op)? || !gpu_operation_is_safe(op) {
        return Err(PilError::ValueError(format!(
            "operation {} has no bounded exact GPU descriptor",
            registry::variant_key(op)
        )));
    }
    if matches!(op, PipelineOp::RankFilter { size, rank } if *rank >= size.saturating_mul(*size)) {
        return Err(PilError::ValueError(
            "GPU rank lies outside its kernel".into(),
        ));
    }
    // The pixel shader silently ignores invalid coordinates; core/Pillow
    // reject them. A direct Rust descriptor must fail before upload too.
    if matches!(op, PipelineOp::PutPixel { x, y, .. } if *x >= dimensions.0 || *y >= dimensions.1) {
        return Err(PilError::ValueError(
            "GPU pixel coordinate lies outside the current image".into(),
        ));
    }
    Ok(())
}

fn plan_auxiliaries<'a>(
    op: &'a PipelineOp,
    mode: &str,
    dimensions: (u32, u32),
    followed_by_mirror: bool,
    depth: usize,
    gpu: &GpuInner,
    bounds: &mut GraphBounds,
) -> Result<PlannedAuxiliaries<'a>, PilError> {
    match op {
        PipelineOp::Paste {
            source,
            w,
            h,
            mask: None,
            ..
        } => plan_unmasked_paste_auxiliary(source, *w, *h, mode, depth, gpu, bounds),
        PipelineOp::Paste {
            source,
            w,
            h,
            mask: Some(mask),
            mask_alpha: false,
            ..
        } => plan_masked_paste_auxiliary(source, mask, *w, *h, mode, depth, gpu, bounds),
        PipelineOp::Paste { mask: Some(_), .. } => Err(PilError::ValueError(
            "GPU batch masked Paste requires a non-alpha native L mask".into(),
        )),
        PipelineOp::AlphaComposite {
            source,
            dest: (0, 0),
            src: (0, 0),
        } if mode == "RGBA" && followed_by_mirror => {
            plan_alpha_composite_auxiliary(source, dimensions, depth, gpu, bounds)
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
            plan_byte_composition_auxiliary(other, mode, dimensions, depth, gpu, bounds)
        }
        _ => Ok(PlannedAuxiliaries::default()),
    }
}

fn plan_unmasked_paste_auxiliary<'a>(
    source: &'a crate::Image,
    width: i32,
    height: i32,
    mode: &str,
    depth: usize,
    gpu: &GpuInner,
    bounds: &mut GraphBounds,
) -> Result<PlannedAuxiliaries<'a>, PilError> {
    let child = plan_graph_inner(source, gpu, depth + 1, bounds)?;
    if child.mode != mode
        || u32::try_from(width).ok() != Some(child.width)
        || u32::try_from(height).ok() != Some(child.height)
    {
        return Err(PilError::ValueError(
            "GPU paste requires matching native modes and source region".into(),
        ));
    }
    Ok(PlannedAuxiliaries {
        secondary: Some(Box::new(child)),
        ..PlannedAuxiliaries::default()
    })
}

fn plan_masked_paste_auxiliary<'a>(
    source: &'a crate::Image,
    mask: &'a crate::Image,
    width: i32,
    height: i32,
    mode: &str,
    depth: usize,
    gpu: &GpuInner,
    bounds: &mut GraphBounds,
) -> Result<PlannedAuxiliaries<'a>, PilError> {
    let source_child = plan_graph_inner(source, gpu, depth + 1, bounds)?;
    let mask_child = plan_graph_inner(mask, gpu, depth + 1, bounds)?;
    let region = (u32::try_from(width).ok(), u32::try_from(height).ok());
    let compact_broadcast =
        (source_child.width, source_child.height) == (1, 1) && region != (Some(1), Some(1));
    if !matches!(mode, "L" | "RGB" | "HSV" | "CMYK")
        || source_child.mode != mode
        || mask_child.mode != "L"
        || width <= 0
        || height <= 0
        || region != (Some(mask_child.width), Some(mask_child.height))
        || (!compact_broadcast
            && (source_child.width, source_child.height) != (mask_child.width, mask_child.height))
    {
        return Err(PilError::ValueError(
            "GPU masked Paste requires a native L mask and matching L/RGB/HSV/CMYK inputs".into(),
        ));
    }
    Ok(PlannedAuxiliaries {
        secondary: Some(Box::new(source_child)),
        tertiary: Some(Box::new(mask_child)),
        ..PlannedAuxiliaries::default()
    })
}

fn plan_alpha_composite_auxiliary<'a>(
    source: &'a crate::Image,
    dimensions: (u32, u32),
    depth: usize,
    gpu: &GpuInner,
    bounds: &mut GraphBounds,
) -> Result<PlannedAuxiliaries<'a>, PilError> {
    let child = plan_graph_inner(source, gpu, depth + 1, bounds)?;
    if child.mode != "RGBA"
        || (child.width, child.height) != dimensions
        || source.size().ok() != Some(dimensions)
    {
        return Err(PilError::ValueError(
            "GPU alpha-composite mirror requires matching full-frame RGBA inputs".into(),
        ));
    }
    Ok(PlannedAuxiliaries {
        secondary: Some(Box::new(child)),
        ..PlannedAuxiliaries::default()
    })
}

fn plan_byte_composition_auxiliary<'a>(
    other: &'a crate::Image,
    mode: &str,
    dimensions: (u32, u32),
    depth: usize,
    gpu: &GpuInner,
    bounds: &mut GraphBounds,
) -> Result<PlannedAuxiliaries<'a>, PilError> {
    let child = plan_graph_inner(other, gpu, depth + 1, bounds)?;
    if child.mode != mode || (child.width, child.height) != dimensions {
        return Err(PilError::ValueError(
            "GPU byte composition requires identical native modes and dimensions".into(),
        ));
    }
    Ok(PlannedAuxiliaries {
        secondary: Some(Box::new(child)),
        composed_byte: true,
        ..PlannedAuxiliaries::default()
    })
}

fn select_layout(
    op: &PipelineOp,
    mode: &str,
    channels: u8,
    dimensions: (u32, u32),
    limits: &wgpu::Limits,
) -> Result<Layout, PilError> {
    let mut layout = Layout::default();
    if is_relocation(op) {
        layout.relocation = true;
        return Ok(layout);
    }
    if is_byte_operation(op, mode) {
        layout.byte = true;
        return Ok(layout);
    }
    if let Some(flags) = mode_specific_flags(op, mode, channels)? {
        layout.flags = flags;
        return Ok(layout);
    }
    select_generic_layout(op, mode, dimensions, limits, layout)
}

fn mode_specific_flags(
    op: &PipelineOp,
    mode: &str,
    channels: u8,
) -> Result<Option<Flags>, PilError> {
    let mut flags = Flags::default();
    match op {
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
        PipelineOp::Eval { lut } if lut.len() == usize::from(channels) * 256 && mode == "RGB" => {
            flags.rgb = true
        }
        PipelineOp::MedianFilter { size: 3 }
        | PipelineOp::MinFilter { size: 3 }
        | PipelineOp::MaxFilter { size: 3 } => match mode {
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
        PipelineOp::BoxBlur { radius: 1 } | PipelineOp::GaussianBlur { .. }
            if matches!(mode, "L" | "LA") =>
        {
            flags.byte_filter = true
        }
        PipelineOp::Constant { .. } => {}
        PipelineOp::Grayscale if matches!(mode, "L" | "LA") => flags.extract = true,
        PipelineOp::Grayscale if mode == "RGB" => flags.grayscale_rgb = true,
        PipelineOp::Grayscale if matches!(mode, "RGBA" | "CMYK" | "RGBX") => {}
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
        PipelineOp::Convert {
            mode: ColorMode::LA,
            matrix: None,
            dither: None,
        } if mode == "RGB" => flags.rgb = true,
        _ => return Ok(None),
    }
    Ok(Some(flags))
}

fn select_generic_layout(
    op: &PipelineOp,
    mode: &str,
    dimensions: (u32, u32),
    limits: &wgpu::Limits,
    mut layout: Layout,
) -> Result<Layout, PilError> {
    match op {
        PipelineOp::Resize {
            filter: ResampleFilter::Nearest,
            w,
            h,
        } if mode == "RGBA" && u64::from(*w) + u64::from(*h) <= 2048 => {}
        PipelineOp::Resize { w, h, filter } if mode == "F" => {
            let resource_bytes = gpu_f_resize_ordered_stage_resource_bytes(
                dimensions,
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
            layout.ordered_f_resize = true;
            layout.arena_bytes = u64::try_from(resource_bytes)
                .map_err(|_| PilError::ValueError("GPU stage resource size overflow".into()))?;
        }
        // Generic shaders are native for RGBA. Restrict host-controlled and
        // auxiliary graphs until their native plans exist.
        _ if mode == "RGBA" && rgba_standalone_native(op) => {}
        _ => {
            return Err(PilError::ValueError(format!(
                "operation {} has no composed native {mode} layout",
                registry::variant_key(op)
            )));
        }
    }
    Ok(layout)
}

fn is_relocation(op: &PipelineOp) -> bool {
    matches!(
        op,
        PipelineOp::Transpose { .. }
            | PipelineOp::Flip
            | PipelineOp::Mirror
            | PipelineOp::Crop { .. }
            | PipelineOp::CropBorder { .. }
            | PipelineOp::Expand { .. }
    )
}

fn is_byte_operation(op: &PipelineOp, mode: &str) -> bool {
    matches!(
        op,
        PipelineOp::Duplicate | PipelineOp::Invert | PipelineOp::InvertChops
    ) || (matches!(
        op,
        PipelineOp::Solarize { .. } | PipelineOp::Posterize { bits: 1..=8 }
    ) && matches!(mode, "L" | "RGB"))
}

fn output_dimensions(op: &PipelineOp, dimensions: (u32, u32)) -> Result<(u32, u32), PilError> {
    match op_output_dims(op, dimensions.0, dimensions.1) {
        Some(output) => Ok(output),
        None if op_has_explicit_output_dimensions(op) => Err(PilError::ValueError(
            "GPU graph has an unresolved output shape".into(),
        )),
        None => Ok(dimensions),
    }
}

fn stage_capacity_pixels(
    op: &PipelineOp,
    dimensions: (u32, u32),
    output: (u32, u32),
) -> Result<usize, PilError> {
    let (width, height) = dimensions;
    let mut pixels = CheckedDims::new(width, height, 1)?
        .total_pixels()
        .max(CheckedDims::new(output.0, output.1, 1)?.total_pixels());
    if matches!(op, PipelineOp::Resize { .. }) {
        // The horizontal pass uses destination width and source height.
        pixels = pixels.max(CheckedDims::new(output.0, height, 1)?.total_pixels());
    }
    if width == 0 || height == 0 || pixels > GPU_BUFFER_CAPACITY as usize {
        return Err(PilError::ValueError(
            "GPU graph dimensions exceed native bounds".into(),
        ));
    }
    Ok(pixels)
}

fn validate_dispatch(
    op: &PipelineOp,
    mode: &str,
    dimensions: (u32, u32),
    limits: &wgpu::Limits,
) -> Result<(), PilError> {
    if gpu_dispatch_dimensions_require_cpu(
        std::slice::from_ref(op),
        dimensions,
        limits.max_compute_workgroups_per_dimension,
        Some(mode),
        false,
    ) {
        return Err(PilError::ValueError(
            "GPU dispatch dimensions exceed device limits".into(),
        ));
    }
    Ok(())
}
