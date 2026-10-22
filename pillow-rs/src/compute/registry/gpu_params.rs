//! GPU uniform parameter packing and geometry proofs.

use super::gpu_support::gpu_chops_affine_lut;
use super::{
    filter_offset_i32, gpu_blend_alpha_params, gpu_brightness_factor_int,
    gpu_color_saturation_factor_params, gpu_sharpness_factor_int,
};
use crate::pipeline::{ColorMode, PipelineOp, ResampleFilter, TransposeMethod};

#[cfg(feature = "gpu")]
fn separable_box_blur_params(radius: u32) -> [u32; 3] {
    let radius = radius.min(64);
    let window = radius.saturating_mul(2).saturating_add(1);
    let weight = ((1u64 << 24) / u64::from(window)) as u32;
    [radius, weight, 0]
}

/// Compute the exact fixed-point parameters used by Pillow's one-dimensional
/// box blur for a finite non-negative radius.  The integer radius selects the
/// replicated samples in the rolling window; the fractional remainder is
/// represented by the two edge weights.  Keeping this calculation in the
/// scalar control plane lets the GPU data plane use the same 24-bit contract
/// for both tuple and fractional public inputs.
#[cfg(feature = "gpu")]
pub(crate) fn separable_box_blur_params_f32(radius: f32) -> Option<[u32; 3]> {
    if !radius.is_finite() || radius < 0.0 || radius > 64.0 {
        return None;
    }
    let integer_radius = radius as u32;
    let window = integer_radius.saturating_mul(2).saturating_add(1);
    let weight = (((1u64 << 24) as f32 / (radius * 2.0 + 1.0)) as u32).min(1 << 24);
    let used = u64::from(window).saturating_mul(u64::from(weight));
    let edge_weight = ((1u64 << 24).saturating_sub(used) / 2) as u32;
    Some([integer_radius, weight, edge_weight])
}

/// Compute the exact fractional box-blur parameters used by Pillow's
/// three-pass Gaussian approximation. The GPU horizontal/vertical kernels
/// consume these fixed-point words directly, so a Gaussian operation can be
/// expanded into six ordered dispatches without a CPU readback between them.
#[cfg(feature = "gpu")]
fn separable_gaussian_blur_radius_and_fraction(sigma: f32) -> Option<(u32, f32)> {
    if !sigma.is_finite() {
        return None;
    }
    // Pillow's ImagingGaussianBlur normalizes each radius by magnitude. Keep
    // the GPU parameter proof aligned with the CPU implementation and the
    // oracle: negative finite radii are valid and produce the same result as
    // their positive counterparts.
    let sigma = sigma.abs();
    if sigma == 0.0 {
        return Some((0, 0.0));
    }
    let sigma2 = f64::from(sigma) * f64::from(sigma) / 3.0;
    let l = ((12.0 * sigma2 + 1.0).sqrt() - 1.0) / 2.0;
    let l = l.floor();
    let l1 = l + 1.0;
    let a_num = (2.0 * l + 1.0) * (l * l1 - 3.0 * sigma2);
    let a_den = 6.0 * (sigma2 - l1 * l1);
    let a = a_num / a_den;
    let radius = (l + a) as f32;
    // The rolling WGSL kernels and their bounded loop contract support the
    // complete radius range through 64.  Keep the host proof aligned with
    // that shader limit so small images with a large Pillow sigma do not take
    // an unnecessary CPU path.
    if !radius.is_finite() || radius < 0.0 || radius > 64.0 {
        return None;
    }
    Some((radius as u32, radius))
}

#[cfg(feature = "gpu")]
pub(crate) fn separable_gaussian_blur_radius(sigma: f32) -> Option<u32> {
    separable_gaussian_blur_radius_and_fraction(sigma).map(|(radius, _)| radius)
}

#[cfg(feature = "gpu")]
pub(crate) fn separable_gaussian_blur_params(sigma: f32) -> [u32; 3] {
    let Some((radius, fractional_radius)) = separable_gaussian_blur_radius_and_fraction(sigma)
    else {
        return separable_box_blur_params(0);
    };
    let window = radius.saturating_mul(2).saturating_add(1);
    let weight = (((1u64 << 24) as f32 / (fractional_radius * 2.0 + 1.0)) as u32).min(1 << 24);
    let used = u64::from(window) * u64::from(weight);
    let edge_weight = ((1u64 << 24).saturating_sub(used) / 2) as u32;
    [radius, weight, edge_weight]
}

