use super::boxed::raw_to_dynamic_owned;
use super::coefficients::pil_round;
#[cfg(not(feature = "parallel"))]
use super::coefficients::resize_u8_coefficients_fit_i32;
use super::float::pixel_at;
use super::horizontal_passes::{horizontal_pass_rows, horizontal_pass_rows_alpha};
#[cfg(not(feature = "parallel"))]
use super::integer_modes::{
    pil_resize_cmyk_i32, pil_resize_hsv_i32, pil_resize_la_i32, pil_resize_luma_i32,
    pil_resize_rgba_i32,
};
use super::vertical_passes::{vertical_pass_rows, vertical_pass_rows_alpha};
use super::*;
use crate::pipeline::ResampleFilter;
use crate::raster::{DynamicImage, Mode};

/// Preserve the original image's exact mode identity when its carrier matches.
pub(crate) fn pil_preserve_mode(original: &DynamicImage, result: DynamicImage) -> DynamicImage {
    let mode = original.mode();
    if result.mode() == mode {
        return result;
    }
    result
        .with_mode(mode)
        .unwrap_or_else(|unmodified| unmodified)
}

struct NearestResizeGeometry {
    source_width: u32,
    source_height: u32,
    destination_width: u32,
    destination_height: u32,
    channels: usize,
    scale_y: f64,
    pixel_count: usize,
}

#[inline(always)]
// Native byte images already have the exact sample layout required by
// Pillow's affine nearest path. Copy complete source pixels directly instead
// of expanding each one through `pixel_at`'s four-channel f64 representation
// and then pushing bytes individually. This keeps the coordinate calculation
// identical while making the common byte-image path bandwidth-bound.
fn pil_resize_nearest_native_bytes(
    img: &DynamicImage,
    orig_img: &DynamicImage,
    geometry: &NearestResizeGeometry,
    xintab: &[u32],
) -> Option<DynamicImage> {
    let source = img.as_bytes();
    let expected_source_len = (geometry.source_width as usize)
        .checked_mul(geometry.channels)?
        .checked_mul(geometry.source_height as usize)?;
    if source.len() != expected_source_len {
        return None;
    }
    let mut out_bytes = vec![0u8; geometry.pixel_count * geometry.channels];
    let source_stride = geometry.source_width as usize * geometry.channels;
    let destination_stride = geometry.destination_width as usize * geometry.channels;
    let mut yo = geometry.scale_y * 0.5;
    for dy in 0..geometry.destination_height as usize {
        let sy = if yo >= geometry.source_height as f64 {
            geometry.source_height - 1
        } else {
            yo as u32
        } as usize;
        let source_row = sy * source_stride;
        let destination_row = dy * destination_stride;
        if geometry.channels == 1 {
            let destination = &mut out_bytes[destination_row..destination_row + destination_stride];
            for (destination_pixel, &sx) in destination.iter_mut().zip(xintab) {
                *destination_pixel = source[source_row + sx as usize];
            }
        } else {
            for (dx, &sx) in xintab.iter().enumerate() {
                let source_start = source_row + sx as usize * geometry.channels;
                let destination_start = destination_row + dx * geometry.channels;
                out_bytes[destination_start..destination_start + geometry.channels]
                    .copy_from_slice(&source[source_start..source_start + geometry.channels]);
            }
        }
        yo += geometry.scale_y;
    }
    let result = raw_to_dynamic_owned(
        out_bytes,
        geometry.destination_width,
        geometry.destination_height,
        geometry.channels,
    );
    Some(pil_preserve_mode(orig_img, result))
}

