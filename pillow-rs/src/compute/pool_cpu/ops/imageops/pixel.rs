use super::histogram::{apply_native_luma_lut_row, native_luma_lut_tables};
use super::*;

#[inline]
fn apply_autocontrast_row(
    source: &[u8],
    row: &mut [u8],
    channels: usize,
    native_rgb_bytes: bool,
    lut: &[u8],
    luma_lut_tables: Option<&[u8x16; 16]>,
) {
    if let Some(tables) = luma_lut_tables {
        apply_native_luma_lut_row(source, row, tables);
        return;
    }
    if native_rgb_bytes {
        for (source, output) in source.chunks_exact(3).zip(row.chunks_exact_mut(3)) {
            output[0] = lut[usize::from(source[0])];
            output[1] = lut[256 + usize::from(source[1])];
            output[2] = lut[512 + usize::from(source[2])];
        }
        return;
    }
    for (index, (input, output)) in source.iter().zip(row).enumerate() {
        let channel = index % channels;
        *output = lut[channel * 256 + usize::from(*input)];
    }
}

fn apply_point_bytes<F>(bytes: &mut [u8], transform: F)
where
    F: Fn(&mut [u8]),
{
    transform(bytes);
}
/// Autocontrast: stretch image contrast based on histogram cutoff.
/// PIL: per-channel histogram, find lo/hi at cutoff percentiles for each channel,
/// then linearly map [lo, hi] to [0, 255] using truncation (int() cast).
pub fn op_autocontrast(
    img: &DynamicImage,
    cutoff: f64,
    mask: Option<&std::sync::Arc<crate::image::Image>>,
) -> Result<DynamicImage, PilError> {
    let channels = img.color().channel_count() as usize;
    let (w, h) = (img.width(), img.height());
    // Pillow's ImageOps.autocontrast preserves an empty image instead of
    // sending its zero-pixel buffer through the ordinary allocation guard.
    // Keep this public behavior here so the CPU lane matches the SIMD scalar
    // implementation and retains the source mode for a 0×0 result.
    if w == 0 || h == 0 {
        return Ok(img.clone());
    }

    let lut = autocontrast_lut(img, cutoff, mask)?;
    let raw = img.as_bytes();
    let native_rgb_bytes = channels == 3 && matches!(img, DynamicImage::ImageRgb8(_));
    // Pillow still returns a distinct image when every RGB LUT is the
    // identity. Preserve that copy behavior while skipping the byte-wise LUT
    // pass for full-range images and masks that select no pixels.
    if native_rgb_bytes && crate::compute::pool_cpu::point_lut_is_identity(&lut, channels) {
        return Ok(img.clone());
    }
    // RGB has a separate LUT per interleaved byte band. Map it from immutable
    // source bytes into one output allocation instead of cloning and then
    // reading the source again in the per-byte pass.
    let mut out = if native_rgb_bytes {
        vec![0; raw.len()]
    } else {
        raw.to_vec()
    };
    let stride = w as usize * channels;
    let luma_lut_tables = if channels == 1 {
        native_luma_lut_tables(&lut)
    } else {
        None
    };

    for y in 0..h as usize {
        let row_start = y * stride;
        apply_autocontrast_row(
            &raw[row_start..row_start + stride],
            &mut out[row_start..row_start + stride],
            channels,
            native_rgb_bytes,
            &lut,
            luma_lut_tables.as_ref(),
        );
    }
    let result = crate::image_utils::raw_bytes_to_image(w, h, out, channels)?;
    Ok(preserve_mode(img, result))
}

/// Equalize: histogram equalization matching PIL's algorithm.
/// Build LUT from non-zero histogram bins, using PIL's step formula.
pub fn op_equalize(img: &DynamicImage) -> Result<DynamicImage, PilError> {
    op_equalize_with_mask(img, None)
}

/// Equalize with a validated histogram mask, retaining native L/RGB storage.
pub(crate) fn op_equalize_with_mask(
    img: &DynamicImage,
    mask: Option<&std::sync::Arc<crate::image::Image>>,
) -> Result<DynamicImage, PilError> {
    let mask = mask.map(|mask| mask.materialized_shared()).transpose()?;
    let converted;
    let source = if matches!(
        img,
        DynamicImage::ImageLuma8(_) | DynamicImage::ImageRgb8(_)
    ) {
        img
    } else {
        converted = DynamicImage::ImageRgb8(img.to_rgb8());
        &converted
    };
    let channels = usize::from(source.color().channel_count());
    let lut = equalize_lut(source, channels, mask.as_deref())
        .ok_or_else(|| PilError::ValueError("images do not match".into()))?;
    let result = if crate::compute::pool_cpu::point_lut_is_identity(&lut, channels) {
        source.clone()
    } else {
        super::super::effects::op_eval(source, &lut)?
    };
    Ok(preserve_mode(img, result))
}

