//! Filter operation registration.

use super::prelude::{PilError, Registry};

pub(super) fn register(m: &mut Registry) -> Result<(), PilError> {
    m.insert(
        "Filter3x3",
        gpu_entry!(crate::compute::pool_cpu::ops::filter::ops::prepare_pipeline, "filter_3x3.wgsl"),
    );
    m.insert(
        "Filter5x5",
        gpu_entry!(crate::compute::pool_cpu::ops::filter::ops::prepare_pipeline, "filter_5x5.wgsl"),
    );
    m.insert(
        "GaussianBlur",
        gpu_entry!(crate::compute::pool_cpu::ops::filter::ops::prepare_pipeline, "gaussian_blur.wgsl"),
    );
    m.insert(
        "BoxBlur",
        gpu_entry!(crate::compute::pool_cpu::ops::filter::ops::prepare_pipeline, "box_blur.wgsl"),
    );
    m.insert(
        "MedianFilter",
        gpu_entry!(crate::compute::pool_cpu::ops::filter::ops::prepare_pipeline, "median_filter.wgsl"),
    );
    m.insert(
        "MaxFilter",
        gpu_entry!(crate::compute::pool_cpu::ops::filter::ops::prepare_pipeline, "max_filter.wgsl"),
    );
    m.insert(
        "MinFilter",
        gpu_entry!(crate::compute::pool_cpu::ops::filter::ops::prepare_pipeline, "min_filter.wgsl"),
    );
    m.insert(
        "RankFilter",
        gpu_entry!(crate::compute::pool_cpu::ops::filter::ops::prepare_pipeline, "rank_filter.wgsl"),
    );
    Ok(())
}