#[inline(always)]
fn pil_resize_nearest(
    img: &DynamicImage,
    orig_img: &DynamicImage,
    sw: u32,
    sh: u32,
    dw: u32,
    dh: u32,
    channels: usize,
) -> DynamicImage {
    // PIL's NEAREST resize uses ImagingScaleAffine with cumulative f64 stepping.
    // From _imaging.c for NEAREST filter:
    //   a[0] = (double)(box[2] - box[0]) / xsize   (= sw / dw)
    //   a[2] = box[0]                                (= 0)
    // Then: xo = a[2] + a[0] * 0.5
    //       for each x: xin = (int)(xo); xo += a[0]
    let scale_x = sw as f64 / dw as f64;
    let scale_y = sh as f64 / dh as f64;
    let n = (dw * dh) as usize;
    let mut out_bytes: Vec<u8> = Vec::with_capacity(n * channels);
    // Precompute x-mapping table matching PIL's xintab approach
    let mut xintab: Vec<u32> = Vec::with_capacity(dw as usize);
    let mut xo = scale_x * 0.5;
    for _dx in 0..dw {
        let xi = xo as u32;
        xintab.push(if xi >= sw { sw - 1 } else { xi });
        xo += scale_x;
    }

    let geometry = NearestResizeGeometry {
        source_width: sw,
        source_height: sh,
        destination_width: dw,
        destination_height: dh,
        channels,
        scale_y,
        pixel_count: n,
    };
    if let Some(result) = pil_resize_nearest_native_bytes(img, orig_img, &geometry, &xintab) {
        return result;
    }

    // PIL also uses cumulative stepping for y: yo = a[4] * 0.5
    let mut yo = scale_y * 0.5;
    for _dy in 0..dh {
        let sy = if yo >= sh as f64 { sh - 1 } else { yo as u32 };
        for dx in 0..dw {
            let sx = xintab[dx as usize];
            let p = pixel_at(img, sx, sy);
            for c in 0..channels {
                // `pixel_at` exposes grayscale-alpha pixels as RGBA
                // (`[luma, luma, luma, alpha]`), while the resize buffer
                // keeps their native two-byte `[luma, alpha]` layout.
                // Select the alpha lane explicitly so PA/LA nearest
                // resize does not copy luma into the alpha sample.
                let rgba_channel = if channels == 2 && c == 1 { 3 } else { c };
                out_bytes.push(pil_round(p[rgba_channel]));
            }
        }
        yo += scale_y;
    }
    let result = raw_to_dynamic_owned(out_bytes, dw, dh, channels);
    pil_preserve_mode(orig_img, result)
}

#[inline(always)]
fn pil_resize_convolution(
    img: &DynamicImage,
    orig_img: &DynamicImage,
    sw: u32,
    sh: u32,
    dw: u32,
    dh: u32,
    channels: usize,
    needs_alpha: bool,
    h_coeffs: &super::coefficients::FilterCoeffs,
    v_coeffs: &super::coefficients::FilterCoeffs,
) -> DynamicImage {
    // Allocate intermediate image (sh rows × dw columns × channels)
    let mut intermediate = vec![0u8; (sh * dw) as usize * channels];

    // Horizontal pass: each source row is independent and can be written into
    // its own intermediate row.
    if needs_alpha {
        horizontal_pass_rows_alpha(
            img.as_bytes(),
            sw,
            sh,
            channels,
            h_coeffs,
            dw,
            &mut intermediate,
        );
    } else {
        horizontal_pass_rows(
            img.as_bytes(),
            sw,
            sh,
            channels,
            h_coeffs,
            dw,
            &mut intermediate,
        );
    }

    // Allocate output image
    let mut out_bytes = vec![0u8; (dw * dh) as usize * channels];

    // Vertical output rows are also independent once the intermediate image
    // exists, so they use the same disjoint-row write helper.
    if needs_alpha {
        vertical_pass_rows_alpha(
            &intermediate,
            sh,
            dw,
            dh,
            channels,
            v_coeffs,
            &mut out_bytes,
            0,
        );
    } else {
        vertical_pass_rows(
            &intermediate,
            sh,
            dw,
            dh,
            channels,
            v_coeffs,
            &mut out_bytes,
            0,
        );
    }

    // Build DynamicImage from bytes
    let result = raw_to_dynamic_owned(out_bytes, dw, dh, channels);

    pil_preserve_mode(orig_img, result)
}