/// Invert: subtract each pixel value from 255 (all channels, matching PIL's point()).
pub fn op_invert(img: &DynamicImage) -> Result<DynamicImage, PilError> {
    let channels = img.color().channel_count() as usize;
    let (w, h) = (img.width(), img.height());
    let raw = img.as_bytes();
    let out = raw.iter().map(|value| u8::MAX - *value).collect();
    let result = match channels {
        1 => crate::raster::GrayImage::from_raw(w, h, out)
            .map(DynamicImage::ImageLuma8)
            .ok_or_else(|| PilError::InternalError("invert L buffer shape mismatch".to_string()))?,
        2 => crate::raster::GrayAlphaImage::from_raw(w, h, out)
            .map(DynamicImage::ImageLumaA8)
            .ok_or_else(|| {
                PilError::InternalError("invert LA buffer shape mismatch".to_string())
            })?,
        3 => crate::raster::RgbImage::from_raw(w, h, out)
            .map(DynamicImage::ImageRgb8)
            .ok_or_else(|| {
                PilError::InternalError("invert RGB buffer shape mismatch".to_string())
            })?,
        _ => crate::raster::RgbaImage::from_raw(w, h, out)
            .map(DynamicImage::ImageRgba8)
            .ok_or_else(|| {
                PilError::InternalError("invert RGBA buffer shape mismatch".to_string())
            })?,
    };
    Ok(result)
}

/// Copy complete native rows in reverse order. The caller selects a
/// mode-specific pixel width and reconstructs that same storage type; this
/// helper never expands samples to RGBA or interprets their color values.
fn flip_native_rows<const BYTES_PER_PIXEL: usize>(
    source: &[u8],
    width: u32,
    height: u32,
) -> Option<Vec<u8>> {
    let row_bytes = usize::try_from(width).ok()?.checked_mul(BYTES_PER_PIXEL)?;
    let total_bytes = row_bytes.checked_mul(usize::try_from(height).ok()?)?;
    if source.len() != total_bytes {
        return None;
    }
    if height == 0 || row_bytes == 0 {
        return Some(source.to_vec());
    }

    let mut output = Vec::with_capacity(total_bytes);
    for output_y in 0..height as usize {
        let source_y = height as usize - 1 - output_y;
        let start = source_y.checked_mul(row_bytes)?;
        output.extend_from_slice(source.get(start..start.checked_add(row_bytes)?)?);
    }
    Some(output)
}

fn flip_mode1(image: &GrayImage) -> Option<DynamicImage> {
    let (width, height) = image.dimensions();
    let pixels = flip_native_rows::<1>(image.as_raw(), width, height)?;
    Some(DynamicImage::ImageLuma8(GrayImage::from_raw(
        width, height, pixels,
    )?))
}

fn flip_l(image: &GrayImage) -> Option<DynamicImage> {
    let (width, height) = image.dimensions();
    let samples = flip_native_rows::<1>(image.as_raw(), width, height)?;
    Some(DynamicImage::ImageLuma8(GrayImage::from_raw(
        width, height, samples,
    )?))
}

fn flip_palette(image: &GrayImage) -> Option<DynamicImage> {
    let (width, height) = image.dimensions();
    let indices = flip_native_rows::<1>(image.as_raw(), width, height)?;
    Some(DynamicImage::ImageLuma8(GrayImage::from_raw(
        width, height, indices,
    )?))
}

fn flip_la(image: &GrayAlphaImage) -> Option<DynamicImage> {
    let (width, height) = image.dimensions();
    let luma_alpha = flip_native_rows::<2>(image.as_raw(), width, height)?;
    Some(DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(
        width, height, luma_alpha,
    )?))
}

fn flip_pa(image: &GrayAlphaImage) -> Option<DynamicImage> {
    let (width, height) = image.dimensions();
    let palette_alpha = flip_native_rows::<2>(image.as_raw(), width, height)?;
    Some(DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(
        width,
        height,
        palette_alpha,
    )?))
}

fn flip_rgb(image: &RgbImage) -> Option<DynamicImage> {
    let (width, height) = image.dimensions();
    let rgb_samples = flip_native_rows::<3>(image.as_raw(), width, height)?;
    Some(DynamicImage::ImageRgb8(RgbImage::from_raw(
        width,
        height,
        rgb_samples,
    )?))
}

