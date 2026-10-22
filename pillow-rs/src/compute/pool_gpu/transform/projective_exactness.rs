use super::super::*;
use super::*;

/// Prove a nearest projective/quad/mesh transform for raw byte samples.
///
/// Pillow's `src/libImaging/Geometry.c` `quad_transform` and mesh path evaluate
/// the inverse map in `f64`, while the projective shader evaluates destination
/// coordinates in `f32` and then rounds its selected source coordinate. Admit
/// only a bounded proof where every coefficient, destination coordinate,
/// intermediate arithmetic result, and final source coordinate select the
/// same source pixel in both domains.
/// This covers proof-certified f32-representable maps for ordinary packed byte
/// modes and raw indexed samples without claiming parity for arbitrary
/// homographies or mesh records. Nearest Quad and Mesh maps use the same
/// exhaustive source-selection proof, with explicit finite-intermediate
/// guards for their generic f32 bilinear arithmetic. Filtered projective
/// transforms remain host controlled unless an L/LA/RGB/RGBA/PA Perspective,
/// Quad, or complete one-record Mesh map is a unit-scale integer relocation or
/// a constant half-pixel source coordinate, or an interior integer coordinate
/// with the dedicated Bicubic tap proof. The Bilinear/relocation envelopes
/// land exactly on one source pixel, while the Bicubic envelope uses the
/// integer `[-1, 5, 5, -1] / 8` tap reduction; LA/RGBA also mirror Pillow's
/// premultiplied round trip.
// The comparison is intentional: the current shader receives raw destination
// indices and uses `floor(source + 0.5)`, whereas Pillow adds the destination
// pixel center before evaluating the map and then applies `COORD` truncation.
pub(in crate::compute::pool_gpu) fn gpu_projective_nearest_is_exact(
    op: &PipelineOp,
    image: &DynamicImage,
    mode: Option<&str>,
    source_dimensions: (u32, u32),
) -> bool {
    let image_layout_is_valid = match mode {
        Some("P" | "1" | "L") => matches!(image, DynamicImage::ImageLuma8(_)),
        Some("LA" | "PA") => matches!(image, DynamicImage::ImageLumaA8(_)),
        Some("RGB" | "HSV" | "YCbCr") => matches!(image, DynamicImage::ImageRgb8(_)),
        Some("RGBA" | "RGBX" | "RGBa" | "CMYK") => {
            matches!(image, DynamicImage::ImageRgba8(_))
        }
        _ => false,
    };
    image_layout_is_valid && gpu_projective_uniform_is_exact(op, mode, source_dimensions)
}