#[cfg(feature = "gpu")]
fn no_param_params(op: &PipelineOp) -> Option<Vec<u32>> {
    let params = match op {
        // ── No-param ops ──
        PipelineOp::Invert
        | PipelineOp::Flip
        | PipelineOp::Mirror
        | PipelineOp::Duplicate
        | PipelineOp::InvertChops
        | PipelineOp::Grayscale
        | PipelineOp::AlphaComposite { .. }
        | PipelineOp::Eval { .. }
        | PipelineOp::Multiply { .. }
        | PipelineOp::Screen { .. }
        | PipelineOp::Difference { .. }
        | PipelineOp::Darker { .. }
        | PipelineOp::Lighter { .. }
        | PipelineOp::AddModulo { .. }
        | PipelineOp::SubtractModulo { .. }
        | PipelineOp::LogicalAnd { .. }
        | PipelineOp::LogicalOr { .. }
        | PipelineOp::LogicalXor { .. }
        | PipelineOp::Overlay { .. }
        | PipelineOp::HardLight { .. }
        | PipelineOp::SoftLight { .. } => vec![],

        _ => return None,
    };
    Some(params)
}

#[cfg(feature = "gpu")]
fn adjustment_params(op: &PipelineOp) -> Option<Vec<u32>> {
    let params = match op {
        // ── CompositeModule: mask_alpha ──
        PipelineOp::CompositeModule { mask_alpha, .. } => vec![*mask_alpha as u32],

        // ── Solarize: threshold ──
        PipelineOp::Solarize { threshold } => vec![*threshold as u32],

        // ── Posterize: bits ──
        PipelineOp::Posterize { bits } => vec![*bits as u32],

        // ── Brightness: scalar-verified fixed-point factor ──
        PipelineOp::Brightness { factor } => {
            vec![gpu_brightness_factor_int(*factor).unwrap_or(0)]
        }

        // ── Contrast: Pillow float32 blend factor ──
        PipelineOp::Contrast { factor } => {
            vec![gpu_blend_alpha_params(*factor).unwrap_or(0)]
        }

        // ── ColorSaturation: exact float32 factor bits ──
        PipelineOp::ColorSaturation { factor } => {
            vec![gpu_color_saturation_factor_params(*factor).unwrap_or(0)]
        }

        // ── Sharpness: scalar-proven fixed-point factor ──
        PipelineOp::Sharpness { factor } => {
            vec![gpu_sharpness_factor_int(*factor).unwrap_or(0)]
        }

        // ── Colorize: pack black/white/mid as u32 RGBA (0xAABBGGRR) ──
        PipelineOp::Colorize {
            black,
            white,
            mid,
            blackpoint,
            midpoint,
            whitepoint,
        } => {
            let bk = (black.0 as u32)
                | ((black.1 as u32) << 8)
                | ((black.2 as u32) << 16)
                | (0xff << 24);
            let wh = (white.0 as u32)
                | ((white.1 as u32) << 8)
                | ((white.2 as u32) << 16)
                | (0xff << 24);
            let mid_value = mid
                .map(|mid| {
                    (mid.0 as u32) | ((mid.1 as u32) << 8) | ((mid.2 as u32) << 16) | (0xff << 24)
                })
                .unwrap_or(bk);
            vec![
                bk,
                wh,
                mid_value,
                *blackpoint as u32,
                *midpoint as u32,
                *whitepoint as u32,
                u32::from(mid.is_some()),
            ]
        }

        _ => return None,
    };
    Some(params)
}

#[cfg(feature = "gpu")]
fn chops_params(op: &PipelineOp) -> Option<Vec<u32>> {
    let params = match op {
        // ── Constant: value ──
        PipelineOp::Constant { value } => vec![*value as u32],

        // ── Offset: dx, dy ──
        // Preserve the complete signed i32 bit pattern. The shader performs
        // the modulo using the image dimensions, so reducing here to a fixed
        // 16-bit range would change valid offsets on wide/tall images.
        PipelineOp::Offset { x, y } => vec![*x as u32, *y as u32],

        // ── Add / Subtract: packed parameter-only affine lookup ──
        PipelineOp::Add { scale, offset, .. } => gpu_chops_affine_lut(*scale, *offset, false),
        PipelineOp::Subtract { scale, offset, .. } => gpu_chops_affine_lut(*scale, *offset, true),

        // ── BlendModule: native f32 alpha bits ──
        PipelineOp::BlendModule { alpha, .. } => {
            vec![gpu_blend_alpha_params(*alpha).unwrap_or(0)]
        }

        _ => return None,
    };
    Some(params)
}

