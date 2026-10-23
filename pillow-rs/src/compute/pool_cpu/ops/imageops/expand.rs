use super::*;

#[inline]
fn expand_rgba_row(
    row: &mut [u8],
    y: usize,
    source: &[u8],
    source_stride: usize,
    offset_x: usize,
    offset_y: usize,
    copy_width: usize,
    copy_height: usize,
    fill: [u8; 4],
) {
    for pixel in row.chunks_exact_mut(4) {
        pixel.copy_from_slice(&fill);
    }

    if y < offset_y || y >= offset_y.saturating_add(copy_height) {
        return;
    }
    let source_start = (y - offset_y).saturating_mul(source_stride);
    let copy_bytes = copy_width.saturating_mul(4);
    if copy_bytes == 0 || offset_x >= row.len() || source_start >= source.len() {
        return;
    }
    let output_start = offset_x.saturating_mul(4);
    if output_start >= row.len() {
        return;
    }
    let output_end = output_start.saturating_add(copy_bytes).min(row.len());
    let available = output_end - output_start;
    let source_end = source_start.saturating_add(available).min(source.len());
    let available = source_end - source_start;
    row[output_start..output_start + available].copy_from_slice(&source[source_start..source_end]);
}
fn native_expand_channels(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    match (mode, img) {
        (None, DynamicImage::ImageLuma8(_)) | (Some("L"), DynamicImage::ImageLuma8(_)) => Some(1),
        (None, DynamicImage::ImageLumaA8(_)) | (Some("LA"), DynamicImage::ImageLumaA8(_)) => {
            Some(2)
        }
        (None, DynamicImage::ImageRgb8(_)) | (Some("RGB" | "HSV"), DynamicImage::ImageRgb8(_)) => {
            Some(3)
        }
        (None, DynamicImage::ImageRgba8(_)) | (Some("RGBA"), DynamicImage::ImageRgba8(_)) => {
            Some(4)
        }
        // RGBX shares four-byte storage with RGBA, but the fourth sample is
        // opaque padding. Expand copies it unchanged and uses fill.3 for the
        // new border, so this exact logical/storage pair is safe.
        (Some("RGBX"), DynamicImage::ImageRgba8(_)) => Some(4),
        // CMYK uses the same four-byte carrier, but its fourth byte is K.
        // Expand copies all four samples unchanged and uses the fill tuple in
        // C/M/Y/K order, so admit only this exact logical/storage pair.
        (Some("CMYK"), DynamicImage::ImageRgba8(_)) => Some(4),
        _ => None,
    }
}

/// Copy one of the ordinary byte layouts into its native output channel count.
/// The fill tuple's LA alpha is byte three; the source LA pixel's alpha is byte
/// one. L/LA/RGB/HSV/RGBA stay in their physical format from input through
/// output.
fn expand_native_bytes(
    img: &DynamicImage,
    border: u32,
    fill: (u8, u8, u8, u8),
    channels: usize,
    new_w: u32,
    new_h: u32,
) -> Result<Option<DynamicImage>, PilError> {
    if new_w == 0 || new_h == 0 {
        return Ok(None);
    }
    let source_dims = CheckedDims::new_allow_empty(img.width(), img.height(), channels as u8)?;
    let source = img.as_bytes();
    if source.len() != source_dims.total_bytes() {
        return Ok(None);
    }
    let output_dims = CheckedDims::new(new_w, new_h, channels as u8)?;
    if border == 0 {
        return Ok(Some(img.clone()));
    }

    let fill_sample = match channels {
        1 => [fill.0, 0, 0, 0],
        2 => [fill.0, fill.3, 0, 0],
        3 => [fill.0, fill.1, fill.2, 0],
        4 => [fill.0, fill.1, fill.2, fill.3],
        _ => return Ok(None),
    };
    let fill_pixel = &fill_sample[..channels];
    let fill_row = fill_pixel.repeat(new_w as usize);
    let fill_sides = fill_pixel.repeat(border as usize);
    let source_stride = source_dims.row_stride();
    let source_height = img.height() as usize;
    let offset_y = border as usize;

    // Build each row exactly once. Filling the whole canvas and overwriting
    // its center adds a second full-frame memory pass when the border is thin.
    let mut output = Vec::with_capacity(output_dims.total_bytes());
    for y in 0..new_h as usize {
        if y < offset_y || y >= offset_y + source_height {
            output.extend_from_slice(&fill_row);
        } else {
            output.extend_from_slice(&fill_sides);
            let source_y = y - offset_y;
            let source_start = source_y * source_stride;
            output.extend_from_slice(&source[source_start..source_start + source_stride]);
            output.extend_from_slice(&fill_sides);
        }
    }

    crate::image_utils::raw_bytes_to_image(new_w, new_h, output, channels).map(Some)
}