pub(in crate::compute::pool_gpu) fn gpu_projective_uniform_is_exact(
    op: &PipelineOp,
    mode: Option<&str>,
    source_dimensions: (u32, u32),
) -> bool {
    let PipelineOp::Transform {
        w,
        h,
        method,
        data,
        filter,
        fill_is_none,
        ..
    } = op
    else {
        return false;
    };
    let ordinary_byte_mode = matches!(
        mode,
        Some("L" | "LA" | "RGB" | "RGBA" | "RGBX" | "RGBa" | "CMYK" | "HSV" | "YCbCr")
    );
    let filtered_constant = gpu_projective_filtered_constant_is_admitted(
        method.clone(),
        data,
        *filter,
        mode,
        source_dimensions,
        (*w, *h),
    );
    let filtered_relocation = filtered_constant
        || gpu_projective_filtered_relocation_is_admitted(
            method.clone(),
            data,
            *filter,
            mode,
            (*w, *h),
        );
    let palette_alpha_relocation = mode == Some("PA")
        && (gpu_palette_alpha_projective_relocation_is_admitted(
            method.clone(),
            data,
            *filter,
            (*w, *h),
        ) || filtered_relocation);
    // Perspective, Quad, and Mesh maps use the shader's raw-gid f32
    // arithmetic, while the exhaustive source-selection proof below compares
    // it with Pillow's centered f64/COORD result for every output pixel.
    // Finite-intermediate guards in the proof keep generic Quad/Mesh
    // bilinear overflow from bypassing the shader sampler's bounds checks.
    // Filtered transforms intentionally remain limited to zero-weight
    // relocations, constant half-pixel maps, and the interior integer
    // Bilinear/Bicubic envelopes proven above.
    let ordinary_projective_geometry_is_admitted = match method {
        TransformMethod::Perspective => true,
        TransformMethod::Quad | TransformMethod::Mesh => true,
        TransformMethod::Affine => false,
    };
    if !matches!(
        mode,
        Some(
            "P" | "1"
                | "L"
                | "LA"
                | "PA"
                | "RGB"
                | "HSV"
                | "YCbCr"
                | "RGBA"
                | "RGBX"
                | "RGBa"
                | "CMYK"
        )
    ) || (ordinary_byte_mode && !ordinary_projective_geometry_is_admitted)
        || (mode == Some("PA") && !palette_alpha_relocation)
        || !matches!(
            method,
            TransformMethod::Perspective | TransformMethod::Quad | TransformMethod::Mesh
        )
        || (!gpu_transform_uses_nearest(mode, *filter) && !filtered_relocation)
        || source_dimensions.0 == 0
        || source_dimensions.1 == 0
        || *w == 0
        || *h == 0
        || source_dimensions.0 > (1 << 24)
        || source_dimensions.1 > (1 << 24)
        || *w > (1 << 24)
        || *h > (1 << 24)
        || u64::from(*w).saturating_mul(u64::from(*h)) > 1_048_576
    {
        return false;
    }

    let required_data = match method {
        TransformMethod::Perspective | TransformMethod::Quad => 8,
        TransformMethod::Mesh => 12,
        TransformMethod::Affine => return false,
    };
    if data.len() < required_data || (matches!(method, TransformMethod::Mesh) && data.len() != 12) {
        return false;
    }
    // The transform uniform stores all coefficients as f32 bit patterns. Do
    // not admit a value that changes when it crosses that ABI boundary.
    let f32_exact = |value: f64| {
        let narrowed = value as f32;
        narrowed.is_finite() && f64::from(narrowed) == value
    };
    if data[..required_data]
        .iter()
        .copied()
        .any(|value| !f32_exact(value))
    {
        return false;
    }

    // The shader carries one Mesh record and can only represent a partial
    // record's outside-bbox result with an explicit fill word.  Its direct
    // and axis-swapped unit relocation lowering is exact for an in-output
    // integer bbox; keep fractional/scaled/partially clipped records and
    // no-fill records on the host path rather than letting the generic
    // shader's clipping or default fill semantics stand in for Geometry.c.
    let mesh_is_full_output = !matches!(method, TransformMethod::Mesh)
        || data[..4] == [0.0, 0.0, f64::from(*w), f64::from(*h)];
    let mesh_is_partial_relocation = matches!(method, TransformMethod::Mesh)
        && gpu_mesh_unit_relocation_is_admitted(data, *w, *h);
    if matches!(method, TransformMethod::Mesh)
        && !mesh_is_full_output
        && !mesh_is_partial_relocation
    {
        return false;
    }
    if matches!(method, TransformMethod::Mesh) && !mesh_is_full_output && *fill_is_none {
        return false;
    }
    // The constant filtered proof already checks the complete source window
    // and exact dyadic arithmetic independently of destination coordinates.
    // Reusing it avoids an image-sized scan when selecting the uniform path.
    if filtered_constant {
        return true;
    }

    let source_at = |dx: f64, dy: f64| -> Option<(f64, f64)> {
        match method {
            TransformMethod::Perspective => {
                let dx = dx + 0.5;
                let dy = dy + 0.5;
                let denominator = data[6] * dx + data[7] * dy + 1.0;
                if denominator == 0.0 || !denominator.is_finite() {
                    return None;
                }
                let sx = (data[0] * dx + data[1] * dy + data[2]) / denominator;
                let sy = (data[3] * dx + data[4] * dy + data[5]) / denominator;
                (sx.is_finite() && sy.is_finite()).then_some((sx, sy))
            }
            TransformMethod::Quad => {
                if gpu_quad_constant_map_is_admitted(data) {
                    return Some((data[0], data[1]));
                }
                let dx = dx + 0.5;
                let dy = dy + 0.5;
                let sw = f64::from(*w);
                let sh = f64::from(*h);
                let u = dx / sw;
                let v = dy / sh;
                let x0 = data[0];
                let y0 = data[1];
                let sx = x0
                    + (data[6] - x0) * u
                    + (data[2] - x0) * v
                    + (data[4] - data[2] - data[6] + x0) * u * v;
                let sy = y0
                    + (data[7] - y0) * u
                    + (data[3] - y0) * v
                    + (data[5] - data[3] - data[7] + y0) * u * v;
                (sx.is_finite() && sy.is_finite()).then_some((sx, sy))
            }
            TransformMethod::Mesh => {
                // The shader emits the fill word outside a record's bbox.
                // Compare source selection only for pixels inside the
                // record, just as Geometry.c's clipped destination loop
                // does; a full-output record naturally passes this check.
                if dx < data[0] || dx >= data[2] || dy < data[1] || dy >= data[3] {
                    return None;
                }
                if gpu_mesh_constant_map_is_admitted(data, *w, *h) {
                    return Some((data[4], data[5]));
                }
                if ordinary_byte_mode && gpu_mesh_unit_relocation_is_admitted(data, *w, *h) {
                    // For the admitted unit relocations, mirror the CPU
                    // quad_transform FMA order directly.  Reconstructing the
                    // same affine map from four bilinear weights would add a
                    // second rounding shape to the proof at large integer
                    // translations, even though Pillow's coefficients have
                    // already reduced to a unit step.
                    let local_x = dx + 0.5 - data[0];
                    let local_y = dy + 0.5 - data[1];
                    let box_width = data[2] - data[0];
                    // Mesh source corners are NW/SW/SE/NE.  The direct
                    // relocation therefore has its NE corner one width to
                    // the right of TL and at the same Y; checking the
                    // opposite diagonal admits a scaled/axis map instead.
                    let direct = data[10] == data[4] + box_width && data[11] == data[5];
                    let (sx, sy) = if direct {
                        (data[4] + local_x, data[5] + local_y)
                    } else {
                        (data[4] + local_y, data[5] + local_x)
                    };
                    return (sx.is_finite() && sy.is_finite()).then_some((sx, sy));
                }
                let bbox_width = data[2] - data[0];
                let bbox_height = data[3] - data[1];
                if bbox_width <= 0.0 || bbox_height <= 0.0 {
                    return None;
                }
                let u = (dx + 0.5 - data[0]) / bbox_width;
                let v = (dy + 0.5 - data[1]) / bbox_height;
                let x0 = data[4];
                let y0 = data[5];
                let sx = (1.0 - u) * (1.0 - v) * x0
                    + u * (1.0 - v) * data[10]
                    + u * v * data[8]
                    + (1.0 - u) * v * data[6];
                let sy = (1.0 - u) * (1.0 - v) * y0
                    + u * (1.0 - v) * data[11]
                    + u * v * data[9]
                    + (1.0 - u) * v * data[7];
                (sx.is_finite() && sy.is_finite()).then_some((sx, sy))
            }
            TransformMethod::Affine => None,
        }
    };
    let shader_source_at = |dx: f32, dy: f32| -> Option<(f32, f32)> {
        let f = |index: usize| data[index] as f32;
        match method {
            TransformMethod::Perspective => {
                if matches!(filter, ResampleFilter::Bilinear | ResampleFilter::Bicubic)
                    && f(6) == 0.0
                    && f(7) == 0.0
                    && f(0) == 0.0
                    && f(1) == 0.0
                    && f(3) == 0.0
                    && f(4) == 0.0
                {
                    return Some((f(2) - 0.5, f(5) - 0.5));
                }
                if gpu_perspective_signed_unit_relocation_is_admitted(data) {
                    let a = f(0);
                    let b = f(1);
                    let d = f(3);
                    let e = f(4);
                    let sx = if a == 1.0 {
                        f(2) + dx
                    } else if a == -1.0 {
                        f(2) - dx - 1.0
                    } else if b == 1.0 {
                        f(2) + dy
                    } else {
                        f(2) - dy - 1.0
                    };
                    let sy = if d == 1.0 {
                        f(5) + if b.abs() == 1.0 { dx } else { dy }
                    } else if d == -1.0 {
                        f(5) - if b.abs() == 1.0 { dx } else { dy } - 1.0
                    } else if e == 1.0 {
                        f(5) + dy
                    } else {
                        f(5) - dy - 1.0
                    };
                    return (sx.is_finite() && sy.is_finite()).then_some((sx, sy));
                }
                if f(6) == 0.0 && f(7) == 0.0 && matches!(filter, ResampleFilter::Nearest) {
                    let center_x = dx + 0.5;
                    let center_y = dy + 0.5;
                    let sx = (f(0) * center_x + f(1) * center_y + f(2)).floor();
                    let sy = (f(3) * center_x + f(4) * center_y + f(5)).floor();
                    return (sx.is_finite() && sy.is_finite()).then_some((sx, sy));
                }
                let denominator = f(6) * dx + f(7) * dy + 1.0;
                if denominator == 0.0 || !denominator.is_finite() {
                    return None;
                }
                let sx = (f(0) * dx + f(1) * dy + f(2)) / denominator;
                let sy = (f(3) * dx + f(4) * dy + f(5)) / denominator;
                (sx.is_finite() && sy.is_finite()).then_some((sx, sy))
            }
            TransformMethod::Quad => {
                let x0 = f(0);
                let y0 = f(1);
                if x0 == f(2) && x0 == f(4) && x0 == f(6) && y0 == f(3) && y0 == f(5) && y0 == f(7)
                {
                    // Geometry.c filters after subtracting 0.5 from the
                    // mapped source coordinate.  A constant half-pixel map
                    // therefore lands exactly on one source pixel, while
                    // the projective shader's bilinear sampler expects the
                    // already-shifted coordinate.
                    if !matches!(filter, ResampleFilter::Nearest) {
                        return Some((x0 - 0.5, y0 - 0.5));
                    }
                    return Some((x0, y0));
                }
                let width = *w as f32;
                let height = *h as f32;
                let u = dx / width;
                let v = dy / height;
                let sx = x0 + (f(6) - x0) * u + (f(2) - x0) * v + (f(4) - f(2) - f(6) + x0) * u * v;
                let sy = y0 + (f(7) - y0) * u + (f(3) - y0) * v + (f(5) - f(3) - f(7) + y0) * u * v;
                (sx.is_finite() && sy.is_finite()).then_some((sx, sy))
            }
            TransformMethod::Mesh => {
                let bx0 = f(0);
                let by0 = f(1);
                let bx1 = f(2);
                let by1 = f(3);
                if dx < bx0 || dx >= bx1 || dy < by0 || dy >= by1 {
                    return None;
                }
                let width = *w as f32;
                let height = *h as f32;
                let x0 = f(4);
                let y0 = f(5);
                if x0 == f(6)
                    && x0 == f(8)
                    && x0 == f(10)
                    && y0 == f(7)
                    && y0 == f(9)
                    && y0 == f(11)
                {
                    if !matches!(filter, ResampleFilter::Nearest) {
                        return Some((x0 - 0.5, y0 - 0.5));
                    }
                    return Some((x0, y0));
                }
                let direct_relocation = bx0 == 0.0
                    && by0 == 0.0
                    && bx1 == width
                    && by1 == height
                    && f(6) == x0
                    && f(7) == y0 + height
                    && f(8) == x0 + width
                    && f(9) == y0 + height
                    && f(10) == x0 + width
                    && f(11) == y0;
                let swapped_relocation = bx0 == 0.0
                    && by0 == 0.0
                    && bx1 == width
                    && by1 == height
                    && f(6) == x0 + height
                    && f(7) == y0
                    && f(8) == x0 + height
                    && f(9) == y0 + width
                    && f(10) == x0
                    && f(11) == y0 + width;
                if direct_relocation {
                    return Some((x0 + dx, y0 + dy));
                }
                if swapped_relocation {
                    return Some((x0 + dy, y0 + dx));
                }
                let bw = (bx1 - bx0).max(1.0);
                let bh = (by1 - by0).max(1.0);
                let u = (dx - bx0) / bw;
                let v = (dy - by0) / bh;
                let sx = (1.0 - u) * (1.0 - v) * x0
                    + u * (1.0 - v) * f(10)
                    + u * v * f(8)
                    + (1.0 - u) * v * f(6);
                let sy = (1.0 - u) * (1.0 - v) * y0
                    + u * (1.0 - v) * f(11)
                    + u * v * f(9)
                    + (1.0 - u) * v * f(7);
                (sx.is_finite() && sy.is_finite()).then_some((sx, sy))
            }
            TransformMethod::Affine => None,
        }
    };

    let host_coordinate = |value: f64| -> Option<i64> {
        if !value.is_finite() {
            return None;
        }
        Some(
            if matches!(
                method,
                TransformMethod::Perspective | TransformMethod::Quad | TransformMethod::Mesh
            ) {
                if value < 0.0 { -1 } else { value as i64 }
            } else {
                (value + 0.5).floor() as i64
            },
        )
    };
    let device_coordinate =
        |value: f32| -> Option<i64> { value.is_finite().then(|| (value + 0.5).floor() as i64) };

    for dy in 0..*h {
        for dx in 0..*w {
            // The generic projective shader checks only a zero denominator
            // before dividing.  Reject any non-finite f32 intermediate here:
            // a NaN source coordinate would bypass the sampler's bounds
            // checks, whereas Pillow's f64 path still resolves a fill or a
            // source pixel.  Constant-denominator maps use a different
            // centered shader branch; this guard is for the nonconstant
            // arithmetic admitted by the proof above.
            if matches!(method, TransformMethod::Perspective) && (data[6] != 0.0 || data[7] != 0.0)
            {
                let f = |index: usize| data[index] as f32;
                let dx = dx as f32;
                let dy = dy as f32;
                let denominator = f(6) * dx + f(7) * dy + 1.0;
                if denominator != 0.0
                    && (!denominator.is_finite()
                        || !(f(0) * dx + f(1) * dy + f(2)).is_finite()
                        || !(f(3) * dx + f(4) * dy + f(5)).is_finite())
                {
                    return false;
                }
            }
            if matches!(method, TransformMethod::Quad)
                && !gpu_quad_constant_map_is_admitted(data)
                && !gpu_quad_shader_arithmetic_is_finite(data, *w, *h, dx as f32, dy as f32)
            {
                return false;
            }
            if matches!(method, TransformMethod::Mesh)
                && !gpu_mesh_constant_map_is_admitted(data, *w, *h)
                && !gpu_mesh_unit_relocation_is_admitted(data, *w, *h)
                && !gpu_mesh_shader_arithmetic_is_finite(data, *w, *h, dx as f32, dy as f32)
            {
                return false;
            }
            let host = source_at(f64::from(dx), f64::from(dy));
            let device = shader_source_at(dx as f32, dy as f32);
            if matches!(method, TransformMethod::Quad | TransformMethod::Mesh)
                && device.is_some_and(|(x, y)| {
                    !gpu_projective_shader_coordinate_is_safe(x)
                        || !gpu_projective_shader_coordinate_is_safe(y)
                })
            {
                return false;
            }
            let host_sample = host.and_then(|(x, y)| {
                let x = host_coordinate(x)?;
                let y = host_coordinate(y)?;
                (x >= 0
                    && x < i64::from(source_dimensions.0)
                    && y >= 0
                    && y < i64::from(source_dimensions.1))
                .then_some((x, y))
            });
            let device_sample = device.and_then(|(x, y)| {
                let x = device_coordinate(x)?;
                let y = device_coordinate(y)?;
                (x >= 0
                    && x < i64::from(source_dimensions.0)
                    && y >= 0
                    && y < i64::from(source_dimensions.1))
                .then_some((x, y))
            });
            if host_sample != device_sample {
                return false;
            }
        }
    }
    true
}
