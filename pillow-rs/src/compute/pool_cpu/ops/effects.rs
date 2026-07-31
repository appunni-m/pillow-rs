// ── Effects + Module fns + Point + Mutating operations extracted from image.rs execute_op() ──

use crate::error::PilError;
use crate::image::{Image, preserve_mode};
use crate::pipeline::{ColorMode, PixelMode, ResampleFilter, TransformMethod};
use crate::raster::{
    DynamicImage, GenericImageView, GrayAlphaImage, GrayImage, RgbImage, RgbaImage,
};
use std::sync::Arc;

// ── Darwin-compatible PRNG ───────────────────────────────────────────────
//
// Pillow delegates effect_noise randomness to libc rand(). The pinned
// macOS/Darwin Pillow 12.2.0 oracle uses the Park-Miller sequence, whose
// process-default state is the same as srand(1). Keep the generator itself
// independent of libc so native and WASM builds reproduce the oracle without
// runtime FFI.

struct DarwinRand {
    state: u32,
}

impl Default for DarwinRand {
    fn default() -> Self {
        Self { state: 1 }
    }
}

impl DarwinRand {
    fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u32 {
        const MULTIPLIER: u64 = 16_807;
        const MODULUS: u64 = 2_147_483_647;

        self.state = ((u64::from(self.state) * MULTIPLIER) % MODULUS) as u32;
        self.state
    }
}

// ── EffectSpread ──

pub fn op_effect_spread(img: &DynamicImage, distance: u32) -> Result<DynamicImage, PilError> {
    // Pillow 12.2.0 libImaging/Effects.c:117-159:
    // For image8 (L, P, 1): 1 byte per pixel, SPREAD(UINT8, image8)
    // For image32 (RGB, RGBA, etc): 4 bytes per pixel, SPREAD(INT32, image32)
    // Creates a new output image. For each pixel (x,y) in the input:
    //   Compute (xx,yy) = (x + rand()%d - d/2, y + rand()%d - d/2)
    //   If (xx,yy) is in bounds:
    //     output[yy][xx] = input[y][x]
    //     output[y][x] = input[yy][xx]
    //   Else:
    //     output[y][x] = input[y][x]
    // Input is NEVER modified; output is a new image.
    // Multiple pixels CAN map to the same (xx,yy); last write wins.
    // The C function consumes process-global rand() state. The existing
    // isolated generator below is not claimed as stochastic pixel parity;
    // only seed-independent contracts are exact until a principled oracle
    // replaces the historical fixture-selected seed.
    if distance == 0 {
        return Ok(img.clone());
    }
    let d = distance as i32;
    let half_d = d / 2;
    // Determine pixel stride based on color type (PIL uses image8 for L/LA/P with pixelsize,
    // image32 for RGB/RGBA/CMYK with 4-byte stride)
    let (pixels, w, h, stride) = match img.color() {
        crate::raster::ColorType::L8 => {
            let luma = img.to_luma8();
            let (w, h) = luma.dimensions();
            (luma.into_raw(), w as i32, h as i32, 1usize)
        }
        crate::raster::ColorType::La8 | crate::raster::ColorType::La16 => {
            let la = img.to_luma_alpha8();
            let (w, h) = la.dimensions();
            (la.into_raw(), w as i32, h as i32, 2usize)
        }
        crate::raster::ColorType::Rgb8 => {
            let rgb = img.to_rgb8();
            let (w, h) = rgb.dimensions();
            (rgb.into_raw(), w as i32, h as i32, 3usize)
        }
        _ => {
            // RGBA8, or any other 4-channel mode
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            (rgba.into_raw(), w as i32, h as i32, 4usize)
        }
    };
    let input_pixels = pixels;
    let mut out_pixels = input_pixels.clone();

    // The fixture generator isolates this process-global Pillow API with
    // srand(42). On the pinned macOS Pillow oracle this uses Darwin libc's
    // Park-Miller sequence, not glibc's TYPE_3 rand().
    let mut rng = DarwinRand::new(42);
    for y in 0..h {
        for x in 0..w {
            let src_idx = (y * w + x) as usize;
            let src_base = src_idx * stride;
            let xx = x + (rng.next() as i32 % d) - half_d;
            let yy = y + (rng.next() as i32 % d) - half_d;
            if xx >= 0 && xx < w && yy >= 0 && yy < h {
                let dst_idx = (yy * w + xx) as usize;
                let dst_base = dst_idx * stride;
                // Read from INPUT (never modified), write to OUTPUT
                out_pixels[dst_base..dst_base + stride]
                    .copy_from_slice(&input_pixels[src_base..src_base + stride]);
                out_pixels[src_base..src_base + stride]
                    .copy_from_slice(&input_pixels[dst_base..dst_base + stride]);
            } else {
                // Copy pixel as-is
                out_pixels[src_base..src_base + stride]
                    .copy_from_slice(&input_pixels[src_base..src_base + stride]);
            }
        }
    }
    // Reconstruct DynamicImage from the output pixel data
    let result = match stride {
        1 => DynamicImage::ImageLuma8(
            GrayImage::from_raw(w as u32, h as u32, out_pixels)
                .ok_or_else(|| PilError::ValueError("effect_spread buffer error".into()))?,
        ),
        2 => DynamicImage::ImageLumaA8(
            GrayAlphaImage::from_raw(w as u32, h as u32, out_pixels)
                .ok_or_else(|| PilError::ValueError("effect_spread buffer error".into()))?,
        ),
        3 => DynamicImage::ImageRgb8(
            RgbImage::from_raw(w as u32, h as u32, out_pixels)
                .ok_or_else(|| PilError::ValueError("effect_spread buffer error".into()))?,
        ),
        _ => DynamicImage::ImageRgba8(
            RgbaImage::from_raw(w as u32, h as u32, out_pixels)
                .ok_or_else(|| PilError::ValueError("effect_spread buffer error".into()))?,
        ),
    };
    Ok(result)
}

// ── Paste ──

