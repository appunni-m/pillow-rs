//! Native I;16 Transform handling, including Pillow's special byte-filter ABI.

use crate::compute::pool_cpu::ops::transform::ops::TransformRequest;
use crate::error::PilError;
use crate::pipeline::{ResampleFilter, TransformMethod};

#[derive(Clone, Copy)]
pub(crate) enum ByteOrder {
    Native,
    Little,
    Big,
}

pub(crate) fn execute(
    source: &[u16],
    source_dimensions: (u32, u32),
    request: TransformRequest<'_>,
    order: ByteOrder,
) -> Result<Vec<u16>, PilError> {
    let nearest = matches!(request.filter, ResampleFilter::Nearest);
    let fill_value = request
        .fill
        .map_or(0, |fill| u16::from_le_bytes([fill.0, fill.1]));
    let filtered_fill = order.encode(fill_value);
    let output = if nearest {
        execute_nearest(source, source_dimensions, request, fill_value)?
    } else {
        execute_filtered(source, source_dimensions, request, order, filtered_fill)?
    };
    Ok(output)
}

pub(crate) fn execute_into(
    source: &[u16],
    source_dimensions: (u32, u32),
    request: TransformRequest<'_>,
    order: ByteOrder,
    output: &mut [u16],
) -> Result<(), PilError> {
    let output_len = (request.width as usize)
        .checked_mul(request.height as usize)
        .ok_or_else(|| PilError::ValueError("image dimensions are too large".into()))?;
    if output.len() != output_len {
        return Err(PilError::InternalError(
            "I;16 Transform destination buffer shape mismatch".into(),
        ));
    }
    let fill_value = request
        .fill
        .map_or(0, |fill| u16::from_le_bytes([fill.0, fill.1]));
    if matches!(request.filter, ResampleFilter::Nearest) {
        execute_nearest_into(source, source_dimensions, request, fill_value, output)
    } else {
        execute_filtered_into(
            source,
            source_dimensions,
            request,
            order,
            order.encode(fill_value),
            output,
        )
    }
}

pub(crate) fn apply_mesh_record(
    source: &[u16],
    destination: &mut [u16],
    source_dimensions: (u32, u32),
    request: TransformRequest<'_>,
    order: ByteOrder,
    box_coordinates: (i64, i64, i64, i64),
    coefficients: [f64; 8],
    source_scratch: &mut Vec<u8>,
    output_plane: &mut Option<Vec<u8>>,
) -> Result<(), PilError> {
    let source_bytes = bytemuck::cast_slice(source);
    let (dst_w, dst_h) = (request.width, request.height);
    if matches!(request.filter, ResampleFilter::Nearest) {
        if let Some(plane) = output_plane.take() {
            finish_mesh(destination, &plane, order)?;
        }
        return super::projective::transform_mesh_record_with_coefficients(
            source_bytes,
            source_dimensions,
            (dst_w, dst_h),
            box_coordinates,
            coefficients,
            bytemuck::cast_slice_mut(destination),
            2,
            request.filter,
            request.fill_is_none,
        );
    }
    let fill_value = if request.fill_is_none {
        0
    } else {
        request
            .fill
            .map_or(0, |fill| u16::from_le_bytes([fill.0, fill.1]))
    };
    prepare_output_plane(destination, order, output_plane)?;
    filtered_mesh_source_plane(source, source_dimensions, order, source_scratch)?;
    let plane = output_plane.as_mut().ok_or_else(invalid_mesh_shape)?;
    super::projective::transform_mesh_record_with_coefficients(
        source_scratch,
        source_dimensions,
        (dst_w, dst_h),
        box_coordinates,
        coefficients,
        plane,
        1,
        request.filter,
        request.fill_is_none,
    )?;
    set_filtered_mesh_high_byte(
        destination,
        request,
        source_dimensions,
        box_coordinates,
        coefficients,
        order,
        fill_value,
    );
    Ok(())
}

