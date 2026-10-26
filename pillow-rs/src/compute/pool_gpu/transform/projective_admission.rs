use super::super::*;
use super::*;

/// Check whether mesh control points describe an integral unit-scale relocation.
pub(in crate::compute::pool_gpu) fn gpu_mesh_unit_relocation_is_admitted(
    data: &[f64],
    w: u32,
    h: u32,
) -> bool {
    if data.len() != 12 {
        return false;
    }

    // ImagingGenericTransform clips each record's destination box before it
    // calls Geometry.c's local quad mapper.  Admit a partial record only when
    // the box is wholly inside the output and all four bounds are integers;
    // otherwise the Rust CPU path's truncating/clipping conversion would not
    // be represented by the shader's f32 comparisons.  A full-output record
    // is the existing special case of this same shape.
    let integral = |value: f64| value.is_finite() && value.fract() == 0.0;
    let [bx0, by0, bx1, by1] = [data[0], data[1], data[2], data[3]];
    if ![bx0, by0, bx1, by1].into_iter().all(integral)
        || bx0 < 0.0
        || by0 < 0.0
        || bx1 > f64::from(w)
        || by1 > f64::from(h)
        || bx1 <= bx0
        || by1 <= by0
    {
        return false;
    }
    let box_width = bx1 - bx0;
    let box_height = by1 - by0;
    if !data[4..12].iter().all(|value| value.is_finite()) {
        return false;
    }
    if !data[4..12].iter().copied().all(integral) {
        // A fractional translation is not a raw relocation: nearest uses
        // Geometry.c's truncation while the projective sampler rounds, and
        // filtered requests would retain nonzero interpolation weights.
        return false;
    }

    // Mesh records use Pillow's corner order: top-left, bottom-left,
    // bottom-right, top-right.  These two forms are the only non-identity
    // records admitted for ordinary bytes: unit-scale relocation over either
    // a complete output or an in-output partial box, and the corresponding
    // axis swap.  The exhaustive proof below still checks every source
    // selection and fill boundary; this shape guard prevents arbitrary
    // bilinear corner arithmetic from entering the shader merely because one
    // small image happened to compare equal.
    let tx = data[4];
    let ty = data[5];
    let direct = [
        tx,
        ty,
        tx,
        ty + box_height,
        tx + box_width,
        ty + box_height,
        tx + box_width,
        ty,
    ];
    let swapped = [
        tx,
        ty,
        tx + box_height,
        ty,
        tx + box_height,
        ty + box_width,
        tx,
        ty + box_width,
    ];
    data[4..12] == direct || data[4..12] == swapped
}

/// Return whether a complete one-record Mesh maps every destination sample to
/// one constant f32 source coordinate. The generic shader may still do
/// different f32 arithmetic for the bilinear weights, so the image-aware
/// proof below must compare every source selection before admitting it.
pub(in crate::compute::pool_gpu) fn gpu_mesh_constant_map_is_admitted(
    data: &[f64],
    w: u32,
    h: u32,
) -> bool {
    if data.len() != 12 || data[..4] != [0.0, 0.0, f64::from(w), f64::from(h)] {
        return false;
    }
    let x = data[4];
    let y = data[5];
    x.is_finite()
        && y.is_finite()
        && (x as f32).is_finite()
        && (y as f32).is_finite()
        && f64::from(x as f32) == x
        && f64::from(y as f32) == y
        && data[6..12].chunks_exact(2).all(|pair| pair == [x, y])
}