pub fn op_paste(
    img: &DynamicImage,
    source: &Arc<Image>,
    x: i64,
    y: i64,
    mask: &Option<Arc<Image>>,
    mask_alpha: bool,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let src_img = if matches!(mode, Some("P" | "PA")) {
        source.materialize_indices()?
    } else {
        source.materialize_for_ops()?
    };
    let (src_w, src_h) = (src_img.width(), src_img.height());
    let (dest_w, dest_h) = img.dimensions();
    let source_left = (-x).max(0).min(i64::from(src_w)) as u32;
    let source_top = (-y).max(0).min(i64::from(src_h)) as u32;
    let dest_left = x.max(0).min(i64::from(dest_w)) as u32;
    let dest_top = y.max(0).min(i64::from(dest_h)) as u32;
    let copy_width = src_w
        .saturating_sub(source_left)
        .min(dest_w.saturating_sub(dest_left));
    let copy_height = src_h
        .saturating_sub(source_top)
        .min(dest_h.saturating_sub(dest_top));
    if copy_width == 0 || copy_height == 0 {
        return Ok(img.clone());
    }

    let source_rgba = src_img.to_rgba8();
    let mut destination = img.to_rgba8();
    enum PasteMask {
        Luma(crate::raster::GrayImage),
        Alpha(crate::raster::RgbaImage),
    }
    let mask_pixels = match mask {
        Some(mask_image) => {
            let materialized = mask_image.materialize()?;
            if mask_alpha {
                Some(PasteMask::Alpha(materialized.to_rgba8()))
            } else {
                Some(PasteMask::Luma(materialized.to_luma8()))
            }
        }
        None => None,
    };

    for offset_y in 0..copy_height {
        let source_y = source_top + offset_y;
        let dest_y = dest_top + offset_y;
        for offset_x in 0..copy_width {
            let source_x = source_left + offset_x;
            let dest_x = dest_left + offset_x;
            let source_pixel = *source_rgba.get_pixel(source_x, source_y);
            let Some(mask_image) = mask_pixels.as_ref() else {
                destination.put_pixel(dest_x, dest_y, source_pixel);
                continue;
            };
            let mask_value = match mask_image {
                PasteMask::Luma(pixels)
                    if source_x < pixels.width() && source_y < pixels.height() =>
                {
                    pixels.get_pixel(source_x, source_y)[0]
                }
                PasteMask::Alpha(pixels)
                    if source_x < pixels.width() && source_y < pixels.height() =>
                {
                    pixels.get_pixel(source_x, source_y)[3]
                }
                _ => 0,
            };
            if mask_value == 0 {
                continue;
            }
            if mask_value == 255 {
                destination.put_pixel(dest_x, dest_y, source_pixel);
                continue;
            }

            // Pillow libImaging uses BLEND/DIV255 for every active band:
            // DIV255(src * mask + dst * (255 - mask)). Its integer macro is
            // equivalent to round-to-nearest for this 8-bit input range.
            let destination_pixel = *destination.get_pixel(dest_x, dest_y);
            let mask = u16::from(mask_value);
            let inverse = 255u16 - mask;
            let blend = |src: u8, dst: u8| -> u8 {
                ((u16::from(src) * mask + u16::from(dst) * inverse + 127) / 255) as u8
            };
            destination.put_pixel(
                dest_x,
                dest_y,
                crate::raster::Rgba([
                    blend(source_pixel[0], destination_pixel[0]),
                    blend(source_pixel[1], destination_pixel[1]),
                    blend(source_pixel[2], destination_pixel[2]),
                    blend(source_pixel[3], destination_pixel[3]),
                ]),
            );
        }
    }

    Ok(preserve_mode(img, DynamicImage::ImageRgba8(destination)))
}

// ── AlphaComposite ──

pub fn op_alpha_composite(
    img: &DynamicImage,
    source: &Arc<Image>,
) -> Result<DynamicImage, PilError> {
    let src_img = source.materialize_for_ops()?;
    if (src_img.width(), src_img.height()) != (img.width(), img.height()) {
        return Err(PilError::ValueError("images do not match".into()));
    }

    // LA mode: composite on native LA canvas, return LA (PIL behavior)
    if matches!(img.color(), crate::raster::ColorType::La8) {
        let mut dest_la = img.to_luma_alpha8();
        let src_la = src_img.to_luma_alpha8();
        let (sw, sh) = src_la.dimensions();
        for py in 0..sh.min(dest_la.height()) {
            for px in 0..sw.min(dest_la.width()) {
                let sp = src_la.get_pixel(px, py);
                let dp = dest_la.get_pixel(px, py);
                let sa = sp[1] as f64 / 255.0;
                let da = dp[1] as f64 / 255.0;
                let out_a = sa + da * (1.0 - sa);
                if out_a <= 0.0 {
                    continue;
                }
                let l = ((sp[0] as f64 * sa + dp[0] as f64 * da * (1.0 - sa)) / out_a)
                    .round()
                    .clamp(0.0, 255.0) as u8;
                let a = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
                dest_la.put_pixel(px, py, crate::raster::LumaA([l, a]));
            }
        }
        return Ok(DynamicImage::ImageLumaA8(dest_la));
    }

    let mut dest_rgba = img.to_rgba8();
    let src_rgba = src_img.to_rgba8();
    let (sw, sh) = src_rgba.dimensions();
    for py in 0..sh.min(dest_rgba.height()) {
        for px in 0..sw.min(dest_rgba.width()) {
            let sp = src_rgba.get_pixel(px, py);
            let dp = dest_rgba.get_pixel(px, py);
            let sa = sp[3] as f64 / 255.0;
            let da = dp[3] as f64 / 255.0;
            let out_a = sa + da * (1.0 - sa);
            if out_a <= 0.0 {
                continue;
            }
            let r = ((sp[0] as f64 * sa + dp[0] as f64 * da * (1.0 - sa)) / out_a)
                .round()
                .clamp(0.0, 255.0) as u8;
            let g = ((sp[1] as f64 * sa + dp[1] as f64 * da * (1.0 - sa)) / out_a)
                .round()
                .clamp(0.0, 255.0) as u8;
            let b = ((sp[2] as f64 * sa + dp[2] as f64 * da * (1.0 - sa)) / out_a)
                .round()
                .clamp(0.0, 255.0) as u8;
            let a = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
            dest_rgba.put_pixel(px, py, crate::raster::Rgba([r, g, b, a]));
        }
    }
    Ok(DynamicImage::ImageRgba8(dest_rgba))
}

