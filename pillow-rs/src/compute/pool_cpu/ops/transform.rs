use crate::error::PilError;
use crate::image::preserve_mode;
use crate::ops::pil_resize::{premultiply_alpha, unpremultiply_alpha};
use crate::pipeline::{ResampleFilter, TransformMethod};
use crate::raster::DynamicImage;

#[path = "transform/affine.rs"]
mod affine;
#[path = "transform/projective.rs"]
mod projective;
#[path = "transform/scalar.rs"]
mod scalar;
#[cfg(test)]
#[path = "transform/tests.rs"]
mod tests;

use affine::{
    transform_affine_generic, transform_affine_luma_alpha_nearest, transform_affine_luma_nearest,
    transform_affine_rgb_nearest,
};
use projective::{transform_luma16, transform_mesh, transform_projective_generic};
use scalar::{transform_affine_f32, transform_mesh_scalar32, transform_projective_scalar32};

pub(crate) use scalar::affine_nearest_fixed;

pub fn op_transform(
    img: &DynamicImage,
    w: u32,
    h: u32,
    method: &TransformMethod,
    data: &[f64],
    filter: &ResampleFilter,
    fill: Option<(u8, u8, u8, u8)>,
    fill_is_none: bool,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if matches!(img, DynamicImage::ImageLuma16(_)) {
        return transform_luma16(
            img,
            w,
            h,
            method,
            data,
            filter,
            fill,
            fill_is_none,
            explicit_mode,
        );
    }
    if explicit_mode == Some("I") && !matches!(filter, ResampleFilter::Nearest) {
        return match method {
            TransformMethod::Affine => {
                let mut projective = data.to_vec();
                projective.extend([0.0, 0.0]);
                transform_projective_scalar32::<false>(
                    img,
                    w,
                    h,
                    &projective,
                    filter,
                    fill,
                    fill_is_none,
                    false,
                )
            }
            TransformMethod::Perspective | TransformMethod::Quad => {
                transform_projective_scalar32::<false>(
                    img,
                    w,
                    h,
                    data,
                    filter,
                    fill,
                    fill_is_none,
                    matches!(method, TransformMethod::Quad),
                )
            }
            TransformMethod::Mesh => {
                transform_mesh_scalar32::<false>(img, w, h, data, fill, fill_is_none, filter)
            }
        };
    }

    match method {
        TransformMethod::Affine => {
            // `Image::transform_public` and the maintained affine wrappers
            // validate the exact six-coefficient contract before queuing the
            // operation. A malformed transform descriptor is outside the
            // supported public input boundary, so the executor can index the
            // validated coefficients directly.
            let (aff_a, aff_b, aff_c, aff_d, aff_e, aff_f) =
                (data[0], data[1], data[2], data[3], data[4], data[5]);
            let p_mode = explicit_mode == Some("P") || explicit_mode == Some("1");
            let i_f_mode = explicit_mode == Some("I") || explicit_mode == Some("F");
            if explicit_mode == Some("F") && !matches!(filter, ResampleFilter::Nearest) {
                let result = transform_affine_f32(img, w, h, data, filter, fill)?;
                return Ok(preserve_mode(img, result));
            }
            let use_nearest = matches!(filter, ResampleFilter::Nearest) || p_mode || i_f_mode;

            // Pillow's ImagingTransformAffine resamples LA/RGBA through the
            // premultiplied La/RGBa modes, then unpremultiplies the result.
            // The fill sample is written as supplied into that temporary mode
            // and is unpremultiplied along with the sampled pixels.
            let needs_alpha_roundtrip = !use_nearest
                && !matches!(
                    explicit_mode,
                    Some("PA") | Some("La") | Some("RGBa") | Some("RGBX")
                )
                && !matches!(explicit_mode, Some("CMYK" | "I" | "F"))
                && matches!(
                    img.color(),
                    crate::raster::ColorType::La8 | crate::raster::ColorType::Rgba8
                );
            let premultiplied = needs_alpha_roundtrip.then(|| premultiply_alpha(img));
            let work = premultiplied.as_ref().unwrap_or(img);
            let transform_fill = fill;

            if matches!(explicit_mode, None | Some("L")) && use_nearest {
                if let Some(result) = transform_affine_luma_nearest(
                    work,
                    w,
                    h,
                    [aff_a, aff_b, aff_c, aff_d, aff_e, aff_f],
                    transform_fill,
                )? {
                    return Ok(preserve_mode(img, result));
                }
            }
            if matches!(explicit_mode, None | Some("LA")) && use_nearest {
                if let Some(result) = transform_affine_luma_alpha_nearest(
                    work,
                    w,
                    h,
                    [aff_a, aff_b, aff_c, aff_d, aff_e, aff_f],
                    transform_fill,
                )? {
                    return Ok(preserve_mode(img, result));
                }
            }
            if matches!(explicit_mode, None | Some("RGB" | "HSV" | "YCbCr")) && use_nearest {
                if let Some(result) = transform_affine_rgb_nearest(
                    work,
                    w,
                    h,
                    [aff_a, aff_b, aff_c, aff_d, aff_e, aff_f],
                    transform_fill,
                )? {
                    return Ok(preserve_mode(img, result));
                }
            }

            let result = transform_affine_generic(
                work,
                w,
                h,
                aff_a,
                aff_b,
                aff_c,
                aff_d,
                aff_e,
                aff_f,
                transform_fill,
                use_nearest,
                matches!(filter, ResampleFilter::Bicubic),
            )?;
            let result = if needs_alpha_roundtrip {
                unpremultiply_alpha(&result)
            } else {
                result
            };
            Ok(preserve_mode(img, result))
        }
        &TransformMethod::Mesh => {
            // `Image::transform_public` validates every mesh record and
            // flattens it into complete twelve-value elements before queuing
            // this operation. The old public raw-data wrapper was removed, so
            // malformed mesh descriptors are outside the supported input
            // boundary.
            if explicit_mode == Some("F") && !matches!(filter, ResampleFilter::Nearest) {
                let result =
                    transform_mesh_scalar32::<true>(img, w, h, data, fill, fill_is_none, filter)?;
                return Ok(preserve_mode(img, result));
            }
            // Pillow's non-nearest Image.transform path premultiplies LA/RGBA
            // before invoking the mesh filter and unpremultiplies afterward.
            // Mesh records otherwise operate on the same native byte layout
            // as the scalar transform implementation.
            let needs_alpha_roundtrip = !matches!(filter, ResampleFilter::Nearest)
                && !matches!(
                    explicit_mode,
                    Some("PA" | "La" | "RGBa" | "RGBX" | "CMYK" | "I" | "F")
                )
                && matches!(
                    img.color(),
                    crate::raster::ColorType::La8 | crate::raster::ColorType::Rgba8
                );
            let premultiplied = needs_alpha_roundtrip.then(|| premultiply_alpha(img));
            let work = premultiplied.as_ref().unwrap_or(img);
            let result = transform_mesh(work, w, h, data, fill, *filter, fill_is_none)?;
            let result = if needs_alpha_roundtrip {
                unpremultiply_alpha(&result)
            } else {
                result
            };
            Ok(preserve_mode(img, result))
        }
        &TransformMethod::Perspective => {
            // `Image::transform_public` and the maintained perspective
            // wrapper validate the exact eight-coefficient contract before
            // queuing this operation.
            if explicit_mode == Some("F") && !matches!(filter, ResampleFilter::Nearest) {
                let result = transform_projective_scalar32::<true>(
                    img,
                    w,
                    h,
                    &data[..8],
                    filter,
                    fill,
                    fill_is_none,
                    false,
                )?;
                return Ok(preserve_mode(img, result));
            }
            // Pillow's filtered projective path uses the same premultiplied
            // La/RGBa round trip as affine and mesh transforms. Without it,
            // straight RGB/luma channels from transparent pixels bleed into
            // the filtered result. PA, RGBa, RGBX, CMYK, I, and F already
            // carry native raw/scalar representations and must stay untouched.
            let needs_alpha_roundtrip = !matches!(filter, ResampleFilter::Nearest)
                && !matches!(
                    explicit_mode,
                    Some("PA" | "La" | "RGBa" | "RGBX" | "CMYK" | "I" | "F")
                )
                && matches!(
                    img.color(),
                    crate::raster::ColorType::La8 | crate::raster::ColorType::Rgba8
                );
            let premultiplied = needs_alpha_roundtrip.then(|| premultiply_alpha(img));
            let work = premultiplied.as_ref().unwrap_or(img);
            let result = transform_projective_generic(work, w, h, &data[..8], filter, fill, false)?;
            let result = if needs_alpha_roundtrip {
                unpremultiply_alpha(&result)
            } else {
                result
            };
            Ok(preserve_mode(img, result))
        }
        &TransformMethod::Quad => {
            // `Image::transform_public` and the maintained quad wrapper
            // validate the exact eight-coordinate contract before queuing
            // this operation.
            if explicit_mode == Some("F") && !matches!(filter, ResampleFilter::Nearest) {
                let result = transform_projective_scalar32::<true>(
                    img,
                    w,
                    h,
                    &data[..8],
                    filter,
                    fill,
                    fill_is_none,
                    true,
                )?;
                return Ok(preserve_mode(img, result));
            }
            // Geometry.c's Quad filter also works on the premultiplied
            // temporary for LA/RGBA. Keep PA/raw-premultiplied and scalar
            // compatibility modes out of this conversion just as in the
            // affine and mesh paths.
            let needs_alpha_roundtrip = !matches!(filter, ResampleFilter::Nearest)
                && !matches!(
                    explicit_mode,
                    Some("PA" | "La" | "RGBa" | "RGBX" | "CMYK" | "I" | "F")
                )
                && matches!(
                    img.color(),
                    crate::raster::ColorType::La8 | crate::raster::ColorType::Rgba8
                );
            let premultiplied = needs_alpha_roundtrip.then(|| premultiply_alpha(img));
            let work = premultiplied.as_ref().unwrap_or(img);
            let result = transform_projective_generic(work, w, h, &data[..8], filter, fill, true)?;
            let result = if needs_alpha_roundtrip {
                unpremultiply_alpha(&result)
            } else {
                result
            };
            Ok(preserve_mode(img, result))
        }
    }
}
