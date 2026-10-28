//! CPU resize orchestration with explicit Pillow mode routing.

use crate::error::PilError;
use crate::ops::pil_resize::{premultiply_alpha, unpremultiply_alpha};
use crate::pipeline::ResampleFilter;
use crate::raster::DynamicImage;

mod kernels;

pub(crate) use crate::ops::pil_resize::resample_kernel;
pub(super) use kernels::f::*;
pub(super) use kernels::i::*;

/// Route resize to a typed mode kernel only after the wrapper mode and native
/// sample storage agree. The wrapper owns Pillow mode; `DynamicImage` only
/// describes the pixel bytes that the selected operation receives.
pub(crate) fn execute_resize(
    image: &DynamicImage,
    width: u32,
    height: u32,
    filter: &ResampleFilter,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    match mode {
        Some("F") if matches!(image, DynamicImage::ImageRgba8(_)) => {
            kernels::f::resize_f(image, width, height, filter)
        }
        Some("I") if matches!(image, DynamicImage::ImageRgba8(_)) => {
            kernels::i::resize_i(image, width, height, filter)
        }
        Some("1") => Ok(kernels::one::resize_one(image, width, height, filter)),
        _ => Ok(kernels::default::resize(image, width, height, filter, mode)),
    }
}

/// Execute a resize over a floating source region, preserving Pillow's
/// operation ordering for tall images and premultiplied-alpha modes.
pub(crate) fn execute_resize_boxed(
    image: &DynamicImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: (f64, f64, f64, f64),
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let resize = |source: &DynamicImage, width, height, bounds: (f64, f64, f64, f64), mode| {
        if mode == Some("I") && !matches!(filter, ResampleFilter::Nearest) {
            kernels::i::resize_i_boxed(
                source, width, height, bounds.0, bounds.1, bounds.2, bounds.3, filter,
            )
        } else {
            // Nearest copies complete stored samples, including F's scalar
            // words. Keep F's logical mode so the typed F path can copy them;
            // other modes use the generic raw-sample path.
            let mode = if matches!(filter, ResampleFilter::Nearest) && mode != Some("F") {
                None
            } else {
                mode
            };
            Ok(kernels::default::resize_boxed(
                source, width, height, bounds, filter, mode,
            ))
        }
    };

    if u64::from(image.height()) <= u64::from(image.width()) * 100 || height >= image.height() {
        return resize(image, width, height, bounds, mode);
    }

    // Pillow changes pass order for very tall inputs. Keep alpha premultiplied
    // across both passes: an intermediate unpremultiply/re-premultiply rounds
    // the colors again and changes the final bytes.
    let alpha_mode = match (image, mode) {
        (DynamicImage::ImageRgba8(_), None | Some("RGBA")) => Some("RGBa"),
        (DynamicImage::ImageLumaA8(_), None | Some("LA")) => Some("La"),
        _ => None,
    }
    .filter(|_| !matches!(filter, ResampleFilter::Nearest));
    let source = alpha_mode.map(|_| premultiply_alpha(image));
    let working_mode = alpha_mode.or(mode);
    let vertical = resize(
        source.as_ref().unwrap_or(image),
        image.width(),
        height,
        (0.0, bounds.1, f64::from(image.width()), bounds.3),
        working_mode,
    )?;
    let result = resize(
        &vertical,
        width,
        height,
        (bounds.0, 0.0, bounds.2, f64::from(height)),
        working_mode,
    )?;
    Ok(if alpha_mode.is_some() {
        unpremultiply_alpha(&result)
    } else {
        result
    })
}

#[cfg(test)]
mod tests;
