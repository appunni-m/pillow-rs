use crate::error::PilError;
use crate::pipeline::{PipelineOp, PixelMode};
use crate::raster::{DynamicImage, GenericImage, GrayAlphaImage, GrayImage, RgbImage, RgbaImage};

pub fn op_put_pixel(
    img: &DynamicImage,
    x: u32,
    y: u32,
    color: (u8, u8, u8, u8),
) -> Result<DynamicImage, PilError> {
    // Every supported Image::putpixel entry point validates coordinates before
    // queuing PipelineOp::PutPixel. A malformed internal descriptor is outside
    // the public input boundary, so the executor writes the validated pixel
    // directly.
    match img.clone() {
        DynamicImage::ImageLuma8(mut l) => {
            l.put_pixel(x, y, crate::raster::Luma([color.0]));
            Ok(DynamicImage::ImageLuma8(l))
        }
        DynamicImage::ImageLumaA8(mut la) => {
            la.put_pixel(x, y, crate::raster::LumaA([color.0, color.3]));
            Ok(DynamicImage::ImageLumaA8(la))
        }
        DynamicImage::ImageRgb8(mut rgb) => {
            rgb.put_pixel(x, y, crate::raster::Rgb([color.0, color.1, color.2]));
            Ok(DynamicImage::ImageRgb8(rgb))
        }
        DynamicImage::ImageRgba8(mut rgba) => {
            rgba.put_pixel(
                x,
                y,
                crate::raster::Rgba([color.0, color.1, color.2, color.3]),
            );
            Ok(DynamicImage::ImageRgba8(rgba))
        }
        _ => Err(PilError::NotImplementedError(
            "putpixel not supported for this image type".into(),
        )),
    }
}

fn apply_put_pixel_batch<I, F>(
    image: &mut I,
    ops: &[PipelineOp],
    mut pixel_from_color: F,
) -> Result<(), PilError>
where
    I: GenericImage,
    F: FnMut((u8, u8, u8, u8)) -> I::Pixel,
{
    for op in ops {
        let PipelineOp::PutPixel { x, y, color, .. } = op else {
            return Err(PilError::ValueError("expected PutPixel op".into()));
        };
        if !image.in_bounds(*x, *y) {
            return Err(PilError::IndexError("image index out of range".into()));
        }
        image.put_pixel(*x, *y, pixel_from_color(*color));
    }
    Ok(())
}

/// Applies adjacent byte-oriented pixel writes after cloning the source once.
///
/// Public putpixel validates every coordinate before queuing its operation, so
/// the batch preserves the original ordered writes while avoiding one full
/// image copy per queued pixel.
pub fn op_put_pixel_batch(
    img: &DynamicImage,
    ops: &[PipelineOp],
) -> Result<DynamicImage, PilError> {
    match img {
        DynamicImage::ImageLuma8(image) => {
            let mut output = image.clone();
            apply_put_pixel_batch(&mut output, ops, |color| crate::raster::Luma([color.0]))?;
            Ok(DynamicImage::ImageLuma8(output))
        }
        DynamicImage::ImageLumaA8(image) => {
            let mut output = image.clone();
            apply_put_pixel_batch(&mut output, ops, |color| {
                crate::raster::LumaA([color.0, color.3])
            })?;
            Ok(DynamicImage::ImageLumaA8(output))
        }
        DynamicImage::ImageRgb8(image) => {
            let mut output = image.clone();
            apply_put_pixel_batch(&mut output, ops, |color| {
                crate::raster::Rgb([color.0, color.1, color.2])
            })?;
            Ok(DynamicImage::ImageRgb8(output))
        }
        DynamicImage::ImageRgba8(image) => {
            let mut output = image.clone();
            apply_put_pixel_batch(&mut output, ops, |color| {
                crate::raster::Rgba([color.0, color.1, color.2, color.3])
            })?;
            Ok(DynamicImage::ImageRgba8(output))
        }
        _ => Err(PilError::NotImplementedError(
            "putpixel not supported for this image type".into(),
        )),
    }
}