#[cfg(feature = "gpu")]
fn filter_params(op: &PipelineOp) -> Option<Vec<u32>> {
    let params = match op {
        // ── BoxBlur: horizontal and vertical radius/weight triplets ──
        PipelineOp::BoxBlur { radius } => {
            let params = separable_box_blur_params(*radius);
            params.into_iter().chain(params).collect()
        }
        // BoxBlurXY uses the same six-word parameter block. The pass count is
        // used by the GPU encoder to repeat the horizontal and vertical
        // dispatches, so it does not need to be copied into the uniform block.
        PipelineOp::BoxBlurXY {
            radius_x,
            radius_y,
            passes,
        } => {
            let x = separable_box_blur_params_f32(*radius_x)
                .unwrap_or_else(|| separable_box_blur_params(0));
            let y = separable_box_blur_params_f32(*radius_y)
                .unwrap_or_else(|| separable_box_blur_params(0));
            let _ = passes;
            x.into_iter().chain(y).collect()
        }

        // ── MedianFilter / MaxFilter / MinFilter: size ──
        PipelineOp::MedianFilter { size }
        | PipelineOp::MaxFilter { size }
        | PipelineOp::MinFilter { size } => vec![*size],

        // ── RankFilter: size, rank ──
        PipelineOp::RankFilter { size, rank } => vec![*size, *rank],

        // ── Filter3x3: kernel [9] f32 bits + offset + rational denominator ──
        PipelineOp::Filter3x3 {
            kernel,
            scale,
            offset,
        } => {
            let s = *scale;
            let mut params = Vec::with_capacity(11);
            for k in kernel.iter() {
                params.push((k / s).to_bits());
            }
            params.push(filter_offset_i32(*offset).unwrap_or_default() as u32);
            // The byte kernel follows Pillow's f32/FMA contraction order.
            // Keep the optional integer mode disabled here: integer
            // rationalization can move an exact half-way value across the
            // truncation boundary even when the coefficients reconstruct
            // their normalized f32 values.
            params.push(0);
            params
        }

        // ── Filter5x5: kernel [25] f32 bits + offset + rational denominator ──
        PipelineOp::Filter5x5 {
            kernel,
            scale,
            offset,
        } => {
            let s = *scale;
            let mut params = Vec::with_capacity(27);
            for k in kernel.iter() {
                params.push((k / s).to_bits());
            }
            params.push(filter_offset_i32(*offset).unwrap_or_default() as u32);
            // See the 3x3 path: preserve Pillow's f32/FMA rounding rather
            // than replacing it with integer rational arithmetic.
            params.push(0);
            params
        }

        _ => return None,
    };
    Some(params)
}

#[cfg(feature = "gpu")]
fn format_geometry_params(op: &PipelineOp) -> Option<Vec<u32>> {
    let params = match op {
        // ── Resize: dst_w, dst_h ──
        PipelineOp::Resize { w, h, .. } => vec![*w, *h],
        PipelineOp::ResizeBoxed { w, h, .. } => vec![*w, *h],

        // ── Transpose: op_code ──
        PipelineOp::Transpose { method } => {
            let code: u32 = match method {
                TransposeMethod::FlipLeftRight => 0,
                TransposeMethod::FlipTopBottom => 1,
                TransposeMethod::Rotate90 => 2,
                TransposeMethod::Rotate180 => 3,
                TransposeMethod::Rotate270 => 4,
                TransposeMethod::Transpose => 5,
                TransposeMethod::Transverse => 6,
            };
            vec![code]
        }

        // ── Scale: factor as fixed-point (factor * 65536) ──
        PipelineOp::Scale { factor, .. } => vec![(factor * 65536.0) as u32],

        // ── Convert: encode ColorMode as u32 code ──
        PipelineOp::Convert { mode, .. } => {
            let code: u32 = match mode {
                ColorMode::L => 0,
                ColorMode::LA => 1,
                ColorMode::RGB => 2,
                ColorMode::RGBA => 3,
                ColorMode::CMYK => 4,
                ColorMode::YCbCr => 5,
                ColorMode::HSV => 6,
                ColorMode::I => 7,
                ColorMode::F => 8,
                ColorMode::P => 9,
                ColorMode::Mode1 => 10,
            };
            vec![code]
        }

        _ => return None,
    };
    Some(params)
}