/// PIL-compatible resize using two-pass separable interpolation.
///
/// PIL's approach:
/// 1. Precompute horizontal and vertical filter coefficients
/// 2. Horizontal pass: for each source row, compute each output column's
///    weighted sum, round to u8, and store in intermediate image
/// 3. Vertical pass: for each output column at each output row, compute
///    weighted sum from the intermediate rows, round to u8
///
/// For RGBA and LA modes, premultiplies alpha before resizing (matching PIL's
/// RGBa/La internal handling).
pub fn pil_resize(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    filter: ResampleFilter,
) -> DynamicImage {
    // Handle identity
    if (dst_w, dst_h) == (img.width(), img.height()) {
        return img.clone();
    }
    if let Some(resized) = crate::compute::pool_cpu::ops::resize::ops::native_typed_resize(
        img, dst_w, dst_h, filter, None,
    )
    {
        return resized;
    }
    // Handle empty
    if dst_w == 0 || dst_h == 0 || img.width() == 0 || img.height() == 0 {
        return DynamicImage::new_rgba8(dst_w, dst_h);
    }

    if let Some(resized) = crate::compute::pool_cpu::ops::resize::ops::native_luma16_resize(
        img, dst_w, dst_h, filter, None,
    )
    {
        return resized;
    }

    // Retain original image for final mode preservation
    let orig_img = img;

    // CMYK/F/I stored as RGBA8 but 4th channel is NOT alpha (K/float/int byte).
    // RGBa is already premultiplied storage, so PIL resamples its channels
    // directly rather than premultiplying them a second time.
    // PIL does NOT premultiply alpha for these modes.
    let mode = img.mode();
    let is_cmyk = mode == Mode::ImageCmyk8;
    // Nearest-neighbour resizing copies one source sample directly.  Pillow's
    // ImagingScaleAffine path does not premultiply LA/RGBA for that filter;
    // doing so here would round a constant (173, 127) or (17, 83, 149, 127)
    // down by one during the unnecessary premultiply/unpremultiply cycle.
    let needs_alpha = !matches!(filter, ResampleFilter::Nearest)
        && matches!(mode, Mode::ImageRgba8 | Mode::ImageLumaA8);
    let (sw, sh) = (img.width(), img.height());
    let (dw, dh) = (dst_w, dst_h);

    // Determine channel count
    let channels = match img.color() {
        crate::raster::ColorType::L8 => 1usize,
        crate::raster::ColorType::La8 => 2usize,
        crate::raster::ColorType::Rgb8 => 3usize,
        _ => 4usize,
    };

    // Pillow's ImagingResample and ImagingScaleAffine both preserve an
    // all-zero native byte image exactly: every weighted sample is zero and
    // the destination has no edge or alpha work to perform.  The generic
    // two-pass loops still build and walk both coefficient tables, which was
    // the first CPU divergence in the small ImageOps.contain/cover rows.
    // Keep this bounded to byte-backed layouts; typed F/I paths have their
    // own representation-preserving fast paths below.
    let native_byte_image = matches!(
        img,
        DynamicImage::ImageLuma8(_)
            | DynamicImage::ImageLumaA8(_)
            | DynamicImage::ImageRgb8(_)
            | DynamicImage::ImageRgba8(_)
    );
    if native_byte_image && img.as_bytes().iter().all(|&value| value == 0) {
        let output_len = (dw as usize)
            .checked_mul(dh as usize)
            .and_then(|pixels| pixels.checked_mul(channels))
            .unwrap_or(0);
        let result = raw_to_dynamic_owned(vec![0; output_len], dw, dh, channels);
        return pil_preserve_mode(orig_img, result);
    }

    // PIL's _resize C code uses ImagingTransform with AFFINE for NEAREST filter
    // (single-pixel sampling), NOT the two-pass pipeline. Box and all other filters
    // go through ImagingResample (two-pass convolution).
    // The AFFINE formula is:
    //   xin = a[0] * (x + 0.5) + a[2]   (a[2] = box[0] = 0)
    //   ix = (int)floor(xin)
    // A tiny epsilon is subtracted because the C code computes the scale factor
    // using float then double promotion, causing exact-integer boundaries to
    // nudge down by ~1e-15.
    if matches!(filter, ResampleFilter::Nearest) {
        return pil_resize_nearest(img, orig_img, sw, sh, dw, dh, channels);
    }

    // Precompute horizontal and vertical coefficients for two-pass pipeline
    let h_coeffs = precompute_coeffs(dw, sw, filter);
    let v_coeffs = precompute_coeffs(dh, sh, filter);

    #[cfg(not(feature = "parallel"))]
    if mode == Mode::ImageLuma8 && matches!(img, DynamicImage::ImageLuma8(_)) {
        if let Some(output) = pil_resize_luma_i32(img, dw, dh, &h_coeffs, &v_coeffs) {
            return pil_preserve_mode(orig_img, raw_to_dynamic_owned(output, dw, dh, 1));
        }
    }

    #[cfg(not(feature = "parallel"))]
    if mode == Mode::ImageLumaA8 && matches!(img, DynamicImage::ImageLumaA8(_)) {
        if let Some(output) = pil_resize_la_i32(img, dw, dh, &h_coeffs, &v_coeffs) {
            return pil_preserve_mode(orig_img, raw_to_dynamic_owned(output, dw, dh, 2));
        }
    }

    #[cfg(not(feature = "parallel"))]
    if needs_alpha && matches!(img, DynamicImage::ImageRgba8(_)) {
        if let Some(output) = pil_resize_rgba_i32(img, dw, dh, &h_coeffs, &v_coeffs) {
            return pil_preserve_mode(orig_img, raw_to_dynamic_owned(output, dw, dh, 4));
        }
    }

    #[cfg(not(feature = "parallel"))]
    if mode == Mode::ImageHsv8
        && matches!(img, DynamicImage::ImageHsv8(_))
        && matches!(filter, ResampleFilter::Bicubic)
        && resize_u8_coefficients_fit_i32(&h_coeffs)
        && resize_u8_coefficients_fit_i32(&v_coeffs)
    {
        if let Some(result) = pil_resize_hsv_i32(img, dw, dh, &h_coeffs, &v_coeffs) {
            return pil_preserve_mode(orig_img, result);
        }
    }

    #[cfg(not(feature = "parallel"))]
    if is_cmyk && matches!(img, DynamicImage::ImageCmyk8(_)) {
        if let Some(result) = pil_resize_cmyk_i32(img, dw, dh, &h_coeffs, &v_coeffs) {
            return pil_preserve_mode(orig_img, result);
        }
    }

    pil_resize_convolution(
        img,
        orig_img,
        sw,
        sh,
        dw,
        dh,
        channels,
        needs_alpha,
        &h_coeffs,
        &v_coeffs,
    )
}

