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
        execute_filtered(
            source,
            source_dimensions,
            request,
            order,
            filtered_fill,
        )?
    };
    Ok(output)
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
        source_plane.extend(row.iter().flat_map(|&sample| order.encode(sample)).take(row_width));
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
