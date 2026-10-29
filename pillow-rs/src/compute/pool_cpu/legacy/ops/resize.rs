//! CPU resize family: public operation entry points and kernel modules.

use crate::error::PilError;
use crate::image::preserve_mode;
use crate::ops::pil_resize::{
    pil_resize, pil_resize_boxed, premultiply_alpha, unpremultiply_alpha,
};
use crate::pipeline::ResampleFilter;
use crate::raster::DynamicImage;

#[path = "resize/filter.rs"]
mod filter;
#[path = "resize/float.rs"]
mod float;
#[path = "resize/integer.rs"]
mod integer;

pub(crate) use filter::resample_kernel;
pub(super) use float::*;
pub(super) use integer::*;

/// Execute a Resize operation.
/// F-mode uses float interpolation; I-mode uses int32 interpolation;
/// all other modes use the standard PIL-compatible two-pass resize.
pub(crate) fn execute_resize(
    img: &DynamicImage,
    w: u32,
    h: u32,
    filter: &ResampleFilter,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // Only use F/I resize paths when the image is already stored as Rgba8
    // (4 bytes per pixel), meaning it has been converted to F/I mode already.
    // If the image is still RGB (3 bytes per pixel), use normal resize regardless
    // of explicit_mode, because the F/I convert hasn't happened yet in the pipeline.
    if explicit_mode == Some("F") && matches!(img, DynamicImage::ImageRgba8(_)) {
        return resize_f(img, w, h, filter);
    }
    if explicit_mode == Some("I") && matches!(img, DynamicImage::ImageRgba8(_)) {
        return resize_i(img, w, h, filter);
    }
    // Mode "1": convert to L, resize, then convert back to "1" by thresholding at 128.
    // PIL's C extension handles mode "1" internally with bit-unpacking, but our
    // pil_resize works on Luma8 which has equivalent data. The two-pass BOX filter
    // (NEAREST) produces averages; the conversion back to "1" thresholds them.
    if explicit_mode == Some("1") {
        // Image is already Luma8 with {0,255}. Resize via pil_resize (which uses
        // the BOX filter for NEAREST, matching PIL's behavior for mode "1").
        let result = pil_resize(img, w, h, *filter, explicit_mode);
        // After resize, threshold back to binary {0, 255}: pixel >= 128 => 255 else 0
        let gray = result.to_luma8();
        let (rw, rh) = gray.dimensions();
        let mut out = crate::raster::GrayImage::new(rw, rh);
        for (op, ip) in out.pixels_mut().zip(gray.pixels()) {
            op[0] = if ip[0] >= 128 { 255 } else { 0 };
        }
        return Ok(preserve_mode(img, DynamicImage::ImageLuma8(out)));
    }
    let result = pil_resize(img, w, h, *filter, explicit_mode);
    Ok(preserve_mode(img, result))
}

/// Resize a floating source region with Pillow's typed and pass-order rules.
pub(crate) fn execute_resize_boxed(
    img: &DynamicImage,
    w: u32,
    h: u32,
    filter: ResampleFilter,
    bounds: (f64, f64, f64, f64),
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let resize = |image: &DynamicImage, width, height, bounds: (f64, f64, f64, f64), mode| {
        if mode == Some("I") && !matches!(filter, ResampleFilter::Nearest) {
            resize_i_boxed(
                image, width, height, bounds.0, bounds.1, bounds.2, bounds.3, filter,
            )
        } else {
            // Nearest copies complete stored samples, including F's scalar
            // words. Keep F's logical mode so the typed F path can copy those
            // words directly; other modes use the generic raw-sample path.
            let mode = if matches!(filter, ResampleFilter::Nearest) && mode != Some("F") {
                None
            } else {
                mode
            };
            Ok(pil_resize_boxed(
                image, width, height, bounds.0, bounds.1, bounds.2, bounds.3, filter, mode,
            ))
        }
    };
    if u64::from(img.height()) <= u64::from(img.width()) * 100 || h >= img.height() {
        return resize(img, w, h, bounds, mode);
    }
    // Pillow changes pass order for very tall inputs. Keep alpha premultiplied
    // across both passes: an intermediate unpremultiply/re-premultiply rounds
    // the colors again and changes the final bytes.
    let alpha_mode = match (img, mode) {
        (DynamicImage::ImageRgba8(_), None | Some("RGBA")) => Some("RGBa"),
        (DynamicImage::ImageLumaA8(_), None | Some("LA")) => Some("La"),
        _ => None,
    }
    .filter(|_| !matches!(filter, ResampleFilter::Nearest));
    let source = alpha_mode.map(|_| premultiply_alpha(img));
    let working_mode = alpha_mode.or(mode);
    let vertical = resize(
        source.as_ref().unwrap_or(img),
        img.width(),
        h,
        (0.0, bounds.1, f64::from(img.width()), bounds.3),
        working_mode,
    )?;
    let result = resize(
        &vertical,
        w,
        h,
        (bounds.0, 0.0, bounds.2, f64::from(h)),
        working_mode,
    )?;
    Ok(if alpha_mode.is_some() {
        unpremultiply_alpha(&result)
    } else {
        result
    })
}

#[cfg(test)]
#[path = "resize/tests.rs"]
mod tests;