// ── PutData ──

pub fn op_put_data(
    img: &DynamicImage,
    data: &[u8],
    mode: PixelMode,
) -> Result<DynamicImage, PilError> {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let expected = w * h * mode.channels();
    // PIL: putdata accepts data shorter than the image — only the first
    // data.len() bytes are replaced; remaining pixels stay unchanged.
    let n_copy = data.len().min(expected);
    if expected != 0 && n_copy == expected {
        // A complete byte payload replaces every sample. When the source
        // already has the requested native layout, construct the result from
        // the payload directly instead of converting/cloning the old frame
        // and copying the same bytes over it.
        let dimensions = (w as u32, h as u32);
        match (mode, img) {
            (PixelMode::L, DynamicImage::ImageLuma8(_)) => {
                if let Some(output) =
                    GrayImage::from_raw(dimensions.0, dimensions.1, data[..expected].to_vec())
                {
                    return Ok(DynamicImage::ImageLuma8(output));
                }
            }
            (PixelMode::LA, DynamicImage::ImageLumaA8(_)) => {
                if let Some(output) =
                    GrayAlphaImage::from_raw(dimensions.0, dimensions.1, data[..expected].to_vec())
                {
                    return Ok(DynamicImage::ImageLumaA8(output));
                }
            }
            (PixelMode::RGB, DynamicImage::ImageRgb8(_)) => {
                if let Some(output) =
                    RgbImage::from_raw(dimensions.0, dimensions.1, data[..expected].to_vec())
                {
                    return Ok(DynamicImage::ImageRgb8(output));
                }
            }
            (PixelMode::RGBA, DynamicImage::ImageRgba8(_)) => {
                if let Some(output) =
                    RgbaImage::from_raw(dimensions.0, dimensions.1, data[..expected].to_vec())
                {
                    return Ok(DynamicImage::ImageRgba8(output));
                }
            }
            _ => {}
        }
    }
    match mode {
        PixelMode::RGB | PixelMode::YCbCr | PixelMode::HSV => {
            let orig = img.to_rgb8();
            let mut pixels = orig.into_raw();
            pixels[..n_copy].copy_from_slice(&data[..n_copy]);
            let rgb = RgbImage::from_raw(w as u32, h as u32, pixels)
                .ok_or_else(|| PilError::ValueError("putdata: buffer error".into()))?;
            Ok(DynamicImage::ImageRgb8(rgb))
        }
        PixelMode::L | PixelMode::P | PixelMode::Mode1 => {
            let orig = img.to_luma8();
            let mut pixels = orig.into_raw();
            pixels[..n_copy].copy_from_slice(&data[..n_copy]);
            let gray = GrayImage::from_raw(w as u32, h as u32, pixels)
                .ok_or_else(|| PilError::ValueError("putdata: buffer error".into()))?;
            Ok(DynamicImage::ImageLuma8(gray))
        }
        PixelMode::LA | PixelMode::PA => {
            let orig = img.to_luma_alpha8();
            let mut pixels = orig.into_raw();
            pixels[..n_copy].copy_from_slice(&data[..n_copy]);
            let la = GrayAlphaImage::from_raw(w as u32, h as u32, pixels)
                .ok_or_else(|| PilError::ValueError("putdata: buffer error".into()))?;
            Ok(DynamicImage::ImageLumaA8(la))
        }
        _ => {
            let orig = img.to_rgba8();
            let mut pixels = orig.into_raw();
            pixels[..n_copy].copy_from_slice(&data[..n_copy]);
            let rgba = RgbaImage::from_raw(w as u32, h as u32, pixels)
                .ok_or_else(|| PilError::ValueError("putdata: buffer error".into()))?;
            Ok(DynamicImage::ImageRgba8(rgba))
        }
    }
}