#[cfg(feature = "gpu")]
fn channel_mutation_params(op: &PipelineOp) -> Option<Vec<u32>> {
    let params = match op {
        // ── PutAlpha: alpha as u32 ──
        PipelineOp::PutAlpha { alpha, mode } => vec![*alpha as u32, mode.code()],
        PipelineOp::PutAlphaData { mode, .. } => vec![0, mode.code()],

        // ── PutPixel: x, y, color packed as RGBA u32 ──
        PipelineOp::PutPixel { x, y, color, .. } => {
            let (r, g, b, a) = *color;
            let packed = (r as u32) | ((g as u32) << 8) | ((b as u32) << 16) | ((a as u32) << 24);
            vec![*x, *y, packed]
        }

        _ => return None,
    };
    Some(params)
}

#[cfg(feature = "gpu")]
fn image_geometry_params(op: &PipelineOp) -> Option<Vec<u32>> {
    let params = match op {
        // ── Crop: left, top, width, height ──
        PipelineOp::Crop {
            left,
            top,
            right,
            bottom,
        } => {
            vec![*left, *top, *right - *left, *bottom - *top]
        }

        // ── Reduce: horizontal and vertical factors ──
        PipelineOp::Reduce { x_factor, y_factor } => vec![*x_factor, *y_factor],

        // ── Thumbnail: dst_w, dst_h ──
        PipelineOp::Thumbnail { w, h, .. } => vec![*w, *h],

        // ── Contain: dst_w, dst_h ──
        PipelineOp::Contain { w, h, .. } => vec![*w, *h],

        // ── Cover: new_w, new_h, crop_x, crop_y ──
        PipelineOp::Cover { w, h, .. } => vec![*w, *h, 0, 0],

        // ── Fit: new_w, new_h, crop_x, crop_y ──
        PipelineOp::Fit { w, h, .. } => vec![*w, *h, 0, 0],

        // ── Paste: src_w, src_h, paste_x, paste_y, has_mask, mask_alpha ──
        PipelineOp::Paste {
            w,
            h,
            x,
            y,
            mask,
            mask_alpha,
            ..
        } => {
            vec![
                (*w).max(0) as u32,
                (*h).max(0) as u32,
                *x as u32,
                *y as u32,
                mask.is_some() as u32,
                *mask_alpha as u32,
            ]
        }

        _ => return None,
    };
    Some(params)
}

#[cfg(feature = "gpu")]
fn transform_params(op: &PipelineOp) -> Option<Vec<u32>> {
    let params = match op {
        // ── Transform: dst_w, dst_h, a..f bits, fill_color, filter_code ──
        PipelineOp::Transform {
            w,
            h,
            data,
            filter,
            fill,
            palette_fill,
            ..
        } => {
            let a = (data.first().copied().unwrap_or(0.0) as f32).to_bits();
            let b = (data.get(1).copied().unwrap_or(0.0) as f32).to_bits();
            let c = (data.get(2).copied().unwrap_or(0.0) as f32).to_bits();
            let d = (data.get(3).copied().unwrap_or(0.0) as f32).to_bits();
            let e = (data.get(4).copied().unwrap_or(0.0) as f32).to_bits();
            let f = (data.get(5).copied().unwrap_or(0.0) as f32).to_bits();
            let resolved_fill = palette_fill.map(|index| (index, 0, 0, 255)).or(*fill);
            let fill_color = match resolved_fill {
                Some((r, g, b, a)) => {
                    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16) | ((a as u32) << 24)
                }
                None => 0,
            };
            let filter_code = match filter {
                ResampleFilter::Nearest => 0u32,
                ResampleFilter::Bilinear => 1,
                ResampleFilter::Bicubic => 2,
                ResampleFilter::Lanczos => 3,
                ResampleFilter::Box => 4,
                ResampleFilter::Hamming => 5,
            };
            vec![*w, *h, a, b, c, d, e, f, fill_color, filter_code]
        }

        _ => return None,
    };
    Some(params)
}

