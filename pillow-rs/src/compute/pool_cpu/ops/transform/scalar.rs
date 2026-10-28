use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::{DynamicImage, RgbaImage};

#[inline]
fn affine_f32_bilinear_horizontal(left: f32, right: f32, distance: f64) -> f64 {
    // Geometry.c's FLOAT32 path subtracts the f32 samples before promoting
    // that difference, then uses an f64 fused multiply-add.
    f64::from(right - left).mul_add(distance, f64::from(left))
}

#[inline]
fn affine_f32_bicubic_horizontal(samples: [f32; 4], distance: f64) -> f64 {
    // The native filter computes the polynomial coefficients in f32 (the
    // source type) and evaluates the Horner chain in f64 with FMA. Keep the
    // coefficient order from Geometry.c's BICUBIC macro.
    let [v1, v2, v3, v4] = samples;
    let p1 = f64::from(v2);
    let p2 = f64::from(v3 - v1);
    let p3 = f64::from((v1 - v2).mul_add(2.0, v3) - v4);
    let p4 = f64::from((v2 - v1 - v3) + v4);
    let inner = distance.mul_add(p4, p3);
    let middle = distance.mul_add(inner, p2);
    distance.mul_add(middle, p1)
}

#[inline]
fn affine_f32_bicubic_vertical(samples: [f64; 4], distance: f64) -> f64 {
    // Vertical rows are already f64 intermediates in Geometry.c, so the same
    // Horner expression keeps all coefficient arithmetic in f64.
    let [v1, v2, v3, v4] = samples;
    let p1 = v2;
    let p2 = -v1 + v3;
    let p3 = (v1 - v2).mul_add(2.0, v3) - v4;
    let p4 = -v1 + v2 - v3 + v4;
    let inner = distance.mul_add(p4, p3);
    let middle = distance.mul_add(inner, p2);
    distance.mul_add(middle, p1)
}

#[inline(always)]
fn transform_affine_f32_pixels(
    source: &[f32],
    source_dimensions: (u32, u32),
    destination_dimensions: (u32, u32),
    affine: [f64; 6],
    filter: &ResampleFilter,
    fill_word: f32,
    output: &mut [f32],
) {
    let (src_w, src_h) = source_dimensions;
    let (dst_w, dst_h) = destination_dimensions;
    let (a, b, c, d, e, f) = (
        affine[0], affine[1], affine[2], affine[3], affine[4], affine[5],
    );
    let src_w_i = i64::from(src_w);
    let src_h_i = i64::from(src_h);

    for dy in 0..dst_h {
        for dx in 0..dst_w {
            let output_index = (dy as usize) * dst_w as usize + dx as usize;
            let xin = f64::from(dx) + 0.5;
            let yin = f64::from(dy) + 0.5;
            // Pillow's Geometry.c affine callback is compiled as a multiply
            // followed by fused multiply-add, then a separate translation
            // add. Keep `b * yin`/`e * yin` out of the fused term with the
            // translation: a one-ULP coordinate change can select a wildly
            // different FLOAT32 neighbour at a map boundary.
            let sx = a.mul_add(xin, b * yin) + c;
            let sy = d.mul_add(xin, e * yin) + f;

            if !sx.is_finite()
                || !sy.is_finite()
                || sx < 0.0
                || sx >= f64::from(src_w)
                || sy < 0.0
                || sy >= f64::from(src_h)
            {
                continue;
            }

            if matches!(filter, ResampleFilter::Nearest) {
                let ix = sx as i64;
                let iy = sy as i64;
                if ix >= 0 && ix < src_w_i && iy >= 0 && iy < src_h_i {
                    output[output_index] = source[(iy as usize) * src_w as usize + ix as usize];
                }
                continue;
            }

            let sample_x = sx - 0.5;
            let sample_y = sy - 0.5;
            let floor_x = sample_x.floor() as i64;
            let floor_y = sample_y.floor() as i64;
            let dx_fraction = sample_x - floor_x as f64;
            let dy_fraction = sample_y - floor_y as f64;

            match filter {
                ResampleFilter::Bilinear => {
                    let x0 = floor_x.clamp(0, src_w_i - 1) as usize;
                    let x1 = (floor_x + 1).clamp(0, src_w_i - 1) as usize;
                    let y0 = floor_y.clamp(0, src_h_i - 1) as usize;
                    let y1 = (floor_y + 1).clamp(0, src_h_i - 1) as usize;
                    let top = affine_f32_bilinear_horizontal(
                        source[y0 * src_w as usize + x0],
                        source[y0 * src_w as usize + x1],
                        dx_fraction,
                    );
                    let bottom = affine_f32_bilinear_horizontal(
                        source[y1 * src_w as usize + x0],
                        source[y1 * src_w as usize + x1],
                        dx_fraction,
                    );
                    output[output_index] = (bottom - top).mul_add(dy_fraction, top) as f32;
                }
                ResampleFilter::Bicubic => {
                    let x = floor_x - 1;
                    let y = floor_y - 1;
                    let mut rows = [0.0f64; 4];
                    for row in 0..4i64 {
                        let yy = y + row;
                        if row > 0 && (yy < 0 || yy >= src_h_i) {
                            rows[row as usize] = rows[row as usize - 1];
                            continue;
                        }
                        let cy = yy.clamp(0, src_h_i - 1) as usize;
                        let samples = [
                            source[cy * src_w as usize + (x).clamp(0, src_w_i - 1) as usize],
                            source[cy * src_w as usize + (x + 1).clamp(0, src_w_i - 1) as usize],
                            source[cy * src_w as usize + (x + 2).clamp(0, src_w_i - 1) as usize],
                            source[cy * src_w as usize + (x + 3).clamp(0, src_w_i - 1) as usize],
                        ];
                        rows[row as usize] = affine_f32_bicubic_horizontal(samples, dx_fraction);
                    }
                    output[output_index] = affine_f32_bicubic_vertical(rows, dy_fraction) as f32;
                }
                // Image.transform accepts only nearest, bilinear, and bicubic
                // filters. Keep an internal fallback deterministic if a direct
                // Rust caller constructs another descriptor.
                _ => output[output_index] = source.first().copied().unwrap_or(fill_word),
            }
        }
    }
}