fn flip_rgba(image: &RgbaImage) -> Option<DynamicImage> {
    let (width, height) = image.dimensions();
    let rgba_samples = flip_native_rows::<4>(image.as_raw(), width, height)?;
    Some(DynamicImage::ImageRgba8(RgbaImage::from_raw(
        width,
        height,
        rgba_samples,
    )?))
}

/// Flip vertically while retaining the source mode's native sample layout.
pub fn op_flip(img: &DynamicImage, mode: Option<&str>) -> Result<DynamicImage, PilError> {
    let result = match (mode, img) {
        (Some("1"), DynamicImage::ImageLuma8(image)) => flip_mode1(image),
        (Some("L"), DynamicImage::ImageLuma8(image)) => flip_l(image),
        (Some("P"), DynamicImage::ImageLuma8(image)) => flip_palette(image),
        (Some("LA"), DynamicImage::ImageLumaA8(image)) => flip_la(image),
        (Some("PA"), DynamicImage::ImageLumaA8(image)) => flip_pa(image),
        (Some("RGB"), DynamicImage::ImageRgb8(image)) => flip_rgb(image),
        (Some("RGBA"), DynamicImage::ImageRgba8(image)) => flip_rgba(image),
        (None, DynamicImage::ImageLuma8(image)) => flip_l(image),
        (None, DynamicImage::ImageLumaA8(image)) => flip_la(image),
        (None, DynamicImage::ImageRgb8(image)) => flip_rgb(image),
        (None, DynamicImage::ImageRgba8(image)) => flip_rgba(image),
        _ => None,
    };
    Ok(result.unwrap_or_else(|| img.flipv()))
}

/// Mirror horizontally.
#[inline]
fn mirror_byte_row<const CHANNELS: usize>(source: &[u8], output: &mut [u8]) {
    for (destination, source_pixel) in output
        .chunks_exact_mut(CHANNELS)
        .zip(source.rchunks_exact(CHANNELS))
    {
        match CHANNELS {
            1 => destination[0] = source_pixel[0],
            2 => {
                destination[0] = source_pixel[0];
                destination[1] = source_pixel[1];
            }
            3 => {
                destination[0] = source_pixel[0];
                destination[1] = source_pixel[1];
                destination[2] = source_pixel[2];
            }
            4 => {
                destination[0] = source_pixel[0];
                destination[1] = source_pixel[1];
                destination[2] = source_pixel[2];
                destination[3] = source_pixel[3];
            }
            _ => unreachable!("Mirror supports only byte pixels with 1–4 channels"),
        }
    }
}

#[inline]
fn mirror_byte_rows<const CHANNELS: usize>(source: &[u8], output: &mut [u8], row_len: usize) {
    for (source_row, output_row) in source
        .chunks_exact(row_len)
        .zip(output.chunks_exact_mut(row_len))
    {
        mirror_byte_row::<CHANNELS>(source_row, output_row);
    }
}

pub fn op_mirror(img: &DynamicImage) -> Result<DynamicImage, PilError> {
    let channels = match img {
        DynamicImage::ImageLuma8(_) => 1,
        DynamicImage::ImageLumaA8(_) => 2,
        DynamicImage::ImageRgb8(_) => 3,
        DynamicImage::ImageRgba8(_) => 4,
        _ => return Ok(img.fliph()),
    };
    let (width, height) = (img.width(), img.height());
    if width == 0 || height == 0 {
        return Ok(img.fliph());
    }

    // Preserve the existing image crate path when limits have changed since
    // this image was created, and validate all allocation arithmetic before
    // reserving the destination buffer.
    let Ok(dims) = crate::checked_dims::CheckedDims::new(width, height, channels as u8) else {
        return Ok(img.fliph());
    };
    let row_len = dims.row_stride();
    let total_len = dims.total_bytes();
    let Some(source) = img.as_bytes().get(..total_len) else {
        return Ok(img.fliph());
    };

    // Reverse complete native pixels within each row. DynamicImage::fliph
    // routes byte images through per-pixel get_pixel/from_fn closures; these
    // contiguous spans avoid that coordinate and bounds work without changing
    // channel bytes or touching typed image paths.
    let mut output = dims.alloc_buffer();
    match channels {
        1 => mirror_byte_rows::<1>(source, &mut output, row_len),
        2 => mirror_byte_rows::<2>(source, &mut output, row_len),
        3 => mirror_byte_rows::<3>(source, &mut output, row_len),
        4 => mirror_byte_rows::<4>(source, &mut output, row_len),
        _ => unreachable!("Mirror supports only byte pixels with 1–4 channels"),
    }

    let result = crate::image_utils::raw_bytes_to_image(width, height, output, channels)?;
    Ok(preserve_mode(img, result))
}