/// Return whether a Perspective map is a signed unit-axis relocation.
///
/// Pillow's `src/libImaging/Geometry.c` evaluates the inverse map at each
/// destination pixel center and then applies `COORD` truncation. For a
/// reflected unit axis, the shader's raw destination index plus
/// `floor(source + 0.5)` is one pixel off unless the center offset is reduced
/// explicitly. Keep the shape narrow; arbitrary affine arithmetic remains
/// subject to the exhaustive source-selection proof below.
pub(in crate::compute::pool_gpu) fn gpu_perspective_signed_unit_relocation_is_admitted(
    data: &[f64],
) -> bool {
    if data.len() < 8 || data[6] != 0.0 || data[7] != 0.0 {
        return false;
    }
    let integer_f32 = |value: f64| {
        value.is_finite()
            && value.fract() == 0.0
            && (value as f32).is_finite()
            && f64::from(value as f32) == value
    };
    if !integer_f32(data[2]) || !integer_f32(data[5]) {
        return false;
    }
    matches!(
        data[..6],
        [1.0, 0.0, _, 0.0, 1.0, _]
            | [-1.0, 0.0, _, 0.0, 1.0, _]
            | [1.0, 0.0, _, 0.0, -1.0, _]
            | [-1.0, 0.0, _, 0.0, -1.0, _]
            | [0.0, 1.0, _, 1.0, 0.0, _]
            | [0.0, -1.0, _, 1.0, 0.0, _]
            | [0.0, 1.0, _, -1.0, 0.0, _]
            | [0.0, -1.0, _, -1.0, 0.0, _]
    )
}

/// Return whether a Quad is a complete unit-scale relocation in Pillow's
/// NW/SW/SE/NE corner order. Direct identity and axis-swapped forms keep
/// every destination sample on an existing source pixel; arbitrary corner
/// arithmetic remains behind the exhaustive host/device source-selection
/// proof.
pub(in crate::compute::pool_gpu) fn gpu_quad_unit_relocation_is_admitted(
    data: &[f64],
    w: u32,
    h: u32,
) -> bool {
    if data.len() < 8 {
        return false;
    }
    let width = f64::from(w);
    let height = f64::from(h);
    // A translated Quad is still a raw relocation when both source origins
    // are integral and f32-exact.  Pillow's Geometry.c evaluates the map at
    // the destination center; the filtered callback then subtracts 0.5, so
    // an integer translation leaves zero interpolation weights.  Fractional
    // origins remain behind the centered f64/host proof because the shader's
    // raw-gid sampler rounds them differently.
    let integer_f32 = |value: f64| {
        value.is_finite()
            && value.fract() == 0.0
            && (value as f32).is_finite()
            && f64::from(value as f32) == value
    };
    let tx = data[0];
    let ty = data[1];
    if !integer_f32(tx) || !integer_f32(ty) {
        return false;
    }
    let direct = [
        tx,
        ty,
        tx,
        ty + height,
        tx + width,
        ty + height,
        tx + width,
        ty,
    ];
    // The swapped map exchanges destination axes, so its source extents are
    // height on X and width on Y.  Using `(width, height)` here only works
    // for square outputs and admits a scaled map for every other shape.
    let swapped = [
        tx,
        ty,
        tx + height,
        ty,
        tx + height,
        ty + width,
        tx,
        ty + width,
    ];
    data[..8] == direct || data[..8] == swapped
}

/// Return whether a Quad maps every destination sample to one constant f32
/// source coordinate. Nearest sampling needs only the resulting
/// source selection; the exhaustive proof still rejects any f32 arithmetic
/// that could move that selection across a fill or pixel boundary.
pub(in crate::compute::pool_gpu) fn gpu_quad_constant_map_is_admitted(data: &[f64]) -> bool {
    if data.len() != 8 {
        return false;
    }
    let x = data[0];
    let y = data[1];
    x.is_finite()
        && y.is_finite()
        && (x as f32).is_finite()
        && (y as f32).is_finite()
        && f64::from(x as f32) == x
        && f64::from(y as f32) == y
        && data[2..8].chunks_exact(2).all(|pair| pair == [x, y])
}