/// Apply Pillow's FLOAT32 affine transform filters to an explicit `F` image.
///
/// `F` images use the four-byte `RgbaImage` transport internally, but their
/// samples are IEEE-754 words rather than four independent byte bands.  The
/// generic byte affine path therefore cannot be used for bilinear/bicubic
/// transforms: it would interpolate each byte and can even copy a source word
/// into a neighbouring output byte.  Pillow's `Geometry.c` FLOAT32 filters
/// promote each source word to `double`, compute horizontal rows with the
/// compiler's f32-difference/f64-FMA order, then store one f32 word.
pub(super) fn transform_affine_f32(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    data: &[f64],
    filter: &ResampleFilter,
    fill: Option<(u8, u8, u8, u8)>,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageRgba8(rgba) = img else {
        return Err(PilError::InternalError(
            "F affine transform requires four-byte storage".into(),
        ));
    };
    let (src_w, src_h) = rgba.dimensions();
    let source_len = usize::try_from(src_w)
        .ok()
        .and_then(|width| {
            usize::try_from(src_h)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| PilError::ValueError("image dimensions are too large".into()))?;
    let output_len = usize::try_from(dst_w)
        .ok()
        .and_then(|width| {
            usize::try_from(dst_h)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| PilError::ValueError("image dimensions are too large".into()))?;
    let raw = rgba.as_raw();
    let source = raw
        .chunks_exact(4)
        .take(source_len)
        .map(|word| f32::from_le_bytes([word[0], word[1], word[2], word[3]]))
        .collect::<Vec<_>>();
    if source.len() != source_len {
        return Err(PilError::InternalError(
            "F affine source buffer shape mismatch".into(),
        ));
    }

    let fill_word = fill
        .map(|color| f32::from_le_bytes([color.0, color.1, color.2, color.3]))
        .unwrap_or(0.0);
    let mut output = vec![fill_word; output_len];
    transform_affine_f32_pixels(
        &source,
        (src_w, src_h),
        (dst_w, dst_h),
        [data[0], data[1], data[2], data[3], data[4], data[5]],
        filter,
        fill_word,
        &mut output,
    );

    let bytes = output
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect::<Vec<_>>();
    let out = RgbaImage::from_raw(dst_w, dst_h, bytes)
        .ok_or_else(|| PilError::InternalError("F affine output shape mismatch".into()))?;
    Ok(DynamicImage::ImageRgba8(out))
}

/// Sample one FLOAT32 word with Pillow's generic-transform filter contract.
///
/// `Geometry.c` uses the same FLOAT32 filter callbacks for affine, perspective,
/// quad, and mesh transforms. Keep this helper separate from the byte path:
/// each source pixel is one f32 word, horizontal rows use f32 coefficient
/// arithmetic promoted to f64, and the final value is stored as f32.
#[inline]
pub(super) fn sample_transform_f32(
    source: &[u8],
    src_w: u32,
    src_h: u32,
    sx: f64,
    sy: f64,
    filter: &ResampleFilter,
) -> f32 {
    let src_w_i = i64::from(src_w);
    let src_h_i = i64::from(src_h);
    let sample_x = sx - 0.5;
    let sample_y = sy - 0.5;
    let floor_x = sample_x.floor() as i64;
    let floor_y = sample_y.floor() as i64;
    let dx = sample_x - floor_x as f64;
    let dy = sample_y - floor_y as f64;
    let at = |x: i64, y: i64| -> f32 {
        let x = x.clamp(0, src_w_i - 1) as usize;
        let y = y.clamp(0, src_h_i - 1) as usize;
        let index = (y * src_w as usize + x) * 4;
        f32::from_le_bytes(
            source[index..index + 4]
                .try_into()
                .expect("native sample width"),
        )
    };

    match filter {
        ResampleFilter::Bilinear => {
            // Keep the source-type subtraction before the f64 FMA, matching
            // Geometry.c's FLOAT32 BILINEAR macro rather than evaluating four
            // independent f64 products.
            let left_top = at(floor_x, floor_y);
            let right_top = at(floor_x + 1, floor_y);
            let left_bottom = at(floor_x, floor_y + 1);
            let right_bottom = at(floor_x + 1, floor_y + 1);
            let top_delta = f64_from_f32_c(f32_sub_c(right_top, left_top));
            let bottom_delta = f64_from_f32_c(f32_sub_c(right_bottom, left_bottom));
            let top = f64_fma_c(top_delta, dx, f64_from_f32_c(left_top));
            let bottom = f64_fma_c(bottom_delta, dx, f64_from_f32_c(left_bottom));
            f32_from_f64_c(f64_fma_c(f64_sub_c(bottom, top), dy, top))
        }
        ResampleFilter::Bicubic => {
            #[inline]
            fn horizontal(samples: [f32; 4], distance: f64) -> f64 {
                let [v1, v2, v3, v4] = samples;
                let p1 = f64_from_f32_c(v2);
                let p2 = f64_from_f32_c(f32_sub_c(v3, v1));
                let p3 = f64_from_f32_c(f32_sub_c(f32_fma_c(f32_sub_c(v1, v2), 2.0, v3), v4));
                let p4 = f64_from_f32_c(f32_add_c(f32_sub_c(f32_sub_c(v2, v1), v3), v4));
                let inner = f64_fma_c(distance, p4, p3);
                let middle = f64_fma_c(distance, inner, p2);
                f64_fma_c(distance, middle, p1)
            }

            let base_x = floor_x - 1;
            let base_y = floor_y - 1;
            let mut rows = [0.0f64; 4];
            for (row, output) in rows.iter_mut().enumerate() {
                let y = base_y + row as i64;
                *output = horizontal(
                    [
                        at(base_x, y),
                        at(base_x + 1, y),
                        at(base_x + 2, y),
                        at(base_x + 3, y),
                    ],
                    dx,
                );
            }
            let [v1, v2, v3, v4] = rows;
            let p1 = v2;
            // LLVM lowers `-v1 + v3` as `v3 - v1`; retaining that operand
            // order matters when both rows contain distinct NaN payloads.
            let p2 = f64_sub_c(v3, v1);
            let p3 = f64_sub_c(f64_fma_c(f64_sub_c(v1, v2), 2.0, v3), v4);
            let p4 = f64_add_c(f64_sub_c(f64_add_c(f64_neg_c(v1), v2), v3), v4);
            let inner = f64_fma_c(dy, p4, p3);
            let middle = f64_fma_c(dy, inner, p2);
            f32_from_f64_c(f64_fma_c(dy, middle, p1))
        }
        // The public transform parser accepts only nearest, bilinear, and
        // bicubic. Keep an internal fallback deterministic if a direct Rust
        // caller constructs another descriptor.
        _ => at(floor_x, floor_y),
    }
}

/// The I filter subtracts in i32 before promoting to f64. Reuse Rotate's
/// ordered integer cubic polynomial rather than interpreting its storage as RGBA.
#[inline]
fn sample_transform_i32(
    source: &[u8],
    src_w: u32,
    src_h: u32,
    sx: f64,
    sy: f64,
    filter: &ResampleFilter,
) -> i32 {
    let x = (sx - 0.5).floor() as i64;
    let y = (sy - 0.5).floor() as i64;
    let dx = sx - 0.5 - x as f64;
    let dy = sy - 0.5 - y as f64;
    let at = |x: i64, y: i64| {
        let x = x.clamp(0, i64::from(src_w) - 1) as usize;
        let y = y.clamp(0, i64::from(src_h) - 1) as usize;
        let index = (y * src_w as usize + x) * 4;
        i32::from_le_bytes(
            source[index..index + 4]
                .try_into()
                .expect("native sample width"),
        )
    };
    let value = if matches!(filter, ResampleFilter::Bicubic) {
        let rows = std::array::from_fn(|row| {
            let yy = y + row as i64 - 1;
            crate::compute::pool_cpu::ops::geometry::rotate_cubic_horizontal_i32(
                [at(x - 1, yy), at(x, yy), at(x + 1, yy), at(x + 2, yy)],
                dx,
            )
        });
        crate::compute::pool_cpu::ops::geometry::rotate_cubic_vertical_f64(rows, dy)
    } else {
        let p00 = at(x, y);
        let p10 = at(x + 1, y);
        let p01 = at(x, y + 1);
        let p11 = at(x + 1, y + 1);
        let top = f64::from(p10.wrapping_sub(p00)) * dx + f64::from(p00);
        let bottom = f64::from(p11.wrapping_sub(p01)) * dx + f64::from(p01);
        (bottom - top) * dy + top
    };
    value as i32
}

#[inline]
fn sample_transform_scalar32<const IS_FLOAT: bool>(
    source: &[u8],
    src_w: u32,
    src_h: u32,
    sx: f64,
    sy: f64,
    filter: &ResampleFilter,
) -> [u8; 4] {
    if IS_FLOAT {
        sample_transform_f32(source, src_w, src_h, sx, sy, filter).to_le_bytes()
    } else {
        sample_transform_i32(source, src_w, src_h, sx, sy, filter).to_le_bytes()
    }
}

/// Apply Pillow's FLOAT32 filters to a perspective or quadrilateral map.
///
/// The map is evaluated at destination pixel centers using the arm64 FMA
/// grouping emitted for `perspective_transform`/`quad_transform` in
/// `src/libImaging/Geometry.c`; only then does the filter apply its 0.5
/// source-coordinate shift and edge clipping.
#[inline]
#[cfg(target_arch = "wasm32")]
fn quiet_f32_nan(value: f32) -> f32 {
    f32::from_bits(value.to_bits() | (1 << 22))
}

#[inline]
#[cfg(target_arch = "wasm32")]
fn canonical_f32_nan() -> f32 {
    f32::from_bits(0x7fc0_0000)
}

#[inline]
pub(super) fn f32_sub_c(left: f32, right: f32) -> f32 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        left - right
    }
    #[cfg(target_arch = "wasm32")]
    {
        if left.is_nan() {
            return quiet_f32_nan(left);
        }
        if right.is_nan() {
            return quiet_f32_nan(right);
        }
        if left.is_infinite()
            && right.is_infinite()
            && left.is_sign_positive() != right.is_sign_positive()
        {
            return canonical_f32_nan();
        }
        left - right
    }
}