/// Expand: add a border of `border` pixels with `fill` color around the image.
/// The fill is a 4-tuple (r,g,b,a). Indexed `P`/`PA` inputs retain their raw
/// sample layout; the tuple's first byte is the `P` index and the first and
/// fourth bytes are the `PA` index/alpha pair.
pub fn op_expand(
    img: &DynamicImage,
    border: u32,
    fill: (u8, u8, u8, u8),
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (new_width, new_height) = expanded_dimensions(img, border)?;
    if let Some(expanded) = expand_indexed_mode(img, border, fill, mode, new_width, new_height) {
        return Ok(expanded);
    }
    if let Some(channels) = native_expand_channels(img, mode) {
        if let Some(expanded) =
            expand_native_bytes(img, border, fill, channels, new_width, new_height)?
        {
            return Ok(expanded);
        }
    }
    expand_rgba_compat(img, border, fill, new_width, new_height)
}

fn expanded_dimensions(img: &DynamicImage, border: u32) -> Result<(u32, u32), PilError> {
    let border_twice = border
        .checked_mul(2)
        .ok_or_else(|| PilError::DimensionError("expand border overflows dimensions".into()))?;
    let width = img
        .width()
        .checked_add(border_twice)
        .ok_or_else(|| PilError::DimensionError("expanded image width overflows".into()))?;
    let height = img
        .height()
        .checked_add(border_twice)
        .ok_or_else(|| PilError::DimensionError("expanded image height overflows".into()))?;
    Ok((width, height))
}

fn expand_indexed_mode(
    img: &DynamicImage,
    border: u32,
    fill: (u8, u8, u8, u8),
    mode: Option<&str>,
    new_width: u32,
    new_height: u32,
) -> Option<DynamicImage> {
    match mode {
        Some("P") => {
            let source = img.to_luma8();
            let mut expanded = crate::raster::GrayImage::from_pixel(
                new_width,
                new_height,
                crate::raster::Luma([fill.0]),
            );
            for y in 0..img.height() {
                for x in 0..img.width() {
                    expanded.put_pixel(x + border, y + border, *source.get_pixel(x, y));
                }
            }
            Some(DynamicImage::ImageLuma8(expanded))
        }
        Some("PA") => {
            let source = img.to_luma_alpha8();
            let mut expanded = crate::raster::GrayAlphaImage::from_pixel(
                new_width,
                new_height,
                crate::raster::LumaA([fill.0, fill.3]),
            );
            for y in 0..img.height() {
                for x in 0..img.width() {
                    expanded.put_pixel(x + border, y + border, *source.get_pixel(x, y));
                }
            }
            Some(DynamicImage::ImageLumaA8(expanded))
        }
        _ => None,
    }
}

fn expand_rgba_compat(
    img: &DynamicImage,
    border: u32,
    fill: (u8, u8, u8, u8),
    new_width: u32,
    new_height: u32,
) -> Result<DynamicImage, PilError> {
    let source = img.to_rgba8();
    let (source_width, source_height) = source.dimensions();
    let dims = CheckedDims::new(new_width, new_height, 4)?;
    #[cfg(feature = "parallel")]
    let height = new_height as usize;
    let row_stride = dims.row_stride();
    let source_stride = source_width as usize * 4;
    let offset_x = border as usize;
    let offset_y = border as usize;
    let copy_width = source_width.min(new_width.saturating_sub(border)) as usize;
    let copy_height = source_height.min(new_height.saturating_sub(border)) as usize;
    let fill = [fill.0, fill.1, fill.2, fill.3];
    let source = source.as_raw();
    let mut output = dims.alloc_buffer();

    #[cfg(feature = "parallel")]
    if dims.total_pixels() >= POINT_PARALLEL_PIXEL_THRESHOLD {
        crate::par_rows_mut!(
            &mut output,
            row_stride,
            height,
            |_row_start, _row_end, y, row| {
                expand_rgba_row(
                    row,
                    y as usize,
                    source,
                    source_stride,
                    offset_x,
                    offset_y,
                    copy_width,
                    copy_height,
                    fill,
                );
            }
        );
    } else {
        fill_rgba_rows(
            &mut output,
            row_stride,
            source,
            source_stride,
            offset_x,
            offset_y,
            copy_width,
            copy_height,
            fill,
        );
    }
    #[cfg(not(feature = "parallel"))]
    fill_rgba_rows(
        &mut output,
        row_stride,
        source,
        source_stride,
        offset_x,
        offset_y,
        copy_width,
        copy_height,
        fill,
    );

    let expanded = crate::image_utils::raw_bytes_to_image(new_width, new_height, output, 4)?;
    Ok(preserve_mode(img, expanded))
}

