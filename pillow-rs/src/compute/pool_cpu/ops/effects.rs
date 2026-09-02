// ── Effects + Module fns + Point + Mutating operations extracted from image.rs execute_op() ──

use crate::error::PilError;
use crate::image::{Image, preserve_mode};
use crate::ops::pil_resize::{premultiply_alpha, unpremultiply_alpha};
use crate::pipeline::{ColorMode, PixelMode, ResampleFilter, TransformMethod};
use crate::raster::{
    DynamicImage, GenericImageView, GrayAlphaImage, GrayImage, ImageBuffer, RgbImage, RgbaImage,
};
use std::sync::{Arc, Mutex, OnceLock};

#[cfg(feature = "parallel")]
const EFFECT_PARALLEL_PIXEL_THRESHOLD: usize = 512 * 512;

fn apply_effect_rows<F>(
    bytes: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    transform: F,
) where
    F: Fn(usize, &mut [u8]) + Send + Sync,
{
    if bytes.is_empty() || width == 0 || height == 0 {
        return;
    }
    let stride = width.saturating_mul(channels);
    #[cfg(feature = "parallel")]
    if width.saturating_mul(height) >= EFFECT_PARALLEL_PIXEL_THRESHOLD {
        crate::par_rows_mut!(bytes, stride, height, |_row_start, _row_end, y, row| {
            transform(y as usize, row);
        });
    } else {
        for (y, row) in bytes.chunks_exact_mut(stride).take(height).enumerate() {
            transform(y, row);
        }
    }
    #[cfg(not(feature = "parallel"))]
    for (y, row) in bytes.chunks_exact_mut(stride).take(height).enumerate() {
        transform(y, row);
    }
}

#[inline]
fn blend_row(first: &[u8], second: &[u8], output: &mut [u8], alpha: f64) {
    for ((destination, &left), &right) in output.iter_mut().zip(first.iter()).zip(second.iter()) {
        *destination = (left as f64 * (1.0 - alpha) + right as f64 * alpha).clamp(0.0, 255.0) as u8;
    }
}

// ── Darwin-compatible PRNG ───────────────────────────────────────────────
//
// Pillow delegates effect_spread/effect_noise randomness to process-global
// libc rand(). The pinned macOS/Darwin Pillow 12.2.0 oracle uses the
// Park-Miller sequence, whose process-default state is the same as srand(1).
// Keep the generator independent of libc so native and WASM builds reproduce
// the oracle without runtime FFI, while retaining the shared state and call
// consumption that the public APIs expose.

pub(crate) struct DarwinRand {
    state: u32,
}

impl Default for DarwinRand {
    fn default() -> Self {
        Self { state: 1 }
    }
}

impl DarwinRand {
    pub(crate) fn next(&mut self) -> u32 {
        const MULTIPLIER: u64 = 16_807;
        const MODULUS: u64 = 2_147_483_647;

        self.state = ((u64::from(self.state) * MULTIPLIER) % MODULUS) as u32;
        self.state
    }
}

static PROCESS_RNG: OnceLock<Mutex<DarwinRand>> = OnceLock::new();

fn process_rng() -> &'static Mutex<DarwinRand> {
    PROCESS_RNG.get_or_init(|| Mutex::new(DarwinRand::default()))
}