#[inline]
pub(super) fn f32_add_c(left: f32, right: f32) -> f32 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        left + right
    }
    #[cfg(target_arch = "wasm32")]
    {
        if left.is_nan() {
            return quiet_f32_nan(left);
        }
        if right.is_nan() {
            return quiet_f32_nan(right);
        }
        if left.is_infinite()
            && right.is_infinite()
            && left.is_sign_positive() != right.is_sign_positive()
        {
            return canonical_f32_nan();
        }
        left + right
    }
}

#[inline]
pub(super) fn f32_fma_c(left: f32, right: f32, addend: f32) -> f32 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        left.mul_add(right, addend)
    }
    #[cfg(target_arch = "wasm32")]
    {
        if addend.is_nan() {
            return quiet_f32_nan(addend);
        }
        if left.is_nan() {
            return quiet_f32_nan(left);
        }
        if right.is_nan() {
            return quiet_f32_nan(right);
        }
        if (left == 0.0 && right.is_infinite()) || (left.is_infinite() && right == 0.0) {
            return canonical_f32_nan();
        }
        if left.is_infinite()
            && right.is_finite()
            && addend.is_infinite()
            && (left.is_sign_positive() == right.is_sign_positive()) != addend.is_sign_positive()
        {
            return canonical_f32_nan();
        }
        left.mul_add(right, addend)
    }
}