fn fill_rgba_rows(
    output: &mut [u8],
    row_stride: usize,
    source: &[u8],
    source_stride: usize,
    offset_x: usize,
    offset_y: usize,
    copy_width: usize,
    copy_height: usize,
    fill: [u8; 4],
) {
    for (y, row) in output.chunks_exact_mut(row_stride).enumerate() {
        expand_rgba_row(
            row,
            y,
            source,
            source_stride,
            offset_x,
            offset_y,
            copy_width,
            copy_height,
            fill,
        );
    }
}

#[cfg(test)]
mod expand_native_tests {
    use super::{native_expand_channels, op_expand};
    use crate::raster::{DynamicImage, RgbImage, RgbaImage};

    #[test]
    fn expand_hsv_keeps_three_native_bytes() {
        let image = DynamicImage::ImageRgb8(
            RgbImage::from_raw(1, 1, vec![23, 47, 89]).expect("HSV source"),
        );
        let expanded =
            op_expand(&image, 1, (7, 11, 13, 17), Some("HSV")).expect("native HSV expand");
        let DynamicImage::ImageRgb8(expanded) = expanded else {
            panic!("HSV expansion must retain three-byte physical storage");
        };
        assert_eq!(
            expanded.as_raw(),
            &[
                7, 11, 13, 7, 11, 13, 7, 11, 13, 7, 11, 13, 23, 47, 89, 7, 11, 13, 7, 11, 13, 7,
                11, 13, 7, 11, 13,
            ]
        );
    }

    #[test]
    fn expand_cmyk_keeps_k_as_the_fourth_native_sample() {
        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(1, 1, vec![23, 47, 89, 131]).expect("CMYK source"),
        );
        assert_eq!(native_expand_channels(&image, Some("CMYK")), Some(4));
        let mismatched = DynamicImage::ImageRgb8(
            RgbImage::from_raw(1, 1, vec![23, 47, 89]).expect("mismatched CMYK storage"),
        );
        assert_eq!(native_expand_channels(&mismatched, Some("CMYK")), None);

        let expanded =
            op_expand(&image, 1, (7, 11, 13, 17), Some("CMYK")).expect("native CMYK expand");
        let DynamicImage::ImageRgba8(expanded) = expanded else {
            panic!("CMYK expansion must retain four-byte storage");
        };
        assert_eq!(
            expanded.as_raw(),
            &[
                7, 11, 13, 17, 7, 11, 13, 17, 7, 11, 13, 17, 7, 11, 13, 17, 23, 47, 89, 131, 7, 11,
                13, 17, 7, 11, 13, 17, 7, 11, 13, 17, 7, 11, 13, 17,
            ]
        );
    }

    #[test]
    fn expand_rgbx_keeps_x_bytes_and_uses_fill_x_for_the_border() {
        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(2, 1, vec![23, 47, 89, 131, 29, 31, 37, 41]).expect("RGBX source"),
        );
        assert_eq!(native_expand_channels(&image, Some("RGBX")), Some(4));
        let mismatched = DynamicImage::ImageRgb8(
            RgbImage::from_raw(2, 1, vec![23, 47, 89, 29, 31, 37])
                .expect("mismatched RGBX storage"),
        );
        assert_eq!(native_expand_channels(&mismatched, Some("RGBX")), None);

        let expanded =
            op_expand(&image, 1, (7, 11, 13, 17), Some("RGBX")).expect("native RGBX expand");
        let DynamicImage::ImageRgba8(expanded) = expanded else {
            panic!("RGBX expansion must retain four-byte physical storage");
        };
        let fill = [7, 11, 13, 17];
        let mut expected = Vec::with_capacity(4 * 3 * 4);
        expected.extend_from_slice(&fill.repeat(4));
        expected.extend_from_slice(&fill);
        expected.extend_from_slice(&[23, 47, 89, 131, 29, 31, 37, 41]);
        expected.extend_from_slice(&fill);
        expected.extend_from_slice(&fill.repeat(4));
        assert_eq!(expanded.as_raw(), &expected);
    }
}