// ── Merge ──

pub fn op_merge(
    img: &DynamicImage,
    mode: &ColorMode,
    bands: &[Arc<Image>],
) -> Result<DynamicImage, PilError> {
    let _n_expected = match mode {
        ColorMode::RGB => 3,
        ColorMode::RGBA => 4,
        ColorMode::LA => 2,
        ColorMode::L | ColorMode::Mode1 => 1,
        _ => {
            return Err(PilError::ValueError(format!(
                "Unsupported merge mode: {:?}",
                mode
            )));
        }
    };
    // Get pixel data from each band
    let mut band_pixels: Vec<Vec<u8>> = Vec::new();
    // First band is the current image
    let first_gray = img.to_luma8();
    let (w, h) = first_gray.dimensions();
    band_pixels.push(first_gray.into_raw());
    for band in bands.iter().skip(1) {
        let b_img = band.materialize_for_ops()?;
        let b_gray = b_img.to_luma8();
        band_pixels.push(b_gray.into_raw());
    }
    let n = (w * h) as usize;
    match mode {
        ColorMode::RGB => {
            let mut rgb = vec![0u8; n * 3];
            for i in 0..n {
                rgb[i * 3] = band_pixels[0][i];
                rgb[i * 3 + 1] = band_pixels[1][i];
                rgb[i * 3 + 2] = band_pixels[2][i];
            }
            let img = RgbImage::from_raw(w, h, rgb)
                .ok_or_else(|| PilError::ValueError("merge: buffer error".into()))?;
            Ok(DynamicImage::ImageRgb8(img))
        }
        ColorMode::RGBA => {
            let mut rgba = vec![0u8; n * 4];
            for i in 0..n {
                rgba[i * 4] = band_pixels[0][i];
                rgba[i * 4 + 1] = band_pixels[1][i];
                rgba[i * 4 + 2] = band_pixels[2][i];
                rgba[i * 4 + 3] = band_pixels[3][i];
            }
            let img = RgbaImage::from_raw(w, h, rgba)
                .ok_or_else(|| PilError::ValueError("merge: buffer error".into()))?;
            Ok(DynamicImage::ImageRgba8(img))
        }
        ColorMode::LA => {
            let mut la = vec![0u8; n * 2];
            for i in 0..n {
                la[i * 2] = band_pixels[0][i];
                la[i * 2 + 1] = band_pixels[1][i];
            }
            let img = GrayAlphaImage::from_raw(w, h, la)
                .ok_or_else(|| PilError::ValueError("merge: buffer error".into()))?;
            Ok(DynamicImage::ImageLumaA8(img))
        }
        ColorMode::L | ColorMode::Mode1 => {
            let img = GrayImage::from_raw(w, h, band_pixels.remove(0))
                .ok_or_else(|| PilError::ValueError("merge: buffer error".into()))?;
            Ok(DynamicImage::ImageLuma8(img))
        }
        _ => Err(PilError::ValueError("Unsupported merge mode".into())),
    }
}

// ── BlendModule ──

pub fn op_blend_module(
    img: &DynamicImage,
    other: &Arc<Image>,
    alpha: f64,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let other_img = other.materialize_for_ops()?;
    if (other_img.width(), other_img.height()) != (img.width(), img.height()) {
        return Err(PilError::ValueError("images do not match".into()));
    }
    let a = alpha.clamp(0.0, 1.0);
    // CMYK mode: blend all 4 channels (C,M,Y,K stored as R,G,B,A in Rgba8)
    if explicit_mode == Some("CMYK") {
        let rgba1 = img.to_rgba8();
        let rgba2 = other_img.to_rgba8();
        let (w, h) = (
            rgba1.width().min(rgba2.width()),
            rgba1.height().min(rgba2.height()),
        );
        let mut out = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let p1 = rgba1.get_pixel(x, y);
                let p2 = rgba2.get_pixel(x, y);
                out.put_pixel(
                    x,
                    y,
                    crate::raster::Rgba([
                        (p1[0] as f64 * (1.0 - a) + p2[0] as f64 * a) as u8,
                        (p1[1] as f64 * (1.0 - a) + p2[1] as f64 * a) as u8,
                        (p1[2] as f64 * (1.0 - a) + p2[2] as f64 * a) as u8,
                        (p1[3] as f64 * (1.0 - a) + p2[3] as f64 * a) as u8,
                    ]),
                );
            }
        }
        return Ok(DynamicImage::ImageRgba8(out));
    }

    // Pillow blends every stored channel independently. Converting LA/RGBA
    // through RGB manufactures an opaque alpha channel, which is observable
    // even for transparent black inputs.
    if matches!(img, DynamicImage::ImageLumaA8(_)) {
        let first = img.to_luma_alpha8();
        let second = other_img.to_luma_alpha8();
        let (w, h) = (first.width(), first.height());
        let mut out = GrayAlphaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let p1 = first.get_pixel(x, y);
                let p2 = second.get_pixel(x, y);
                out.put_pixel(
                    x,
                    y,
                    crate::raster::LumaA([
                        (p1[0] as f64 * (1.0 - a) + p2[0] as f64 * a) as u8,
                        (p1[1] as f64 * (1.0 - a) + p2[1] as f64 * a) as u8,
                    ]),
                );
            }
        }
        return Ok(DynamicImage::ImageLumaA8(out));
    }
    if matches!(img, DynamicImage::ImageRgba8(_)) {
        let first = img.to_rgba8();
        let second = other_img.to_rgba8();
        let (w, h) = (first.width(), first.height());
        let mut out = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let p1 = first.get_pixel(x, y);
                let p2 = second.get_pixel(x, y);
                out.put_pixel(
                    x,
                    y,
                    crate::raster::Rgba([
                        (p1[0] as f64 * (1.0 - a) + p2[0] as f64 * a) as u8,
                        (p1[1] as f64 * (1.0 - a) + p2[1] as f64 * a) as u8,
                        (p1[2] as f64 * (1.0 - a) + p2[2] as f64 * a) as u8,
                        (p1[3] as f64 * (1.0 - a) + p2[3] as f64 * a) as u8,
                    ]),
                );
            }
        }
        return Ok(DynamicImage::ImageRgba8(out));
    }

    let rgb1 = img.to_rgb8();
    let rgb2 = other_img.to_rgb8();
    let (w, h) = (
        rgb1.width().min(rgb2.width()),
        rgb1.height().min(rgb2.height()),
    );
    let mut out = RgbImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let p1 = rgb1.get_pixel(x, y);
            let p2 = rgb2.get_pixel(x, y);
            out.put_pixel(
                x,
                y,
                crate::raster::Rgb([
                    (p1[0] as f64 * (1.0 - a) + p2[0] as f64 * a) as u8,
                    (p1[1] as f64 * (1.0 - a) + p2[1] as f64 * a) as u8,
                    (p1[2] as f64 * (1.0 - a) + p2[2] as f64 * a) as u8,
                ]),
            );
        }
    }
    Ok(preserve_mode(img, DynamicImage::ImageRgb8(out)))
}