#[cfg(test)]
mod typed_nearest_tests {
    use super::{pil_resize, pixel_at};
    use crate::pipeline::ResampleFilter;
    use crate::raster::{DynamicImage, ImageBuffer, Rgb, Rgba};

    fn typed_images() -> Vec<DynamicImage> {
        typed_integer_images()
            .into_iter()
            .chain(typed_float_images())
            .collect()
    }

    fn typed_integer_images() -> [DynamicImage; 7] {
        [
            DynamicImage::ImageI16(
                ImageBuffer::from_raw(3, 2, vec![0, 1, 32_768, 65_535, 257, 65_407])
                    .expect("L16 samples"),
            ),
            DynamicImage::ImageI16L(
                ImageBuffer::from_raw(3, 2, vec![0, 1, 32_768, 65_535, 257, 65_407])
                    .expect("I;16L samples"),
            ),
            DynamicImage::ImageI16B(
                ImageBuffer::from_raw(3, 2, vec![0, 1, 32_768, 65_535, 257, 65_407])
                    .expect("I;16B samples"),
            ),
            DynamicImage::ImageI16N(
                ImageBuffer::from_raw(3, 2, vec![0, 1, 32_768, 65_535, 257, 65_407])
                    .expect("I;16N samples"),
            ),
            DynamicImage::ImageLumaA16(
                ImageBuffer::from_raw(
                    3,
                    2,
                    vec![
                        0, 65_535, 1, 32_768, 32_768, 1, 65_535, 0, 257, 65_407, 129, 65_406,
                    ],
                )
                .expect("LA16 samples"),
            ),
            DynamicImage::ImageRgb16(
                ImageBuffer::from_raw(
                    3,
                    2,
                    vec![
                        0, 1, 65_535, 129, 32_768, 32_767, 257, 65_406, 65_535, 1, 0, 32_768,
                        65_407, 257, 32_768, 32_767, 128, 1,
                    ],
                )
                .expect("RGB16 samples"),
            ),
            DynamicImage::ImageRgba16(
                ImageBuffer::from_raw(
                    3,
                    2,
                    vec![
                        0, 1, 65_535, 32_768, 129, 32_768, 32_767, 65_535, 257, 65_406, 65_535, 0,
                        65_535, 1, 0, 257, 65_407, 257, 32_768, 32_767, 128, 1, 65_407, 65_535,
                    ],
                )
                .expect("RGBA16 samples"),
            ),
        ]
    }

