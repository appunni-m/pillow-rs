//! GIF encoder — wraps the `gif` crate.
//!
//! Supports:
//! - `L8`: raw palette indices with a grayscale palette
//! - `Rgb8`: quantized to a 256-color palette
//! - `Rgba8`: quantized to a 256-color palette plus transparency
use crate::encode_options::EncodeOptions;
use crate::types::{ColorType, DecodedImage};
use gif::{Encoder, Frame, Repeat};
/// Encode a `DecodedImage` as GIF bytes.
///
/// For L8 images the pixel values are used directly as palette indices with a
/// grayscale palette. RGB8 and RGBA8 images are quantized to a palette of at
/// most 256 unique colors using a simple nearest-neighbor approach.
///
/// Returns `None` for unsupported color types or images with no pixels.
pub fn encode(img: &DecodedImage, _opts: &EncodeOptions) -> Option<Vec<u8>> {
    let w = img.width as u16;
    let h = img.height as u16;
    if w == 0 || h == 0 {
        return None;
    }
    let mut buf = Vec::new();
    // Each arm must drop `encoder` (which borrows `buf`) before returning.
    let _result = match img.color {
        ColorType::L8 => {
            // Use pixel values directly as indices into a grayscale palette.
            let mut palette = Vec::with_capacity(256 * 3);
            for i in 0u16..256 {
                let v = i as u8;
                palette.push(v); // R
                palette.push(v); // G
                palette.push(v); // B
            }
            let mut frame = Frame::default();
            frame.width = w;
            frame.height = h;
            frame.palette = Some(palette);
            frame.buffer = std::borrow::Cow::Owned(img.pixels.clone());
            {
                let mut encoder = Encoder::new(&mut buf, w, h, &[]).ok()?;
                encoder.set_repeat(Repeat::Infinite).ok()?;
                encoder.write_frame(&frame).ok()?;
            }
            Some(())
        }
        ColorType::Rgb8 => {
            let (palette, indices) = quantize_rgb(&img.pixels)?;
            let mut frame = Frame::default();
            frame.width = w;
            frame.height = h;
            frame.palette = Some(palette);
            frame.buffer = std::borrow::Cow::Owned(indices);
            {
                let mut encoder = Encoder::new(&mut buf, w, h, &[]).ok()?;
                encoder.set_repeat(Repeat::Infinite).ok()?;
                encoder.write_frame(&frame).ok()?;
            }
            Some(())
        }
        ColorType::Rgba8 => {
            let (palette, indices, transparent_idx) = quantize_rgba(&img.pixels);
            let mut frame = Frame::default();
            frame.width = w;
            frame.height = h;
            frame.palette = Some(palette);
            frame.transparent = transparent_idx;
            frame.buffer = std::borrow::Cow::Owned(indices);
            {
                let mut encoder = Encoder::new(&mut buf, w, h, &[]).ok()?;
                encoder.set_repeat(Repeat::Infinite).ok()?;
                encoder.write_frame(&frame).ok()?;
            }
            Some(())
        }
        _ => None,
    };
    if _result.is_some() { Some(buf) } else { None }
}
/// Quantize RGB8 pixels to a palette (max 256 colors).
///
/// Returns `(palette, indices)` where palette is a flat vec of RGB triplets
/// and indices are the per-pixel palette index values.
fn quantize_rgb(pixels: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    if !pixels.len().is_multiple_of(3) {
        return None;
    }
    let mut palette: Vec<[u8; 3]> = Vec::new();
    let mut indices = Vec::with_capacity(pixels.len() / 3);
    for chunk in pixels.chunks_exact(3) {
        let color = [chunk[0], chunk[1], chunk[2]];
        match find_color(&palette, &color) {
            Some(idx) => indices.push(idx as u8),
            None => {
                if palette.len() < 256 {
                    let idx = palette.len() as u8;
                    palette.push(color);
                    indices.push(idx);
                } else {
                    // Palette full: find nearest neighbor
                    let nearest = find_nearest(&palette, &color);
                    indices.push(nearest as u8);
                }
            }
        }
    }
    // Flatten palette to RGB triplets
    let mut flat = Vec::with_capacity(palette.len() * 3);
    for c in &palette {
        flat.push(c[0]);
        flat.push(c[1]);
        flat.push(c[2]);
    }
    Some((flat, indices))
}
/// Quantize RGBA8 pixels to a palette with optional transparency.
///
/// Returns `(palette, indices, optional_transparent_index)`.
fn quantize_rgba(pixels: &[u8]) -> (Vec<u8>, Vec<u8>, Option<u8>) {
    let mut palette: Vec<[u8; 3]> = Vec::new();
    // Track which palette entries have transparent pixels
    let mut transparent_idx: Option<u8> = None;
    let pixel_count = pixels.len() / 4;
    let mut indices = Vec::with_capacity(pixel_count);
    // First pass: collect unique opaque (R,G,B) colors
    let mut transparent_color: Option<[u8; 3]> = None;
    for chunk in pixels.chunks_exact(4) {
        let alpha = chunk[3];
        let rgb = [chunk[0], chunk[1], chunk[2]];
        if alpha < 128 {
            // Transparent pixel — note the color
            transparent_idx = Some(0);
            transparent_color = Some(rgb);
            // Don't add to palette yet; we'll assign index 0
            indices.push(0);
        } else {
            // Opaque pixel — add to palette if new
            let palette_start = if transparent_idx.is_some() { 1 } else { 0 };
            match find_color_in_range(&palette, palette_start, &rgb) {
                Some(idx) => indices.push(idx as u8),
                None => {
                    if palette.len() < 256 {
                        let idx = palette.len() as u8;
                        palette.push(rgb);
                        indices.push(idx);
                    } else {
                        let nearest = find_nearest_in_range(&palette, palette_start, &rgb);
                        indices.push(nearest as u8);
                    }
                }
            }
        }
    }
    // Build flat palette. If we have transparent pixels, index 0 is the
    // transparent entry (use the first transparent color found).
    let mut flat = Vec::with_capacity(palette.len() * 3);
    if let Some(trgb) = transparent_color {
        flat.push(trgb[0]);
        flat.push(trgb[1]);
        flat.push(trgb[2]);
    }
    for c in &palette {
        flat.push(c[0]);
        flat.push(c[1]);
        flat.push(c[2]);
    }
    (flat, indices, transparent_idx)
}
/// Find a color in the palette. Returns its index if found.
fn find_color(palette: &[[u8; 3]], color: &[u8; 3]) -> Option<usize> {
    palette.iter().position(|c| c == color)
}
/// Find a color in the palette starting from `start` offset.
fn find_color_in_range(palette: &[[u8; 3]], start: usize, color: &[u8; 3]) -> Option<usize> {
    palette[start..]
        .iter()
        .position(|c| c == color)
        .map(|i| i + start)
}
/// Find the nearest color in the palette by Euclidean distance.
fn find_nearest(palette: &[[u8; 3]], color: &[u8; 3]) -> usize {
    let mut best = 0;
    let mut best_dist = u32::MAX;
    for (i, entry) in palette.iter().enumerate() {
        let dr = entry[0] as i32 - color[0] as i32;
        let dg = entry[1] as i32 - color[1] as i32;
        let db = entry[2] as i32 - color[2] as i32;
        let dist = (dr * dr + dg * dg + db * db) as u32;
        if dist < best_dist {
            best_dist = dist;
            best = i;
        }
    }
    best
}
/// Find the nearest color in the palette starting from `start` offset.
fn find_nearest_in_range(palette: &[[u8; 3]], start: usize, color: &[u8; 3]) -> usize {
    let mut best = start;
    let mut best_dist = u32::MAX;
    for (i, entry) in palette.iter().enumerate().skip(start) {
        let dr = entry[0] as i32 - color[0] as i32;
        let dg = entry[1] as i32 - color[1] as i32;
        let db = entry[2] as i32 - color[2] as i32;
        let dist = (dr * dr + dg * dg + db * db) as u32;
        if dist < best_dist {
            best_dist = dist;
            best = i;
        }
    }
    best
}
