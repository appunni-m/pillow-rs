//! Typed I;16 affine kernels, preserving the archived byte-filter contract.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::rotate::helpers::vector_interpolation;
use crate::compute::pool_cpu::ops::transform::helpers::{
    luma16::ByteOrder, vector_affine_bilinear,
};
use crate::compute::pool_cpu::ops::transform::ops::TransformRequest;
use crate::error::PilError;
use crate::pipeline::{ResampleFilter, TransformMethod};
use wide::{f64x8, u16x8};

pub(crate) fn execute(
    source: &[u16],
    source_dimensions: (u32, u32),
    request: TransformRequest<'_>,
    order: ByteOrder,
) -> Result<Vec<u16>, PilError> {
    let (source_layout, destination, fill) = prepare(source, source_dimensions, request)?;
    let mut output = vec![0; destination.total_pixels()];
    execute_prepared(
        source,
        source_layout,
        destination,
        request,
        order,
        fill,
        &mut output,
    )?;
    Ok(output)
}

pub(crate) fn execute_into(
    source: &[u16],
    source_dimensions: (u32, u32),
    request: TransformRequest<'_>,
    order: ByteOrder,
    output: &mut [u16],
) -> Result<(), PilError> {
    let (source_layout, destination, fill) = prepare(source, source_dimensions, request)?;
    if output.len() != destination.total_pixels() {
        return Err(PilError::InternalError(
            "I;16 Transform destination buffer shape mismatch".into(),
        ));
    }
    execute_prepared(
        source,
        source_layout,
        destination,
        request,
        order,
        fill,
        output,
    )
}

fn prepare(
    source: &[u16],
    source_dimensions: (u32, u32),
    request: TransformRequest<'_>,
) -> Result<(CheckedDims, CheckedDims, u16), PilError> {
    if !matches!(request.method, TransformMethod::Affine) {
        return Err(PilError::InternalError(
            "I;16 architecture Transform received a non-affine request".into(),
        ));
    }
    let source_layout = CheckedDims::new(source_dimensions.0, source_dimensions.1, 2)?;
    let destination = CheckedDims::new(request.width, request.height, 2)?;
    if source.len() != source_layout.total_pixels() {
        return Err(PilError::InternalError(
            "I;16 Transform source buffer shape mismatch".into(),
        ));
    }
    let fill = request
        .fill
        .map_or(0, |color| u16::from_le_bytes([color.0, color.1]));
    Ok((source_layout, destination, fill))
}

fn execute_prepared(
    source: &[u16],
    source_layout: CheckedDims,
    destination: CheckedDims,
    request: TransformRequest<'_>,
    order: ByteOrder,
    fill: u16,
    output: &mut [u16],
) -> Result<(), PilError> {
    match request.filter {
        ResampleFilter::Nearest => {
            execute_nearest_into(source, source_layout, destination, request, fill, output)
        }
        ResampleFilter::Bilinear => execute_bilinear_into(
            source,
            source_layout,
            destination,
            request,
            order,
            fill,
            output,
        ),
        _ => Err(PilError::InternalError(
            "I;16 architecture Transform received an unsupported filter".into(),
        )),
    }
}

fn execute_nearest_into(
    source: &[u16],
    source_layout: CheckedDims,
    destination: CheckedDims,
    request: TransformRequest<'_>,
    fill: u16,
    output: &mut [u16],
) -> Result<(), PilError> {
    let [a, b, c, d, e, f] = request.data else {
        return Err(PilError::InternalError(
            "I;16 affine Transform coefficients changed after admission".into(),
        ));
    };
    let source_width = source_layout.width as usize;
    let source_height = source_layout.height as usize;
    let width = destination.width as usize;
    let height = destination.height as usize;
    let mut vector_blocks = 0u64;

    for y in 0..height {
        let mut x = 0usize;
        while x < width {
            let count = (width - x).min(vector_interpolation::LANES);
            let destination_x = f64x8::new(std::array::from_fn(|lane| {
                if lane < count {
                    (x + lane) as f64 + 0.5
                } else {
                    0.0
                }
            }));
            let destination_y = f64x8::splat(y as f64 + 0.5);
            let source_x = (vector_interpolation::fused_mul_add(
                f64x8::splat(*a),
                destination_x,
                f64x8::splat(*b) * destination_y,
            ) + f64x8::splat(*c))
            .to_array();
            let source_y = (vector_interpolation::fused_mul_add(
                f64x8::splat(*d),
                destination_x,
                f64x8::splat(*e) * destination_y,
            ) + f64x8::splat(*f))
            .to_array();
            let mut samples = [fill; vector_interpolation::LANES];
            for lane in 0..count {
                let input_x = floor_coordinate(source_x[lane]);
                let input_y = floor_coordinate(source_y[lane]);
                if input_x >= 0
                    && input_x < source_width as i64
                    && input_y >= 0
                    && input_y < source_height as i64
                {
                    samples[lane] = source[input_y as usize * source_width + input_x as usize];
                }
            }
            let output_start = y * width + x;
            output[output_start..output_start + count]
                .copy_from_slice(&u16x8::new(samples).to_array()[..count]);
            vector_blocks = vector_blocks.saturating_add(1);
            x += count;
        }
    }
    record_vector_work(vector_blocks);
    Ok(())
}

fn execute_bilinear_into(
    source: &[u16],
    source_layout: CheckedDims,
    destination: CheckedDims,
    request: TransformRequest<'_>,
    order: ByteOrder,
    fill: u16,
    output: &mut [u16],
) -> Result<(), PilError> {
    let fill_bytes = order.encode(fill);
    let source_width = source_layout.width as usize;
    let mut source_plane = Vec::with_capacity(source_layout.total_pixels());
    for row in source.chunks_exact(source_width) {
        source_plane.extend(
            row.iter()
                .flat_map(|&sample| order.encode(sample))
                .take(source_width),
        );
    }
    let byte_plane_request = TransformRequest {
        fill: Some((fill_bytes[0], 0, 0, 0)),
        ..request
    };
    // Pillow's I;16 filtered affine path samples the first `width` bytes of
    // each native row, then restores the untouched second fill byte. Keep this
    // scratch byte plane local; the input and output remain typed I;16 images.
    let output_plane = vector_affine_bilinear::execute::<1, false>(
        &source_plane,
        (source_layout.width, source_layout.height),
        byte_plane_request,
    )?;
    if output_plane.len() != destination.total_pixels() {
        return Err(PilError::InternalError(
            "I;16 bilinear Transform output plane shape mismatch".into(),
        ));
    }
    for (destination, sample) in output.iter_mut().zip(output_plane) {
        *destination = order.decode([sample, fill_bytes[1]]);
    }
    Ok(())
}

fn floor_coordinate(value: f64) -> i64 {
    if value.is_finite() {
        value.floor() as i64
    } else {
        -1
    }
}

fn record_vector_work(blocks: u64) {
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
}