/// Posterize: reduce the number of bits per channel.
pub fn op_posterize(img: &DynamicImage, bits: u8) -> Result<DynamicImage, PilError> {
    let mask = !((1u8 << (8 - bits)) - 1);
    if let DynamicImage::ImageLuma8(source) = img {
        // ImageOps.posterize accepts native L bytes. Expanding L to RGB and
        // then narrowing it back triples the working set and memory traffic.
        let mut gray = source.clone();
        apply_point_bytes(gray.as_mut(), |row| {
            for value in row {
                *value &= mask;
            }
        });
        return Ok(DynamicImage::ImageLuma8(gray));
    }

    if let DynamicImage::ImageRgb8(source) = img {
        // Transform borrowed native RGB bytes into the final result in one pass.
        let bytes = source
            .as_raw()
            .iter()
            .map(|&channel| channel & mask)
            .collect();
        let rgb = crate::raster::RgbImage::from_raw(source.width(), source.height(), bytes)
            .ok_or_else(|| PilError::InternalError("posterize RGB buffer mismatch".into()))?;
        return Ok(preserve_mode(img, DynamicImage::ImageRgb8(rgb)));
    }
    let mut rgb = img.to_rgb8();

    for channel in rgb.as_mut() {
        *channel &= mask;
    }
    Ok(preserve_mode(img, DynamicImage::ImageRgb8(rgb)))
}

/// Solarize: invert pixels where value >= threshold.
/// PIL uses >=, not >.
pub fn op_solarize(img: &DynamicImage, threshold: u8) -> Result<DynamicImage, PilError> {
    if matches!(
        img,
        DynamicImage::ImageLuma8(_) | DynamicImage::ImageRgb8(_)
    ) {
        // Every stored byte is a color sample in the public L/RGB contract.
        // Write the result once instead of expanding L to RGB and converting back.
        let bytes = img
            .as_bytes()
            .iter()
            .map(|&value| {
                if value >= threshold {
                    255 - value
                } else {
                    value
                }
            })
            .collect();
        let channels = if matches!(img, DynamicImage::ImageLuma8(_)) {
            1
        } else {
            3
        };
        return crate::image_utils::raw_bytes_to_image_allow_empty(
            img.width(),
            img.height(),
            bytes,
            channels,
        );
    }
    let t = threshold;
    let mut rgb = img.to_rgb8();
    apply_point_bytes(rgb.as_mut(), |row| {
        for pixel in row.chunks_exact_mut(3) {
            for channel in pixel {
                if *channel >= t {
                    // PIL uses >=, not >
                    *channel = u8::MAX - *channel;
                }
            }
        }
    });
    Ok(preserve_mode(img, DynamicImage::ImageRgb8(rgb)))
}

/// Gather one byte channel from tightly packed native pixels. x86_64 uses an
/// SSSE3 shuffle kernel when available; other supported SIMD targets use the
/// native `u8x16` implementation, with a scalar tail for incomplete blocks.
#[cfg(test)]
mod posterize_tests {
    use super::op_posterize;
    use crate::raster::{DynamicImage, GrayImage, RgbImage};

    #[test]
    fn native_l_and_rgb_preserve_each_sample_for_every_supported_bit_depth() {
        let luma = (0..=u8::MAX).collect::<Vec<_>>();
        let rgb = luma
            .iter()
            .flat_map(|&value| [value, value.wrapping_add(71), value.wrapping_add(149)])
            .collect::<Vec<_>>();
        let luma = DynamicImage::ImageLuma8(GrayImage::from_raw(256, 1, luma).unwrap());
        let rgb = DynamicImage::ImageRgb8(RgbImage::from_raw(256, 1, rgb).unwrap());

        for bits in 1..=8 {
            let mask = !((1u8 << (8 - bits)) - 1);
            let expected_luma = luma
                .as_bytes()
                .iter()
                .map(|value| value & mask)
                .collect::<Vec<_>>();
            let expected_rgb = rgb
                .as_bytes()
                .iter()
                .map(|value| value & mask)
                .collect::<Vec<_>>();
            let result_luma = op_posterize(&luma, bits).unwrap();
            let result_rgb = op_posterize(&rgb, bits).unwrap();

            assert!(matches!(result_luma, DynamicImage::ImageLuma8(_)));
            assert!(matches!(result_rgb, DynamicImage::ImageRgb8(_)));
            assert_eq!(result_luma.as_bytes(), expected_luma);
            assert_eq!(result_rgb.as_bytes(), expected_rgb);
        }
    }
}
