use super::boxed::raw_to_dynamic_owned;
use super::coefficients::pil_round;
#[cfg(not(feature = "parallel"))]
use super::coefficients::resize_u8_coefficients_fit_i32;
use super::float::{pixel_at, typed_color_resize_bytes};
use super::horizontal_passes::{horizontal_pass_rows, horizontal_pass_rows_alpha};
#[cfg(not(feature = "parallel"))]
use super::integer_modes::{
    pil_resize_cmyk_i32, pil_resize_hsv_i32, pil_resize_la_i32, pil_resize_luma_i32,
    pil_resize_rgba_i32,
};
use super::luma16::pil_resize_luma16;
use super::vertical_passes::{vertical_pass_rows, vertical_pass_rows_alpha};
use super::*;
use crate::pipeline::ResampleFilter;
use crate::raster::DynamicImage;

/// Preserve the original image's color mode.
pub(crate) fn pil_preserve_mode(original: &DynamicImage, result: DynamicImage) -> DynamicImage {
    let orig_color = original.color();
    let res_color = result.color();
    if orig_color == res_color {
        return result;
    }
    match orig_color {
        crate::raster::ColorType::L8 => DynamicImage::ImageLuma8(result.to_luma8()),
        crate::raster::ColorType::La8 => DynamicImage::ImageLumaA8(result.to_luma_alpha8()),
        crate::raster::ColorType::Rgb8 => DynamicImage::ImageRgb8(result.to_rgb8()),
        crate::raster::ColorType::Rgba8 => DynamicImage::ImageRgba8(result.to_rgba8()),
        _ => result,
    }
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
    if !matches!(
        img,
        DynamicImage::ImageLuma8(_)
            | DynamicImage::ImageLumaA8(_)
            | DynamicImage::ImageRgb8(_)
            | DynamicImage::ImageRgba8(_)
    ) {
        return None;
    }
    let source = img.as_bytes();
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
    explicit_mode: Option<&str>,
) -> DynamicImage {
    // Handle identity
    if (dst_w, dst_h) == (img.width(), img.height()) {
        return img.clone();
    }
    // Handle empty
    if dst_w == 0 || dst_h == 0 || img.width() == 0 || img.height() == 0 {
        return DynamicImage::new_rgba8(dst_w, dst_h);
    }

    // Pillow keeps I;16* samples in the native 16-bit resize path.  The
    // generic pixel accessor below is byte-oriented and would otherwise
    // convert this mode to RGBA8 before preserving only its mode label.
    if let DynamicImage::ImageLuma16(luma) = img {
        return pil_resize_luma16(luma, dst_w, dst_h, filter, explicit_mode, None);
    }

    if let Some(byte_mode) = typed_color_resize_bytes(img) {
        return pil_resize(&byte_mode, dst_w, dst_h, filter, None);
    }

    // Retain original image for final mode preservation
    let orig_img = img;

    // CMYK/F/I stored as RGBA8 but 4th channel is NOT alpha (K/float/int byte).
    // RGBa is already premultiplied storage, so PIL resamples its channels
    // directly rather than premultiplying them a second time.
    // PIL does NOT premultiply alpha for these modes.
    let is_cmyk = explicit_mode == Some("CMYK");
    let is_fi = explicit_mode == Some("F") || explicit_mode == Some("I");
    // Nearest-neighbour resizing copies one source sample directly.  Pillow's
    // ImagingScaleAffine path does not premultiply LA/RGBA for that filter;
    // doing so here would round a constant (173, 127) or (17, 83, 149, 127)
    // down by one during the unnecessary premultiply/unpremultiply cycle.
    let needs_alpha = !matches!(filter, ResampleFilter::Nearest)
        && !is_cmyk
        && !is_fi
        && !matches!(explicit_mode, Some("RGBa" | "RGBX" | "La" | "PA"))
        && matches!(
            img.color(),
            crate::raster::ColorType::Rgba8 | crate::raster::ColorType::La8
        );
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
    if matches!(explicit_mode, None | Some("L")) && matches!(img, DynamicImage::ImageLuma8(_)) {
        if let Some(output) = pil_resize_luma_i32(img, dw, dh, &h_coeffs, &v_coeffs) {
            return pil_preserve_mode(orig_img, raw_to_dynamic_owned(output, dw, dh, 1));
        }
    }

    #[cfg(not(feature = "parallel"))]
    if matches!(explicit_mode, None | Some("LA")) && matches!(img, DynamicImage::ImageLumaA8(_)) {
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
    if explicit_mode == Some("HSV")
        && matches!(img, DynamicImage::ImageRgb8(_))
        && matches!(filter, ResampleFilter::Bicubic)
        && resize_u8_coefficients_fit_i32(&h_coeffs)
        && resize_u8_coefficients_fit_i32(&v_coeffs)
    {
        if let Some(result) = pil_resize_hsv_i32(img, dw, dh, &h_coeffs, &v_coeffs) {
            return pil_preserve_mode(orig_img, result);
        }
    }

    #[cfg(not(feature = "parallel"))]
    if is_cmyk && matches!(img, DynamicImage::ImageRgba8(_)) {
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

/// Resize RGBX bytes into an existing, full-width output window.
///
/// ImageOps.pad can use this when contain preserves the destination width and
/// adds only top/bottom borders. The exact Pillow coefficient tables and
/// horizontal/vertical kernels remain unchanged; only the temporary resized
/// output allocation is removed. RGBX byte three is data, so this route must
/// never enable alpha premultiplication.
pub(crate) fn pil_resize_rgbx_into_window(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    filter: ResampleFilter,
    output: &mut [u8],
) -> bool {
    let DynamicImage::ImageRgba8(_) = img else {
        return false;
    };
    if matches!(filter, ResampleFilter::Nearest)
        || dst_w == 0
        || dst_h == 0
        || img.width() == 0
        || img.height() == 0
    {
        return false;
    }
    let Ok(source_dims) = crate::checked_dims::CheckedDims::new(img.width(), img.height(), 4)
    else {
        return false;
    };
    let Ok(output_dims) = crate::checked_dims::CheckedDims::new(dst_w, dst_h, 4) else {
        return false;
    };
    if img.as_bytes().len() != source_dims.total_bytes()
        || output.len() != output_dims.total_bytes()
    {
        return false;
    }
    let Ok(source_height) = usize::try_from(img.height()) else {
        return false;
    };
    let Ok(output_width) = usize::try_from(dst_w) else {
        return false;
    };
    let Some(intermediate_len) = source_height
        .checked_mul(output_width)
        .and_then(|pixels| pixels.checked_mul(4))
    else {
        return false;
    };

    let horizontal = precompute_coeffs(dst_w, img.width(), filter);
    let vertical = precompute_coeffs(dst_h, img.height(), filter);
    let mut intermediate = vec![0u8; intermediate_len];
    horizontal_pass_rows(
        img.as_bytes(),
        img.width(),
        img.height(),
        4,
        &horizontal,
        dst_w,
        &mut intermediate,
    );
    vertical_pass_rows(
        &intermediate,
        img.height(),
        dst_w,
        dst_h,
        4,
        &vertical,
        output,
        0,
    );
    true
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

    fn typed_integer_images() -> [DynamicImage; 4] {
        [
            DynamicImage::ImageLuma16(
                ImageBuffer::from_raw(3, 2, vec![0, 1, 32_768, 65_535, 257, 65_407])
                    .expect("L16 samples"),
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

    fn typed_public_byte_mode(image: &DynamicImage) -> DynamicImage {
        match image {
            DynamicImage::ImageLumaA16(_) => DynamicImage::ImageLumaA8(image.to_luma_alpha8()),
            DynamicImage::ImageRgb16(_) | DynamicImage::ImageRgb32F(_) => {
                DynamicImage::ImageRgb8(image.to_rgb8())
            }
            DynamicImage::ImageRgba16(_) | DynamicImage::ImageRgba32F(_) => {
                DynamicImage::ImageRgba8(image.to_rgba8())
            }
            _ => unreachable!("only multi-channel typed inputs use this conversion"),
        }
    }

    #[test]
    fn typed_nearest_resize_keeps_the_logical_byte_mode_layout() {
        let destination_width = 4;
        let destination_height = 3;
        for image in typed_images() {
            let rgba = image.to_rgba8();
            for y in 0..image.height() {
                for x in 0..image.width() {
                    let expected = rgba.get_pixel(x, y).0.map(f64::from);
                    assert_eq!(pixel_at(&image, x, y), expected);
                }
            }

            // L16 has its own native nearest path. The other typed layouts
            // exercise pixel_at from the generic affine-nearest fallback.
            if matches!(image, DynamicImage::ImageLuma16(_)) {
                continue;
            }
            let byte_mode = typed_public_byte_mode(&image);
            let expected = pil_resize(
                &byte_mode,
                destination_width,
                destination_height,
                ResampleFilter::Nearest,
                None,
            );
            let resized = pil_resize(
                &image,
                destination_width,
                destination_height,
                ResampleFilter::Nearest,
                None,
            );
            assert_eq!(resized.color(), expected.color());
            assert_eq!(resized.as_bytes(), expected.as_bytes());
        }
    }

    #[test]
    fn typed_filtered_resize_uses_the_logical_byte_mode_layout() {
        let destination = (4, 3);
        let box_bounds = (0.25, 0.5, 2.75, 1.5);
        for image in typed_images().into_iter().skip(1) {
            assert_typed_filtered_resize_layout(&image, destination, box_bounds);
        }
    }

    fn assert_typed_filtered_resize_layout(
        image: &DynamicImage,
        destination: (u32, u32),
        box_bounds: (f64, f64, f64, f64),
    ) {
        let byte_mode = typed_public_byte_mode(image);
        assert_typed_standard_resize_layout(image, &byte_mode, destination);
        assert_typed_boxed_resize_layout(image, &byte_mode, destination, box_bounds);
    }

    fn assert_typed_standard_resize_layout(
        image: &DynamicImage,
        byte_mode: &DynamicImage,
        destination: (u32, u32),
    ) {
        let normal_expected = pil_resize(
            byte_mode,
            destination.0,
            destination.1,
            ResampleFilter::Bicubic,
            None,
        );
        let normal_actual = pil_resize(
            image,
            destination.0,
            destination.1,
            ResampleFilter::Bicubic,
            None,
        );
        assert_eq!(
            normal_actual.color(),
            normal_expected.color(),
            "standard resize layout for {:?}",
            image.color()
        );
        assert_eq!(
            normal_actual.as_bytes(),
            normal_expected.as_bytes(),
            "standard resize samples for {:?}",
            image.color()
        );
    }

    fn assert_typed_boxed_resize_layout(
        image: &DynamicImage,
        byte_mode: &DynamicImage,
        destination: (u32, u32),
        box_bounds: (f64, f64, f64, f64),
    ) {
        let boxed_expected = super::pil_resize_boxed(
            byte_mode,
            destination.0,
            destination.1,
            box_bounds.0,
            box_bounds.1,
            box_bounds.2,
            box_bounds.3,
            ResampleFilter::Bicubic,
            None,
        );
        let boxed_actual = super::pil_resize_boxed(
            image,
            destination.0,
            destination.1,
            box_bounds.0,
            box_bounds.1,
            box_bounds.2,
            box_bounds.3,
            ResampleFilter::Bicubic,
            None,
        );
        assert_eq!(
            boxed_actual.color(),
            boxed_expected.color(),
            "boxed resize layout for {:?}",
            image.color()
        );
        assert_eq!(
            boxed_actual.as_bytes(),
            boxed_expected.as_bytes(),
            "boxed resize samples for {:?}",
            image.color()
        );
    }
}
