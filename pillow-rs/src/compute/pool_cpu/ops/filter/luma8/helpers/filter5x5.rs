use crate::compute::pool_cpu::ops::filter::helpers;

/// Execute the exact uniform or binomial L vector kernel when it applies.
pub(crate) fn execute_special(
    raw: &[u8],
    width: u32,
    height: u32,
    kernel: &[f32; 25],
    scale: f32,
    offset: f32,
) -> Option<Vec<u8>> {
    let uniform = helpers::uniform5x5_parameters_match(kernel, scale, offset);
    let binomial = helpers::binomial5x5_parameters_match(kernel, scale, offset);
    if !uniform && !binomial {
        return None;
    }
    let mut output = raw.to_vec();
    let result = if uniform {
        super::uniform5x5_kernel::filter(
            raw,
            &mut output,
            width as usize,
            height as usize,
        )
    } else {
        super::binomial5x5_kernel::filter(
            raw,
            &mut output,
            width as usize,
            height as usize,
        )
    }?;
    crate::compute::record_pipeline_operation_path("vector");
    if result.0 != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(result.0);
    }
    if result.1 != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(result.1);
    }
    Some(output)
}