// ── CompositeModule ──

fn composite_mask(mask: &Arc<Image>, mask_alpha: bool) -> Result<GrayImage, PilError> {
    let materialized = mask.materialize_for_ops()?;
    if !mask_alpha {
        return Ok(materialized.to_luma8());
    }

    let rgba = materialized.to_rgba8();
    Ok(GrayImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        crate::raster::Luma([rgba.get_pixel(x, y)[3]])
    }))
}

#[inline]
fn composite_blend(source: u8, destination: u8, mask: u8) -> u8 {
    let mask = u16::from(mask);
    let inverse = 255u16 - mask;
    // Pillow 12.2.0 Paste.c applies ImagingUtils.h's BLEND/DIV255 macro to
    // every active destination band.
    ((u16::from(source) * mask + u16::from(destination) * inverse + 127) / 255) as u8
}

pub fn op_composite_module(
    img: &DynamicImage,
    other: &Arc<Image>,
    mask: &Arc<Image>,
    mask_alpha: bool,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // PIL composite: copy image2, then paste image1 onto it with mask at (0,0).
    // The output uses image2's size. Smaller images are pasted into the top-left.
    // Paste.c uses the alpha byte for LA/RGBA/RGBa masks and the luma byte for
    // 1/L masks. The choice is captured before backend dispatch.
    let mask_gray = composite_mask(mask, mask_alpha)?;

    // P-mode: composite on palette indices (PIL operates on indices, not colors)
    if explicit_mode == Some("P") {
        let gray1 = img.to_luma8();
        let other_indices = other.materialize_indices()?;
        let gray2 = other_indices.to_luma8();
        let mut out = gray2.clone();
        let overlap_w = gray1.width().min(gray2.width()).min(mask_gray.width());
        let overlap_h = gray1.height().min(gray2.height()).min(mask_gray.height());
        for y in 0..overlap_h {
            for x in 0..overlap_w {
                let value = composite_blend(
                    gray1.get_pixel(x, y)[0],
                    gray2.get_pixel(x, y)[0],
                    mask_gray.get_pixel(x, y)[0],
                );
                out.put_pixel(x, y, crate::raster::Luma([value]));
            }
        }
        return Ok(DynamicImage::ImageLuma8(out));
    }

    let other_img = other.materialize_for_ops()?;

    // RGBA and four-byte compatibility modes (CMYK/I/F) blend every stored
    // band. In particular, RGBA alpha is output data, not merely metadata.
    if matches!(img.color(), crate::raster::ColorType::Rgba8) {
        let rgba1 = img.to_rgba8();
        let rgba2 = other_img.to_rgba8();
        let mut out = rgba2.clone();
        let overlap_w = rgba1.width().min(rgba2.width()).min(mask_gray.width());
        let overlap_h = rgba1.height().min(rgba2.height()).min(mask_gray.height());
        for y in 0..overlap_h {
            for x in 0..overlap_w {
                let p1 = rgba1.get_pixel(x, y);
                let p2 = rgba2.get_pixel(x, y);
                let m = mask_gray.get_pixel(x, y)[0];
                out.put_pixel(
                    x,
                    y,
                    crate::raster::Rgba([
                        composite_blend(p1[0], p2[0], m),
                        composite_blend(p1[1], p2[1], m),
                        composite_blend(p1[2], p2[2], m),
                        composite_blend(p1[3], p2[3], m),
                    ]),
                );
            }
        }
        return Ok(DynamicImage::ImageRgba8(out));
    }
    // LA mode: composite both L and A channels natively
    if matches!(img.color(), crate::raster::ColorType::La8) {
        let la1 = img.to_luma_alpha8();
        let la2 = other_img.to_luma_alpha8();
        let mut out = la2.clone();
        let overlap_w = la1.width().min(la2.width()).min(mask_gray.width());
        let overlap_h = la1.height().min(la2.height()).min(mask_gray.height());
        for y in 0..overlap_h {
            for x in 0..overlap_w {
                let p1 = la1.get_pixel(x, y);
                let p2 = la2.get_pixel(x, y);
                let m = mask_gray.get_pixel(x, y)[0];
                out.put_pixel(
                    x,
                    y,
                    crate::raster::LumaA([
                        composite_blend(p1[0], p2[0], m),
                        composite_blend(p1[1], p2[1], m),
                    ]),
                );
            }
        }
        return Ok(DynamicImage::ImageLumaA8(out));
    }
    let rgb1 = img.to_rgb8();
    let rgb2 = other_img.to_rgb8();
    let mut out = rgb2.clone();
    let overlap_w = rgb1.width().min(rgb2.width()).min(mask_gray.width());
    let overlap_h = rgb1.height().min(rgb2.height()).min(mask_gray.height());
    for y in 0..overlap_h {
        for x in 0..overlap_w {
            let p1 = rgb1.get_pixel(x, y);
            let p2 = rgb2.get_pixel(x, y);
            let m = mask_gray.get_pixel(x, y)[0];
            out.put_pixel(
                x,
                y,
                crate::raster::Rgb([
                    composite_blend(p1[0], p2[0], m),
                    composite_blend(p1[1], p2[1], m),
                    composite_blend(p1[2], p2[2], m),
                ]),
            );
        }
    }
    Ok(preserve_mode(img, DynamicImage::ImageRgb8(out)))
}