#[inline]
pub(super) fn f64_from_f32_c(value: f32) -> f64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        f64::from(value)
    }
    #[cfg(target_arch = "wasm32")]
    {
        if !value.is_nan() {
            return f64::from(value);
        }
        let bits = value.to_bits();
        let payload = u64::from((bits & 0x007f_ffff) | (1 << 22)) << 29;
        let sign = u64::from(bits & 0x8000_0000) << 32;
        f64::from_bits(sign | 0x7ff0_0000_0000_0000 | payload)
    }
}

#[inline]
pub(super) fn f32_from_f64_c(value: f64) -> f32 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        value as f32
    }
    #[cfg(target_arch = "wasm32")]
    {
        if !value.is_nan() {
            return value as f32;
        }
        let bits = value.to_bits();
        let payload = ((bits & 0x000f_ffff_ffff_ffff) >> 29) as u32 | (1 << 22);
        let sign = ((bits >> 32) as u32) & 0x8000_0000;
        f32::from_bits(sign | 0x7f80_0000 | payload)
    }
}

#[inline]
pub(super) fn f64_neg_c(value: f64) -> f64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        -value
    }
    #[cfg(target_arch = "wasm32")]
    {
        if value.is_nan() {
            // Rust's optimized ARM/WASM lowering keeps the NaN sign through
            // the unary negation used by the bicubic vertical coefficients.
            // Preserve that payload/sign instead of applying a raw bitwise
            // sign flip, which would make WASM differ from the C oracle.
            value
        } else {
            -value
        }
    }
}