fn gpu_quad_shader_axis_arithmetic_is_finite(points: [f32; 4], u: f32, v: f32) -> bool {
    let [point0, point1, point2, point3] = points;
    let point3_minus_point0 = point3 - point0;
    let point1_minus_point0 = point1 - point0;
    let point2_minus_point1 = point2 - point1;
    let point2_minus_point1_minus_point3 = point2_minus_point1 - point3;
    let cross = point2_minus_point1_minus_point3 + point0;
    let term0 = point3_minus_point0 * u;
    let term1 = point1_minus_point0 * v;
    let term2 = cross * u * v;
    let sample = point0 + term0 + term1 + term2;
    [
        point3_minus_point0,
        point1_minus_point0,
        point2_minus_point1,
        point2_minus_point1_minus_point3,
        cross,
        term0,
        term1,
        term2,
        sample,
    ]
    .into_iter()
    .all(f32::is_finite)
}

/// Check that the generic Quad WGSL bilinear expression has no non-finite
/// f32 intermediate for one output pixel. A finite f64 map can still
/// overflow in the device arithmetic; a resulting NaN would bypass the
/// shader sampler's unordered bounds comparisons. The exhaustive source
/// proof calls this only for nonconstant maps, while relocation/constant
/// branches avoid the generic expression entirely.
pub(in crate::compute::pool_gpu) fn gpu_quad_shader_arithmetic_is_finite(
    data: &[f64],
    w: u32,
    h: u32,
    dx: f32,
    dy: f32,
) -> bool {
    if data.len() < 8 {
        return false;
    }
    let f = |index: usize| data[index] as f32;
    let x0 = f(0);
    let y0 = f(1);
    let x1 = f(2);
    let y1 = f(3);
    let x2 = f(4);
    let y2 = f(5);
    let x3 = f(6);
    let y3 = f(7);
    let width = w as f32;
    let height = h as f32;
    let u = dx / width;
    let v = dy / height;
    let one_minus_u = 1.0 - u;
    let one_minus_v = 1.0 - v;
    [width, height, u, v, one_minus_u, one_minus_v]
        .into_iter()
        .all(f32::is_finite)
        && gpu_quad_shader_axis_arithmetic_is_finite([x0, x1, x2, x3], u, v)
        && gpu_quad_shader_axis_arithmetic_is_finite([y0, y1, y2, y3], u, v)
}

/// Check that the generic one-record Mesh WGSL bilinear expression has no
/// non-finite f32 intermediate for one output pixel.  The bbox is complete
/// before this helper is called, but its arithmetic is retained here so the
/// check mirrors the shader even if callers are later reused for another
/// bounded record shape.
pub(in crate::compute::pool_gpu) fn gpu_mesh_shader_arithmetic_is_finite(
    data: &[f64],
    w: u32,
    h: u32,
    dx: f32,
    dy: f32,
) -> bool {
    if data.len() < 12 {
        return false;
    }
    let f = |index: usize| data[index] as f32;
    let bx0 = f(0);
    let by0 = f(1);
    let bx1 = f(2);
    let by1 = f(3);
    let width = w as f32;
    let height = h as f32;
    let bw = (bx1 - bx0).max(1.0);
    let bh = (by1 - by0).max(1.0);
    let u = (dx - bx0) / bw;
    let v = (dy - by0) / bh;
    let one_minus_u = 1.0 - u;
    let one_minus_v = 1.0 - v;
    let x0 = f(4);
    let y0 = f(5);
    let x1 = f(6);
    let y1 = f(7);
    let x2 = f(8);
    let y2 = f(9);
    let x3 = f(10);
    let y3 = f(11);
    let x_term0 = one_minus_u * one_minus_v * x0;
    let x_term1 = u * one_minus_v * x3;
    let x_term2 = u * v * x2;
    let x_term3 = one_minus_u * v * x1;
    let y_term0 = one_minus_u * one_minus_v * y0;
    let y_term1 = u * one_minus_v * y3;
    let y_term2 = u * v * y2;
    let y_term3 = one_minus_u * v * y1;
    let sx = x_term0 + x_term1 + x_term2 + x_term3;
    let sy = y_term0 + y_term1 + y_term2 + y_term3;
    [
        width,
        height,
        bw,
        bh,
        u,
        v,
        one_minus_u,
        one_minus_v,
        x_term0,
        x_term1,
        x_term2,
        x_term3,
        y_term0,
        y_term1,
        y_term2,
        y_term3,
        sx,
        sy,
    ]
    .into_iter()
    .all(f32::is_finite)
}