fn filtered_mesh_source_plane(
    source: &[u16],
    dimensions: (u32, u32),
    order: ByteOrder,
    output: &mut Vec<u8>,
) -> Result<(), PilError> {
    let row_width = dimensions.0 as usize;
    let expected_len = row_width
        .checked_mul(dimensions.1 as usize)
        .ok_or_else(invalid_mesh_shape)?;
    output.clear();
    output
        .try_reserve_exact(expected_len)
        .map_err(|error| PilError::MemoryError(error.to_string()))?;
    // Pillow's filtered I;16 MESH output matches the first `row_width`
    // encoded bytes in each source row, sampled as one-byte pixels. Preserve
    // that mode-specific output while keeping the source raster typed u16.
    for row in source.chunks(row_width.max(1)) {
        output.extend(
            row.iter()
                .flat_map(|&sample| order.encode(sample))
                .take(row_width),
        );
    }
    if output.len() != expected_len {
        return Err(invalid_mesh_shape());
    }
    Ok(())
}

pub(crate) fn finish_mesh_records(
    destination: &mut [u16],
    order: ByteOrder,
    output_plane: Option<&[u8]>,
) -> Result<(), PilError> {
    let Some(plane) = output_plane else {
        return Ok(());
    };
    finish_mesh(destination, plane, order)
}

fn prepare_output_plane(
    destination: &[u16],
    order: ByteOrder,
    output_plane: &mut Option<Vec<u8>>,
) -> Result<(), PilError> {
    if output_plane.is_none() {
        let mut plane = Vec::new();
        plane
            .try_reserve_exact(destination.len())
            .map_err(|error| PilError::MemoryError(error.to_string()))?;
        plane.extend(destination.iter().map(|&sample| order.encode(sample)[0]));
        *output_plane = Some(plane);
    }
    Ok(())
}

fn finish_mesh(
    destination: &mut [u16],
    output_plane: &[u8],
    order: ByteOrder,
) -> Result<(), PilError> {
    if destination.len() != output_plane.len() {
        return Err(invalid_mesh_shape());
    }
    for (sample, &first) in destination.iter_mut().zip(output_plane) {
        let mut bytes = order.encode(*sample);
        bytes[0] = first;
        *sample = order.decode(bytes);
    }
    Ok(())
}

fn set_filtered_mesh_high_byte(
    destination: &mut [u16],
    request: TransformRequest<'_>,
    source_dimensions: (u32, u32),
    box_coordinates: (i64, i64, i64, i64),
    coefficients: [f64; 8],
    order: ByteOrder,
    fill: u16,
) {
    let (left, top, right, bottom) = clipped_box(box_coordinates, (request.width, request.height));
    let high = order.encode(fill)[1];
    let width = request.width as usize;
    for y in top..bottom {
        for x in left..right {
            let index = y as usize * width + x as usize;
            let local_x = (x - left) as f64 + 0.5;
            let local_y = (y - top) as f64 + 0.5;
            let (source_x, source_y) =
                super::projective::transform_mesh_source_coordinate(coefficients, local_x, local_y);
            let source_is_valid = source_x.is_finite()
                && source_y.is_finite()
                && source_x >= 0.0
                && source_y >= 0.0
                && source_x < f64::from(source_dimensions.0)
                && source_y < f64::from(source_dimensions.1);
            if !source_is_valid && !request.fill_is_none {
                // Keep a previous record's high byte when an explicit-fill
                // sample misses the source, just as the low-byte kernel does.
                continue;
            }
            let mut bytes = order.encode(destination[index]);
            bytes[1] = high;
            destination[index] = order.decode(bytes);
        }
    }
}

fn clipped_box(coordinates: (i64, i64, i64, i64), dimensions: (u32, u32)) -> (i64, i64, i64, i64) {
    let (left, top, right, bottom) = coordinates;
    let (width, height) = dimensions;
    (
        left.clamp(0, i64::from(width)),
        top.clamp(0, i64::from(height)),
        right.clamp(0, i64::from(width)),
        bottom.clamp(0, i64::from(height)),
    )
}

fn invalid_mesh_shape() -> PilError {
    PilError::InternalError("Transform I;16 MESH buffer shape mismatch".into())
}