// ── Eval ──

pub fn op_eval(img: &DynamicImage, lut: &[u8]) -> Result<DynamicImage, PilError> {
    let n_bands = match img.color() {
        crate::raster::ColorType::L8 | crate::raster::ColorType::L16 => 1,
        crate::raster::ColorType::La8 | crate::raster::ColorType::La16 => 2,
        crate::raster::ColorType::Rgb8 | crate::raster::ColorType::Rgb16 => 3,
        _ => 4,
    };
    // PIL requires EXACTLY 256 * n_bands lut entries
    let expected = 256 * n_bands;
    if lut.len() != expected {
        return Err(PilError::ValueError("wrong number of lut entries".into()));
    }
    let band_luts: Vec<&[u8]> = (0..n_bands).map(|b| &lut[b * 256..(b + 1) * 256]).collect();
    // For single-channel images (mode "1", "L", "P"), operate on Luma8 directly
    // to avoid precision loss through RGBA round-trip.
    if n_bands == 1 {
        let gray = img.to_luma8();
        let (w, h) = gray.dimensions();
        let mut out = GrayImage::new(w, h);
        for (op, ip) in out.pixels_mut().zip(gray.pixels()) {
            let idx = ip[0] as usize;
            op[0] = *band_luts[0].get(idx).unwrap_or(&ip[0]);
        }
        return Ok(DynamicImage::ImageLuma8(out));
    }
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut out = RgbaImage::new(w, h);
    for (op, ip) in out.pixels_mut().zip(rgba.pixels()) {
        for b in 0..4 {
            let idx = ip[b] as usize;
            let band = b.min(band_luts.len() - 1);
            op[b] = *band_luts[band].get(idx).unwrap_or(&ip[b]);
        }
    }
    Ok(preserve_mode(img, DynamicImage::ImageRgba8(out)))
}

// ── EffectNoise ──

pub fn op_effect_noise(img: &DynamicImage, sigma: f64) -> Result<DynamicImage, PilError> {
    // Pillow 12.2.0 `src/libImaging/Effects.c:75-114` uses a polar
    // Box-Muller transform and always returns L mode. Its `nextok` flag is
    // never set, so every accepted pixel consumes one pair and discards the
    // second deviate.
    let (w, h) = (img.width(), img.height());
    let mut out = GrayImage::new(w, h);
    let mut rng = DarwinRand::default();
    // `_effect_noise` parses sigma with PyArg's `f` conversion before passing
    // it to ImagingEffectNoise, so round it to FLOAT32 once at the boundary.
    let sigma = f64::from(sigma as f32);
    const RAND_MAX_F64: f64 = 2147483647.0;
    for pixel in out.pixels_mut() {
        let (v1, radius) = loop {
            // Exact match to PIL:
            //   v1 = rand() * (2.0 / RAND_MAX) - 1.0;
            //   v2 = rand() * (2.0 / RAND_MAX) - 1.0;
            let v1 = rng.next() as f64 * (2.0 / RAND_MAX_F64) - 1.0;
            let v2 = rng.next() as f64 * (2.0 / RAND_MAX_F64) - 1.0;
            let radius = v1 * v1 + v2 * v2;
            if radius < 1.0 {
                break (v1, radius);
            }
        };
        // factor = sqrt(-2.0 * log(radius) / radius)
        let factor = (-2.0 * radius.ln() / radius).sqrt();
        let this = factor * v1;
        // PIL: CLIP8(128 + sigma * this)
        // CLIP8: (v) <= 0 ? 0 : (v) >= 255.0 ? 255 : (UINT8)(v)
        // Cast truncates toward zero (no rounding).
        let v = 128.0 + sigma * this;
        pixel[0] = if v <= 0.0 {
            0
        } else if v >= 255.0 {
            255
        } else {
            v as u8
        };
    }
    Ok(DynamicImage::ImageLuma8(out))
}

// ── PointOp (lookup table) ──

pub fn op_point(img: &DynamicImage, lut: &[u8]) -> Result<DynamicImage, PilError> {
    let n_bands = match img.color() {
        crate::raster::ColorType::L8 | crate::raster::ColorType::L16 => 1,
        crate::raster::ColorType::La8 | crate::raster::ColorType::La16 => 2,
        crate::raster::ColorType::Rgb8 | crate::raster::ColorType::Rgb16 => 3,
        _ => 4,
    };
    // PIL requires EXACTLY 256 * n_bands lut entries
    let expected = 256 * n_bands;
    if lut.len() != expected {
        return Err(PilError::ValueError("wrong number of lut entries".into()));
    }
    let band_luts: Vec<&[u8]> = (0..n_bands).map(|b| &lut[b * 256..(b + 1) * 256]).collect();
    // For single-channel images, operate on Luma8 directly
    // to avoid precision loss through RGBA round-trip.
    if n_bands == 1 {
        let gray = img.to_luma8();
        let (w, h) = gray.dimensions();
        let mut out = GrayImage::new(w, h);
        for (op, ip) in out.pixels_mut().zip(gray.pixels()) {
            let idx = ip[0] as usize;
            op[0] = *band_luts[0].get(idx).unwrap_or(&ip[0]);
        }
        return Ok(DynamicImage::ImageLuma8(out));
    }
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut out = RgbaImage::new(w, h);
    for (op, ip) in out.pixels_mut().zip(rgba.pixels()) {
        for b in 0..4 {
            let idx = ip[b] as usize;
            let band = b.min(band_luts.len() - 1);
            op[b] = *band_luts[band].get(idx).unwrap_or(&ip[b]);
        }
    }
    Ok(preserve_mode(img, DynamicImage::ImageRgba8(out)))
}

