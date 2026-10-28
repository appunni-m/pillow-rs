//! CPU Reduce family routing by the Pillow mode carried by the image wrapper.

use crate::error::PilError;
use crate::raster::DynamicImage;

mod kernels;

#[derive(Clone, Copy)]
struct ReduceAlphaLayout {
    premultiplied_alpha: bool,
    native_rgba_reduce: bool,
}

fn alpha_layout(img: &DynamicImage, explicit_mode: Option<&str>) -> ReduceAlphaLayout {
    let premultiplied_alpha = matches!(
        img.color(),
        crate::raster::ColorType::La8 | crate::raster::ColorType::Rgba8
    ) && !matches!(
        explicit_mode,
        Some("CMYK" | "RGBa" | "La" | "PA" | "RGBX" | "F" | "I")
    );
    ReduceAlphaLayout {
        premultiplied_alpha,
        native_rgba_reduce: premultiplied_alpha
            && explicit_mode == Some("RGBA")
            && matches!(img, DynamicImage::ImageRgba8(_)),
    }
}

#[cfg(test)]
use kernels::execute_reduce_rgb_sparse_2x2;
use kernels::{execute_reduce_default, execute_reduce_rgb};
pub(super) use kernels::{
    execute_reduce_rgb_with_sparse_rows, reduce_f_thumbnail, reduce_i_thumbnail,
};

/// Execute Reduce using only the implementation selected by the wrapper mode.
pub(crate) fn execute_reduce(
    img: &DynamicImage,
    x_factor: u32,
    y_factor: u32,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if x_factor < 2 && y_factor < 2 {
        return Ok(img.clone());
    }
    let fx = x_factor.max(1);
    let fy = y_factor.max(1);
    let default = || execute_reduce_default(img, fx, fy, alpha_layout(img, explicit_mode));
    match (explicit_mode, img) {
        (Some("F"), DynamicImage::ImageRgba8(_)) => reduce_f_thumbnail(
            img,
            img.width().div_ceil(fx),
            img.height().div_ceil(fy),
            fx,
            fy,
        ),
        (Some("I"), DynamicImage::ImageRgba8(_)) => reduce_i_thumbnail(
            img,
            img.width().div_ceil(fx),
            img.height().div_ceil(fy),
            fx,
            fy,
        ),
        (Some("RGB"), DynamicImage::ImageRgb8(_)) => {
            if let Some(output) = execute_reduce_rgb(img, fx, fy, explicit_mode)? {
                Ok(output)
            } else {
                default()
            }
        }
        _ => default(),
    }
}

#[cfg(test)]
#[path = "reduce/tests.rs"]
mod tests;