#[cfg(feature = "gpu")]
fn data_mutation_params(op: &PipelineOp) -> Option<Vec<u32>> {
    let params = match op {
        // ── PutData: data length ──
        PipelineOp::PutData { data, mode } => vec![data.len() as u32, mode.code()],

        // ── CropBorder: border ──
        PipelineOp::CropBorder { border } => vec![*border],

        // ── Expand: border, fill_color ──
        // src_w/src_h come from the header (cur_w/cur_h), not from extract_params.
        PipelineOp::Expand { border, fill } => {
            let (r, g, b, a) = *fill;
            vec![
                *border,
                (r as u32) | ((g as u32) << 8) | ((b as u32) << 16) | ((a as u32) << 24),
            ]
        }

        // ── Merge: num_bands ──
        PipelineOp::Merge { bands, .. } => vec![bands.len() as u32],

        // ── RemapPalette: LUT uploaded via extract_lut, no params ──
        PipelineOp::RemapPalette { .. } => vec![],

        _ => return None,
    };
    Some(params)
}

#[cfg(feature = "gpu")]
fn placement_params(op: &PipelineOp) -> Option<Vec<u32>> {
    let params = match op {
        // ── Pad: src_w, src_h, fill, centering_x, centering_y, scale_x, scale_y ──
        PipelineOp::Pad {
            w,
            h,
            color,
            centering,
            ..
        } => {
            let fill = match color {
                Some((r, g, b, a)) => {
                    (*r as u32) | ((*g as u32) << 8) | ((*b as u32) << 16) | ((*a as u32) << 24)
                }
                None => 0xff000000u32,
            };
            let cx = (centering.0.clamp(0.0, 1.0) * 65536.0) as u32;
            let cy = (centering.1.clamp(0.0, 1.0) * 65536.0) as u32;
            vec![*w, *h, fill, cx, cy, 65536u32, 65536u32]
        }

        // ── Rotate: cos_theta, sin_theta, fill, expand ──
        PipelineOp::Rotate {
            angle,
            fill,
            expand,
            ..
        } => {
            let cos_t = (angle.cos() as f32).to_bits();
            let sin_t = (angle.sin() as f32).to_bits();
            let fill_color = match fill {
                Some((r, g, b, a)) => {
                    (*r as u32) | ((*g as u32) << 8) | ((*b as u32) << 16) | ((*a as u32) << 24)
                }
                None => 0u32,
            };
            vec![cos_t, sin_t, fill_color, *expand as u32]
        }

        _ => return None,
    };
    Some(params)
}

#[cfg(feature = "gpu")]
fn effects_params(op: &PipelineOp) -> Option<Vec<u32>> {
    let params = match op {
        // ── GaussianBlur: identical horizontal/vertical triplets ──
        PipelineOp::GaussianBlur { sigma } => {
            let params = separable_gaussian_blur_params(*sigma);
            params.into_iter().chain(params).collect()
        }

        // ── Autocontrast: cutoff ──
        PipelineOp::Autocontrast { cutoff, .. } => vec![(*cutoff as f32).to_bits()],

        // ── Equalize: no params ──
        PipelineOp::Equalize | PipelineOp::EqualizeMasked { .. } => vec![],

        // ── Color3DLut: size dims ──
        PipelineOp::Color3DLut { size, .. } => vec![size.0, size.1, size.2],

        // ── ExtractBand: channel index ──
        PipelineOp::ExtractBand { index } => vec![*index as u32],

        _ => return None,
    };
    Some(params)
}

/// Extracts GPU shader parameter words from a pipeline operation.
///
/// The returned `Vec<u32>` follows each shader's `Params` struct after the
/// shared four-word header: `width`, `height`, `pad0`, `pad1`. Operations with
/// no shader parameter block return an empty vector.
#[cfg(feature = "gpu")]
pub fn extract_params(op: &PipelineOp) -> Vec<u32> {
    if let Some(params) = no_param_params(op) {
        return params;
    }
    if let Some(params) = adjustment_params(op) {
        return params;
    }
    if let Some(params) = chops_params(op) {
        return params;
    }
    if let Some(params) = filter_params(op) {
        return params;
    }
    if let Some(params) = format_geometry_params(op) {
        return params;
    }
    if let Some(params) = channel_mutation_params(op) {
        return params;
    }
    if let Some(params) = image_geometry_params(op) {
        return params;
    }
    if let Some(params) = transform_params(op) {
        return params;
    }
    if let Some(params) = data_mutation_params(op) {
        return params;
    }
    if let Some(params) = placement_params(op) {
        return params;
    }
    if let Some(params) = effects_params(op) {
        return params;
    }
    Vec::new()
}