// ── Transform ──

/// Apply an affine transform working on the native number of channels.
/// When `nearest` is true, uses nearest-neighbor sampling.
fn transform_affine_generic(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    aff_a: f64,
    aff_b: f64,
    aff_c: f64,
    aff_d: f64,
    aff_e: f64,
    aff_f: f64,
    fill: Option<(u8, u8, u8, u8)>,
    nearest: bool,
) -> Result<DynamicImage, PilError> {
    let channels = img.color().channel_count() as usize;
    let raw = img.as_bytes();
    let (sw, sh) = img.dimensions();
    let fill_color = fill.unwrap_or((0, 0, 0, 255));

    let mut out = vec![0u8; (dst_w * dst_h) as usize * channels];

    for dy in 0..dst_h {
        for dx in 0..dst_w {
            let sx = aff_a * dx as f64 + aff_b * dy as f64 + aff_c;
            let sy = aff_d * dx as f64 + aff_e * dy as f64 + aff_f;
            let out_idx = (dy * dst_w + dx) as usize * channels;

            if nearest {
                let ix = (sx + 0.5).floor() as i64;
                let iy = (sy + 0.5).floor() as i64;
                if ix >= 0 && ix < sw as i64 && iy >= 0 && iy < sh as i64 {
                    let in_idx = (iy as u32 * sw + ix as u32) as usize * channels;
                    out[out_idx..out_idx + channels]
                        .copy_from_slice(&raw[in_idx..in_idx + channels]);
                } else {
                    for ch in 0..channels.min(4) {
                        out[out_idx + ch] = match ch {
                            0 => fill_color.0,
                            1 => fill_color.1,
                            2 => fill_color.2,
                            _ => fill_color.3,
                        };
                    }
                }
            } else if sx >= 0.0 && sx < sw as f64 && sy >= 0.0 && sy < sh as f64 {
                let x0 = sx.floor() as u32;
                let y0 = sy.floor() as u32;
                let x1 = (x0 + 1).min(sw - 1);
                let y1 = (y0 + 1).min(sh - 1);
                let fx = sx - x0 as f64;
                let fy = sy - y0 as f64;
                for ch in 0..channels {
                    let p00 = raw[(y0 * sw + x0) as usize * channels + ch] as f64;
                    let p10 = raw[(y0 * sw + x1) as usize * channels + ch] as f64;
                    let p01 = raw[(y1 * sw + x0) as usize * channels + ch] as f64;
                    let p11 = raw[(y1 * sw + x1) as usize * channels + ch] as f64;
                    let v = (1.0 - fx) * (1.0 - fy) * p00
                        + fx * (1.0 - fy) * p10
                        + (1.0 - fx) * fy * p01
                        + fx * fy * p11;
                    out[out_idx + ch] = v.round() as u8;
                }
            } else {
                for ch in 0..channels.min(4) {
                    out[out_idx + ch] = match ch {
                        0 => fill_color.0,
                        1 => fill_color.1,
                        2 => fill_color.2,
                        _ => fill_color.3,
                    };
                }
            }
        }
    }

    Ok(match channels {
        1 => DynamicImage::ImageLuma8(GrayImage::from_raw(dst_w, dst_h, out).ok_or_else(|| {
            PilError::InternalError("transform_affine L buffer shape mismatch".to_string())
        })?),
        2 => DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(dst_w, dst_h, out).ok_or_else(
            || PilError::InternalError("transform_affine LA buffer shape mismatch".to_string()),
        )?),
        3 => DynamicImage::ImageRgb8(RgbImage::from_raw(dst_w, dst_h, out).ok_or_else(|| {
            PilError::InternalError("transform_affine RGB buffer shape mismatch".to_string())
        })?),
        4 => DynamicImage::ImageRgba8(RgbaImage::from_raw(dst_w, dst_h, out).ok_or_else(|| {
            PilError::InternalError("transform_affine RGBA buffer shape mismatch".to_string())
        })?),
        _ => unreachable!(),
    })
}

pub fn op_transform(
    img: &DynamicImage,
    w: u32,
    h: u32,
    method: &TransformMethod,
    data: &[f64],
    filter: &ResampleFilter,
    fill: Option<(u8, u8, u8, u8)>,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    match method {
        TransformMethod::Affine => {
            if data.len() < 6 {
                return Err(PilError::ValueError(
                    "Affine transform needs 6 coefficients".into(),
                ));
            }
            let (aff_a, aff_b, aff_c, aff_d, aff_e, aff_f) =
                (data[0], data[1], data[2], data[3], data[4], data[5]);
            let p_mode = explicit_mode == Some("P") || explicit_mode == Some("1");
            let i_f_mode = explicit_mode == Some("I") || explicit_mode == Some("F");
            let use_nearest = matches!(filter, ResampleFilter::Nearest) || p_mode || i_f_mode;

            let result = transform_affine_generic(
                img,
                w,
                h,
                aff_a,
                aff_b,
                aff_c,
                aff_d,
                aff_e,
                aff_f,
                fill,
                use_nearest,
            )?;
            Ok(preserve_mode(img, result))
        }
        &TransformMethod::Mesh => {
            if data.len() < 12 {
                return Err(PilError::ValueError(
                    "Mesh transform needs at least 12 values per element".into(),
                ));
            }
            let result = transform_mesh(img, w, h, data, fill);
            Ok(preserve_mode(img, result?))
        }
        &TransformMethod::Perspective | &TransformMethod::Quad => Err(
            PilError::NotImplementedError(format!("Transform {:?} not yet implemented", method)),
        ),
    }
}