/// Run a backend's scalar RNG control section while preserving Pillow's
/// process-global random stream. Pixel consumers may batch the values into a
/// native vector kernel without taking a separate or reordered RNG stream.
pub(crate) fn with_process_rng<T>(f: impl FnOnce(&mut DarwinRand) -> T) -> Result<T, PilError> {
    let mut rng = process_rng()
        .lock()
        .map_err(|_| PilError::InternalError("effect RNG lock poisoned".into()))?;
    Ok(f(&mut rng))
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
    let mapping = effect_spread_mapping(img.width(), img.height(), distance)?;
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
    let mut out_pixels = vec![0u8; input_pixels.len()];
    for (destination, &source) in mapping.iter().enumerate() {
        let source = source as usize;
        let destination_base = destination * stride;
        let source_base = source * stride;
        out_pixels[destination_base..destination_base + stride]
            .copy_from_slice(&input_pixels[source_base..source_base + stride]);
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

/// Build the exact per-pixel relocation map used by Pillow's spread effect.
/// The map is a control-plane representation of the C scatter loop: every
/// source pixel consumes two values from the process-global Park–Miller stream,
/// and each in-bounds pair updates both affected output locations from the
/// immutable input.  A GPU/other backend can then gather through this map
/// without reproducing the inherently order-dependent scatter writes.
pub(crate) fn effect_spread_mapping(
    width: u32,
    height: u32,
    distance: u32,
) -> Result<Vec<u32>, PilError> {
    let pixels = usize::try_from(width)
        .ok()
        .and_then(|w| usize::try_from(height).ok().and_then(|h| w.checked_mul(h)))
        .ok_or_else(|| PilError::ValueError("effect_spread image dimensions overflow".into()))?;
    let mut mapping = (0..pixels)
        .map(|index| u32::try_from(index).unwrap_or(u32::MAX))
        .collect::<Vec<_>>();
    if distance == 0 || pixels == 0 {
        return Ok(mapping);
    }
    let d = i32::try_from(distance)
        .map_err(|_| PilError::ValueError("effect_spread distance is too large".into()))?;
    let half_d = d / 2;
    let width_i32 = i32::try_from(width)
        .map_err(|_| PilError::ValueError("effect_spread width is too large".into()))?;
    let height_i32 = i32::try_from(height)
        .map_err(|_| PilError::ValueError("effect_spread height is too large".into()))?;
    with_process_rng(|rng| {
        for y in 0..height_i32 {
            for x in 0..width_i32 {
                let source = (y * width_i32 + x) as usize;
                let xx = x + (rng.next() as i32 % d) - half_d;
                let yy = y + (rng.next() as i32 % d) - half_d;
                if xx >= 0 && xx < width_i32 && yy >= 0 && yy < height_i32 {
                    let destination = (yy * width_i32 + xx) as usize;
                    mapping[destination] = u32::try_from(source).unwrap_or(u32::MAX);
                    mapping[source] = u32::try_from(destination).unwrap_or(u32::MAX);
                }
            }
        }
    })?;
    Ok(mapping)
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
    // Image::paste validates mask dimensions against the paste region before
    // queuing this operation, so every loop coordinate is valid for the mask.
    // Keep malformed PipelineOp fallback arms out of this public-input path.

    if img.color() == crate::raster::ColorType::L16 {
        // Pillow's Paste.c keeps I;16 samples as unsigned 16-bit values.
        // The general RGBA8 path below is correct for byte-oriented modes but
        // would truncate the high byte of an I;16 image.
        let source_luma = src_img.to_luma16();
        let mut destination = img.to_luma16();
        enum Luma16PasteMask {
            Luma(crate::raster::GrayImage),
            Alpha(crate::raster::RgbaImage),
        }
        let mask_pixels = match mask {
            Some(mask_image) => {
                let materialized = mask_image.materialize()?;
                if mask_alpha {
                    Some(Luma16PasteMask::Alpha(materialized.to_rgba8()))
                } else {
                    Some(Luma16PasteMask::Luma(materialized.to_luma8()))
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
                let source_value = source_luma.get_pixel(source_x, source_y)[0];
                let Some(mask_image) = mask_pixels.as_ref() else {
                    destination.put_pixel(dest_x, dest_y, crate::raster::Luma([source_value]));
                    continue;
                };
                let mask_value = match mask_image {
                    Luma16PasteMask::Luma(pixels) => pixels.get_pixel(source_x, source_y)[0],
                    Luma16PasteMask::Alpha(pixels) => pixels.get_pixel(source_x, source_y)[3],
                };
                if mask_value == 0 {
                    continue;
                }
                if mask_value == 255 {
                    destination.put_pixel(dest_x, dest_y, crate::raster::Luma([source_value]));
                    continue;
                }

                let destination_value = destination.get_pixel(dest_x, dest_y)[0];
                let mask = u32::from(mask_value);
                let inverse = 255 - mask;
                let blended = ((u32::from(source_value) * mask
                    + u32::from(destination_value) * inverse
                    + 127)
                    / 255) as u16;
                destination.put_pixel(dest_x, dest_y, crate::raster::Luma([blended]));
            }
        }

        return Ok(DynamicImage::ImageLuma16(destination));
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
                PasteMask::Luma(pixels) => pixels.get_pixel(source_x, source_y)[0],
                PasteMask::Alpha(pixels) => pixels.get_pixel(source_x, source_y)[3],
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

#[inline]
fn alpha_composite_div255(value: u32) -> u32 {
    // Pillow's ImagingUtils.h SHIFTFORDIV255 macro. AlphaComposite.c relies
    // on this fixed-point division rather than real-number rounding.
    ((value >> 8) + value) >> 8
}

#[inline]
fn alpha_composite_channel(
    source: u32,
    destination: u32,
    source_alpha: u32,
    destination_alpha: u32,
) -> u8 {
    if source_alpha == 0 {
        return destination.min(255) as u8;
    }
    let blend = destination_alpha * (255 - source_alpha);
    let out_alpha_255 = source_alpha * 255 + blend;
    let coefficient_source = source_alpha * 255 * 255 * (1 << 7) / out_alpha_255;
    let coefficient_destination = (255 << 7) - coefficient_source;
    let blended = source * coefficient_source + destination * coefficient_destination;
    (alpha_composite_div255(blended + (0x80 << 7)) >> 7).min(255) as u8
}

#[inline]
fn alpha_composite_alpha(source_alpha: u32, destination_alpha: u32) -> u8 {
    if source_alpha == 0 {
        return destination_alpha.min(255) as u8;
    }
    let blend = destination_alpha * (255 - source_alpha);
    let out_alpha_255 = source_alpha * 255 + blend;
    alpha_composite_div255(out_alpha_255 + 0x80).min(255) as u8
}

pub fn op_alpha_composite(
    img: &DynamicImage,
    source: &Arc<Image>,
) -> Result<DynamicImage, PilError> {
    let src_img = source.materialize_for_ops()?;
    // Image::alpha_composite validates mode and dimensions before queuing this
    // operation; this executor only receives matching source and destination
    // images from supported public inputs.

    // LA mode: composite on native LA canvas, return LA (PIL behavior)
    if matches!(img.color(), crate::raster::ColorType::La8) {
        let mut dest_la = img.to_luma_alpha8();
        let src_la = src_img.to_luma_alpha8();
        let (sw, sh) = src_la.dimensions();
        let width = sw.min(dest_la.width()) as usize;
        let height = sh.min(dest_la.height()) as usize;
        let source = src_la.as_raw();
        let source_stride = sw as usize * 2;
        apply_effect_rows(dest_la.as_mut(), width, height, 2, |row_index, row| {
            let source_start = row_index * source_stride;
            let source_row = &source[source_start..source_start + width * 2];
            for (sp, dp) in source_row.chunks_exact(2).zip(row.chunks_exact_mut(2)) {
                let source_alpha = u32::from(sp[1]);
                let destination_alpha = u32::from(dp[1]);
                dp[0] = alpha_composite_channel(
                    u32::from(sp[0]),
                    u32::from(dp[0]),
                    source_alpha,
                    destination_alpha,
                );
                dp[1] = alpha_composite_alpha(source_alpha, destination_alpha);
            }
        });
        return Ok(DynamicImage::ImageLumaA8(dest_la));
    }

    let mut dest_rgba = img.to_rgba8();
    let src_rgba = src_img.to_rgba8();
    let (sw, sh) = src_rgba.dimensions();
    let width = sw.min(dest_rgba.width()) as usize;
    let height = sh.min(dest_rgba.height()) as usize;
    let source = src_rgba.as_raw();
    let source_stride = sw as usize * 4;
    apply_effect_rows(dest_rgba.as_mut(), width, height, 4, |row_index, row| {
        let source_start = row_index * source_stride;
        let source_row = &source[source_start..source_start + width * 4];
        for (sp, dp) in source_row.chunks_exact(4).zip(row.chunks_exact_mut(4)) {
            let source_alpha = u32::from(sp[3]);
            let destination_alpha = u32::from(dp[3]);
            dp[0] = alpha_composite_channel(
                u32::from(sp[0]),
                u32::from(dp[0]),
                source_alpha,
                destination_alpha,
            );
            dp[1] = alpha_composite_channel(
                u32::from(sp[1]),
                u32::from(dp[1]),
                source_alpha,
                destination_alpha,
            );
            dp[2] = alpha_composite_channel(
                u32::from(sp[2]),
                u32::from(dp[2]),
                source_alpha,
                destination_alpha,
            );
            dp[3] = alpha_composite_alpha(source_alpha, destination_alpha);
        }
    });
    Ok(DynamicImage::ImageRgba8(dest_rgba))
}

// ── Merge ──

pub fn op_merge(
    img: &DynamicImage,
    mode: &ColorMode,
    bands: &[Arc<Image>],
    logical_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // Image::merge validates the supported mode, band count, band modes, and
    // dimensions before queuing this operation. The executor therefore only
    // handles the validated mode matrix below.
    // A single I/F band is already the complete native destination buffer.
    // Pillow's ImagingMerge accepts only the same typed source mode here, so
    // retaining the first buffer preserves signed integer/IEEE-754 bytes
    // exactly instead of reducing them through an 8-bit luma conversion.
    if matches!(mode, ColorMode::I | ColorMode::F) {
        return Ok(img.clone());
    }

    // Get pixel data from each byte band.
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
        ColorMode::RGB | ColorMode::YCbCr | ColorMode::HSV => {
            let mut rgb = vec![0u8; n * 3];
            let lab = logical_mode == Some("LAB");
            apply_effect_rows(&mut rgb, w as usize, h as usize, 3, |row_index, row| {
                let source_start = row_index * w as usize;
                for (pixel_index, pixel) in row.chunks_exact_mut(3).enumerate() {
                    let source_index = source_start + pixel_index;
                    pixel[0] = band_pixels[0][source_index];
                    pixel[1] = if lab {
                        band_pixels[1][source_index].wrapping_add(128)
                    } else {
                        band_pixels[1][source_index]
                    };
                    pixel[2] = if lab {
                        band_pixels[2][source_index].wrapping_add(128)
                    } else {
                        band_pixels[2][source_index]
                    };
                }
            });
            let img = RgbImage::from_raw(w, h, rgb)
                .ok_or_else(|| PilError::ValueError("merge: buffer error".into()))?;
            Ok(DynamicImage::ImageRgb8(img))
        }
        ColorMode::RGBA => {
            let mut rgba = vec![0u8; n * 4];
            apply_effect_rows(&mut rgba, w as usize, h as usize, 4, |row_index, row| {
                let source_start = row_index * w as usize;
                for (pixel_index, pixel) in row.chunks_exact_mut(4).enumerate() {
                    let source_index = source_start + pixel_index;
                    pixel[0] = band_pixels[0][source_index];
                    pixel[1] = band_pixels[1][source_index];
                    pixel[2] = band_pixels[2][source_index];
                    pixel[3] = band_pixels[3][source_index];
                }
            });
            let img = RgbaImage::from_raw(w, h, rgba)
                .ok_or_else(|| PilError::ValueError("merge: buffer error".into()))?;
            Ok(DynamicImage::ImageRgba8(img))
        }
        ColorMode::CMYK => {
            let mut rgba = vec![0u8; n * 4];
            apply_effect_rows(&mut rgba, w as usize, h as usize, 4, |row_index, row| {
                let source_start = row_index * w as usize;
                for (pixel_index, pixel) in row.chunks_exact_mut(4).enumerate() {
                    let source_index = source_start + pixel_index;
                    pixel[0] = band_pixels[0][source_index];
                    pixel[1] = band_pixels[1][source_index];
                    pixel[2] = band_pixels[2][source_index];
                    pixel[3] = band_pixels[3][source_index];
                }
            });
            let img = RgbaImage::from_raw(w, h, rgba)
                .ok_or_else(|| PilError::ValueError("merge: buffer error".into()))?;
            Ok(DynamicImage::ImageRgba8(img))
        }
        ColorMode::LA => {
            let mut la = vec![0u8; n * 2];
            apply_effect_rows(&mut la, w as usize, h as usize, 2, |row_index, row| {
                let source_start = row_index * w as usize;
                for (pixel_index, pixel) in row.chunks_exact_mut(2).enumerate() {
                    let source_index = source_start + pixel_index;
                    pixel[0] = band_pixels[0][source_index];
                    pixel[1] = band_pixels[1][source_index];
                }
            });
            let img = GrayAlphaImage::from_raw(w, h, la)
                .ok_or_else(|| PilError::ValueError("merge: buffer error".into()))?;
            Ok(DynamicImage::ImageLumaA8(img))
        }
        ColorMode::L | ColorMode::Mode1 | ColorMode::P => {
            let img = GrayImage::from_raw(w, h, band_pixels.remove(0))
                .ok_or_else(|| PilError::ValueError("merge: buffer error".into()))?;
            Ok(DynamicImage::ImageLuma8(img))
        }
        // Exhaustiveness for ColorMode variants that Image::merge rejects
        // before it queues PipelineOp::Merge; not a supported input path.
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
    // `Image::blend` validates both image dimensions before queuing this
    // operation. A malformed `PipelineOp::BlendModule` is outside the
    // supported module-level input boundary, so this executor does not repeat
    // that validation.
    // Pillow 12.2.0 `Blend.c::ImagingBlend` interpolates for alpha in [0, 1]
    // and clips extrapolation results to [0, 255] for any other alpha.
    let a = alpha;
    // CMYK mode: blend all 4 channels (C,M,Y,K stored as R,G,B,A in Rgba8)
    if explicit_mode == Some("CMYK") {
        let rgba1 = img.to_rgba8();
        let rgba2 = other_img.to_rgba8();
        let (w, h) = (
            rgba1.width().min(rgba2.width()),
            rgba1.height().min(rgba2.height()),
        );
        let row_bytes = w as usize * 4;
        let first = rgba1.as_raw();
        let second = rgba2.as_raw();
        let mut output = vec![0u8; row_bytes * h as usize];
        apply_effect_rows(&mut output, w as usize, h as usize, 4, |row_index, row| {
            let start = row_index * row_bytes;
            blend_row(
                &first[start..start + row_bytes],
                &second[start..start + row_bytes],
                row,
                a,
            );
        });
        let out = RgbaImage::from_raw(w, h, output)
            .ok_or_else(|| PilError::ValueError("blend: buffer error".into()))?;
        return Ok(DynamicImage::ImageRgba8(out));
    }

    // Pillow blends every stored channel independently. Converting LA/RGBA
    // through RGB manufactures an opaque alpha channel, which is observable
    // even for transparent black inputs.
    if matches!(img, DynamicImage::ImageLumaA8(_)) {
        let first = img.to_luma_alpha8();
        let second = other_img.to_luma_alpha8();
        let (w, h) = (first.width(), first.height());
        let row_bytes = w as usize * 2;
        let first_bytes = first.as_raw();
        let second_bytes = second.as_raw();
        let mut output = vec![0u8; row_bytes * h as usize];
        apply_effect_rows(&mut output, w as usize, h as usize, 2, |row_index, row| {
            let start = row_index * row_bytes;
            blend_row(
                &first_bytes[start..start + row_bytes],
                &second_bytes[start..start + row_bytes],
                row,
                a,
            );
        });
        let out = GrayAlphaImage::from_raw(w, h, output)
            .ok_or_else(|| PilError::ValueError("blend: buffer error".into()))?;
        return Ok(DynamicImage::ImageLumaA8(out));
    }
    if matches!(img, DynamicImage::ImageRgba8(_)) {
        let first = img.to_rgba8();
        let second = other_img.to_rgba8();
        let (w, h) = (first.width(), first.height());
        let row_bytes = w as usize * 4;
        let first_bytes = first.as_raw();
        let second_bytes = second.as_raw();
        let mut output = vec![0u8; row_bytes * h as usize];
        apply_effect_rows(&mut output, w as usize, h as usize, 4, |row_index, row| {
            let start = row_index * row_bytes;
            blend_row(
                &first_bytes[start..start + row_bytes],
                &second_bytes[start..start + row_bytes],
                row,
                a,
            );
        });
        let out = RgbaImage::from_raw(w, h, output)
            .ok_or_else(|| PilError::ValueError("blend: buffer error".into()))?;
        return Ok(DynamicImage::ImageRgba8(out));
    }

    let rgb1 = img.to_rgb8();
    let rgb2 = other_img.to_rgb8();
    let (w, h) = (
        rgb1.width().min(rgb2.width()),
        rgb1.height().min(rgb2.height()),
    );
    let row_bytes = w as usize * 3;
    let first = rgb1.as_raw();
    let second = rgb2.as_raw();
    let mut output = vec![0u8; row_bytes * h as usize];
    apply_effect_rows(&mut output, w as usize, h as usize, 3, |row_index, row| {
        let start = row_index * row_bytes;
        blend_row(
            &first[start..start + row_bytes],
            &second[start..start + row_bytes],
            row,
            a,
        );
    });
    let out = RgbImage::from_raw(w, h, output)
        .ok_or_else(|| PilError::ValueError("blend: buffer error".into()))?;
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

/// Apply a byte LUT directly in the image's native storage layout.
///
/// The generic Eval implementation widens multi-band images to RGBA before
/// applying the table.  That preserves mode semantics, but a fused point
/// chain already carries one table per native band, so widening creates an
/// avoidable second full-frame buffer and conversion.  Keep this fast path
/// limited to the four native byte layouts and exact table lengths; all typed,
/// palette, and malformed descriptors retain the established implementation.
fn eval_native_byte_lut(img: &DynamicImage, lut: &[u8]) -> Option<DynamicImage> {
    match img {
        DynamicImage::ImageLuma8(gray) if lut.len() == 256 => {
            let mut output = gray.clone();
            for value in output.as_mut() {
                *value = lut[usize::from(*value)];
            }
            Some(DynamicImage::ImageLuma8(output))
        }
        DynamicImage::ImageLumaA8(la) if lut.len() == 512 => {
            let mut output = la.clone();
            for pixel in output.as_mut().chunks_exact_mut(2) {
                pixel[0] = lut[usize::from(pixel[0])];
                pixel[1] = lut[256 + usize::from(pixel[1])];
            }
            Some(DynamicImage::ImageLumaA8(output))
        }
        DynamicImage::ImageRgb8(rgb) if lut.len() == 768 => {
            let mut output = rgb.clone();
            for pixel in output.as_mut().chunks_exact_mut(3) {
                pixel[0] = lut[usize::from(pixel[0])];
                pixel[1] = lut[256 + usize::from(pixel[1])];
                pixel[2] = lut[512 + usize::from(pixel[2])];
            }
            Some(DynamicImage::ImageRgb8(output))
        }
        DynamicImage::ImageRgba8(rgba) if lut.len() == 1024 => {
            let mut output = rgba.clone();
            for pixel in output.as_mut().chunks_exact_mut(4) {
                pixel[0] = lut[usize::from(pixel[0])];
                pixel[1] = lut[256 + usize::from(pixel[1])];
                pixel[2] = lut[512 + usize::from(pixel[2])];
                pixel[3] = lut[768 + usize::from(pixel[3])];
            }
            Some(DynamicImage::ImageRgba8(output))
        }
        _ => None,
    }
}

pub fn op_eval(img: &DynamicImage, lut: &[u8]) -> Result<DynamicImage, PilError> {
    let n_bands = match img.color() {
        crate::raster::ColorType::L8 | crate::raster::ColorType::L16 => 1,
        crate::raster::ColorType::La8 | crate::raster::ColorType::La16 => 2,
        crate::raster::ColorType::Rgb8 | crate::raster::ColorType::Rgb16 => 3,
        _ => 4,
    };
    // `image_eval_validated` checks the exact band length before queuing the
    // operation, and internal PointOp fusion constructs complete tables. A
    // malformed Eval descriptor is outside the supported public input
    // boundary, so the executor does not duplicate that validation.
    let band_luts: Vec<&[u8]> = (0..n_bands).map(|b| &lut[b * 256..(b + 1) * 256]).collect();
    if let Some(output) = eval_native_byte_lut(img, lut) {
        return Ok(output);
    }
    // For single-channel images (mode "1", "L", "P"), operate on Luma8 directly
    // to avoid precision loss through RGBA round-trip.
    if n_bands == 1 {
        let gray = img.to_luma8();
        let (w, h) = gray.dimensions();
        let mut out = GrayImage::new(w, h);
        for (op, ip) in out.pixels_mut().zip(gray.pixels()) {
            let idx = ip[0] as usize;
            op[0] = band_luts[0][idx];
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
            op[b] = band_luts[band][idx];
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
    let values = effect_noise_values(w, h, sigma)?;
    let out = GrayImage::from_raw(w, h, values)
        .ok_or_else(|| PilError::ValueError("effect_noise buffer error".into()))?;
    Ok(DynamicImage::ImageLuma8(out))
}

/// Generate the exact L-mode bytes for EffectNoise while consuming the shared
/// Pillow-compatible process RNG in source order.  Keeping this scalar control
/// section separate lets GPU encode the generated bytes as an auxiliary gather
/// buffer without inventing a second random stream.
pub(crate) fn effect_noise_values(
    width: u32,
    height: u32,
    sigma: f64,
) -> Result<Vec<u8>, PilError> {
    let pixels = usize::try_from(width)
        .ok()
        .and_then(|w| usize::try_from(height).ok().and_then(|h| w.checked_mul(h)))
        .ok_or_else(|| PilError::ValueError("effect_noise image dimensions overflow".into()))?;
    let mut values = vec![0u8; pixels];
    let sigma = f64::from(sigma as f32);
    with_process_rng(|rng| {
        // `_effect_noise` parses sigma with PyArg's `f` conversion before passing
        // it to ImagingEffectNoise, so round it to FLOAT32 once at the boundary.
        const RAND_MAX_F64: f64 = 2147483647.0;
        for pixel in &mut values {
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
            *pixel = if v <= 0.0 {
                0
            } else if v >= 255.0 {
                255
            } else {
                v as u8
            };
        }
    })?;
    Ok(values)
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
    let fill_color = fill.unwrap_or_else(|| {
        if matches!(
            img.color(),
            crate::raster::ColorType::La8 | crate::raster::ColorType::Rgba8
        ) {
            (0, 0, 0, 0)
        } else {
            (0, 0, 0, 255)
        }
    });

    let mut out = vec![0u8; (dst_w * dst_h) as usize * channels];

    for dy in 0..dst_h {
        for dx in 0..dst_w {
            // Geometry.c's affine_transform() evaluates destination pixel
            // centers before applying the matrix. The nearest filter then
            // truncates non-negative coordinates, while interpolating filters
            // subtract 0.5 inside their own filter prologue.
            let sx = aff_a * (dx as f64 + 0.5) + aff_b * (dy as f64 + 0.5) + aff_c;
            let sy = aff_d * (dx as f64 + 0.5) + aff_e * (dy as f64 + 0.5) + aff_f;
            let out_idx = (dy * dst_w + dx) as usize * channels;

            if nearest {
                // Matches Geometry.c's COORD(): negative coordinates are
                // rejected and non-negative values use C's truncating cast,
                // rather than round-to-nearest.
                let ix = if sx < 0.0 { -1 } else { sx as i64 };
                let iy = if sy < 0.0 { -1 } else { sy as i64 };
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
                // Matches Geometry.c's BILINEAR_HEAD(): bounds are checked
                // in center space, then the filter moves to the surrounding
                // sample corners. XCLIP/YCLIP keep the half-pixel border
                // valid without indexing a negative floor coordinate.
                let sample_x = sx - 0.5;
                let sample_y = sy - 0.5;
                let x_floor = sample_x.floor() as i64;
                let y_floor = sample_y.floor() as i64;
                let x0 = x_floor.clamp(0, sw as i64 - 1) as u32;
                let y0 = y_floor.clamp(0, sh as i64 - 1) as u32;
                let x1 = (x_floor + 1).clamp(0, sw as i64 - 1) as u32;
                let y1 = (y_floor + 1).clamp(0, sh as i64 - 1) as u32;
                let fx = sample_x - x_floor as f64;
                let fy = sample_y - y_floor as f64;
                for ch in 0..channels {
                    let p00 = raw[(y0 * sw + x0) as usize * channels + ch] as f64;
                    let p10 = raw[(y0 * sw + x1) as usize * channels + ch] as f64;
                    let p01 = raw[(y1 * sw + x0) as usize * channels + ch] as f64;
                    let p11 = raw[(y1 * sw + x1) as usize * channels + ch] as f64;
                    let v = (1.0 - fx) * (1.0 - fy) * p00
                        + fx * (1.0 - fy) * p10
                        + (1.0 - fx) * fy * p01
                        + fx * fy * p11;
                    // Pillow's ImagingTransformAffine byte path stores the
                    // weighted sample by truncating toward zero; rounding
                    // here turns exact half-way samples one value too high.
                    out[out_idx + ch] = v.clamp(0.0, 255.0) as u8;
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

/// Apply a projective or quadrilateral transform to native 8-bit samples.
///
/// Pillow represents perspective transforms as an inverse homography and
/// quadrilateral transforms as a bilinear map from output coordinates to the
/// four source corners. Keeping both calculations here lets the public core
/// route methods 2 and 3 through the same lazy pipeline as affine transforms.
fn transform_projective_generic(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    data: &[f64],
    filter: &ResampleFilter,
    fill: Option<(u8, u8, u8, u8)>,
    quad: bool,
) -> Result<DynamicImage, PilError> {
    let channels = img.color().channel_count() as usize;
    let raw = img.as_bytes();
    let (src_w, src_h) = img.dimensions();
    let fill_color = fill.unwrap_or((0, 0, 0, 255));
    let pixel_count = usize::try_from(dst_w)
        .ok()
        .and_then(|width| {
            usize::try_from(dst_h)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| PilError::ValueError("image dimensions are too large".into()))?;
    let byte_count = pixel_count
        .checked_mul(channels)
        .ok_or_else(|| PilError::ValueError("image dimensions are too large".into()))?;
    let mut out = vec![0u8; byte_count];
    let nearest = matches!(filter, ResampleFilter::Nearest);

    let source_at = |dx: f64, dy: f64| -> Option<(f64, f64)> {
        if quad {
            if dst_w == 0 || dst_h == 0 {
                return None;
            }
            // Pillow's Image.__transformer first converts QUAD's raw
            // NW/SW/SE/NE corners to these bilinear coefficients, then
            // Geometry.c evaluates them at destination pixel centers.
            let dx = dx + 0.5;
            let dy = dy + 0.5;
            let x0 = data[0];
            let y0 = data[1];
            let sw = f64::from(dst_w);
            let sh = f64::from(dst_h);
            let inverse_width = 1.0 / sw;
            let inverse_height = 1.0 / sh;
            let x1 = (data[6] - x0) * inverse_width;
            let x2 = (data[2] - x0) * inverse_height;
            let x3 = (data[4] - data[2] - data[6] + x0) * inverse_width * inverse_height;
            let y1 = (data[7] - y0) * inverse_width;
            let y2 = (data[3] - y0) * inverse_height;
            let y3 = (data[5] - data[3] - data[7] + y0) * inverse_width * inverse_height;
            let x = x0 + x1 * dx + x2 * dy + x3 * dx * dy;
            let y = y0 + y1 * dx + y2 * dy + y3 * dx * dy;
            return Some((x, y));
        }

        // Pillow 12.2.0's libImaging/Geometry.c `perspective_transform`
        // evaluates the inverse map at each destination pixel center.
        let dx = dx + 0.5;
        let dy = dy + 0.5;
        let denominator = data[6] * dx + data[7] * dy + 1.0;
        if denominator == 0.0 || !denominator.is_finite() {
            return None;
        }
        let x = (data[0] * dx + data[1] * dy + data[2]) / denominator;
        let y = (data[3] * dx + data[4] * dy + data[5]) / denominator;
        (x.is_finite() && y.is_finite()).then_some((x, y))
    };

    let fill_sample = |destination: &mut [u8]| {
        for (channel, value) in destination.iter_mut().enumerate() {
            *value = match channel {
                0 => fill_color.0,
                1 => fill_color.1,
                2 => fill_color.2,
                _ => fill_color.3,
            };
        }
    };
    let nearest_coordinate = |value: f64| {
        if value < 0.0 { -1 } else { value as i64 }
    };

    for dy in 0..dst_h {
        for dx in 0..dst_w {
            let out_idx = (dy as usize * dst_w as usize + dx as usize) * channels;
            let destination = &mut out[out_idx..out_idx + channels];
            let Some((sx, sy)) = source_at(f64::from(dx), f64::from(dy)) else {
                fill_sample(destination);
                continue;
            };

            if nearest {
                // Geometry.c defines COORD(v) as -1 for negative values and
                // a C cast otherwise, i.e. truncation toward zero rather
                // than rounding to the nearest source pixel. Both
                // perspective and quad maps have already been evaluated at
                // destination pixel centers above.
                let ix = nearest_coordinate(sx);
                let iy = nearest_coordinate(sy);
                if ix >= 0 && ix < i64::from(src_w) && iy >= 0 && iy < i64::from(src_h) {
                    let in_idx = (iy as usize * src_w as usize + ix as usize) * channels;
                    destination.copy_from_slice(&raw[in_idx..in_idx + channels]);
                } else {
                    fill_sample(destination);
                }
                continue;
            }

            if src_w == 0
                || src_h == 0
                || sx < 0.0
                || sx >= f64::from(src_w)
                || sy < 0.0
                || sy >= f64::from(src_h)
            {
                fill_sample(destination);
                continue;
            }
            // Geometry.c's bilinear filter checks the unshifted source
            // coordinate, subtracts 0.5, then uses FLOOR and edge clipping.
            // Its horizontal interpolation is performed before the vertical
            // interpolation, and the final UINT8 cast truncates toward zero.
            let sample_x = sx - 0.5;
            let sample_y = sy - 0.5;
            let floor_x = sample_x.floor() as i64;
            let floor_y = sample_y.floor() as i64;
            let x0 = floor_x.clamp(0, i64::from(src_w - 1)) as usize;
            let x1 = (floor_x + 1).clamp(0, i64::from(src_w - 1)) as usize;
            let y0 = floor_y.clamp(0, i64::from(src_h - 1)) as usize;
            let y1 = (floor_y + 1).clamp(0, i64::from(src_h - 1)) as usize;
            let fx = sample_x - floor_x as f64;
            let fy = sample_y - floor_y as f64;
            for channel in 0..channels {
                let p00 = raw[(y0 * src_w as usize + x0) * channels + channel] as f64;
                let p10 = raw[(y0 * src_w as usize + x1) * channels + channel] as f64;
                let p01 = raw[(y1 * src_w as usize + x0) * channels + channel] as f64;
                let p11 = raw[(y1 * src_w as usize + x1) * channels + channel] as f64;
                let horizontal_top = (p10 - p00).mul_add(fx, p00);
                let horizontal_bottom = (p11 - p01).mul_add(fx, p01);
                destination[channel] = (horizontal_bottom - horizontal_top)
                    .mul_add(fy, horizontal_top)
                    .clamp(0.0, 255.0) as u8;
            }
        }
    }

    Ok(match channels {
        1 => DynamicImage::ImageLuma8(GrayImage::from_raw(dst_w, dst_h, out).ok_or_else(|| {
            PilError::InternalError("transform projective L buffer shape mismatch".into())
        })?),
        2 => DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(dst_w, dst_h, out).ok_or_else(
            || PilError::InternalError("transform projective LA buffer shape mismatch".into()),
        )?),
        3 => DynamicImage::ImageRgb8(RgbImage::from_raw(dst_w, dst_h, out).ok_or_else(|| {
            PilError::InternalError("transform projective RGB buffer shape mismatch".into())
        })?),
        4 => DynamicImage::ImageRgba8(RgbaImage::from_raw(dst_w, dst_h, out).ok_or_else(|| {
            PilError::InternalError("transform projective RGBA buffer shape mismatch".into())
        })?),
        _ => unreachable!(),
    })
}

/// Apply the nearest-neighbor affine path to native unsigned 16-bit samples.
///
/// The byte-oriented transform helper cannot process `I;16` as one-byte
/// luma: its source stride is two bytes per sample and its output must remain
/// `ImageLuma16`. Keep this path native so scalar fill values retain both
/// bytes instead of being duplicated into an 8-bit result.
fn transform_affine_luma16(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    data: &[f64],
    fill: Option<(u8, u8, u8, u8)>,
) -> Result<DynamicImage, PilError> {
    let source = img.to_luma16();
    let (src_w, src_h) = source.dimensions();
    let (a, b, c, d, e, f) = (data[0], data[1], data[2], data[3], data[4], data[5]);
    let fill = fill.map_or(0, |color| u16::from_le_bytes([color.0, color.1]));
    let mut output = vec![fill; (dst_w as usize).saturating_mul(dst_h as usize)];

    for dy in 0..dst_h {
        for dx in 0..dst_w {
            let sx = a * f64::from(dx) + b * f64::from(dy) + c;
            let sy = d * f64::from(dx) + e * f64::from(dy) + f;
            let ix = (sx + 0.5).floor() as i64;
            let iy = (sy + 0.5).floor() as i64;
            if ix >= 0 && ix < i64::from(src_w) && iy >= 0 && iy < i64::from(src_h) {
                output[(dy as usize) * dst_w as usize + dx as usize] =
                    source.get_pixel(ix as u32, iy as u32)[0];
            }
        }
    }

    ImageBuffer::from_raw(dst_w, dst_h, output)
        .map(DynamicImage::ImageLuma16)
        .ok_or_else(|| PilError::InternalError("transform I;16 buffer shape mismatch".into()))
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
            // `Image::transform_public` and the maintained affine wrappers
            // validate the exact six-coefficient contract before queuing the
            // operation. A malformed transform descriptor is outside the
            // supported public input boundary, so the executor can index the
            // validated coefficients directly.
            if img.color() == crate::raster::ColorType::L16 {
                return transform_affine_luma16(img, w, h, data, fill);
            }
            let (aff_a, aff_b, aff_c, aff_d, aff_e, aff_f) =
                (data[0], data[1], data[2], data[3], data[4], data[5]);
            let p_mode = explicit_mode == Some("P") || explicit_mode == Some("1");
            let i_f_mode = explicit_mode == Some("I") || explicit_mode == Some("F");
            let use_nearest = matches!(filter, ResampleFilter::Nearest) || p_mode || i_f_mode;

            // Pillow's ImagingTransformAffine resamples LA/RGBA through the
            // premultiplied La/RGBa modes, then unpremultiplies the result.
            // The fill sample is written as supplied into that temporary mode
            // and is unpremultiplied along with the sampled pixels.
            let needs_alpha_roundtrip = !use_nearest
                && !matches!(explicit_mode, Some("PA") | Some("RGBa"))
                && !matches!(explicit_mode, Some("CMYK" | "I" | "F"))
                && matches!(
                    img.color(),
                    crate::raster::ColorType::La8 | crate::raster::ColorType::Rgba8
                );
            let work = if needs_alpha_roundtrip {
                premultiply_alpha(img)
            } else {
                img.clone()
            };
            let transform_fill = fill;

            let result = transform_affine_generic(
                &work,
                w,
                h,
                aff_a,
                aff_b,
                aff_c,
                aff_d,
                aff_e,
                aff_f,
                transform_fill,
                use_nearest,
            )?;
            let result = if needs_alpha_roundtrip {
                unpremultiply_alpha(&result)
            } else {
                result
            };
            Ok(preserve_mode(img, result))
        }
        &TransformMethod::Mesh => {
            // `Image::transform_public` validates every mesh record and
            // flattens it into complete twelve-value elements before queuing
            // this operation. The old public raw-data wrapper was removed, so
            // malformed mesh descriptors are outside the supported input
            // boundary.
            // Pillow's non-nearest Image.transform path premultiplies LA/RGBA
            // before invoking the mesh filter and unpremultiplies afterward.
            // Mesh records otherwise operate on the same native byte layout
            // as the scalar transform implementation.
            let needs_alpha_roundtrip = !matches!(filter, ResampleFilter::Nearest)
                && !matches!(explicit_mode, Some("PA" | "RGBa" | "RGBX"))
                && matches!(
                    img.color(),
                    crate::raster::ColorType::La8 | crate::raster::ColorType::Rgba8
                );
            let work = if needs_alpha_roundtrip {
                premultiply_alpha(img)
            } else {
                img.clone()
            };
            let result = transform_mesh(&work, w, h, data, fill, *filter)?;
            let result = if needs_alpha_roundtrip {
                unpremultiply_alpha(&result)
            } else {
                result
            };
            Ok(preserve_mode(img, result))
        }
        &TransformMethod::Perspective => {
            // `Image::transform_public` and the maintained perspective
            // wrapper validate the exact eight-coefficient contract before
            // queuing this operation.
            let result = transform_projective_generic(img, w, h, &data[..8], filter, fill, false)?;
            Ok(preserve_mode(img, result))
        }
        &TransformMethod::Quad => {
            // `Image::transform_public` and the maintained quad wrapper
            // validate the exact eight-coordinate contract before queuing
            // this operation.
            let result = transform_projective_generic(img, w, h, &data[..8], filter, fill, true)?;
            Ok(preserve_mode(img, result))
        }
    }
}

// ── PutPixel ──

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

/// Replaces the alpha channel from an L-mask, matching Pillow's image-backed
/// `putalpha` (P: index retained, CMYK: promoted through RGB to RGBA).
pub fn op_put_alpha_data(
    img: &DynamicImage,
    mask: &crate::raster::DynamicImage,
    mode: PixelMode,
) -> DynamicImage {
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

fn transform_mesh(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    mesh_data: &[f64],
    fill: Option<(u8, u8, u8, u8)>,
    filter: ResampleFilter,
) -> Result<DynamicImage, PilError> {
    transform_mesh_with_filter(img, dst_w, dst_h, mesh_data, fill, filter)
}

/// Apply Pillow's piecewise quadrilateral mesh transform using the exact
/// destination-box and source-filter contracts of `Geometry.c`.
///
/// `Image.transform(..., Transform.MESH, ...)` lowers each `(box, quad)`
/// record to a local QUAD transform. The C implementation clips the output
/// box, evaluates each local pixel at its center, uses `COORD` (truncate
/// toward zero, with negative values outside) for nearest sampling, and uses
/// horizontal-first filtered interpolation after subtracting `0.5` from the
/// source coordinate. Keep those details here instead of treating a mesh as
/// a global normalized bilinear map: clipping changes the local origin while
/// the coefficients still use the original box dimensions.
fn transform_mesh_with_filter(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    mesh_data: &[f64],
    fill: Option<(u8, u8, u8, u8)>,
    filter: ResampleFilter,
) -> Result<DynamicImage, PilError> {
    let channels = img.color().channel_count() as usize;
    let raw = img.as_bytes();
    let (sw, sh) = img.dimensions();
    let sw_f = sw as f64;
    let sh_f = sh as f64;
    let fill_color = fill.unwrap_or((0, 0, 0, 255));

    let output_len = usize::try_from(dst_w)
        .ok()
        .and_then(|width| {
            usize::try_from(dst_h)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| PilError::ValueError("image dimensions are too large".into()))?;
    let fill_bytes = [fill_color.0, fill_color.1, fill_color.2, fill_color.3];
    let mut out = vec![0u8; output_len];
    for destination in out.chunks_exact_mut(channels) {
        destination.copy_from_slice(&fill_bytes[..channels]);
    }

    // Process each mesh element
    for mesh in mesh_data.chunks_exact(12) {
        let x0_d = mesh[0] as i64;
        let y0_d = mesh[1] as i64;
        let x1_d = mesh[2] as i64;
        let y1_d = mesh[3] as i64;
        let width = x1_d.saturating_sub(x0_d);
        let height = y1_d.saturating_sub(y0_d);
        // Python computes these divisors before entering the C transform.
        // Valid mesh boxes are positive; an empty/inverted box contributes no
        // pixels here and, importantly, cannot create an unbounded loop.
        if width <= 0 || height <= 0 {
            continue;
        }

        let x0_s = mesh[4];
        let y0_s = mesh[5];
        let x1_s = mesh[6];
        let y1_s = mesh[7];
        let x2_s = mesh[8];
        let y2_s = mesh[9];
        let x3_s = mesh[10];
        let y3_s = mesh[11];
        let width_f = width as f64;
        let height_f = height as f64;
        let inverse_width = 1.0 / width_f;
        let inverse_height = 1.0 / height_f;
        let coefficients = [
            x0_s,
            (x3_s - x0_s) * inverse_width,
            (x1_s - x0_s) * inverse_height,
            (x2_s - x1_s - x3_s + x0_s) * inverse_width * inverse_height,
            y0_s,
            (y3_s - y0_s) * inverse_width,
            (y1_s - y0_s) * inverse_height,
            (y2_s - y1_s - y3_s + y0_s) * inverse_width * inverse_height,
        ];

        // ImagingGenericTransform clips the box, then passes coordinates
        // relative to that clipped lower-left corner to the map function.
        let bx0 = x0_d.max(0).min(i64::from(dst_w));
        let by0 = y0_d.max(0).min(i64::from(dst_h));
        let bx1 = x1_d.max(0).min(i64::from(dst_w));
        let by1 = y1_d.max(0).min(i64::from(dst_h));
        if bx0 >= bx1 || by0 >= by1 {
            continue;
        }

        for destination_y in by0..by1 {
            let local_y = (destination_y - by0) as f64 + 0.5;
            for destination_x in bx0..bx1 {
                let local_x = (destination_x - bx0) as f64 + 0.5;
                // Pillow src/libImaging/Geometry.c quad_transform is compiled
                // into two linear
                // fused additions followed by a rounded x-product and a
                // final fused y-product on this target. Keep the cross-term
                // multiplication separate from the final `mul_add`; folding
                // it into `local_x * local_y` changes boundary truncation.
                let sx_linear = coefficients[2]
                    .mul_add(local_y, coefficients[1].mul_add(local_x, coefficients[0]));
                let sx = (coefficients[3] * local_x).mul_add(local_y, sx_linear);
                let sy_linear = coefficients[6]
                    .mul_add(local_y, coefficients[5].mul_add(local_x, coefficients[4]));
                let sy = (coefficients[7] * local_x).mul_add(local_y, sy_linear);
                let out_idx =
                    ((destination_y as usize * dst_w as usize) + destination_x as usize) * channels;

                if matches!(filter, ResampleFilter::Nearest) {
                    // Geometry.c `COORD`: negative values are outside and
                    // nonnegative values are truncated toward zero.
                    let ix = if sx.is_finite() && sx >= 0.0 {
                        sx as i64
                    } else {
                        -1
                    };
                    let iy = if sy.is_finite() && sy >= 0.0 {
                        sy as i64
                    } else {
                        -1
                    };
                    if ix >= 0 && ix < i64::from(sw) && iy >= 0 && iy < i64::from(sh) {
                        let source_idx = ((iy as usize * sw as usize) + ix as usize) * channels;
                        out[out_idx..out_idx + channels]
                            .copy_from_slice(&raw[source_idx..source_idx + channels]);
                    }
                    continue;
                }

                if !sx.is_finite()
                    || !sy.is_finite()
                    || sx < 0.0
                    || sx >= sw_f
                    || sy < 0.0
                    || sy >= sh_f
                {
                    continue;
                }

                let (x, y, dx, dy) = match filter {
                    ResampleFilter::Bilinear => {
                        let x = (sx - 0.5).floor() as i64;
                        let y = (sy - 0.5).floor() as i64;
                        (x, y, sx - 0.5 - x as f64, sy - 0.5 - y as f64)
                    }
                    ResampleFilter::Bicubic => {
                        let x = (sx - 0.5).floor() as i64 - 1;
                        let y = (sy - 0.5).floor() as i64 - 1;
                        (x, y, sx - 0.5 - (x + 1) as f64, sy - 0.5 - (y + 1) as f64)
                    }
                    _ => continue,
                };

                for channel in 0..channels {
                    let value = match filter {
                        ResampleFilter::Bilinear => {
                            let x0 = x.clamp(0, i64::from(sw - 1)) as usize;
                            let x1 = (x + 1).clamp(0, i64::from(sw - 1)) as usize;
                            let y0 = y.clamp(0, i64::from(sh - 1)) as usize;
                            let y1 = (y + 1).clamp(0, i64::from(sh - 1)) as usize;
                            let p00 = raw[(y0 * sw as usize + x0) * channels + channel] as f64;
                            let p10 = raw[(y0 * sw as usize + x1) * channels + channel] as f64;
                            let p01 = raw[(y1 * sw as usize + x0) * channels + channel] as f64;
                            let p11 = raw[(y1 * sw as usize + x1) * channels + channel] as f64;
                            let top = (p10 - p00).mul_add(dx, p00);
                            let bottom = (p11 - p01).mul_add(dx, p01);
                            (bottom - top).mul_add(dy, top)
                        }
                        ResampleFilter::Bicubic => {
                            let mut rows = [0.0; 4];
                            for (row, output) in rows.iter_mut().enumerate() {
                                let yy = y + row as i64;
                                let cy = yy.clamp(0, i64::from(sh - 1)) as usize;
                                let samples = [
                                    raw[(cy * sw as usize
                                        + (x).clamp(0, i64::from(sw - 1)) as usize)
                                        * channels
                                        + channel] as f64,
                                    raw[(cy * sw as usize
                                        + (x + 1).clamp(0, i64::from(sw - 1)) as usize)
                                        * channels
                                        + channel] as f64,
                                    raw[(cy * sw as usize
                                        + (x + 2).clamp(0, i64::from(sw - 1)) as usize)
                                        * channels
                                        + channel] as f64,
                                    raw[(cy * sw as usize
                                        + (x + 3).clamp(0, i64::from(sw - 1)) as usize)
                                        * channels
                                        + channel] as f64,
                                ];
                                *output = cubic_sample(samples, dx);
                            }
                            cubic_sample(rows, dy)
                        }
                        _ => unreachable!(),
                    };
                    out[out_idx + channel] = match filter {
                        ResampleFilter::Bicubic => value.clamp(0.0, 255.0) as u8,
                        _ => value as u8,
                    };
                }
            }
        }
    }

    crate::image_utils::raw_bytes_to_image(dst_w, dst_h, out, channels)
}

#[inline]
fn cubic_sample(samples: [f64; 4], distance: f64) -> f64 {
    let p1 = samples[1];
    let p2 = -samples[0] + samples[2];
    let p3 = (samples[0] - samples[1]).mul_add(2.0, samples[2]) - samples[3];
    let p4 = -samples[0] + samples[1] - samples[2] + samples[3];
    let inner = distance.mul_add(p4, p3);
    let middle = distance.mul_add(inner, p2);
    distance.mul_add(middle, p1)
}

#[cfg(test)]
mod tests {
    use super::{cubic_sample, op_transform, transform_mesh, transform_projective_generic};
    use crate::pipeline::{ResampleFilter, TransformMethod};
    use crate::raster::{DynamicImage, GenericImageView, GrayImage, RgbImage, RgbaImage};

    fn varied_luma_source() -> DynamicImage {
        let raw: Vec<u8> = (0..4)
            .flat_map(|y| (0..5).map(move |x| (x * 37 + y * 11 + 3) as u8))
            .collect();
        DynamicImage::ImageLuma8(GrayImage::from_raw(5, 4, raw).expect("luma source"))
    }

    fn varied_rgb_source() -> DynamicImage {
        let raw: Vec<u8> = (0..4)
            .flat_map(|y| {
                (0..5).flat_map(move |x| {
                    let value = (x * 37 + y * 11 + 3) as u8;
                    [value, value.wrapping_add(37), value.wrapping_add(74)]
                })
            })
            .collect();
        DynamicImage::ImageRgb8(RgbImage::from_raw(5, 4, raw).expect("rgb source"))
    }

    fn varied_rgba_source() -> DynamicImage {
        let raw: Vec<u8> = (0..8)
            .flat_map(|y| {
                (0..9).flat_map(move |x| {
                    let index = (y * 9 + x) * 4;
                    [
                        (index * 37 + (index / 4) * 11 + 3) as u8,
                        ((index + 1) * 37 + ((index + 1) / 4) * 11 + 3) as u8,
                        ((index + 2) * 37 + ((index + 2) / 4) * 11 + 3) as u8,
                        ((index + 3) * 37 + ((index + 3) / 4) * 11 + 3) as u8,
                    ]
                })
            })
            .collect();
        DynamicImage::ImageRgba8(RgbaImage::from_raw(9, 8, raw).expect("rgba source"))
    }

    const VARIED_MESH: [f64; 12] = [
        -1.0, -1.0, 6.0, 5.0, -0.4, 0.2, 4.6, 3.4, 4.1, -0.3, 0.3, 3.7,
    ];

    #[test]
    fn mesh_nearest_matches_pillow_centers_and_truncation() {
        let result = transform_mesh(
            &varied_luma_source(),
            6,
            5,
            &VARIED_MESH,
            Some((7, 0, 0, 0)),
            ResampleFilter::Nearest,
        )
        .expect("mesh transform");
        let expected = [
            3, 14, 14, 14, 25, 25, 14, 14, 14, 51, 62, 62, 51, 51, 51, 51, 51, 51, 99, 88, 88, 88,
            88, 88, 136, 136, 125, 125, 125, 125,
        ];
        assert_eq!(result.as_bytes(), expected);
    }

    #[test]
    fn mesh_bilinear_matches_pillow_horizontal_first_filter() {
        let result = transform_mesh(
            &varied_rgb_source(),
            6,
            5,
            &VARIED_MESH,
            Some((7, 9, 11, 13)),
            ResampleFilter::Bilinear,
        )
        .expect("mesh transform");
        let expected = [
            4, 41, 78, 9, 46, 83, 14, 51, 88, 18, 55, 92, 23, 60, 97, 27, 64, 101, 23, 60, 97, 28,
            65, 102, 33, 70, 107, 38, 75, 112, 42, 79, 116, 47, 84, 121, 59, 96, 133, 60, 97, 134,
            62, 99, 136, 64, 101, 138, 66, 103, 140, 68, 105, 142, 94, 131, 168, 93, 130, 167, 92,
            129, 166, 90, 127, 164, 89, 126, 163, 88, 125, 162, 129, 166, 203, 125, 162, 199, 121,
            158, 195, 117, 154, 191, 113, 150, 187, 109, 146, 183,
        ];
        assert_eq!(result.as_bytes(), expected);
    }

    #[test]
    fn mesh_bilinear_preserves_native_quad_fma_order() {
        let image = DynamicImage::ImageLuma8(
            GrayImage::from_raw(3, 2, vec![1, 5, 13, 14, 28, 45]).expect("luma source"),
        );
        let mesh = [
            1.0, -5.0, 5.0, 6.0, -0.25, -2.125, 3.375, 4.375, -0.875, -2.875, 10.5, 3.125,
        ];

        let result = transform_mesh(
            &image,
            8,
            7,
            &mesh,
            Some((0, 0, 0, 255)),
            ResampleFilter::Bilinear,
        )
        .expect("mesh transform");

        // At (2, 5), Pillow's compiled quad_transform maps to y=0.75 and
        // bilinear filtering returns 21. Keeping the cross-term multiply
        // separate from the final fused y-product preserves that boundary.
        assert_eq!(result.as_bytes()[5 * 8 + 2], 21);
    }

    #[test]
    fn mesh_rgba_bilinear_matches_pillow_alpha_roundtrip() {
        let result = op_transform(
            &varied_rgba_source(),
            9,
            8,
            &TransformMethod::Mesh,
            &[2.0, 0.0, 9.0, 7.0, 7.1, -0.5, 1.2, 3.7, 4.8, 8.3, -1.2, 6.9],
            &ResampleFilter::Bilinear,
            Some((7, 9, 11, 13)),
            Some("RGBA"),
        )
        .expect("mesh transform");
        assert!(matches!(result, DynamicImage::ImageRgba8(_)));
        assert_eq!(result.dimensions(), (9, 8));
        let bytes = result.as_bytes();
        assert_eq!(&bytes[0..4], &[137, 176, 215, 13]);
        assert_eq!(
            &bytes[(3 * 9 + 4) * 4..(3 * 9 + 5) * 4],
            &[64, 101, 141, 63]
        );
        assert_eq!(&bytes[(5 * 9 + 8) * 4..(5 * 9 + 9) * 4], &[8, 47, 85, 117]);
        assert_eq!(
            &bytes[(6 * 9 + 7) * 4..(6 * 9 + 8) * 4],
            &[49, 87, 124, 119]
        );
    }

    #[test]
    fn mesh_bicubic_matches_pillow_clamped_filter() {
        let result = transform_mesh(
            &varied_luma_source(),
            6,
            5,
            &VARIED_MESH,
            Some((7, 0, 0, 0)),
            ResampleFilter::Bicubic,
        )
        .expect("mesh transform");
        let expected = [
            0, 2, 8, 14, 19, 28, 18, 23, 29, 33, 37, 41, 63, 65, 67, 68, 70, 71, 94, 94, 93, 92,
            91, 89, 125, 121, 118, 114, 109, 104,
        ];
        assert_eq!(result.as_bytes(), expected);
    }

    #[test]
    fn cubic_sample_preserves_native_horner_fma_order() {
        // This tap sequence occurs at the top edge of a one-column RGBX
        // source. Pillow src/libImaging/Geometry.c BICUBIC uses fused Horner
        // steps and returns 37; separate multiply/add operations return 38.
        let value = cubic_sample([174.0, 36.0, 38.0, 84.0], 0.9999999999999996);
        assert_eq!(value as u8, 37);
    }

    #[test]
    fn perspective_nearest_matches_pillow_center_and_truncation() {
        let width = 9;
        let height = 8;
        let raw: Vec<u8> = (0..height)
            .flat_map(|y| (0..width).map(move |x| (x * 37 + y * 11 + 3) as u8))
            .collect();
        let image =
            DynamicImage::ImageLuma8(GrayImage::from_raw(width, height, raw).expect("luma source"));
        let data = [1.0, 0.07, 0.4, -0.03, 1.0, 0.2, 0.001, -0.002];

        let result = transform_projective_generic(
            &image,
            8,
            7,
            &data,
            &ResampleFilter::Nearest,
            Some((17, 0, 0, 255)),
            false,
        )
        .expect("perspective transform");
        let expected = [
            3, 40, 77, 114, 151, 188, 225, 6, 51, 88, 125, 162, 162, 199, 236, 17, 62, 99, 136,
            173, 210, 247, 28, 65, 73, 110, 147, 184, 221, 2, 39, 76, 84, 121, 158, 195, 232, 13,
            50, 87, 95, 132, 169, 206, 243, 24, 61, 98, 106, 143, 180, 217, 254, 35, 72, 109,
        ];

        assert_eq!(result.as_bytes(), expected);
    }

    #[test]
    fn quad_nearest_matches_pillow_center_and_truncation() {
        let width = 9;
        let height = 8;
        let raw: Vec<u8> = (0..height)
            .flat_map(|y| (0..width).map(move |x| (x * 37 + y * 11 + 3) as u8))
            .collect();
        let image =
            DynamicImage::ImageLuma8(GrayImage::from_raw(width, height, raw).expect("luma source"));
        // Pillow's QUAD input order is NW, SW, SE, NE.
        let data = [0.2, 0.1, 8.7, 0.2, 9.1, 7.8, -0.4, 8.3];

        let result = transform_projective_generic(
            &image,
            8,
            7,
            &data,
            &ResampleFilter::Nearest,
            Some((17, 0, 0, 255)),
            true,
        )
        .expect("quad transform");
        let expected = [
            3, 14, 25, 36, 47, 58, 69, 80, 40, 51, 62, 73, 84, 95, 106, 117, 114, 125, 136, 147,
            158, 169, 180, 191, 151, 162, 173, 184, 195, 206, 217, 228, 188, 199, 210, 221, 232,
            243, 254, 9, 225, 236, 247, 2, 13, 61, 72, 83, 43, 54, 65, 76, 87, 98, 109, 120,
        ];

        assert_eq!(result.as_bytes(), expected);
    }

    #[test]
    fn projective_bilinear_matches_pillow_center_and_edge_sampling() {
        let width = 9;
        let height = 8;
        let raw: Vec<u8> = (0..height)
            .flat_map(|y| (0..width).map(move |x| (x * 37 + y * 11 + 3) as u8))
            .collect();
        let image =
            DynamicImage::ImageLuma8(GrayImage::from_raw(width, height, raw).expect("luma source"));
        let cases = [
            (
                false,
                [1.0, 0.07, 0.4, -0.03, 1.0, 0.2, 0.001, -0.002],
                [
                    21, 57, 94, 130, 167, 203, 138, 20, 34, 71, 108, 144, 181, 217, 130, 34, 48,
                    85, 122, 158, 195, 226, 123, 48, 62, 99, 135, 172, 209, 78, 26, 62, 76, 113,
                    149, 186, 223, 71, 40, 76, 90, 127, 163, 200, 237, 64, 54, 91, 104, 141, 178,
                    214, 223, 55, 68, 105,
                ],
            ),
            (
                true,
                [0.2, 0.1, 8.7, 0.2, 9.1, 7.8, -0.4, 8.3],
                [
                    14, 23, 31, 40, 49, 60, 71, 80, 59, 69, 78, 87, 97, 106, 115, 122, 105, 115,
                    124, 134, 144, 154, 164, 172, 150, 160, 171, 181, 192, 202, 212, 222, 195, 206,
                    212, 181, 191, 200, 207, 18, 141, 146, 138, 19, 31, 42, 54, 65, 30, 42, 54, 66,
                    78, 90, 102, 114,
                ],
            ),
        ];

        for (quad, data, expected) in cases {
            let result = transform_projective_generic(
                &image,
                8,
                7,
                &data,
                &ResampleFilter::Bilinear,
                Some((17, 0, 0, 255)),
                quad,
            )
            .expect("bilinear projective transform");
            assert_eq!(result.as_bytes(), expected, "quad={quad}");
        }
    }
}