#[inline]
pub(super) fn f64_sub_c(left: f64, right: f64) -> f64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        left - right
    }
    #[cfg(target_arch = "wasm32")]
    {
        if left.is_nan() {
            return left;
        }
        if right.is_nan() {
            return right;
        }
        if left.is_infinite()
            && right.is_infinite()
            && left.is_sign_positive() != right.is_sign_positive()
        {
            return f64::from_bits(0x7ff8_0000_0000_0000);
        }
        left - right
    }
}

#[inline]
pub(super) fn f64_add_c(left: f64, right: f64) -> f64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        left + right
    }
    #[cfg(target_arch = "wasm32")]
    {
        if left.is_nan() {
            return left;
        }
        if right.is_nan() {
            return right;
        }
        if left.is_infinite()
            && right.is_infinite()
            && left.is_sign_positive() != right.is_sign_positive()
        {
            return f64::from_bits(0x7ff8_0000_0000_0000);
        }
        left + right
    }
}

#[inline]
pub(super) fn f64_fma_c(left: f64, right: f64, addend: f64) -> f64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        left.mul_add(right, addend)
    }
    #[cfg(target_arch = "wasm32")]
    {
        if addend.is_nan() {
            return addend;
        }
        if left.is_nan() {
            return left;
        }
        if right.is_nan() {
            return right;
        }
        if (left == 0.0 && right.is_infinite()) || (left.is_infinite() && right == 0.0) {
            return f64::from_bits(0x7ff8_0000_0000_0000);
        }
        let product = left * right;
        if product.is_infinite()
            && addend.is_infinite()
            && product.is_sign_positive() != addend.is_sign_positive()
        {
            return f64::from_bits(0x7ff8_0000_0000_0000);
        }
        left.mul_add(right, addend)
    }
}

#[path = "scalar/projective.rs"]
mod projective;
pub(super) use projective::{transform_mesh_scalar32, transform_projective_scalar32};

/// Pillow uses 16.16 nearest affine coordinates when the map is not a
/// pure scale and all four output corners fit its fixed-point range. Keep
/// this decision shared with SIMD; generic/SPECIAL sampling has another ABI.
pub(crate) fn affine_nearest_fixed(data: [f64; 6], width: u32, height: u32) -> Option<[i64; 6]> {
    let [a, b, c, d, e, f] = data;
    if b == 0.0 && d == 0.0 {
        return None;
    }
    if ![
        (0.0, 0.0),
        (f64::from(width), 0.0),
        (0.0, f64::from(height)),
        (f64::from(width), f64::from(height)),
    ]
    .into_iter()
    .all(|(x, y)| {
        (a.mul_add(x, b * y) + c).abs() < 32768.0 && (d.mul_add(x, e * y) + f).abs() < 32768.0
    }) {
        return None;
    }
    let fixed = |value: f64| value.mul_add(65536.0, 0.5).floor() as i64;
    Some([
        fixed(a),
        fixed(b),
        fixed(c + a * 0.5 + b * 0.5),
        fixed(d),
        fixed(e),
        fixed(f + d * 0.5 + e * 0.5),
    ])
}