/// Return whether a finite projective coordinate can be safely rounded and
/// converted by the WGSL nearest sampler.  Negative values are checked before
/// the shader's `u32` conversion and are therefore valid fill coordinates;
/// positive values must remain below the first unrepresentable `u32` value.
pub(in crate::compute::pool_gpu) fn gpu_projective_shader_coordinate_is_safe(value: f32) -> bool {
    if !value.is_finite() {
        return false;
    }
    let rounded = value + 0.5;
    rounded.is_finite() && (value < 0.0 || rounded.floor() < 4_294_967_296.0)
}

/// Prove constant quarter-grid Bilinear and integer Bicubic maps.
///
/// Geometry.c validates the original source coordinate before subtracting
/// 0.5 for its filter window.  An interior integer coordinate therefore
/// becomes an exact half-pixel `(n - 0.5)` in the shader, with all four
/// neighbors in bounds and nonzero 0.5 weights.  Coordinates on the source
/// edge are intentionally excluded: Pillow clamps their filter window while
/// the shader's shifted bounds check would classify `0` or `width`
/// differently. LA/RGBA use the same byte proof after exact integer
/// premultiplication of each tap and unpremultiplication of the stored result.
/// Bilinear admits sixteenth-grid coordinates: horizontal interpolation has
/// denominator sixteen and vertical interpolation denominator 256. Geometry.c's
/// `BILINEAR_BODY` intermediates have magnitude at most 255, so both f32 and
/// f64 represent every operation exactly, including contracted FMAs.
/// This proof is independent of the image's byte values.
/// Bicubic additionally requires two valid source pixels on each side: at a
/// quarter-grid source coordinate, Geometry.c's polynomial weights have
/// denominator 64. The shader evaluates both axes as signed integer
/// numerators with denominator 4096, before the same clip/truncate store.
pub(in crate::compute::pool_gpu) fn gpu_projective_filtered_constant_is_admitted(
    method: TransformMethod,
    data: &[f64],
    filter: ResampleFilter,
    mode: Option<&str>,
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
) -> bool {
    if !matches!(filter, ResampleFilter::Bilinear | ResampleFilter::Bicubic)
        || !matches!(
            mode,
            Some("L" | "LA" | "PA" | "RGB" | "RGBA" | "CMYK" | "HSV" | "YCbCr" | "RGBX" | "RGBa")
        )
    {
        return false;
    }
    let bicubic = matches!(filter, ResampleFilter::Bicubic);
    if bicubic && matches!(mode, Some("LA" | "RGBA")) {
        return false;
    }
    let interior_coordinate_f32 = |value: f64, extent: u32| {
        let lower_bound = if bicubic { 2.0 } else { 1.0 };
        let upper_margin = if bicubic { 1.0 } else { 0.0 };
        value.is_finite()
            && (if bicubic { value * 4.0 } else { value * 16.0 }).fract() == 0.0
            && f64::from((value - 0.5) as f32) == value - 0.5
            && value >= lower_bound
            && value + upper_margin < f64::from(extent)
            && (value as f32).is_finite()
            && f64::from(value as f32) == value
    };
    match method {
        TransformMethod::Perspective => {
            data.len() >= 8
                && data[0] == 0.0
                && data[1] == 0.0
                && data[3] == 0.0
                && data[4] == 0.0
                && data[6] == 0.0
                && data[7] == 0.0
                && interior_coordinate_f32(data[2], source_dimensions.0)
                && interior_coordinate_f32(data[5], source_dimensions.1)
        }
        TransformMethod::Quad => {
            gpu_quad_constant_map_is_admitted(data)
                && interior_coordinate_f32(data[0], source_dimensions.0)
                && interior_coordinate_f32(data[1], source_dimensions.1)
        }
        TransformMethod::Mesh => {
            gpu_mesh_constant_map_is_admitted(data, output_dimensions.0, output_dimensions.1)
                && interior_coordinate_f32(data[4], source_dimensions.0)
                && interior_coordinate_f32(data[5], source_dimensions.1)
        }
        TransformMethod::Affine => false,
    }
}

