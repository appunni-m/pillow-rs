use super::*;
use crate::pipeline::ResampleFilter;
use crate::raster::{DynamicImage, ImageBuffer, Luma};
use std::sync::Arc;

/// Resize a native 16-bit grayscale image through PIL's nearest-neighbor path.
///
/// `I;16*` images carry unsigned 16-bit samples.  Keeping them in the native
/// buffer is required because converting through `to_rgba8()` changes both the
/// sample width and the bytes returned by `tobytes()`.
pub(super) fn pil_resize_luma16_nearest(
    img: &ImageBuffer<Luma<u16>, Vec<u16>>,
    dst_w: u32,
    dst_h: u32,
    bounds: Option<(f64, f64, f64, f64)>,
) -> DynamicImage {
    let sw = img.width();
    let sh = img.height();
    let mut result = ImageBuffer::new(dst_w, dst_h);
    if sw == 0 || sh == 0 {
        return DynamicImage::ImageLuma16(result);
    }
    let (left, top, scale_x, scale_y) = match bounds {
        Some((left, top, right, bottom)) => (
            f64::from(left as f32),
            f64::from(top as f32),
            f64::from(right as f32 - left as f32) / f64::from(dst_w),
            f64::from(bottom as f32 - top as f32) / f64::from(dst_h),
        ),
        None => (0.0, 0.0, sw as f64 / dst_w as f64, sh as f64 / dst_h as f64),
    };

    let mut xintab = Vec::with_capacity(dst_w as usize);
    let mut xo = left + scale_x * 0.5;
    for _ in 0..dst_w {
        let xi = xo as u32;
        xintab.push(if xi >= sw { sw - 1 } else { xi });
        xo += scale_x;
    }

    let mut yo = top + scale_y * 0.5;
    for dy in 0..dst_h {
        let sy = if yo >= sh as f64 { sh - 1 } else { yo as u32 };
        for dx in 0..dst_w {
            let sx = xintab[dx as usize];
            result.put_pixel(dx, dy, *img.get_pixel(sx, sy));
        }
        yo += scale_y;
    }

    DynamicImage::ImageLuma16(result)
}

/// Pillow's `Resample.c` uses a separate byte-oriented implementation for
/// `I;16*` images.  The source and destination images are native `u16`
/// buffers, but the C implementation reads and writes their two bytes using
/// the mode-dependent `bigendian` flag.  Keep that ABI-visible behavior at the
/// core boundary instead of feeding a 16-bit buffer into the u8 resampler.
pub(crate) fn luma16_resample_big_endian(mode: Option<&str>) -> bool {
    match mode {
        // Pillow's Resample.c deliberately selects the big-endian branch for
        // I;16N.  On little-endian hosts this is observable as the historical
        // native-mode byte ordering of the convolution path.
        Some("I;16N") => true,
        Some("I;16B") => true,
        _ => false,
    }
}

pub(crate) fn luma16_resample_read(sample: u16, big_endian: bool) -> u16 {
    let native_bytes = sample.to_ne_bytes();
    if big_endian {
        u16::from_be_bytes(native_bytes)
    } else {
        u16::from_le_bytes(native_bytes)
    }
}

fn clip_u8(value: i64) -> u8 {
    value.clamp(0, 255) as u8
}

pub(crate) fn luma16_resample_write(value: f64, big_endian: bool) -> u16 {
    let rounded = round_up(value) as i64;
    // Resample.c writes each byte through CLIP8 rather than clipping the
    // complete 16-bit result.  Keeping the byte-level operation matters for
    // negative-filter overshoot and for the exact I;16* output bytes.
    let low = clip_u8(rounded % 256);
    let high = clip_u8(rounded >> 8);
    let bytes = if big_endian { [high, low] } else { [low, high] };
    u16::from_ne_bytes(bytes)
}

fn horizontal_pass_luma16(
    src_row: &[u16],
    coeffs: &FilterCoeffsF64,
    out_w: u32,
    big_endian: bool,
    intermediate_row: &mut [u16],
) {
    for ox in 0..out_w as usize {
        let x0 = coeffs.xmin[ox];
        let cnt = coeffs.count[ox];
        if cnt == 0 {
            continue;
        }
        let mut acc = 0.0f64;
        for (cix, &weight) in coeffs.weights[ox].iter().enumerate() {
            let sx = (x0 + cix as i64) as usize;
            acc += luma16_resample_read(src_row[sx], big_endian) as f64 * weight;
        }
        intermediate_row[ox] = luma16_resample_write(acc, big_endian);
    }
}

fn vertical_pass_luma16(
    intermediate: &[u16],
    out_x: u32,
    out_w: u32,
    coeffs: &FilterCoeffsF64,
    out_y: usize,
    big_endian: bool,
) -> u16 {
    let y0 = coeffs.xmin[out_y];
    let cnt = coeffs.count[out_y];
    if cnt == 0 {
        return 0;
    }
    let mut acc = 0.0f64;
    for (cix, &weight) in coeffs.weights[out_y].iter().enumerate() {
        let sy = (y0 + cix as i64) as usize;
        let source = intermediate[sy * out_w as usize + out_x as usize];
        acc += luma16_resample_read(source, big_endian) as f64 * weight;
    }
    luma16_resample_write(acc, big_endian)
}

/// Resize `I;16*` through Pillow's native 16-bit two-pass resampler.
pub(super) fn pil_resize_luma16(
    img: &ImageBuffer<Luma<u16>, Vec<u16>>,
    dst_w: u32,
    dst_h: u32,
    filter: ResampleFilter,
    explicit_mode: Option<&str>,
    bounds: Option<(f64, f64, f64, f64)>,
) -> DynamicImage {
    if matches!(filter, ResampleFilter::Nearest) {
        return pil_resize_luma16_nearest(img, dst_w, dst_h, bounds);
    }

    let (kernel, support) = filter_from_resample(filter);
    let (h_coeffs, v_coeffs) = match bounds {
        Some((left, top, right, bottom)) => (
            Arc::new(precompute_coeffs_f64_boxed(
                dst_w,
                img.width(),
                left,
                right,
                filter,
            )),
            Arc::new(precompute_coeffs_f64_boxed(
                dst_h,
                img.height(),
                top,
                bottom,
                filter,
            )),
        ),
        None => (
            precompute_coeffs_f64(dst_w, img.width(), kernel, support),
            precompute_coeffs_f64(dst_h, img.height(), kernel, support),
        ),
    };
    let big_endian = luma16_resample_big_endian(explicit_mode);

    let mut intermediate = ImageBuffer::<Luma<u16>, Vec<u16>>::new(img.height(), dst_w);
    {
        let intermediate_data: &mut [u16] = &mut *intermediate;
        for sy in 0..img.height() {
            let src_start = (sy * img.width()) as usize;
            let src_end = src_start + img.width() as usize;
            let intermediate_start = (sy * dst_w) as usize;
            let intermediate_end = intermediate_start + dst_w as usize;
            horizontal_pass_luma16(
                &img.as_raw()[src_start..src_end],
                &h_coeffs,
                dst_w,
                big_endian,
                &mut intermediate_data[intermediate_start..intermediate_end],
            );
        }
    }

    let mut output = ImageBuffer::<Luma<u16>, Vec<u16>>::new(dst_w, dst_h);
    for dy in 0..dst_h {
        for dx in 0..dst_w {
            let value = vertical_pass_luma16(
                intermediate.as_raw(),
                dx,
                dst_w,
                &v_coeffs,
                dy as usize,
                big_endian,
            );
            output.put_pixel(dx, dy, Luma([value]));
        }
    }

    DynamicImage::ImageLuma16(output)
}
