use crate::error::PilError;
use crate::image::preserve_mode;
use crate::ops::pil_resize::{premultiply_alpha, unpremultiply_alpha};
use crate::pipeline::{ResampleFilter, TransformMethod};
use crate::raster::DynamicImage;

#[path = "affine.rs"]
mod affine;
#[path = "projective.rs"]
mod projective;
#[path = "scalar.rs"]
mod scalar;
#[cfg(test)]
#[path = "tests.rs"]
mod tests;

use affine::{
    transform_affine_generic, transform_affine_luma_alpha_nearest, transform_affine_luma_nearest,
    transform_affine_rgb_nearest,
};
use projective::{transform_luma16, transform_mesh, transform_projective_generic};
use scalar::{transform_affine_f32, transform_mesh_scalar32, transform_projective_scalar32};

pub(crate) use scalar::affine_nearest_fixed;

fn try_transform_affine_nearest(
    image: &DynamicImage,
    width: u32,
    height: u32,
    coefficients: [f64; 6],
    fill: Option<(u8, u8, u8, u8)>,
    explicit_mode: Option<&str>,
    use_nearest: bool,
) -> Result<Option<DynamicImage>, PilError> {
    if !use_nearest {
        return Ok(None);
    }
    if matches!(explicit_mode, None | Some("L"))
        && let Some(result) =
            transform_affine_luma_nearest(image, width, height, coefficients, fill)?
    {
        return Ok(Some(result));
    }
    if matches!(explicit_mode, None | Some("LA"))
        && let Some(result) =
            transform_affine_luma_alpha_nearest(image, width, height, coefficients, fill)?
    {
        return Ok(Some(result));
    }
    if matches!(explicit_mode, None | Some("RGB" | "HSV" | "YCbCr"))
        && let Some(result) =
            transform_affine_rgb_nearest(image, width, height, coefficients, fill)?
    {
        return Ok(Some(result));
    }
    Ok(None)
}

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
        return transform_i32_filtered(img, w, h, method, data, filter, fill, fill_is_none);
    }

    match method {
        TransformMethod::Affine => {
            transform_affine_dispatch(img, w, h, data, filter, fill, explicit_mode)
        }
        TransformMethod::Mesh => {
            transform_mesh_dispatch(img, w, h, data, filter, fill, fill_is_none, explicit_mode)
        }
        TransformMethod::Perspective => transform_projective_dispatch::<false>(
            img,
            w,
            h,
            data,
            filter,
            fill,
            fill_is_none,
            explicit_mode,
        ),
        TransformMethod::Quad => transform_projective_dispatch::<true>(
            img,
            w,
            h,
            data,
            filter,
            fill,
            fill_is_none,
            explicit_mode,
        ),
    }
}

fn transform_i32_filtered(
    img: &DynamicImage,
    w: u32,
    h: u32,
    method: &TransformMethod,
    data: &[f64],
    filter: &ResampleFilter,
    fill: Option<(u8, u8, u8, u8)>,
    fill_is_none: bool,
) -> Result<DynamicImage, PilError> {
    match method {
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
    }
}

fn transform_affine_dispatch(
    img: &DynamicImage,
    w: u32,
    h: u32,
    data: &[f64],
    filter: &ResampleFilter,
    fill: Option<(u8, u8, u8, u8)>,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // The public wrappers validate the six-coefficient contract before queuing.
    let (aff_a, aff_b, aff_c, aff_d, aff_e, aff_f) =
        (data[0], data[1], data[2], data[3], data[4], data[5]);
    let p_mode = explicit_mode == Some("P") || explicit_mode == Some("1");
    let i_f_mode = explicit_mode == Some("I") || explicit_mode == Some("F");
    if explicit_mode == Some("F") && !matches!(filter, ResampleFilter::Nearest) {
        let result = transform_affine_f32(img, w, h, data, filter, fill)?;
        return Ok(preserve_mode(img, result));
    }
    let use_nearest = matches!(filter, ResampleFilter::Nearest) || p_mode || i_f_mode;

    // Pillow filters LA/RGBA in premultiplied storage, then unpremultiplies.
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

    if let Some(result) = try_transform_affine_nearest(
        work,
        w,
        h,
        [aff_a, aff_b, aff_c, aff_d, aff_e, aff_f],
        fill,
        explicit_mode,
        use_nearest,
    )? {
        return Ok(preserve_mode(img, result));
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
        fill,
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

fn transform_mesh_dispatch(
    img: &DynamicImage,
    w: u32,
    h: u32,
    data: &[f64],
    filter: &ResampleFilter,
    fill: Option<(u8, u8, u8, u8)>,
    fill_is_none: bool,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if explicit_mode == Some("F") && !matches!(filter, ResampleFilter::Nearest) {
        let result = transform_mesh_scalar32::<true>(img, w, h, data, fill, fill_is_none, filter)?;
        return Ok(preserve_mode(img, result));
    }

    // Pillow premultiplies LA/RGBA before non-nearest mesh filtering.
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

fn transform_projective_dispatch<const QUAD: bool>(
    img: &DynamicImage,
    w: u32,
    h: u32,
    data: &[f64],
    filter: &ResampleFilter,
    fill: Option<(u8, u8, u8, u8)>,
    fill_is_none: bool,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if explicit_mode == Some("F") && !matches!(filter, ResampleFilter::Nearest) {
        let result = transform_projective_scalar32::<true>(
            img,
            w,
            h,
            &data[..8],
            filter,
            fill,
            fill_is_none,
            QUAD,
        )?;
        return Ok(preserve_mode(img, result));
    }

    // Filtered projective paths premultiply LA/RGBA to avoid transparent
    // color bleed; palette and scalar modes retain their native storage.
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
    let result = transform_projective_generic(work, w, h, &data[..8], filter, fill, QUAD)?;
    let result = if needs_alpha_roundtrip {
        unpremultiply_alpha(&result)
    } else {
        result
    };
    Ok(preserve_mode(img, result))
}