pub(in crate::compute::pool_gpu) fn gpu_projective_filtered_relocation_is_admitted(
    method: TransformMethod,
    data: &[f64],
    filter: ResampleFilter,
    mode: Option<&str>,
    output_dimensions: (u32, u32),
) -> bool {
    // Pillow's Geometry.c filtered projective path subtracts the destination
    // center offset before bilinear sampling. For integer unit maps and
    // constant half-pixel source coordinates, that produces an integral
    // source coordinate and zero filter weights; the shader's raw-gid
    // projective sampler therefore selects the same source sample. LA/RGBA
    // use Pillow's premultiplied round trip; the projective shader mirrors
    // that round trip for the zero-weight source word. PA is different: its
    // projective path preserves raw index/alpha pairs without premultiplied
    // conversion, so the zero-weight argument applies to PA as well.
    if !matches!(mode, Some("L" | "LA" | "PA" | "RGB" | "RGBA"))
        || !matches!(filter, ResampleFilter::Bilinear | ResampleFilter::Bicubic)
        || data.len() < 8
    {
        return false;
    }

    let integer_translation = |value: f64| {
        value.is_finite()
            && value.fract() == 0.0
            && (value as f32).is_finite()
            && f64::from(value as f32) == value
    };
    let filter_center = |value: f64| {
        let narrowed = value as f32;
        let centered = value - 0.5;
        value.is_finite()
            && narrowed.is_finite()
            && f64::from(narrowed) == value
            && centered.fract() == 0.0
            && (centered as f32).is_finite()
            && f64::from(centered as f32) == centered
    };
    match method {
        TransformMethod::Perspective => {
            let direct_perspective =
                |tx: f64, ty: f64| data[..8] == [1.0, 0.0, tx, 0.0, 1.0, ty, 0.0, 0.0];
            let swapped_perspective =
                |tx: f64, ty: f64| data[..8] == [0.0, 1.0, tx, 1.0, 0.0, ty, 0.0, 0.0];
            let tx = data[2];
            let ty = data[5];
            (integer_translation(tx)
                && integer_translation(ty)
                && (direct_perspective(tx, ty) || swapped_perspective(tx, ty)))
                || (data[..6] == [0.0, 0.0, tx, 0.0, 0.0, ty]
                    && data[6] == 0.0
                    && data[7] == 0.0
                    && filter_center(tx)
                    && filter_center(ty))
        }
        TransformMethod::Quad => {
            // QUAD's direct and axis-swapped unit maps evaluate to integral
            // source coordinates at every raw destination gid. A constant
            // half-pixel map reaches the same integral coordinate after the
            // Geometry.c filter window subtracts 0.5.
            gpu_quad_unit_relocation_is_admitted(data, output_dimensions.0, output_dimensions.1)
                || (gpu_quad_constant_map_is_admitted(data)
                    && filter_center(data[0])
                    && filter_center(data[1]))
        }
        TransformMethod::Mesh => {
            // A complete one-record mesh has the same zero-weight property
            // for unit relocations and constant half-pixel source maps.
            gpu_mesh_unit_relocation_is_admitted(data, output_dimensions.0, output_dimensions.1)
                || (gpu_mesh_constant_map_is_admitted(
                    data,
                    output_dimensions.0,
                    output_dimensions.1,
                ) && filter_center(data[4])
                    && filter_center(data[5]))
        }
        TransformMethod::Affine => false,
    }
}