// ── PutAlpha ──

pub fn op_put_alpha(img: &DynamicImage, alpha: u8, mode: PixelMode) -> DynamicImage {
    if mode == PixelMode::CMYK {
        // Pillow Image.putalpha falls back from ImagingCore.setmode to
        // Convert.c:cmyk2rgb. That path uses MULDIV255 integer rounding before
        // Bands.c:ImagingFillBand replaces the promoted RGBA alpha channel.
        let rgb = crate::color::cmyk_to_rgb(img).to_rgb8();
        let mut rgba = RgbaImage::new(rgb.width(), rgb.height());
        for (output, input) in rgba.pixels_mut().zip(rgb.pixels()) {
            *output = crate::raster::Rgba([input[0], input[1], input[2], alpha]);
        }
        return DynamicImage::ImageRgba8(rgba);
    }
    if matches!(mode, PixelMode::P | PixelMode::PA) {
        // Convert.c:p2pa retains the palette index byte and adds one alpha byte
        // per pixel; the palette itself remains attached at the Image layer.
        let luma = img.to_luma8();
        let mut la = GrayAlphaImage::new(luma.width(), luma.height());
        for (output, input) in la.pixels_mut().zip(luma.pixels()) {
            output[0] = input[0];
            output[1] = alpha;
        }
        return DynamicImage::ImageLumaA8(la);
    }
    if mode == PixelMode::LA
        && let DynamicImage::ImageLumaA8(source) = img
    {
        // LA already stores luma and alpha contiguously. Replace byte 1 in
        // one native copy instead of widening to four-byte RGBA and narrowing
        // back to LA.
        let mut output = source.clone();
        for pixel in output.as_mut().chunks_exact_mut(2) {
            pixel[1] = alpha;
        }
        return DynamicImage::ImageLumaA8(output);
    }

    match img.color() {
        crate::raster::ColorType::L8 => {
            let luma = img.to_luma8();
            let mut la = GrayAlphaImage::new(luma.width(), luma.height());
            for (o, i) in la.pixels_mut().zip(luma.pixels()) {
                o[0] = i[0];
                o[1] = alpha;
            }
            DynamicImage::ImageLumaA8(la)
        }
        crate::raster::ColorType::La8 => {
            let rgba = img.to_rgba8();
            let mut la = GrayAlphaImage::new(rgba.width(), rgba.height());
            for (o, i) in la.pixels_mut().zip(rgba.pixels()) {
                o[0] = i[0];
                o[1] = alpha;
            }
            DynamicImage::ImageLumaA8(la)
        }
        crate::raster::ColorType::Rgb8 => {
            let DynamicImage::ImageRgb8(rgb) = img else {
                unreachable!("Rgb8 color type must use ImageRgb8 storage")
            };
            let mut rgba = RgbaImage::new(rgb.width(), rgb.height());
            for (output, input) in rgba
                .as_mut()
                .chunks_exact_mut(4)
                .zip(rgb.as_raw().chunks_exact(3))
            {
                output[..3].copy_from_slice(input);
                output[3] = alpha;
            }
            DynamicImage::ImageRgba8(rgba)
        }
        _ => {
            let mut rgba = img.to_rgba8();
            for p in rgba.pixels_mut() {
                p[3] = alpha;
            }
            DynamicImage::ImageRgba8(rgba)
        }
    }
}

