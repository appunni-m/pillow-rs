//! CPU rotation family and interpolation kernels.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::image_utils::raw_bytes_to_image;
use crate::ops::pil_resize::{premultiply_alpha, unpremultiply_alpha};
use crate::pipeline::ResampleFilter;
use crate::raster::{DynamicImage, GenericImageView};

#[path = "affine.rs"]
mod affine;
#[path = "cubic.rs"]
mod cubic;
#[path = "scalar.rs"]
mod scalar;

pub(super) use affine::*;
pub(crate) use cubic::{rotate_cubic_horizontal_i32, rotate_cubic_vertical_f64};

/// Execute a Rotate operation.
/// Fast-path for 90-degree multiples; otherwise uses arbitrary rotation.
/// Rotate 90 or 270 degrees without expanding (clip to original size).
/// Matches PIL's behavior: rotate and center the result in the original canvas,
/// filling exposed areas with the fill color.
pub(super) fn rotate_90_non_expand(
    img: &DynamicImage,
    clockwise_270: bool,
    fill: Option<(u8, u8, u8, u8)>,
) -> Result<DynamicImage, PilError> {
    let channels = img.color().channel_count() as usize;
    let (w, h) = img.dimensions();
    let raw = img.as_bytes();
    let fill_color = fill.unwrap_or((0, 0, 0, 0));

    // Initialize output with fill color
    let fill_pixel: Vec<u8> = match channels {
        1 => vec![fill_color.0],
        2 => vec![fill_color.0, fill_color.3],
        3 => vec![fill_color.0, fill_color.1, fill_color.2],
        _ => vec![fill_color.0, fill_color.1, fill_color.2, fill_color.3],
    };
    let dims = CheckedDims::new(w, h, channels as u8)?;
    let mut out = fill_pixel.repeat(dims.total_pixels());

    // Pillow centers the expanded 90-degree result with independent edge
    // rounding. Rust integer division truncates negative halves toward zero,
    // so calculate the floor/ceil offsets explicitly for odd dimension gaps.
    let width_gap = w as f64 - h as f64;
    let height_gap = h as f64 - w as f64;
    let dx_off_90 = (width_gap / 2.0).floor() as i32;
    let dy_off_90 = (height_gap / 2.0).ceil() as i32;
    let dx_off_270 = (width_gap / 2.0).ceil() as i32;
    let dy_off_270 = (height_gap / 2.0).floor() as i32;

    for dy in 0..h {
        for dx in 0..w {
            let (sx, sy) = if clockwise_270 {
                // 270° CCW = 90° CW: expand_true uses input(dy, h-1-dx)
                // dx_true = dx - dx_off, dy_true = dy - dy_off
                // sx = dy_true = dy - dy_off, sy = h - 1 - dx_true
                //     = h - 1 - dx + dx_off
                (
                    dy as i32 - dy_off_270,
                    h as i32 - 1 - dx as i32 + dx_off_270,
                )
            } else {
                // 90° CCW: expand_true uses input(w-1-dy, dx)
                // dx_true = dx - dx_off, dy_true = dy - dy_off
                // sx = w - 1 - dy_true = w - 1 - dy + dy_off, sy = dx_true = dx - dx_off
                (w as i32 - 1 - dy as i32 + dy_off_90, dx as i32 - dx_off_90)
            };
            if sx >= 0 && sx < w as i32 && sy >= 0 && sy < h as i32 {
                let in_idx = (sy as u32 * w + sx as u32) as usize * channels;
                let out_idx = (dy * w + dx) as usize * channels;
                out[out_idx..out_idx + channels].copy_from_slice(&raw[in_idx..in_idx + channels]);
            }
        }
    }

    // Create output dynamic image
    raw_bytes_to_image(w, h, out, channels)
}

pub(crate) fn execute_rotate(
    img: &DynamicImage,
    angle: f64,
    expand: bool,
    fill: Option<(u8, u8, u8, u8)>,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
    filter: ResampleFilter,
    requested_nearest: bool,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let has_custom_transform = center.is_some() || translate.is_some();
    let nearest = requested_nearest || explicit_mode == Some("P") || explicit_mode == Some("1");
    let filter = if nearest {
        ResampleFilter::Nearest
    } else {
        filter
    };
    // LA/La/PA use a public (gray, gray, gray, alpha) fill tuple. Normalize
    // it to native two-band storage; PA and La bypass the alpha round trip.
    let fill = if matches!(explicit_mode, Some("PA" | "LA" | "La")) {
        fill.map(|(index, _, _, alpha)| (index, alpha, 0, alpha))
    } else {
        fill
    };
    // Fast path: exact 90-degree multiples
    // PIL rotates counterclockwise; image crate rotates clockwise.
    // PIL 90° CCW = image crate 270° CW, PIL 270° CCW = image crate 90° CW.
    // Unexpanded non-square 90/270 rotations still use the affine sampler,
    // including its filtered alpha round trip.
    let normalized_angle = angle.rem_euclid(360.0);
    let result = if !has_custom_transform && normalized_angle.abs() <= f64::EPSILON {
        // Pillow's public rotate() returns an exact copy at angle 0 (and
        // every multiple of 360) before considering the requested filter.
        // Keep this fast path ahead of filtered affine sampling so LA/RGBA
        // bytes and alpha channels are not rounded needlessly.
        img.clone()
    } else if !has_custom_transform
        && normalized_angle == 90.0
        && (expand || img.width() == img.height())
    {
        if expand {
            img.rotate270() // 270° CW = 90° CCW (PIL)
        } else {
            rotate_90_non_expand(img, false, fill)?
        }
    } else if !has_custom_transform && normalized_angle == 180.0 {
        img.rotate180()
    } else if !has_custom_transform
        && normalized_angle == 270.0
        && (expand || img.width() == img.height())
    {
        if expand {
            img.rotate90() // 90° CW = 270° CCW (PIL)
        } else {
            rotate_90_non_expand(img, true, fill)?
        }
    } else {
        // Pillow routes non-nearest LA/RGBA transforms through their
        // premultiplied modes (La/RGBa), then converts the interpolated
        // samples back. RGBa is already premultiplied and must remain a direct
        // native-channel path, just as in pil_resize.
        let needs_alpha_roundtrip = !nearest
            && !matches!(
                explicit_mode,
                Some("PA")
                    | Some("RGBa")
                    | Some("La")
                    | Some("RGBX")
                    | Some("CMYK")
                    | Some("F")
                    | Some("I")
            )
            && matches!(
                img.color(),
                crate::raster::ColorType::La8 | crate::raster::ColorType::Rgba8
            );
        let work = if needs_alpha_roundtrip {
            premultiply_alpha(img)
        } else {
            img.clone()
        };
        let rotated = rotate_arbitrary_generic(
            &work,
            normalized_angle,
            expand,
            fill,
            filter,
            nearest,
            center,
            translate,
            explicit_mode,
        )?;
        if needs_alpha_roundtrip {
            unpremultiply_alpha(&rotated)
        } else {
            rotated
        }
    };
    Ok(preserve_mode(img, result))
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