/// Return whether a palette-alpha projective transform is a raw pair
/// relocation that the projective shader can reproduce exactly.
///
/// PA stores one palette index and one per-pixel alpha byte in the native
/// two-band image.  A nearest signed unit-axis map with an integer translation
/// only selects an existing `(index, alpha)` pair; it
/// never expands the palette or performs alpha arithmetic.  Keep this
/// separate from ordinary byte modes: PA's palette metadata and two-band fill
/// contract need their own proof. Pillow does not force PA's non-nearest
/// transform requests to nearest. The filtered Perspective relocation below
/// is admitted only when its weights are provably zero; other PA filtered
/// transforms remain on the exact host path rather than being silently
/// changed to pair-copy sampling. Quad and complete one-record Mesh also
/// admit a constant f32 source coordinate for nearest sampling, while the
/// filtered proof admits only a half-pixel coordinate: Geometry.c's 0.5
/// center shift then makes its filter weights exactly zero. The image-aware
/// proof still checks every output boundary.
pub(in crate::compute::pool_gpu) fn gpu_palette_alpha_projective_relocation_is_admitted(
    method: TransformMethod,
    data: &[f64],
    filter: ResampleFilter,
    output_dimensions: (u32, u32),
) -> bool {
    if !matches!(filter, ResampleFilter::Nearest) || data.len() < 8 {
        return false;
    }
    let integer_f32 = |value: f64| {
        value.is_finite()
            && value.fract() == 0.0
            && (value as f32).is_finite()
            && f64::from(value as f32) == value
    };
    match method {
        TransformMethod::Perspective => {
            let direct = data[..8] == [1.0, 0.0, data[2], 0.0, 1.0, data[5], 0.0, 0.0];
            let swapped = data[..8] == [0.0, 1.0, data[2], 1.0, 0.0, data[5], 0.0, 0.0];
            integer_f32(data[2])
                && integer_f32(data[5])
                && (direct || swapped || gpu_perspective_signed_unit_relocation_is_admitted(data))
        }
        TransformMethod::Quad => {
            gpu_quad_unit_relocation_is_admitted(data, output_dimensions.0, output_dimensions.1)
                || gpu_quad_constant_map_is_admitted(data)
        }
        TransformMethod::Mesh => {
            gpu_mesh_unit_relocation_is_admitted(data, output_dimensions.0, output_dimensions.1)
                || gpu_mesh_constant_map_is_admitted(data, output_dimensions.0, output_dimensions.1)
        }
        TransformMethod::Affine => false,
    }
}

// Geometry is host control, like resize coefficients: these records contain
// only source indices and the exact binary64 fractional distances. Source
// samples and every interpolation/alpha operation remain on the device.
pub(in crate::compute::pool_gpu) const GPU_TRANSFORM_GEOMETRY_WORDS: usize = 13;
// GpuInner requests wgpu's default 128 MiB storage binding limit. Bound the
// aligned per-operation table against that contract independently of image
// allocation and workgroup limits; those are checked by the shared preflight.
const GPU_TRANSFORM_GEOMETRY_MAX_BYTES: u64 = 128 << 20;

pub(in crate::compute::pool_gpu) fn gpu_transform_is_projective(op: &PipelineOp) -> bool {
    matches!(
        op,
        PipelineOp::Transform {
            method: TransformMethod::Perspective | TransformMethod::Quad | TransformMethod::Mesh,
            ..
        }
    )
}

pub(in crate::compute::pool_gpu) fn gpu_transform_uses_geometry_table(
    op: &PipelineOp,
    mode: Option<&str>,
    source: (u32, u32),
) -> bool {
    gpu_transform_is_projective(op) && !gpu_projective_uniform_is_exact(op, mode, source)
}