/// Replaces the alpha channel from an L-mask, matching Pillow's image-backed
/// `putalpha` (P: index retained, CMYK: promoted through RGB to RGBA).
pub fn op_put_alpha_data(
    img: &DynamicImage,
    mask: &crate::raster::DynamicImage,
    mode: PixelMode,
) -> DynamicImage {
    if mode == PixelMode::RGBA
        && let (DynamicImage::ImageRgba8(rgba), DynamicImage::ImageLuma8(alpha)) = (img, mask)
        && rgba.dimensions() == alpha.dimensions()
        && rgba.as_raw().len() == alpha.as_raw().len().saturating_mul(4)
    {
        // RGBA already has the requested storage layout. Clone it once for
        // the result and replace only byte 3 instead of cloning via
        // `to_rgba8()` and copying all four channels into a second frame.
        let mut output = rgba.clone();
        for (pixel, alpha) in output.as_mut().chunks_exact_mut(4).zip(alpha.as_raw()) {
            pixel[3] = *alpha;
        }
        return DynamicImage::ImageRgba8(output);
    }
    if mode == PixelMode::RGB
        && let (DynamicImage::ImageRgb8(rgb), DynamicImage::ImageLuma8(alpha)) = (img, mask)
        && rgb.dimensions() == alpha.dimensions()
        && rgb.as_raw().len() == alpha.as_raw().len().saturating_mul(3)
    {
        // RGB putalpha must return RGBA, but widening the source first and
        // copying that temporary into another RGBA allocation is redundant.
        // Read native three-byte pixels and write the required result once.
        let mut output = RgbaImage::new(rgb.width(), rgb.height());
        for ((destination, source), alpha) in output
            .as_mut()
            .chunks_exact_mut(4)
            .zip(rgb.as_raw().chunks_exact(3))
            .zip(alpha.as_raw())
        {
            destination[..3].copy_from_slice(source);
            destination[3] = *alpha;
        }
        return DynamicImage::ImageRgba8(output);
    }
    if mode == PixelMode::CMYK {
        let rgb = crate::color::cmyk_to_rgb(img).to_rgb8();
        let mut rgba = RgbaImage::new(rgb.width(), rgb.height());
        for ((output, input), mask_px) in rgba
            .pixels_mut()
            .zip(rgb.pixels())
            .zip(mask.to_luma8().pixels())
        {
            *output = crate::raster::Rgba([input[0], input[1], input[2], mask_px.0[0]]);
        }
        return DynamicImage::ImageRgba8(rgba);
    }
    if matches!(mode, PixelMode::P | PixelMode::PA) {
        let luma = img.to_luma8();
        let mut la = GrayAlphaImage::new(luma.width(), luma.height());
        for ((output, input), mask_px) in la
            .pixels_mut()
            .zip(luma.pixels())
            .zip(mask.to_luma8().pixels())
        {
            output[0] = input[0];
            output[1] = mask_px.0[0];
        }
        return DynamicImage::ImageLumaA8(la);
    }
    match img.color() {
        crate::raster::ColorType::L8 => {
            let luma = img.to_luma8();
            let mut la = GrayAlphaImage::new(luma.width(), luma.height());
            for ((o, i), mask_px) in la
                .pixels_mut()
                .zip(luma.pixels())
                .zip(mask.to_luma8().pixels())
            {
                o[0] = i[0];
                o[1] = mask_px.0[0];
            }
            DynamicImage::ImageLumaA8(la)
        }
        crate::raster::ColorType::La8 => {
            // Pillow's putalpha replaces the existing A band in-place for
            // LA images; it does not promote the two-band sample layout to
            // RGBA merely because the mask came from another image.
            let la = img.to_luma_alpha8();
            let mut out = GrayAlphaImage::new(la.width(), la.height());
            for ((output, input), mask_px) in out
                .pixels_mut()
                .zip(la.pixels())
                .zip(mask.to_luma8().pixels())
            {
                output[0] = input[0];
                output[1] = mask_px.0[0];
            }
            DynamicImage::ImageLumaA8(out)
        }
        _ => {
            let rgba = img.to_rgba8();
            let mut out = RgbaImage::new(rgba.width(), rgba.height());
            for ((o, i), mask_px) in out
                .pixels_mut()
                .zip(rgba.pixels())
                .zip(mask.to_luma8().pixels())
            {
                *o = *i;
                o[3] = mask_px.0[0];
            }
            DynamicImage::ImageRgba8(out)
        }
    }
}