fn execute_nearest(
    source: &[u16],
    source_dimensions: (u32, u32),
    request: TransformRequest<'_>,
    fill: u16,
) -> Result<Vec<u16>, PilError> {
    let source_byte_count = source
        .len()
        .checked_mul(2)
        .ok_or_else(|| PilError::ValueError("image dimensions are too large".into()))?;
    let mut source_bytes = Vec::with_capacity(source_byte_count);
    for sample in source {
        source_bytes.extend(sample.to_ne_bytes());
    }
    let fill_bytes = fill.to_ne_bytes();
    let fill = Some((fill_bytes[0], fill_bytes[1], 0, 0));
    let output_bytes = match request.method {
        TransformMethod::Affine => {
            let mut projective = request.data.to_vec();
            projective.extend([0.0, 0.0]);
            super::projective::transform_projective_generic(
                &source_bytes,
                source_dimensions,
                request.width,
                request.height,
                &projective,
                &request.filter,
                fill,
                2,
                false,
            )?
        }
        TransformMethod::Perspective | TransformMethod::Quad => {
            super::projective::transform_projective_generic(
                &source_bytes,
                source_dimensions,
                request.width,
                request.height,
                request.data,
                &request.filter,
                fill,
                2,
                matches!(request.method, TransformMethod::Quad),
            )?
        }
        TransformMethod::Mesh => super::projective::transform_mesh_with_filter(
            &source_bytes,
            source_dimensions,
            request.width,
            request.height,
            request.data,
            fill,
            2,
            request.filter,
            request.fill_is_none,
        )?,
    };
    Ok(output_bytes
        .chunks_exact(2)
        .map(|bytes| u16::from_ne_bytes([bytes[0], bytes[1]]))
        .collect())
}

fn execute_nearest_into(
    source: &[u16],
    source_dimensions: (u32, u32),
    request: TransformRequest<'_>,
    fill: u16,
    output: &mut [u16],
) -> Result<(), PilError> {
    let source_byte_count = source
        .len()
        .checked_mul(2)
        .ok_or_else(|| PilError::ValueError("image dimensions are too large".into()))?;
    let mut source_bytes = Vec::with_capacity(source_byte_count);
    for sample in source {
        source_bytes.extend(sample.to_ne_bytes());
    }
    let fill_bytes = fill.to_ne_bytes();
    let fill = Some((fill_bytes[0], fill_bytes[1], 0, 0));
    let output_bytes = bytemuck::cast_slice_mut(output);
    match request.method {
        TransformMethod::Affine => {
            let mut projective = request.data.to_vec();
            projective.extend([0.0, 0.0]);
            super::projective::transform_projective_generic_into(
                &source_bytes,
                source_dimensions,
                request.width,
                request.height,
                &projective,
                &request.filter,
                fill,
                2,
                false,
                output_bytes,
            )
        }
        TransformMethod::Perspective | TransformMethod::Quad => {
            super::projective::transform_projective_generic_into(
                &source_bytes,
                source_dimensions,
                request.width,
                request.height,
                request.data,
                &request.filter,
                fill,
                2,
                matches!(request.method, TransformMethod::Quad),
                output_bytes,
            )
        }
        TransformMethod::Mesh => super::projective::transform_mesh_with_filter_into(
            &source_bytes,
            source_dimensions,
            request.width,
            request.height,
            request.data,
            fill,
            2,
            request.filter,
            request.fill_is_none,
            output_bytes,
        ),
    }
}