pub(in crate::compute::pool_gpu) fn gpu_transform_geometry_is_admitted(
    op: &PipelineOp,
    mode: Option<&str>,
    source: (u32, u32),
) -> bool {
    let PipelineOp::Transform {
        w,
        h,
        method,
        data,
        filter,
        ..
    } = op
    else {
        return false;
    };
    gpu_transform_is_projective(op)
        && matches!(
            mode,
            None | Some(
                "L" | "LA"
                    | "P"
                    | "PA"
                    | "1"
                    | "RGB"
                    | "RGBA"
                    | "RGBa"
                    | "RGBX"
                    | "CMYK"
                    | "HSV"
                    | "YCbCr"
                    | "F"
                    | "La"
                    | "I"
                    | "I;16"
                    | "I;16L"
                    | "I;16B"
                    | "I;16N"
            )
        )
        && (mode != Some("I") || matches!(filter, ResampleFilter::Nearest))
        && *w > 0
        && *h > 0
        && u64::from(*w) * u64::from(*h)
            <= GPU_TRANSFORM_GEOMETRY_MAX_BYTES / (GPU_TRANSFORM_GEOMETRY_WORDS as u64 * 4)
        && source.0 > 0
        && source.1 > 0
        && u64::from(source.0) * u64::from(source.1) <= u64::from(u32::MAX)
        && source.0 < i32::MAX as u32
        && source.1 < i32::MAX as u32
        && matches!(
            filter,
            ResampleFilter::Nearest | ResampleFilter::Bilinear | ResampleFilter::Bicubic
        )
        && match method {
            TransformMethod::Mesh => data.len().is_multiple_of(12),
            _ => data.len() >= 8,
        }
}

pub(in crate::compute::pool_gpu) fn gpu_transform_geometry_words(
    op: &PipelineOp,
    source: (u32, u32),
    mode: Option<&str>,
) -> Result<Vec<u32>, PilError> {
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
        return Err(PilError::InternalError(
            "GPU geometry requires a transform".into(),
        ));
    };
    if !gpu_transform_geometry_is_admitted(op, mode, source) {
        return Err(PilError::ValueError(
            "GPU transform geometry exceeds bounded storage or sample layout".into(),
        ));
    }
    let nearest = matches!(filter, ResampleFilter::Nearest) || matches!(mode, Some("P" | "1"));
    let cubic = matches!(filter, ResampleFilter::Bicubic) && !nearest;
    let mut geometry = GpuTransformGeometryOutput::new(*w, *h, source, nearest, cubic);
    if matches!(method, TransformMethod::Mesh) {
        gpu_transform_mesh_geometry_words(&mut geometry, data, *fill_is_none);
    } else {
        gpu_transform_regular_geometry_words(&mut geometry, method, data);
    }
    Ok(geometry.words)
}

struct GpuTransformGeometryOutput {
    width: u32,
    height: u32,
    source: (u32, u32),
    nearest: bool,
    cubic: bool,
    words: Vec<u32>,
}

impl GpuTransformGeometryOutput {
    fn new(width: u32, height: u32, source: (u32, u32), nearest: bool, cubic: bool) -> Self {
        Self {
            width,
            height,
            source,
            nearest,
            cubic,
            words: vec![0; (width as usize) * (height as usize) * GPU_TRANSFORM_GEOMETRY_WORDS],
        }
    }

    #[inline(always)]
    fn record(&mut self, x: u32, y: u32, sx: f64, sy: f64, clear_failed: bool) {
        let row = &mut self.words[((y * self.width + x) as usize) * GPU_TRANSFORM_GEOMETRY_WORDS..]
            [..GPU_TRANSFORM_GEOMETRY_WORDS];
        if !sx.is_finite()
            || !sy.is_finite()
            || sx < 0.0
            || sy < 0.0
            || sx >= f64::from(self.source.0)
            || sy >= f64::from(self.source.1)
        {
            if clear_failed {
                row[0] = 2;
            }
            return;
        }
        row[0] = 1;
        if self.nearest {
            row[1] = sy as u32 * self.source.0 + sx as u32;
            return;
        }
        let sample_x = sx - 0.5;
        let sample_y = sy - 0.5;
        let floor_x = sample_x.floor() as i64;
        let floor_y = sample_y.floor() as i64;
        let dx = (sample_x - floor_x as f64).to_bits();
        let dy = (sample_y - floor_y as f64).to_bits();
        row[9..13].copy_from_slice(&[dx as u32, (dx >> 32) as u32, dy as u32, (dy >> 32) as u32]);
        for tap in 0..4 {
            row[1 + tap] = (floor_x + tap as i64 - i64::from(self.cubic))
                .clamp(0, i64::from(self.source.0) - 1) as u32;
            row[5 + tap] = (floor_y + tap as i64 - i64::from(self.cubic))
                .clamp(0, i64::from(self.source.1) - 1) as u32
                * self.source.0;
        }
    }
}