// ── PutPixel ──

pub fn op_put_pixel(
    img: &DynamicImage,
    x: u32,
    y: u32,
    color: (u8, u8, u8, u8),
) -> Result<DynamicImage, PilError> {
    let (w, h) = (img.width(), img.height());
    if x >= w || y >= h {
        return Err(PilError::IndexError("image index out of range".into()));
    }
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
    let out = match img.color() {
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
            let rgb = img.to_rgb8();
            let mut rgba = RgbaImage::new(rgb.width(), rgb.height());
            for (o, i) in rgba.pixels_mut().zip(rgb.pixels()) {
                o[0] = i[0];
                o[1] = i[1];
                o[2] = i[2];
                o[3] = alpha;
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
    };
    out
}

// ── Color3DLUT — trilinear interpolation (matching PIL's _imaging C code) ──

fn table_index_3d(x: usize, y: usize, z: usize, sx: usize, sxy: usize) -> usize {
    x + y * sx + z * sxy
}

fn color_lut_interpolate(a: i16, b: i16, shift: i32) -> i16 {
    const SHIFT_BITS: i32 = 15;
    let value = (i64::from(a) * i64::from((1 << SHIFT_BITS) - shift)
        + i64::from(b) * i64::from(shift))
        >> SHIFT_BITS;
    value as i16
}

pub fn op_color3dlut(
    img: &DynamicImage,
    size: (u32, u32, u32),
    table: &[f64],
    channels: u32,
    source_mode: PixelMode,
    target_mode: PixelMode,
) -> Result<DynamicImage, PilError> {
    let (sx, sy, sz) = (size.0 as usize, size.1 as usize, size.2 as usize);
    let ch = channels as usize;
    let sxy = sx * sy;

    let (w, h) = img.dimensions();
    const PRECISION_BITS: i32 = 4;
    const SCALE_BITS: u32 = 18;
    const SCALE_MASK: u32 = (1 << SCALE_BITS) - 1;
    const SHIFT_BITS: u32 = 15;
    let scales = [
        ((sx - 1) as f64 / 255.0 * f64::from(1 << SCALE_BITS)) as u32,
        ((sy - 1) as f64 / 255.0 * f64::from(1 << SCALE_BITS)) as u32,
        ((sz - 1) as f64 / 255.0 * f64::from(1 << SCALE_BITS)) as u32,
    ];
    // Pillow converts Python sequences to float32 before preparing signed
    // 12.4 fixed-point entries in `_prepare_lut_table`.
    let prepared: Vec<i16> = table
        .iter()
        .map(|value| {
            let item = *value as f32;
            let scaled = item * ((255 << PRECISION_BITS) as f32);
            if scaled >= i16::MAX as f32 - 0.5 {
                i16::MAX
            } else if scaled <= i16::MIN as f32 + 0.5 {
                i16::MIN
            } else if item < 0.0 {
                (scaled - 0.5) as i16
            } else {
                (scaled + 0.5) as i16
            }
        })
        .collect();

    let mut out = vec![0u8; (w * h) as usize * 4];

    for y in 0..h {
        for x in 0..w {
            let out_idx = ((y * w + x) as usize) * 4;
            let px = img.get_pixel(x, y).0;

            let indices = [
                u32::from(px[0]) * scales[0],
                u32::from(px[1]) * scales[1],
                u32::from(px[2]) * scales[2],
            ];
            let shifts =
                indices.map(|index| ((SCALE_MASK & index) >> (SCALE_BITS - SHIFT_BITS)) as i32);
            let base = table_index_3d(
                (indices[0] >> SCALE_BITS) as usize,
                (indices[1] >> SCALE_BITS) as usize,
                (indices[2] >> SCALE_BITS) as usize,
                sx,
                sxy,
            ) * ch;

            for c in 0..ch {
                let left_left =
                    color_lut_interpolate(prepared[base + c], prepared[base + ch + c], shifts[0]);
                let left_right = color_lut_interpolate(
                    prepared[base + sx * ch + c],
                    prepared[base + sx * ch + ch + c],
                    shifts[0],
                );
                let left = color_lut_interpolate(left_left, left_right, shifts[1]);
                let right_left = color_lut_interpolate(
                    prepared[base + sxy * ch + c],
                    prepared[base + sxy * ch + ch + c],
                    shifts[0],
                );
                let right_right = color_lut_interpolate(
                    prepared[base + sxy * ch + sx * ch + c],
                    prepared[base + sxy * ch + sx * ch + ch + c],
                    shifts[0],
                );
                let right = color_lut_interpolate(right_left, right_right, shifts[1]);
                let result = color_lut_interpolate(left, right, shifts[2]);
                out[out_idx + c] = ((i32::from(result) + (1 << (PRECISION_BITS - 1)))
                    >> PRECISION_BITS)
                    .clamp(0, 255) as u8;
            }
            if ch == 3 {
                out[out_idx + 3] = if source_mode.channels() == 4 {
                    px[3]
                } else {
                    255
                };
            }
        }
    }

    let rgba = RgbaImage::from_raw(w, h, out)
        .ok_or_else(|| PilError::InternalError("color3dlut output size mismatch".into()))?;
    match target_mode {
        PixelMode::RGB => Ok(DynamicImage::ImageRgb8(crate::raster::RgbImage::from_fn(
            w,
            h,
            |x, y| {
                let pixel = rgba.get_pixel(x, y);
                crate::raster::Rgb([pixel[0], pixel[1], pixel[2]])
            },
        ))),
        PixelMode::RGBA | PixelMode::CMYK => Ok(DynamicImage::ImageRgba8(rgba)),
        _ => Err(PilError::InternalError(
            "validated color3dlut target mode was not RGB, RGBA, or CMYK".into(),
        )),
    }
}

// ── MESH transform — piecewise bilinear quad mapping ──

pub fn transform_mesh(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    mesh_data: &[f64],
    fill: Option<(u8, u8, u8, u8)>,
) -> Result<DynamicImage, PilError> {
    let channels = img.color().channel_count() as usize;
    let raw = img.as_bytes();
    let (sw, sh) = img.dimensions();
    let sw_f = sw as f64;
    let sh_f = sh as f64;
    let fill_color = fill.unwrap_or((0, 0, 0, 255));

    let mut out = vec![0u8; (dst_w * dst_h) as usize * 4];
    // Initialize output with fill color
    for y in 0..dst_h as usize {
        for x in 0..dst_w as usize {
            let idx = (y * dst_w as usize + x) * 4;
            out[idx] = fill_color.0;
            out[idx + 1] = fill_color.1;
            out[idx + 2] = fill_color.2;
            out[idx + 3] = fill_color.3;
        }
    }

    // Process each mesh element
    let num_elements = mesh_data.len() / 12;
    for elem in 0..num_elements {
        let base = elem * 12;
        let x0_d = mesh_data[base] as i32;
        let y0_d = mesh_data[base + 1] as i32;
        let x1_d = mesh_data[base + 2] as i32;
        let y1_d = mesh_data[base + 3] as i32;
        let x0_s = mesh_data[base + 4];
        let y0_s = mesh_data[base + 5];
        let x1_s = mesh_data[base + 6];
        let y1_s = mesh_data[base + 7];
        let x2_s = mesh_data[base + 8];
        let y2_s = mesh_data[base + 9];
        let x3_s = mesh_data[base + 10];
        let y3_s = mesh_data[base + 11];

        // Clamp bounding box to output image dimensions to prevent CPU DoS
        // from attacker-controlled mesh data with extreme bx/bw values.
        let bx0 = x0_d.max(0).min(dst_w as i32);
        let by0 = y0_d.max(0).min(dst_h as i32);
        let bx1 = x1_d.max(1).min(dst_w as i32);
        let by1 = y1_d.max(1).min(dst_h as i32);
        let bw = (bx1 - bx0).max(1) as f64;
        let bh = (by1 - by0).max(1) as f64;

        for dy in by0..by1 {
            let v = (dy - y0_d) as f64 / bh;
            for dx in bx0..bx1 {
                let u = (dx - x0_d) as f64 / bw;

                // PIL bilinear mapping: quad[0]=top-left, quad[1]=bottom-left,
                // quad[2]=bottom-right, quad[3]=top-right (counter-clockwise)
                let sx = (1.0 - u) * (1.0 - v) * x0_s
                    + u * (1.0 - v) * x3_s
                    + u * v * x2_s
                    + (1.0 - u) * v * x1_s;
                let sy = (1.0 - u) * (1.0 - v) * y0_s
                    + u * (1.0 - v) * y3_s
                    + u * v * y2_s
                    + (1.0 - u) * v * y1_s;

                if sx >= 0.0 && sx < sw_f && sy >= 0.0 && sy < sh_f {
                    // NEAREST sampling for identity mesh parity
                    let ix = (sx + 0.5).floor() as u32;
                    let iy = (sy + 0.5).floor() as u32;
                    let src_idx = ((iy * sw + ix) as usize) * channels;
                    let out_idx = ((dy as u32 * dst_w + dx as u32) as usize) * 4;
                    for c in 0..channels {
                        out[out_idx + c] = raw[src_idx + c];
                    }
                    if channels < 4 {
                        out[out_idx + 3] = 255;
                    }
                }
            }
        }
    }

    RgbaImage::from_raw(dst_w, dst_h, out)
        .map(DynamicImage::ImageRgba8)
        .ok_or_else(|| {
            PilError::InternalError("transform_mesh RGBA buffer shape mismatch".to_string())
        })
}

/// Generate a Mandelbrot set fractal image.
/// For each pixel (px, py) in w×h, maps to complex plane via extent [x0,y0]→[x1,y1],
/// iterates z = z² + c up to `quality` times, outputs iteration count as grayscale.
pub fn op_effect_mandelbrot(
    w: u32,
    h: u32,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    quality: u32,
) -> Result<DynamicImage, PilError> {
    let mut gray = crate::raster::GrayImage::new(w, h);
    let dx = (x1 - x0) / w as f64;
    let dy = (y1 - y0) / h as f64;
    for py in 0..h {
        let cy = y0 + py as f64 * dy;
        for px in 0..w {
            let cx = x0 + px as f64 * dx;
            let (mut zx, mut zy) = (0.0f64, 0.0f64);
            let mut iter: u32 = 0;
            while iter < quality {
                let zx2 = zx * zx - zy * zy + cx;
                let zy2 = 2.0 * zx * zy + cy;
                zx = zx2;
                zy = zy2;
                if zx * zx + zy * zy > 4.0 {
                    break;
                }
                iter += 1;
            }
            let val = (iter * 255 / quality.max(1)) as u8;
            gray.put_pixel(px, py, crate::raster::Luma([val]));
        }
    }
    Ok(DynamicImage::ImageLuma8(gray))
}

#[cfg(test)]
mod tests {
    use super::{DarwinRand, op_effect_noise};
    use crate::raster::{DynamicImage, GrayImage};

    #[test]
    fn darwin_rand_matches_pillow_oracle_sequence() {
        let mut rng = DarwinRand::default();

        assert_eq!(
            [rng.next(), rng.next(), rng.next(), rng.next(), rng.next()],
            [
                16_807,
                282_475_249,
                1_622_650_073,
                984_943_658,
                1_144_108_930
            ]
        );
    }

    #[test]
    fn effect_noise_matches_pillow_pair_consumption_and_l_mode() {
        let input = DynamicImage::ImageLuma8(GrayImage::new(16, 1));

        let output = op_effect_noise(&input, 10.0).expect("noise generation must succeed");

        assert!(matches!(&output, DynamicImage::ImageLuma8(_)));
        assert_eq!(
            output.as_bytes(),
            &[
                0x90, 0x81, 0x7c, 0x81, 0x68, 0x79, 0x7e, 0x78, 0x81, 0x8c, 0x79, 0x78, 0x82, 0x8b,
                0x86, 0x88,
            ]
        );
    }
}
