//! Filter operation registration.
use super::prelude::{
    DynamicImage, PilError, PipelineOp, Registry, execute_box_blur,
    execute_box_blur_xy_with_passes, execute_filter3x3, execute_filter5x5, execute_gaussian_blur,
    execute_max_filter_with_mode, execute_median_filter_with_mode, execute_min_filter_with_mode,
    execute_rank_filter_with_mode,
};
fn register_part_1(m: &mut Registry) {
    m.insert(
        "Filter3x3",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Filter3x3 {
                    kernel,
                    scale,
                    offset,
                } = op
                {
                    execute_filter3x3(img, kernel, *scale, *offset, mode)
                } else {
                    Err(PilError::ValueError("expected Filter3x3 op".into()))
                }
            },
            "filter_3x3.wgsl"
        ),
    );

    m.insert(
        "Filter5x5",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Filter5x5 {
                    kernel,
                    scale,
                    offset,
                } = op
                {
                    execute_filter5x5(img, kernel, *scale, *offset, mode)
                } else {
                    Err(PilError::ValueError("expected Filter5x5 op".into()))
                }
            },
            "filter_5x5.wgsl"
        ),
    );

    m.insert(
        "GaussianBlur",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::GaussianBlur { sigma } = op {
                    execute_gaussian_blur(img, *sigma)
                } else {
                    Err(PilError::ValueError("expected GaussianBlur op".into()))
                }
            },
            "gaussian_blur.wgsl"
        ),
    );

    m.insert(
        "BoxBlur",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                match op {
                    PipelineOp::BoxBlur { radius } => execute_box_blur(img, *radius),
                    PipelineOp::BoxBlurXY {
                        radius_x,
                        radius_y,
                        passes,
                    } => execute_box_blur_xy_with_passes(img, *radius_x, *radius_y, *passes),
                    _ => Err(PilError::ValueError("expected BoxBlur op".into())),
                }
            },
            "box_blur.wgsl"
        ),
    );
}

fn register_part_2(m: &mut Registry) {
    m.insert(
        "MedianFilter",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::MedianFilter { size } = op {
                    execute_median_filter_with_mode(img, *size, mode)
                } else {
                    Err(PilError::ValueError("expected MedianFilter op".into()))
                }
            },
            "median_filter.wgsl"
        ),
    );

    m.insert(
        "MaxFilter",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::MaxFilter { size } = op {
                    execute_max_filter_with_mode(img, *size, mode)
                } else {
                    Err(PilError::ValueError("expected MaxFilter op".into()))
                }
            },
            "max_filter.wgsl"
        ),
    );

    m.insert(
        "MinFilter",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::MinFilter { size } = op {
                    execute_min_filter_with_mode(img, *size, mode)
                } else {
                    Err(PilError::ValueError("expected MinFilter op".into()))
                }
            },
            "min_filter.wgsl"
        ),
    );

    m.insert(
        "RankFilter",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::RankFilter { size, rank } = op {
                    execute_rank_filter_with_mode(img, *size, *rank, mode)
                } else {
                    Err(PilError::ValueError("expected RankFilter op".into()))
                }
            },
            "rank_filter.wgsl"
        ),
    );
}

pub(super) fn register(m: &mut Registry) -> Result<(), PilError> {
    register_part_1(m);
    register_part_2(m);
    Ok(())
}