fn gpu_transform_geometry_coefficients(q: &[f64], width: f64, height: f64) -> [f64; 8] {
    let iw = 1.0 / width;
    let ih = 1.0 / height;
    [
        q[0],
        (q[6] - q[0]) * iw,
        (q[2] - q[0]) * ih,
        (q[4] - q[2] - q[6] + q[0]) * iw * ih,
        q[1],
        (q[7] - q[1]) * iw,
        (q[3] - q[1]) * ih,
        (q[5] - q[3] - q[7] + q[1]) * iw * ih,
    ]
}

#[inline(always)]
fn gpu_transform_geometry_quad(c: [f64; 8], x: f64, y: f64) -> (f64, f64) {
    (
        (c[3] * x).mul_add(y, c[2].mul_add(y, c[1].mul_add(x, c[0]))),
        (c[7] * x).mul_add(y, c[6].mul_add(y, c[5].mul_add(x, c[4]))),
    )
}

fn gpu_transform_mesh_geometry_words(
    output: &mut GpuTransformGeometryOutput,
    data: &[f64],
    fill_is_none: bool,
) {
    for mesh in data.chunks_exact(12) {
        let (x0, y0, x1, y1) = (
            mesh[0] as i64,
            mesh[1] as i64,
            mesh[2] as i64,
            mesh[3] as i64,
        );
        let width = x1.saturating_sub(x0);
        let height = y1.saturating_sub(y0);
        if width <= 0 || height <= 0 {
            continue;
        }
        let c = gpu_transform_geometry_coefficients(&mesh[4..], width as f64, height as f64);
        let bx0 = x0.clamp(0, i64::from(output.width)) as u32;
        let by0 = y0.clamp(0, i64::from(output.height)) as u32;
        let bx1 = x1.clamp(0, i64::from(output.width)) as u32;
        let by1 = y1.clamp(0, i64::from(output.height)) as u32;
        // Geometry.c uses coordinates relative to the clipped box. With
        // explicit fill, a failed later record preserves an earlier sample;
        // omitted fill clears it. Resolve only this geometry precedence.
        for y in by0..by1 {
            for x in bx0..bx1 {
                let (sx, sy) = gpu_transform_geometry_quad(
                    c,
                    f64::from(x - bx0) + 0.5,
                    f64::from(y - by0) + 0.5,
                );
                output.record(x, y, sx, sy, fill_is_none);
            }
        }
    }
}

fn gpu_transform_regular_geometry_words(
    output: &mut GpuTransformGeometryOutput,
    method: &TransformMethod,
    data: &[f64],
) {
    let c = if matches!(method, TransformMethod::Quad) {
        Some(gpu_transform_geometry_coefficients(
            data,
            f64::from(output.width),
            f64::from(output.height),
        ))
    } else {
        None
    };
    for y in 0..output.height {
        for x in 0..output.width {
            let dx = f64::from(x) + 0.5;
            let dy = f64::from(y) + 0.5;
            let (sx, sy) = if let Some(c) = c {
                gpu_transform_geometry_quad(c, dx, dy)
            } else {
                // Geometry.c perspective_transform: fused first two products,
                // then translation and division at destination pixel centers.
                let denominator = data[6].mul_add(dx, data[7] * dy) + 1.0;
                (
                    (data[0].mul_add(dx, data[1] * dy) + data[2]) / denominator,
                    (data[3].mul_add(dx, data[4] * dy) + data[5]) / denominator,
                )
            };
            output.record(x, y, sx, sy, false);
        }
    }
}