    fn typed_float_images() -> [DynamicImage; 2] {
        [typed_rgb32f_image(), typed_rgba32f_image()]
    }

    fn typed_rgb32f_image() -> DynamicImage {
        DynamicImage::ImageRgb32F(
            ImageBuffer::<Rgb<f32>, Vec<f32>>::from_raw(
                3,
                2,
                vec![
                    0.0,
                    0.5,
                    1.0,
                    -1.0,
                    0.25,
                    2.0,
                    f32::NAN,
                    f32::INFINITY,
                    f32::NEG_INFINITY,
                    0.1,
                    0.9,
                    1.1,
                    -0.0,
                    0.001,
                    0.999,
                    0.75,
                    0.33,
                    0.66,
                ],
            )
            .expect("RGB32F samples"),
        )
    }

    fn typed_rgba32f_image() -> DynamicImage {
        DynamicImage::ImageRgba32F(
            ImageBuffer::<Rgba<f32>, Vec<f32>>::from_raw(
                3,
                2,
                vec![
                    0.0,
                    0.5,
                    1.0,
                    1.0,
                    -1.0,
                    0.25,
                    2.0,
                    0.5,
                    f32::NAN,
                    f32::INFINITY,
                    f32::NEG_INFINITY,
                    0.0,
                    0.1,
                    0.9,
                    1.1,
                    0.25,
                    -0.0,
                    0.001,
                    0.999,
                    0.75,
                    0.75,
                    0.33,
                    0.66,
                    0.5,
                ],
            )
            .expect("RGBA32F samples"),
        )
    }

    fn typed_native_bytes_per_pixel(image: &DynamicImage) -> usize {
        match image {
            DynamicImage::ImageI16(_)
            | DynamicImage::ImageI16L(_)
            | DynamicImage::ImageI16B(_)
            | DynamicImage::ImageI16N(_) => 2,
            DynamicImage::ImageLumaA16(_) => 4,
            DynamicImage::ImageRgb16(_) => 6,
            DynamicImage::ImageRgba16(_) => 8,
            DynamicImage::ImageRgb32F(_) => 12,
            DynamicImage::ImageRgba32F(_) => 16,
            _ => unreachable!("only typed high-depth inputs use this helper"),
        }
    }

    #[test]
    fn typed_nearest_resize_preserves_exact_mode_and_native_sample_width() {
        let destination_width = 4;
        let destination_height = 3;
        for image in typed_images() {
            let resized = pil_resize(
                &image,
                destination_width,
                destination_height,
                ResampleFilter::Nearest,
            );
            assert_eq!(resized.mode(), image.mode());
            let expected_bytes = destination_width as usize
                * destination_height as usize
                * typed_native_bytes_per_pixel(&image);
            assert_eq!(resized.as_bytes().len(), expected_bytes);
        }
    }

    #[test]
    fn typed_filtered_resize_preserves_exact_mode_and_native_sample_width() {
        let destination = (4, 3);
        let box_bounds = (0.25, 0.5, 2.75, 1.5);
        for image in typed_images() {
            let standard = pil_resize(
                &image,
                destination.0,
                destination.1,
                ResampleFilter::Bicubic,
            );
            let boxed = super::pil_resize_boxed(
                &image,
                destination.0,
                destination.1,
                box_bounds.0,
                box_bounds.1,
                box_bounds.2,
                box_bounds.3,
                ResampleFilter::Bicubic,
            );
            let expected_bytes = destination.0 as usize
                * destination.1 as usize
                * typed_native_bytes_per_pixel(&image);
            assert_eq!(standard.mode(), image.mode());
            assert_eq!(standard.as_bytes().len(), expected_bytes);
            assert_eq!(boxed.mode(), image.mode());
            assert_eq!(boxed.as_bytes().len(), expected_bytes);
        }
    }
}