fn execute_filtered(
    source: &[u16],
    source_dimensions: (u32, u32),
    request: TransformRequest<'_>,
    order: ByteOrder,
    fill: [u8; 2],
) -> Result<Vec<u16>, PilError> {
    let row_width = usize::try_from(source_dimensions.0)
        .map_err(|_| PilError::ValueError("image dimensions are too large".into()))?;
    let source_plane_len = row_width
        .checked_mul(source_dimensions.1 as usize)
        .ok_or_else(|| PilError::ValueError("image dimensions are too large".into()))?;
    let mut source_plane = Vec::with_capacity(source_plane_len);
    for row in source.chunks(row_width.max(1)) {
        source_plane.extend(
            row.iter()
                .flat_map(|&sample| order.encode(sample))
                .take(row_width),
        );
    }
    let fill_color = Some((fill[0], fill[1], 0, 0));
    let output_plane = match request.method {
        TransformMethod::Affine => super::affine::transform_affine_generic(
            &source_plane,
            source_dimensions,
            request.width,
            request.height,
            request.data[0],
            request.data[1],
            request.data[2],
            request.data[3],
            request.data[4],
            request.data[5],
            fill_color,
            1,
            false,
            matches!(request.filter, ResampleFilter::Bicubic),
        )?,
        TransformMethod::Perspective | TransformMethod::Quad => {
            super::projective::transform_projective_generic(
                &source_plane,
                source_dimensions,
                request.width,
                request.height,
                request.data,
                &request.filter,
                fill_color,
                1,
                matches!(request.method, TransformMethod::Quad),
            )?
        }
        TransformMethod::Mesh => super::projective::transform_mesh_with_filter(
            &source_plane,
            source_dimensions,
            request.width,
            request.height,
            request.data,
            fill_color,
            1,
            request.filter,
            request.fill_is_none,
        )?,
    };
    Ok(output_plane
        .into_iter()
        .map(|first| order.decode([first, fill[1]]))
        .collect())
}

fn execute_filtered_into(
    source: &[u16],
    source_dimensions: (u32, u32),
    request: TransformRequest<'_>,
    order: ByteOrder,
    fill: [u8; 2],
    output: &mut [u16],
) -> Result<(), PilError> {
    let row_width = usize::try_from(source_dimensions.0)
        .map_err(|_| PilError::ValueError("image dimensions are too large".into()))?;
    let source_plane_len = row_width
        .checked_mul(source_dimensions.1 as usize)
        .ok_or_else(|| PilError::ValueError("image dimensions are too large".into()))?;
    let output_plane_len = (request.width as usize)
        .checked_mul(request.height as usize)
        .ok_or_else(|| PilError::ValueError("image dimensions are too large".into()))?;
    let mut source_plane = Vec::with_capacity(source_plane_len);
    for row in source.chunks(row_width.max(1)) {
        source_plane.extend(
            row.iter()
                .flat_map(|&sample| order.encode(sample))
                .take(row_width),
        );
    }
    let mut output_plane = vec![fill[0]; output_plane_len];
    let fill_color = Some((fill[0], fill[1], 0, 0));
    match request.method {
        TransformMethod::Affine => super::affine::transform_affine_generic_into(
            &source_plane,
            source_dimensions,
            request.width,
            request.height,
            request.data[0],
            request.data[1],
            request.data[2],
            request.data[3],
            request.data[4],
            request.data[5],
            fill_color,
            1,
            false,
            matches!(request.filter, ResampleFilter::Bicubic),
            &mut output_plane,
        )?,
        TransformMethod::Perspective | TransformMethod::Quad => {
            super::projective::transform_projective_generic_into(
                &source_plane,
                source_dimensions,
                request.width,
                request.height,
                request.data,
                &request.filter,
                fill_color,
                1,
                matches!(request.method, TransformMethod::Quad),
                &mut output_plane,
            )?
        }
        TransformMethod::Mesh => super::projective::transform_mesh_with_filter_into(
            &source_plane,
            source_dimensions,
            request.width,
            request.height,
            request.data,
            fill_color,
            1,
            request.filter,
            request.fill_is_none,
            &mut output_plane,
        )?,
    }
    for (sample, &first) in output.iter_mut().zip(&output_plane) {
        *sample = order.decode([first, fill[1]]);
    }
    Ok(())
}

impl ByteOrder {
    pub(crate) fn encode(self, value: u16) -> [u8; 2] {
        match self {
            Self::Native => value.to_ne_bytes(),
            Self::Little => value.to_le_bytes(),
            Self::Big => value.to_be_bytes(),
        }
    }

    pub(crate) fn decode(self, bytes: [u8; 2]) -> u16 {
        match self {
            Self::Native => u16::from_ne_bytes(bytes),
            Self::Little => u16::from_le_bytes(bytes),
            Self::Big => u16::from_be_bytes(bytes),
        }
    }
}
