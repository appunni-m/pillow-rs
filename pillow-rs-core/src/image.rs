use image::{DynamicImage, GenericImage, GenericImageView, ImageFormat};
use std::path::PathBuf;
use std::sync::Arc;

use crate::color::color_type_to_mode;
use crate::error::PilError;
use crate::format::parse_format_str;
use crate::pipeline::{
    ColorMode, PipelineOp, ResampleFilter, TransformMethod, TransposeMethod,
};

#[derive(Debug, Clone)]
pub enum Image {
    /// Fully decoded, ready to process or save. Optional explicit PIL mode.
    Loaded(DynamicImage, Option<String>),
    /// Path not yet decoded — lazy.
    Path {
        path: PathBuf,
        format: Option<ImageFormat>,
        is_paletted: bool,
    },
    /// Byte buffer not yet decoded — lazy.
    Bytes {
        data: Arc<Vec<u8>>,
        format: Option<ImageFormat>,
        is_paletted: bool,
    },
    /// Lazy pipeline — operations recorded, not executed.
    /// source: the input image (loaded or another pipeline).
    /// ops: the operations to apply, in order.
    Pipeline {
        source: Arc<Image>,
        ops: Vec<PipelineOp>,
        format: Option<ImageFormat>,
    },
}

impl Image {
    // ── Constructors ──

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        width: u32,
        height: u32,
        mode: &str,
        color: (u8, u8, u8, u8),
    ) -> Result<Self, PilError> {
        let img = match mode {
            "RGB" => DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
                width,
                height,
                image::Rgb([color.0, color.1, color.2]),
            )),
            "RGBA" => DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
                width,
                height,
                image::Rgba([color.0, color.1, color.2, color.3]),
            )),
            "L" => DynamicImage::ImageLuma8(image::GrayImage::from_pixel(
                width,
                height,
                image::Luma([color.0]),
            )),
            "LA" => DynamicImage::ImageLumaA8(image::GrayAlphaImage::from_pixel(
                width,
                height,
                image::LumaA([color.0, color.3]),
            )),
            "1" => DynamicImage::ImageLuma8(image::GrayImage::from_pixel(
                width, height, image::Luma([if color.0 > 127 { 255 } else { 0 }]),
            )),
            // Non-standard modes: stored as closest DynamicImage variant with explicit tag
            "CMYK" => DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
                width, height, image::Rgba([color.0, color.1, color.2, color.3]),
            )),
            "YCbCr" | "HSV" => DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
                width, height, image::Rgb([color.0, color.1, color.2]),
            )),
            "I" | "F" => DynamicImage::ImageLuma8(image::GrayImage::from_pixel(
                width, height, image::Luma([color.0]),
            )),
            _ => {
                return Err(PilError::ValueError(format!(
                    "Unsupported mode: {}",
                    mode
                )))
            }
        };
        let explicit = if matches!(mode, "CMYK" | "YCbCr" | "HSV" | "I" | "F") {
            Some(mode.to_string())
        } else {
            None
        };
        Ok(Image::Loaded(img, explicit))
    }

    /// Create image from raw bytes: `Image.frombytes(mode, size, data)`.
    pub fn frombytes(mode: &str, size: (u32, u32), data: &[u8]) -> Result<Self, PilError> {
        let (w, h) = size;
        if w == 0 || h == 0 {
            return Err(PilError::ValueError("frombytes: size must be > 0".into()));
        }
        let expected = match mode {
            "L" => (w * h) as usize,
            "LA" => (w * h * 2) as usize,
            "RGB" => (w * h * 3) as usize,
            "RGBA" => (w * h * 4) as usize,
            _ => return Err(PilError::ValueError(format!("frombytes: unsupported mode {}", mode))),
        };
        if data.len() < expected {
            return Err(PilError::ValueError(format!(
                "frombytes: expected {} bytes, got {}",
                expected,
                data.len()
            )));
        }
        let img = match mode {
            "L" => DynamicImage::ImageLuma8(
                image::GrayImage::from_raw(w, h, data[..expected].to_vec())
                    .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
            ),
            "RGB" => DynamicImage::ImageRgb8(
                image::RgbImage::from_raw(w, h, data[..expected].to_vec())
                    .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
            ),
            "RGBA" => DynamicImage::ImageRgba8(
                image::RgbaImage::from_raw(w, h, data[..expected].to_vec())
                    .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
            ),
            "LA" => DynamicImage::ImageLumaA8(
                image::GrayAlphaImage::from_raw(w, h, data[..expected].to_vec())
                    .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
            ),
            _ => DynamicImage::new_rgba8(w, h),
        };
        Ok(Image::Loaded(img, None))
    }

    pub fn open(path: &str, format: Option<&str>) -> Result<Self, PilError> {
        let fmt = format
            .and_then(|f| parse_format_str(f).ok())
            .or_else(|| ImageFormat::from_path(PathBuf::from(path)).ok());
        // Check if PNG file has palette chunk
        let is_paletted = fmt == Some(ImageFormat::Png) && {
            std::fs::read(path)
                .map(|data| has_plte_chunk(&data))
                .unwrap_or(false)
        };
        Ok(Image::Path {
            path: PathBuf::from(path),
            format: fmt,
            is_paletted,
        })
    }

    pub fn open_bytes(data: Vec<u8>) -> Result<Self, PilError> {
        let format = {
            let cursor = std::io::Cursor::new(&data);
            image::ImageReader::new(cursor)
                .with_guessed_format()
                .ok()
                .and_then(|r| r.format())
                .or_else(|| detect_format_from_magic(&data))
        };
        let is_paletted = format == Some(ImageFormat::Png) && has_plte_chunk(&data);
        Ok(Image::Bytes {
            data: Arc::new(data),
            format,
            is_paletted,
        })
    }

    // ── Materialize ──

    /// Execute the pipeline chain and return a decoded DynamicImage.
    /// This is where all the lazy work gets done.
    pub fn materialize(&self) -> Result<DynamicImage, PilError> {
        match self {
            Image::Loaded(img, _) => Ok(img.clone()),
            Image::Path { path, .. } => {
                let img = image::open(path).map_err(PilError::ImageError)?;
                Ok(img)
            }
            Image::Bytes { data, .. } => {
                let cursor = std::io::Cursor::new(data.as_ref());
                let reader = image::ImageReader::new(cursor)
                    .with_guessed_format()
                    .map_err(PilError::Io)?;
                reader.decode().map_err(PilError::ImageError)
            }
            Image::Pipeline { source, ops, .. } => {
                let mut img = source.materialize()?;
                for op in ops {
                    img = execute_op(&img, op)?;
                }
                Ok(img)
            }
        }
    }

    // ── Pipeline ops ──

    /// Append an op to the pipeline chain.
    /// If the current Image is already a Pipeline, appends to its ops vec.
    /// Otherwise wraps in a new Pipeline.
    pub fn push_op(source: &Image, op: PipelineOp) -> Image {
        match source {
            Image::Pipeline { source, ops, format } => {
                let mut new_ops = ops.clone();
                new_ops.push(op);
                Image::Pipeline {
                    source: Arc::clone(source),
                    ops: new_ops,
                    format: *format,
                }
            }
            other => Image::Pipeline {
                source: Arc::new(other.clone()),
                ops: vec![op],
                format: None,
            },
        }
    }

    // ── Immediate ops (force materialize) ──

    pub fn getpixel(&self, x: u32, y: u32) -> Result<(u8, u8, u8, u8), PilError> {
        let img = self.materialize()?;
        let rgba = img.get_pixel(x, y).0;
        Ok((rgba[0], rgba.get(1).copied().unwrap_or(0), rgba.get(2).copied().unwrap_or(0), rgba.get(3).copied().unwrap_or(255)))
    }

    /// Set a single pixel. Mutates self in-place.
    pub fn putpixel(&mut self, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) -> Result<(), PilError> {
        // Defer via pipeline — consistent with all other ops
        let new_self = Image::push_op(self, PipelineOp::PutPixel { x, y, color: (r, g, b, a) });
        *self = new_self;
        Ok(())
    }

    pub fn getbands(&self) -> Result<Vec<String>, PilError> {
        // Check explicit mode for non-standard band names
        if let Image::Loaded(_, Some(m)) = self {
            let bands: Vec<String> = match m.as_str() {
                "CMYK" => vec!["C".to_string(), "M".to_string(), "Y".to_string(), "K".to_string()],
                "YCbCr" => vec!["Y".to_string(), "Cb".to_string(), "Cr".to_string()],
                "HSV" => vec!["H".to_string(), "S".to_string(), "V".to_string()],
                "I" | "F" | "P" | "1" => vec![m.clone()],
                _ => vec![],
            };
            if !bands.is_empty() {
                return Ok(bands.iter().map(|s| s.to_string()).collect());
            }
        }
        let img = self.materialize()?;
        let bands = match img.color().channel_count() {
            1 => vec!["L".to_string()],
            2 => vec!["L".to_string(), "A".to_string()],
            3 => vec!["R".to_string(), "G".to_string(), "B".to_string()],
            4 => vec!["R".to_string(), "G".to_string(), "B".to_string(), "A".to_string()],
            _ => vec!["?".to_string()],
        };
        Ok(bands)
    }

    pub fn save(&self, path: &str, format: Option<&str>) -> Result<(), PilError> {
        let img = self.materialize()?;
        let save_format = if let Some(fmt) = format {
            parse_format_str(fmt)?
        } else {
            ImageFormat::from_path(path).map_err(|_| {
                PilError::UnknownFormat("Cannot determine format from path".into())
            })?
        };
        img.save_with_format(path, save_format)
            .map_err(PilError::ImageError)
    }

    pub fn tobytes(&self) -> Result<Vec<u8>, PilError> {
        Ok(self.materialize()?.as_bytes().to_vec())
    }

    /// Encode image to PNG bytes.
    pub fn to_png_bytes(&self) -> Result<Vec<u8>, PilError> {
        let img = self.materialize()?;
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png)
            .map_err(PilError::ImageError)?;
        Ok(buf.into_inner())
    }

    pub fn size(&self) -> Result<(u32, u32), PilError> {
        let img = self.materialize()?;
        Ok((img.width(), img.height()))
    }

    pub fn mode(&self) -> Result<String, PilError> {
        if let Image::Loaded(_, Some(m)) = self {
            return Ok(m.clone());
        }
        let img = self.materialize()?;
        // Check format-based mode for Path/Bytes
        let (fmt, is_paletted) = match self {
            Image::Path { format, is_paletted: ip, .. } => (*format, *ip),
            Image::Bytes { format, is_paletted: ip, .. } => (*format, *ip),
            _ => (None, false),
        };
        let mut detected = detect_format_mode(&img, fmt);
        if detected.is_none() && is_paletted {
            detected = Some("P".to_string());
        }
        if let Some(d) = detected {
            return Ok(d);
        }
        Ok(color_type_to_mode(img.color()).to_string())
    }

    pub fn format_name(&self) -> Option<String> {
        match self {
            Image::Loaded(_, _) => None,
            Image::Path { format, .. } => format.map(|f| format!("{:?}", f).to_uppercase()),
            Image::Bytes { format, .. } => format.map(|f| format!("{:?}", f).to_uppercase()),
            Image::Pipeline { format, .. } => format.map(|f| format!("{:?}", f).to_uppercase()),
        }
    }

    /// Load pixel data (no-op in Rust — data is always loaded). Returns Ok.
    pub fn load(&self) -> Result<(), PilError> {
        self.materialize()?;
        Ok(())
    }

    pub fn copy(&self) -> Self {
        self.clone()
    }

    /// Get pixel data as sequence. Returns per-channel values in display order.
    pub fn getdata(&self, band: Option<i32>) -> Result<Vec<u8>, PilError> {
        let img = self.materialize()?;
        let band = band.unwrap_or(-1);
        if band >= 0 {
            let rgba = img.to_rgba8();
            let b = band.min(3) as usize;
            return Ok(rgba.pixels().map(|p| p[b]).collect());
        }
        match img.color() {
            image::ColorType::L8 | image::ColorType::L16 => {
                let gray = img.to_luma8();
                Ok(gray.into_raw())
            }
            image::ColorType::La8 | image::ColorType::La16 => {
                let ga = img.to_luma_alpha8();
                let mut out = Vec::with_capacity((ga.width() * ga.height() * 2) as usize);
                for p in ga.pixels() {
                    out.push(p[0]);
                    out.push(p[1]);
                }
                Ok(out)
            }
            image::ColorType::Rgb8 | image::ColorType::Rgb16 | image::ColorType::Rgb32F => {
                let rgb = img.to_rgb8();
                Ok(rgb.into_raw())
            }
            _ => {
                let rgba = img.to_rgba8();
                Ok(rgba.into_raw())
            }
        }
    }

    /// Set pixel data from a flat byte sequence (matching image mode dimensions).
    /// Pipelined — data is stored and applied lazily at materialize time.
    pub fn putdata(&mut self, data: &[u8]) -> Result<(), PilError> {
        let new_self = Image::push_op(self, PipelineOp::PutData { data: data.to_vec() });
        *self = new_self;
        Ok(())
    }

    /// Extract a single channel as an L-mode image.
    pub fn getchannel(&self, channel: i32) -> Result<Image, PilError> {
        let img = self.materialize()?;
        let bands = img.color().channel_count();
        let ch = if channel < 0 {
            (bands as i32 + channel) as usize
        } else {
            channel as usize
        };
        if ch >= bands as usize {
            return Err(PilError::ValueError(format!(
                "Channel {} out of range (0-{})",
                channel,
                bands - 1
            )));
        }
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let mut gray = image::GrayImage::new(w, h);
        for (gp, rp) in gray.pixels_mut().zip(rgba.pixels()) {
            gp[0] = rp[ch.min(3)];
        }
        Ok(Image::Loaded(DynamicImage::ImageLuma8(gray), None))
    }

    /// Set/replace alpha channel. Preserves mode: L→LA, RGB→RGBA, LA→LA, RGBA→RGBA.
    /// Set alpha channel: L→LA, RGB→RGBA, LA→replace alpha. Pipelined.
    pub fn putalpha(&mut self, alpha: u8) -> Result<(), PilError> {
        let new_self = Image::push_op(self, PipelineOp::PutAlpha { alpha });
        *self = new_self;
        Ok(())
    }

    /// Get unique colors and their counts.
    /// Returns (count, color) pairs. Color is Vec<u8> matching the image mode.
    #[allow(clippy::type_complexity)]
    pub fn getcolors(&self, maxcolors: u32) -> Result<Option<Vec<(u32, Vec<u8>)>>, PilError> {
        let img = self.materialize()?;
        let n_bands = match img.color() {
            image::ColorType::L8 | image::ColorType::L16 => 1,
            image::ColorType::La8 | image::ColorType::La16 => 2,
            image::ColorType::Rgb8 | image::ColorType::Rgb16 => 3,
            _ => 4,
        };
        let rgba = img.to_rgba8();
        let mut counts: std::collections::HashMap<Vec<u8>, u32> = std::collections::HashMap::new();
        for p in rgba.pixels() {
            let key: Vec<u8> = p.0[..n_bands].to_vec();
            *counts.entry(key).or_insert(0) += 1;
        }
        if counts.len() > maxcolors as usize {
            return Ok(None);
        }
        let result: Vec<_> = counts.into_iter().map(|(k, v)| (v, k)).collect();
        Ok(Some(result))
    }

    /// Get entropy of the image. Uses per-band histogram matching PIL.
    pub fn entropy(&self) -> Result<f64, PilError> {
        let img = self.materialize()?;
        let rgba = img.to_rgba8();
        let n_bands = match img.color() {
            image::ColorType::L8 | image::ColorType::L16 => 1,
            image::ColorType::La8 | image::ColorType::La16 => 2,
            image::ColorType::Rgb8 | image::ColorType::Rgb16 => 3,
            _ => 4,
        };
        let mut hists = vec![[0u32; 256]; n_bands];
        for px in rgba.pixels() {
            for b in 0..n_bands {
                hists[b][px[b] as usize] += 1;
            }
        }
        let total = (rgba.width() * rgba.height() * n_bands as u32) as f64;
        let mut entropy = 0.0f64;
        for band_hist in &hists {
            for &h in band_hist {
                if h > 0 {
                    let p = h as f64 / total;
                    entropy -= p * p.log2();
                }
            }
        }
        Ok(entropy)
    }

    /// Get horizontal and vertical projections.
    /// PIL returns 1 if the column/row contains any non-zero pixel, 0 otherwise.
    pub fn getprojection(&self) -> Result<(Vec<u32>, Vec<u32>), PilError> {
        let img = self.materialize()?;
        let (w, h) = (img.width() as usize, img.height() as usize);
        let mut h_proj = vec![0u32; w];
        let mut v_proj = vec![0u32; h];
        let luma = img.to_luma8();
        for (x, y, px) in luma.enumerate_pixels() {
            if px[0] != 0 {
                h_proj[x as usize] = 1;
                v_proj[y as usize] = 1;
            }
        }
        Ok((h_proj, v_proj))
    }

    /// Convert to X11 bitmap format (returns raw bitmap data).
    pub fn tobitmap(&self) -> Result<Vec<u8>, PilError> {
        let img = self.materialize()?;
        let gray = img.to_luma8();
        let (w, h) = (gray.width(), gray.height());
        let row_bytes = w.div_ceil(8) as usize;
        let mut bmp = vec![0u8; row_bytes * h as usize];
        for y in 0..h {
            for x in 0..w {
                let v = gray.get_pixel(x, y)[0];
                if v < 128 {
                    let byte_idx = (x / 8) as usize;
                    let bit_idx = x % 8;
                    bmp[(y as usize) * row_bytes + byte_idx] |= 1u8 << bit_idx;
                }
            }
        }
        Ok(bmp)
    }

    /// Seek to frame in multi-frame image. Stub for now (no multi-frame support).
    pub fn seek(&self, _frame: u32) -> Result<(), PilError> {
        Ok(())
    }

    /// Return current frame number.
    pub fn tell(&self) -> u32 {
        0
    }

    /// Remap palette using a destination map.
    pub fn remap_palette(&self, dest_map: &[u8]) -> Result<Image, PilError> {
        let img = self.materialize()?;
        let rgb = img.to_rgb8();
        let (w, h) = rgb.dimensions();
        let mut out = image::RgbImage::new(w, h);
        for (op, ip) in out.pixels_mut().zip(rgb.pixels()) {
            op[0] = *dest_map.get(ip[0] as usize).unwrap_or(&ip[0]);
            op[1] = *dest_map.get(ip[1] as usize).unwrap_or(&ip[1]);
            op[2] = *dest_map.get(ip[2] as usize).unwrap_or(&ip[2]);
        }
        Ok(Image::Loaded(DynamicImage::ImageRgb8(out), None))
    }
}

// ── Helper for per-channel binary operations ──

fn channel_op_binary(
    img: &DynamicImage,
    other: &Arc<Image>,
    op: impl Fn(u8, u8) -> u8,
) -> Result<DynamicImage, PilError> {
    let other_img = other.materialize()?;
    let a = img.to_rgb8();
    let b = other_img.to_rgb8();
    let (w, h) = (a.width().min(b.width()), a.height().min(b.height()));
    let mut out = image::RgbImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let pa = a.get_pixel(x, y);
            let pb = b.get_pixel(x, y);
            out.put_pixel(
                x,
                y,
                image::Rgb([op(pa[0], pb[0]), op(pa[1], pb[1]), op(pa[2], pb[2])]),
            );
        }
    }
    Ok(preserve_mode(img, DynamicImage::ImageRgb8(out)))
}

/// Check if PNG data contains a PLTE (palette) chunk.
fn has_plte_chunk(data: &[u8]) -> bool {
    if data.len() < 33 { return false; } // 8 sig + 4 len + 4 IHDR + 13 data + 4 crc = 33 min
    let mut pos = 8; // Skip PNG signature
    while pos + 8 <= data.len() {
        let chunk_type = &data[pos + 4..pos + 8];
        if chunk_type == b"PLTE" {
            return true;
        }
        if chunk_type == b"IDAT" || chunk_type == b"IEND" {
            return false; // PLTE must come before IDAT
        }
        let len = u32::from_be_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
        pos += 12 + len; // length(4) + type(4) + data(len) + crc(4)
    }
    false
}

/// Detect image format from magic bytes.
fn detect_format_from_magic(data: &[u8]) -> Option<ImageFormat> {
    if data.len() >= 3 && &data[..3] == b"GIF" {
        Some(ImageFormat::Gif)
    } else if data.len() >= 8 && &data[..8] == b"\x89PNG\r\n\x1a\n" {
        Some(ImageFormat::Png)
    } else if data.len() >= 2 && &data[..2] == b"BM" {
        Some(ImageFormat::Bmp)
    } else if data.len() >= 2 && &data[..2] == b"\xff\xd8" {
        Some(ImageFormat::Jpeg)
    } else {
        None
    }
}

/// Detect the correct PIL mode from format + color type after decoding.
fn detect_format_mode(img: &DynamicImage, format: Option<ImageFormat>) -> Option<String> {
    match format {
        Some(ImageFormat::Gif) => {
            // GIF: bilevel source becomes L in PIL, RGB source becomes P
            let ch = img.color().channel_count();
            if ch == 1 || ch == 2 || is_bilevel(img) {
                Some("L".to_string())
            } else {
                Some("P".to_string())
            }
        }
        Some(ImageFormat::Png) => {
            let ch = img.color().channel_count();
            if ch == 1 || ch == 2 {
                if is_bilevel(img) {
                    Some("1".to_string())
                } else if ch == 2 {
                    Some("LA".to_string())
                } else {
                    Some("L".to_string())
                }
            } else {
                None // Determined by caller via is_paletted flag
            }
        }
        Some(ImageFormat::Bmp) => {
            if is_grayscale_rgb(img) {
                Some("L".to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Check if all pixel values are exactly 0 or 255 (bilevel image).
fn is_bilevel(img: &DynamicImage) -> bool {
    let luma = img.to_luma8();
    luma.pixels().all(|p| p[0] == 0 || p[0] == 255)
}

/// Check if an RGB image is actually grayscale (all channels equal per pixel).
fn is_grayscale_rgb(img: &DynamicImage) -> bool {
    if img.color().channel_count() < 3 {
        return true;
    }
    let rgb = img.to_rgb8();
    rgb.pixels().all(|p| p[0] == p[1] && p[1] == p[2])
}

/// Helper: preserve the color mode of the input image after operations
/// that may convert to RGBA (e.g., the `image` crate's resize always returns RGBA).
fn preserve_mode(original: &DynamicImage, result: DynamicImage) -> DynamicImage {
    let orig_color = original.color();
    let res_color = result.color();
    if orig_color == res_color {
        return result;
    }
    match orig_color {
        image::ColorType::L8 => DynamicImage::ImageLuma8(result.to_luma8()),
        image::ColorType::La8 => DynamicImage::ImageLumaA8(result.to_luma_alpha8()),
        image::ColorType::Rgb8 => DynamicImage::ImageRgb8(result.to_rgb8()),
        image::ColorType::Rgba8 => DynamicImage::ImageRgba8(result.to_rgba8()),
        _ => result,
    }
}

/// Generic rank filter: sorts neighborhood values and picks the one at `rank`.
fn rank_filter_impl(img: &DynamicImage, size: u32, rank: u32) -> Result<DynamicImage, PilError> {
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let mut out = image::RgbImage::new(w, h);
    let half = (size / 2) as i32;
    let area = (size * size) as usize;
    let rank = rank.min((area - 1) as u32) as usize;

    for y in 0..h {
        for x in 0..w {
            let mut r_vals = Vec::with_capacity(area);
            let mut g_vals = Vec::with_capacity(area);
            let mut b_vals = Vec::with_capacity(area);
            for dy in -half..=half {
                for dx in -half..=half {
                    let sx = (x as i32 + dx).clamp(0, w as i32 - 1) as u32;
                    let sy = (y as i32 + dy).clamp(0, h as i32 - 1) as u32;
                    let p = rgb.get_pixel(sx, sy);
                    r_vals.push(p[0]);
                    g_vals.push(p[1]);
                    b_vals.push(p[2]);
                }
            }
            r_vals.sort_unstable();
            g_vals.sort_unstable();
            b_vals.sort_unstable();
            out.put_pixel(x, y, image::Rgb([r_vals[rank], g_vals[rank], b_vals[rank]]));
        }
    }
    Ok(preserve_mode(img, DynamicImage::ImageRgb8(out)))
}

/// Bilinear interpolation helper.
fn bilerp(v00: u8, v10: u8, v01: u8, v11: u8, fx: f64, fy: f64) -> u8 {
    let top = v00 as f64 * (1.0 - fx) + v10 as f64 * fx;
    let bot = v01 as f64 * (1.0 - fx) + v11 as f64 * fx;
    (top * (1.0 - fy) + bot * fy).round().clamp(0.0, 255.0) as u8
}

/// Execute a single PipelineOp against a DynamicImage.
/// Each op borrows the input, allocates and returns the output.
pub fn execute_op(img: &DynamicImage, op: &PipelineOp) -> Result<DynamicImage, PilError> {
    match op {
        // ── Geometry ──
        PipelineOp::Resize { w, h, filter } => {
            let f = match filter {
                ResampleFilter::Lanczos => image::imageops::FilterType::Lanczos3,
                ResampleFilter::Bilinear => image::imageops::FilterType::Triangle,
                ResampleFilter::Nearest => image::imageops::FilterType::Nearest,
                ResampleFilter::Bicubic => image::imageops::FilterType::CatmullRom,
                ResampleFilter::Box => image::imageops::FilterType::Gaussian,
                ResampleFilter::Hamming => image::imageops::FilterType::Lanczos3,
            };
            let result = DynamicImage::from(image::imageops::resize(img, *w, *h, f));
            Ok(preserve_mode(img, result))
        }
        PipelineOp::Crop { left, top, right, bottom } => {
            let w = right.saturating_sub(*left);
            let h = bottom.saturating_sub(*top);
            Ok(img.crop_imm(*left, *top, w, h))
        }
        PipelineOp::Rotate { angle, expand, fill } => {
            let deg = (angle.round() as i32).rem_euclid(360);
            // Fast path: exact 90-degree multiples
            // PIL rotates counterclockwise; image crate rotates clockwise.
            // PIL 90° CCW = image crate 270° CW, PIL 270° CCW = image crate 90° CW.
            let result = if (deg - 90).abs() < 2 || (deg - 90).abs() >= 358 {
                img.rotate270()  // 270° CW = 90° CCW (PIL)
            } else if (deg - 180).abs() < 2 {
                img.rotate180()
            } else if (deg - 270).abs() < 2 || (deg - 270).abs() >= 358 {
                img.rotate90()   // 90° CW = 270° CCW (PIL)
            } else {
                // Bilinear interpolation for arbitrary angles
                let rgba = img.to_rgba8();
                let (sw, sh) = (rgba.width() as f64, rgba.height() as f64);
                let rad = angle.to_radians();
                let (cos, sin) = (rad.cos(), rad.sin());
                // Compute bounding box of rotated image
                let corners = [(0.0, 0.0), (sw, 0.0), (sw, sh), (0.0, sh)];
                let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
                for &(cx, cy) in &corners {
                    let rx = cx * cos - cy * sin;
                    let ry = cx * sin + cy * cos;
                    min_x = min_x.min(rx); max_x = max_x.max(rx);
                    min_y = min_y.min(ry); max_y = max_y.max(ry);
                }
                let (dw, dh) = if *expand {
                    ((max_x - min_x).ceil() as u32, (max_y - min_y).ceil() as u32)
                } else {
                    (rgba.width(), rgba.height())
                };
                let fill_color = fill.unwrap_or((0, 0, 0, 0));
                let mut out = image::RgbaImage::from_pixel(dw, dh, image::Rgba([fill_color.0, fill_color.1, fill_color.2, fill_color.3]));
                let (ox, oy) = if *expand { (-min_x, -min_y) } else { (0.0, 0.0) };
                // Center rotation around image center
                let cx_src = sw / 2.0;
                let cy_src = sh / 2.0;
                let cx_dst = dw as f64 / 2.0;
                let cy_dst = dh as f64 / 2.0;
                for dy in 0..dh {
                    for dx in 0..dw {
                        // Map destination pixel to source coordinate (inverse rotation)
                        let sx_rel = (dx as f64 + ox - cx_dst) * cos + (dy as f64 + oy - cy_dst) * sin + cx_src;
                        let sy_rel = -(dx as f64 + ox - cx_dst) * sin + (dy as f64 + oy - cy_dst) * cos + cy_src;
                        if sx_rel >= 0.0 && sx_rel < sw - 1.0 && sy_rel >= 0.0 && sy_rel < sh - 1.0 {
                            let sx = sx_rel.floor() as u32;
                            let sy = sy_rel.floor() as u32;
                            let fx = sx_rel - sx as f64;
                            let fy = sy_rel - sy as f64;
                            let p00 = rgba.get_pixel(sx, sy);
                            let p10 = rgba.get_pixel(sx + 1, sy);
                            let p01 = rgba.get_pixel(sx, sy + 1);
                            let p11 = rgba.get_pixel(sx + 1, sy + 1);
                            for c in 0..4 {
                                let v = (1.0 - fx) * (1.0 - fy) * p00[c] as f64
                                    + fx * (1.0 - fy) * p10[c] as f64
                                    + (1.0 - fx) * fy * p01[c] as f64
                                    + fx * fy * p11[c] as f64;
                                out.get_pixel_mut(dx, dy)[c] = v.round() as u8;
                            }
                        }
                    }
                }
                DynamicImage::ImageRgba8(out)
            };
            Ok(preserve_mode(img, result))
        }
        PipelineOp::Transpose { method } => match method {
            TransposeMethod::FlipLeftRight => Ok(img.fliph()),
            TransposeMethod::FlipTopBottom => Ok(img.flipv()),
            TransposeMethod::Rotate90 => Ok(img.rotate90()),
            TransposeMethod::Rotate180 => Ok(img.rotate180()),
            TransposeMethod::Rotate270 => Ok(img.rotate270()),
            TransposeMethod::Transpose => {
                Ok(img.rotate90().fliph())
            }
            TransposeMethod::Transverse => {
                Ok(img.rotate270().fliph())
            }
        },
        PipelineOp::Thumbnail { w, h, filter } => {
            let f = match filter {
                ResampleFilter::Lanczos => image::imageops::FilterType::Lanczos3,
                ResampleFilter::Bilinear => image::imageops::FilterType::Triangle,
                ResampleFilter::Nearest => image::imageops::FilterType::Nearest,
                ResampleFilter::Bicubic => image::imageops::FilterType::CatmullRom,
                ResampleFilter::Box => image::imageops::FilterType::Gaussian,
                ResampleFilter::Hamming => image::imageops::FilterType::Lanczos3,
            };
            let (cur_w, cur_h) = (img.width(), img.height());
            if *w == 0 || *h == 0 {
                return Err(PilError::ValueError("thumbnail size must be > 0".into()));
            }
            let scale = (*w as f64 / cur_w as f64).min(*h as f64 / cur_h as f64);
            let new_w = (cur_w as f64 * scale) as u32;
            let new_h = (cur_h as f64 * scale) as u32;
            let result = DynamicImage::from(image::imageops::resize(img, new_w.max(1), new_h.max(1), f));
            Ok(preserve_mode(img, result))
        }
        PipelineOp::Reduce { factor } => {
            if *factor < 2 {
                return Ok(img.clone());
            }
            let new_w = img.width() / factor;
            let new_h = img.height() / factor;
            let result = DynamicImage::from(image::imageops::resize(img, new_w.max(1), new_h.max(1), image::imageops::FilterType::Nearest));
            Ok(preserve_mode(img, result))
        }

        // ── Color/Convert ──
        PipelineOp::Convert { mode, matrix: _, dither: _ } => match mode {
            ColorMode::L => Ok(DynamicImage::ImageLuma8(crate::color::pil_grayscale(img))),
            ColorMode::LA => {
                let gray = crate::color::pil_grayscale(img);
                let (w, h) = gray.dimensions();
                let mut ga = image::GrayAlphaImage::new(w, h);
                for (gap, gp) in ga.pixels_mut().zip(gray.pixels()) {
                    gap[0] = gp[0];
                    gap[1] = 255;
                }
                Ok(DynamicImage::ImageLumaA8(ga))
            }
            ColorMode::RGB => Ok(DynamicImage::ImageRgb8(img.to_rgb8())),
            ColorMode::RGBA => Ok(DynamicImage::ImageRgba8(img.to_rgba8())),
            ColorMode::Mode1 => {
                // PIL convert to "1": threshold at 128 (no dither for now)
                let gray = crate::color::pil_grayscale(img);
                let (w, h) = gray.dimensions();
                let mut out = image::GrayImage::new(w, h);
                for (op, gp) in out.pixels_mut().zip(gray.pixels()) {
                    op[0] = if gp[0] >= 128 { 255 } else { 0 };
                }
                Ok(DynamicImage::ImageLuma8(out))
            }
            _ => Err(PilError::NotImplementedError(format!(
                "Convert to {:?} not yet implemented",
                mode
            ))),
        },
        PipelineOp::Quantize { colors, dither } => {
            let rgb = img.to_rgb8();
            let (w, h) = rgb.dimensions();
            let n = (w * h) as usize;
            if n == 0 {
                return Err(PilError::ValueError("quantize: empty image".into()));
            }
            let colors = (*colors).clamp(2, 256) as usize;
            // color_quant expects RGBA format (4 bytes per pixel)
            let rgb_raw = rgb.into_raw();
            let mut rgba_data: Vec<u8> = Vec::with_capacity(n * 4);
            for i in 0..n {
                let base = i * 3;
                if base + 2 < rgb_raw.len() {
                    rgba_data.push(rgb_raw[base]);
                    rgba_data.push(rgb_raw[base + 1]);
                    rgba_data.push(rgb_raw[base + 2]);
                    rgba_data.push(255);
                }
            }
            if rgba_data.len() < colors * 4 {
                return Err(PilError::ValueError("quantize: not enough pixel data".into()));
            }
            let nq = color_quant::NeuQuant::new(10, colors, &rgba_data);
            let _ = dither;
            if colors >= 256 {
                let palette = nq.color_map_rgb();
                let mut out = image::RgbImage::new(w, h);
                for (i, op) in out.pixels_mut().enumerate() {
                    if i >= n { break; }
                    let pixel = &rgba_data[i * 4..i * 4 + 4]; // RGBA pixel
                    let idx = nq.index_of(pixel);
                    if idx * 3 + 2 < palette.len() {
                        op[0] = palette[idx * 3];
                        op[1] = palette[idx * 3 + 1];
                        op[2] = palette[idx * 3 + 2];
                    }
                }
                Ok(preserve_mode(img, DynamicImage::ImageRgb8(out)))
            } else {
                let mut out = image::RgbImage::new(w, h);
                for (i, op) in out.pixels_mut().enumerate() {
                    if i >= n { break; }
                    let pixel = &rgba_data[i * 4..i * 4 + 4];
                    let idx = nq.index_of(pixel);
                    if let Some(entry) = nq.lookup(idx) {
                        op[0] = entry[0];
                        op[1] = entry[1];
                        op[2] = entry[2];
                    }
                }
                Ok(preserve_mode(img, DynamicImage::ImageRgb8(out)))
            }
        }
        PipelineOp::RemapPalette { dest_map } => {
            let rgb = img.to_rgb8();
            let (w, h) = rgb.dimensions();
            let mut out = image::RgbImage::new(w, h);
            for (op, ip) in out.pixels_mut().zip(rgb.pixels()) {
                op[0] = *dest_map.get(ip[0] as usize).unwrap_or(&ip[0]);
                op[1] = *dest_map.get(ip[1] as usize).unwrap_or(&ip[1]);
                op[2] = *dest_map.get(ip[2] as usize).unwrap_or(&ip[2]);
            }
            Ok(preserve_mode(img, DynamicImage::ImageRgb8(out)))
        }

        // ── Filters ──
        PipelineOp::Filter3x3 {
            kernel,
            scale,
            offset,
        } => {
            let rgb = img.to_rgb8();
            let (w, h) = rgb.dimensions();
            let inv_scale = 1.0 / scale;
            let mut out = image::RgbImage::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    let mut r = 0f32;
                    let mut g = 0f32;
                    let mut b = 0f32;
                    for ky in 0..3i32 {
                        for kx in 0..3i32 {
                            let sx = (x as i32 + kx - 1).clamp(0, w as i32 - 1) as u32;
                            let sy = (y as i32 + ky - 1).clamp(0, h as i32 - 1) as u32;
                            let px = rgb.get_pixel(sx, sy);
                            let ki = (ky * 3 + kx) as usize;
                            r += px[0] as f32 * kernel[ki];
                            g += px[1] as f32 * kernel[ki];
                            b += px[2] as f32 * kernel[ki];
                        }
                    }
                    out.put_pixel(
                        x,
                        y,
                        image::Rgb([
                            (r * inv_scale + *offset as f32).clamp(0.0, 255.0).round() as u8,
                            (g * inv_scale + *offset as f32).clamp(0.0, 255.0).round() as u8,
                            (b * inv_scale + *offset as f32).clamp(0.0, 255.0).round() as u8,
                        ]),
                    );
                }
            }
            Ok(preserve_mode(img, DynamicImage::ImageRgb8(out)))
        }
        PipelineOp::GaussianBlur { sigma } => Ok(img.blur(*sigma)),
        PipelineOp::BoxBlur { radius } => Ok(img.blur(*radius as f32)),
        PipelineOp::MedianFilter { size } => {
            rank_filter_impl(img, *size, *size * *size / 2)
        }
        PipelineOp::MaxFilter { size } => {
            rank_filter_impl(img, *size, *size * *size - 1)
        }
        PipelineOp::MinFilter { size } => {
            rank_filter_impl(img, *size, 0)
        }
        PipelineOp::RankFilter { size, rank } => {
            rank_filter_impl(img, *size, *rank)
        }

        // ── ImageOps ──
        PipelineOp::Autocontrast { cutoff } => {
            let gray = img.to_luma8();
            let total = gray.len() as f64;
            let low_thresh = (total * cutoff / 100.0) as usize;
            let high_thresh = (total * (100.0 - cutoff) / 100.0) as usize;
            let mut sorted: Vec<u8> = gray.iter().copied().collect();
            sorted.sort_unstable();
            let lo = *sorted.get(low_thresh).unwrap_or(&0);
            let hi = *sorted
                .get(high_thresh.min(sorted.len() - 1))
                .unwrap_or(&255);
            if hi <= lo {
                return Ok(img.clone());
            }
            let mut rgb = img.to_rgb8();
            let scale = 255.0 / (hi - lo) as f64;
            let lo_f = lo as f64;
            for p in rgb.pixels_mut() {
                for c in 0..3 {
                    p[c] = ((p[c] as f64 - lo_f) * scale).clamp(0.0, 255.0) as u8;
                }
            }
            Ok(preserve_mode(img, DynamicImage::ImageRgb8(rgb)))
        }
        PipelineOp::Equalize => {
            let luma = img.to_luma8();
            let mut hist = [0u32; 256];
            for &p in luma.iter() {
                hist[p as usize] += 1;
            }
            let mut cdf = [0u32; 256];
            let mut acc = 0u32;
            for i in 0..256 {
                acc += hist[i];
                cdf[i] = acc;
            }
            let n = luma.len() as f64;
            let mut rgb = img.to_rgb8();
            for (px, lp) in rgb.pixels_mut().zip(luma.pixels()) {
                let mapped = (cdf[lp[0] as usize] as f64 * 255.0 / n).clamp(0.0, 255.0) as u8;
                for c in 0..3 {
                    px[c] = ((px[c] as f64 * mapped as f64 / 255.0).clamp(0.0, 255.0)) as u8;
                }
            }
            Ok(preserve_mode(img, DynamicImage::ImageRgb8(rgb)))
        }
        PipelineOp::Invert => {
            let mut rgb = img.to_rgb8();
            for p in rgb.pixels_mut() {
                for c in 0..3 {
                    p[c] = 255 - p[c];
                }
            }
            Ok(preserve_mode(img, DynamicImage::ImageRgb8(rgb)))
        }
        PipelineOp::Flip => Ok(img.flipv()),
        PipelineOp::Mirror => Ok(img.fliph()),
        PipelineOp::Posterize { bits } => {
            let mask = !((1u8 << (8 - bits)) - 1);
            let mut rgb = img.to_rgb8();
            for p in rgb.pixels_mut() {
                for c in 0..3 {
                    p[c] &= mask;
                }
            }
            Ok(preserve_mode(img, DynamicImage::ImageRgb8(rgb)))
        }
        PipelineOp::Solarize { threshold } => {
            let t = *threshold;
            let mut rgb = img.to_rgb8();
            for p in rgb.pixels_mut() {
                for c in 0..3 {
                    if p[c] >= t {  // PIL uses >=, not >
                        p[c] = 255 - p[c];
                    }
                }
            }
            Ok(preserve_mode(img, DynamicImage::ImageRgb8(rgb)))
        }
        PipelineOp::Grayscale => {
            Ok(DynamicImage::ImageLuma8(crate::color::pil_grayscale(img)))
        }
        PipelineOp::Colorize { black, white } => {
            let gray = img.to_luma8();
            let (w, h) = gray.dimensions();
            let mut out = image::RgbImage::new(w, h);
            let &(br, bg, bb) = black;
            let &(wr, wg, wb) = white;
            for y in 0..h {
                for x in 0..w {
                    let g = gray.get_pixel(x, y)[0] as f64 / 255.0;
                    let r = (br as f64 + g * (wr as f64 - br as f64)) as u8;
                    let gv = (bg as f64 + g * (wg as f64 - bg as f64)) as u8;
                    let b = (bb as f64 + g * (wb as f64 - bb as f64)) as u8;
                    out.put_pixel(x, y, image::Rgb([r, gv, b]));
                }
            }
            Ok(preserve_mode(img, DynamicImage::ImageRgb8(out)))
        }
        PipelineOp::Contain { w, h, .. } => {
            let (w, h) = (*w, *h);
            let (iw, ih) = (img.width(), img.height());
            let ratio = (w as f64 / iw as f64).min(h as f64 / ih as f64);
            let nw = (iw as f64 * ratio) as u32;
            let nh = (ih as f64 * ratio) as u32;
            Ok(img.resize_exact(nw.max(1), nh.max(1), image::imageops::FilterType::Triangle))
        }
        PipelineOp::Cover { w, h, .. } => {
            let (w, h) = (*w, *h);
            let (iw, ih) = (img.width(), img.height());
            let ratio = (w as f64 / iw as f64).max(h as f64 / ih as f64);
            let nw = (iw as f64 * ratio) as u32;
            let nh = (ih as f64 * ratio) as u32;
            let resized = img.resize_exact(nw.max(1), nh.max(1), image::imageops::FilterType::Triangle);
            let x = (nw.saturating_sub(w)) / 2;
            let y = (nh.saturating_sub(h)) / 2;
            Ok(resized.crop_imm(x, y, w, h))
        }
        PipelineOp::Fit { w, h, .. } => {
            let (w, h) = (*w, *h);
            let (iw, ih) = (img.width(), img.height());
            let ratio = (w as f64 / iw as f64).min(h as f64 / ih as f64);
            let nw = (iw as f64 * ratio) as u32;
            let nh = (ih as f64 * ratio) as u32;
            Ok(img.resize_exact(nw.max(1), nh.max(1), image::imageops::FilterType::Triangle))
        }
        PipelineOp::Pad { w, h, color, .. } => {
            let (w, h) = (*w, *h);
            let fill = color.unwrap_or((0, 0, 0, 255));
            let (iw, ih) = (img.width(), img.height());
            let mut padded = DynamicImage::new_rgba8(w, h);
            for py in 0..h { for px in 0..w { padded.put_pixel(px, py, image::Rgba([fill.0, fill.1, fill.2, fill.3])); } }
            let x = (w.saturating_sub(iw)) / 2;
            let y = (h.saturating_sub(ih)) / 2;
            image::imageops::overlay(&mut padded, &img.to_rgba8(), x as i64, y as i64);
            Ok(padded)
        }
        PipelineOp::CropBorder { border } => {
            let b = *border;
            let (w, h) = (img.width(), img.height());
            if 2 * b >= w || 2 * b >= h {
                return Err(PilError::ValueError("crop border exceeds image dimensions".into()));
            }
            Ok(img.crop_imm(b, b, w - 2 * b, h - 2 * b))
        }
        PipelineOp::Scale { factor, filter } => {
            let f = match filter {
                ResampleFilter::Lanczos => image::imageops::FilterType::Lanczos3,
                ResampleFilter::Bilinear => image::imageops::FilterType::Triangle,
                ResampleFilter::Nearest => image::imageops::FilterType::Nearest,
                ResampleFilter::Bicubic => image::imageops::FilterType::CatmullRom,
                ResampleFilter::Box => image::imageops::FilterType::Gaussian,
                ResampleFilter::Hamming => image::imageops::FilterType::Lanczos3,
            };
            let new_w = (img.width() as f64 * factor).round() as u32;
            let new_h = (img.height() as f64 * factor).round() as u32;
            let result = DynamicImage::from(image::imageops::resize(img, new_w.max(1), new_h.max(1), f));
            Ok(preserve_mode(img, result))
        }
        PipelineOp::Expand { border, fill } => {
            let (w, h) = (img.width(), img.height());
            let new_w = w + 2 * border;
            let new_h = h + 2 * border;
            let mut expanded = DynamicImage::new_rgba8(new_w, new_h);
            for py in 0..new_h {
                for px in 0..new_w {
                    expanded.put_pixel(px, py, image::Rgba([fill.0, fill.1, fill.2, fill.3]));
                }
            }
            image::imageops::overlay(&mut expanded, &img.to_rgba8(), *border as i64, *border as i64);
            Ok(preserve_mode(img, expanded))
        }
        // ── ImageChops ──
        PipelineOp::Add { other, scale, offset } => {
            channel_op_binary(img, other, |a, b| {
                ((a as f64 + b as f64) * scale + offset).clamp(0.0, 255.0) as u8
            })
        }
        PipelineOp::Subtract { other, scale, offset } => {
            channel_op_binary(img, other, |a, b| {
                ((a as f64 - b as f64) * scale + offset).clamp(0.0, 255.0) as u8
            })
        }
        PipelineOp::Multiply { other } => channel_op_binary(img, other, |a, b| {
            // PIL uses integer division (truncation): (a*b) // 255
            ((a as u32 * b as u32) / 255) as u8
        }),
        PipelineOp::Screen { other } => channel_op_binary(img, other, |a, b| {
            (255u32 - ((255 - a as u32) * (255 - b as u32) / 255)) as u8
        }),
        PipelineOp::Darker { other } => channel_op_binary(img, other, |a, b| a.min(b)),
        PipelineOp::Lighter { other } => channel_op_binary(img, other, |a, b| a.max(b)),
        PipelineOp::Difference { other } => channel_op_binary(img, other, |a, b| {
            (a as i16 - b as i16).unsigned_abs() as u8
        }),
        PipelineOp::Overlay { other } => channel_op_binary(img, other, |base, blend| {
            // PIL uses float math with rounding
            let b = base as f64 / 255.0;
            let bl = blend as f64 / 255.0;
            if b < 0.5 {
                (2.0 * b * bl * 255.0).round() as u8
            } else {
                (255.0 - 2.0 * (1.0 - b) * (1.0 - bl) * 255.0).round() as u8
            }
        }),
        PipelineOp::HardLight { other } => channel_op_binary(img, other, |base, blend| {
            // PIL: HardLight mirrors Overlay with swapped roles
            let b = base as f64 / 255.0;
            let bl = blend as f64 / 255.0;
            if bl <= 0.5 {
                (2.0 * b * bl * 255.0).round() as u8
            } else {
                (255.0 - 2.0 * (1.0 - b) * (1.0 - bl) * 255.0).round() as u8
            }
        }),
        PipelineOp::SoftLight { other } => channel_op_binary(img, other, |base, blend| {
            // W3C soft-light formula (close to PIL, needs verification)
            let b = base as f64 / 255.0;
            let bl = blend as f64 / 255.0;
            let r = if bl <= 0.5 {
                b - (1.0 - 2.0 * bl) * b * (1.0 - b)
            } else if b <= 0.25 {
                b + (2.0 * bl - 1.0) * (((16.0 * b - 12.0) * b + 4.0) * b - b)
            } else {
                b + (2.0 * bl - 1.0) * (b.sqrt() - b)
            };
            (r * 255.0).round().clamp(0.0, 255.0) as u8
        }),
        PipelineOp::AddModulo { other } => {
            channel_op_binary(img, other, |a, b| a.wrapping_add(b))
        }
        PipelineOp::SubtractModulo { other } => {
            channel_op_binary(img, other, |a, b| a.wrapping_sub(b))
        }
        PipelineOp::LogicalAnd { other } => channel_op_binary(img, other, |a, b| a & b),
        PipelineOp::LogicalOr { other } => channel_op_binary(img, other, |a, b| a | b),
        PipelineOp::LogicalXor { other } => channel_op_binary(img, other, |a, b| a ^ b),
        PipelineOp::Constant { value } => {
            // PIL always returns an L-mode image for constant()
            let (w, h) = (img.width(), img.height());
            let mut out = image::GrayImage::new(w, h);
            for p in out.pixels_mut() {
                p[0] = *value;
            }
            Ok(DynamicImage::ImageLuma8(out))
        }
        PipelineOp::Offset { x, y } => {
            // PIL offset: positive (x,y) shifts content right/down.
            // Map dest (px,py) to source (px-x, py-y) with wrapping.
            let (w, h) = (img.width(), img.height());
            let mut result = DynamicImage::new_rgba8(w, h);
            let src_rgba = img.to_rgba8();
            for py in 0..h {
                for px in 0..w {
                    let sx = (px as i32 - x).rem_euclid(w as i32) as u32;
                    let sy = (py as i32 - y).rem_euclid(h as i32) as u32;
                    result.put_pixel(px, py, *src_rgba.get_pixel(sx, sy));
                }
            }
            Ok(preserve_mode(img, result))
        }
        PipelineOp::Blend { other, alpha } => {
            let other_img = other.materialize()?;
            let a = alpha.clamp(0.0, 1.0);
            let rgb1 = img.to_rgb8();
            let rgb2 = other_img.to_rgb8();
            let (w, h) = (rgb1.width().min(rgb2.width()), rgb1.height().min(rgb2.height()));
            let mut out = image::RgbImage::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    let p1 = rgb1.get_pixel(x, y);
                    let p2 = rgb2.get_pixel(x, y);
                    out.put_pixel(x, y, image::Rgb([
                        (p1[0] as f64 * (1.0 - a) + p2[0] as f64 * a) as u8,
                        (p1[1] as f64 * (1.0 - a) + p2[1] as f64 * a) as u8,
                        (p1[2] as f64 * (1.0 - a) + p2[2] as f64 * a) as u8,
                    ]));
                }
            }
            Ok(preserve_mode(img, DynamicImage::ImageRgb8(out)))
        }
        PipelineOp::Composite { other, mask } => {
            let other_img = other.materialize()?;
            let mask_img = mask.materialize()?;
            let rgb1 = img.to_rgb8();
            let rgb2 = other_img.to_rgb8();
            let mask_gray = mask_img.to_luma8();
            let (w, h) = (rgb1.width().min(rgb2.width()).min(mask_gray.width()),
                          rgb1.height().min(rgb2.height()).min(mask_gray.height()));
            let mut out = image::RgbImage::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    let p1 = rgb1.get_pixel(x, y);
                    let p2 = rgb2.get_pixel(x, y);
                    let m = mask_gray.get_pixel(x, y)[0] as f64 / 255.0;
                    out.put_pixel(x, y, image::Rgb([
                        ((p1[0] as f64 * m + p2[0] as f64 * (1.0 - m)).round()) as u8,
                        ((p1[1] as f64 * m + p2[1] as f64 * (1.0 - m)).round()) as u8,
                        ((p1[2] as f64 * m + p2[2] as f64 * (1.0 - m)).round()) as u8,
                    ]));
                }
            }
            Ok(preserve_mode(img, DynamicImage::ImageRgb8(out)))
        }
        PipelineOp::Duplicate => Ok(img.clone()),
        PipelineOp::InvertChops => {
            let mut rgb = img.to_rgb8();
            for p in rgb.pixels_mut() {
                for c in 0..3 {
                    p[c] = 255 - p[c];
                }
            }
            Ok(preserve_mode(img, DynamicImage::ImageRgb8(rgb)))
        }

        // ── Enhance ──
        PipelineOp::Brightness { factor } => {
            let mut rgb = img.to_rgb8();
            let f = *factor;
            for p in rgb.pixels_mut() {
                for c in 0..3 {
                    p[c] = ((p[c] as f64 * f).clamp(0.0, 255.0)) as u8;
                }
            }
            Ok(preserve_mode(img, DynamicImage::ImageRgb8(rgb)))
        }
        PipelineOp::Contrast { factor } => {
            let mut rgba = img.to_rgba8();
            let contrast = (*factor as f32 - 1.0) + 1.0;
            let w = (259.0 * (contrast + 255.0)) / (255.0 * (259.0 - contrast));
            for p in rgba.pixels_mut() {
                for c in 0..3 {
                    let v = w * (p[c] as f32 - 128.0) + 128.0;
                    p[c] = v.clamp(0.0, 255.0).round() as u8;
                }
            }
            Ok(DynamicImage::ImageRgba8(rgba))
        }
        PipelineOp::ColorSaturation { factor } => {
            let gray = img.to_luma8();
            let mut rgb = img.to_rgb8();
            let f = *factor;
            for (px, gp) in rgb.pixels_mut().zip(gray.pixels()) {
                let g = gp[0] as f64;
                px[0] = ((g + f * (px[0] as f64 - g)).clamp(0.0, 255.0)) as u8;
                px[1] = ((g + f * (px[1] as f64 - g)).clamp(0.0, 255.0)) as u8;
                px[2] = ((g + f * (px[2] as f64 - g)).clamp(0.0, 255.0)) as u8;
            }
            Ok(preserve_mode(img, DynamicImage::ImageRgb8(rgb)))
        }
        PipelineOp::Sharpness { factor } => {
            let f = *factor;
            if f <= 1.0 {
                let sigma = ((1.0 - f) * 5.0).max(0.01) as f32;
                Ok(img.blur(sigma))
            } else {
                let sigma = ((f - 1.0) * 0.5).max(0.01) as f32;
                let blurred = img.blur(sigma);
                let blur_rgb = blurred.to_rgb8();
                let mut rgb = img.to_rgb8();
                let amount = (f - 1.0).min(5.0);
                for (px, bp) in rgb.pixels_mut().zip(blur_rgb.pixels()) {
                    for c in 0..3 {
                        let diff = px[c] as f64 - bp[c] as f64;
                        px[c] = ((px[c] as f64 + diff * amount).clamp(0.0, 255.0)) as u8;
                    }
                }
                Ok(preserve_mode(img, DynamicImage::ImageRgb8(rgb)))
            }
        }

        // ── Effects ──
        PipelineOp::EffectSpread { distance } => {
            if *distance == 0 {
                return Ok(img.clone());
            }
            Ok(img.blur(*distance as f32))
        }
        PipelineOp::PutPixel { x, y, color } => {
            let mut rgba = img.to_rgba8();
            let (w, h) = (rgba.width(), rgba.height());
            if *x >= w || *y >= h {
                return Err(PilError::ValueError(format!(
                    "pixel ({},{}) out of bounds ({}x{})", x, y, w, h
                )));
            }
            rgba.put_pixel(*x, *y, image::Rgba([color.0, color.1, color.2, color.3]));
            Ok(preserve_mode(img, DynamicImage::ImageRgba8(rgba)))
        }
        PipelineOp::PutData { data } => {
            let (w, h) = (img.width() as usize, img.height() as usize);
            let expected = match img.color() {
                image::ColorType::L8 => w * h,
                image::ColorType::La8 => w * h * 2,
                image::ColorType::Rgb8 => w * h * 3,
                _ => w * h * 4,
            };
            if data.len() < expected {
                return Err(PilError::ValueError(format!(
                    "putdata: expected {} bytes, got {}", expected, data.len()
                )));
            }
            match img.color() {
                image::ColorType::Rgb8 => {
                    let rgb = image::RgbImage::from_raw(w as u32, h as u32, data[..expected].to_vec())
                        .ok_or_else(|| PilError::ValueError("putdata: buffer error".into()))?;
                    Ok(DynamicImage::ImageRgb8(rgb))
                }
                image::ColorType::L8 => {
                    let gray = image::GrayImage::from_raw(w as u32, h as u32, data[..expected].to_vec())
                        .ok_or_else(|| PilError::ValueError("putdata: buffer error".into()))?;
                    Ok(DynamicImage::ImageLuma8(gray))
                }
                _ => {
                    let rgba = image::RgbaImage::from_raw(w as u32, h as u32, data[..expected].to_vec())
                        .ok_or_else(|| PilError::ValueError("putdata: buffer error".into()))?;
                    Ok(DynamicImage::ImageRgba8(rgba))
                }
            }
        }
        PipelineOp::PutAlpha { alpha } => {
            let out = match img.color() {
                image::ColorType::L8 => {
                    let luma = img.to_luma8();
                    let mut la = image::GrayAlphaImage::new(luma.width(), luma.height());
                    for (o, i) in la.pixels_mut().zip(luma.pixels()) {
                        o[0] = i[0]; o[1] = *alpha;
                    }
                    DynamicImage::ImageLumaA8(la)
                }
                image::ColorType::La8 => {
                    let rgba = img.to_rgba8();
                    let mut la = image::GrayAlphaImage::new(rgba.width(), rgba.height());
                    for (o, i) in la.pixels_mut().zip(rgba.pixels()) {
                        o[0] = i[0]; o[1] = *alpha;
                    }
                    DynamicImage::ImageLumaA8(la)
                }
                image::ColorType::Rgb8 => {
                    let rgb = img.to_rgb8();
                    let mut rgba = image::RgbaImage::new(rgb.width(), rgb.height());
                    for (o, i) in rgba.pixels_mut().zip(rgb.pixels()) {
                        o[0] = i[0]; o[1] = i[1]; o[2] = i[2]; o[3] = *alpha;
                    }
                    DynamicImage::ImageRgba8(rgba)
                }
                _ => {
                    let mut rgba = img.to_rgba8();
                    for p in rgba.pixels_mut() { p[3] = *alpha; }
                    DynamicImage::ImageRgba8(rgba)
                }
            };
            Ok(out)
        }
        PipelineOp::Paste { source, x, y, w: _w, h: _h, mask } => {
            let src_img = source.materialize()?;
            let (src_w, src_h) = (src_img.width(), src_img.height());
            let paste_x = *x as i64;
            let paste_y = *y as i64;
            let _orig_color = img.color();

            if let Some(mask_img_ref) = mask {
                let mask_img = mask_img_ref.materialize()?;
                let mask_gray = mask_img.to_luma8();
                let mut dest_clone = img.to_rgba8();

                for py in 0..src_h.min(dest_clone.height()) {
                    for px in 0..src_w.min(dest_clone.width()) {
                        let mask_val = if px < mask_gray.width() && py < mask_gray.height() {
                            mask_gray.get_pixel(px, py)[0]
                        } else { 0 };
                        if mask_val == 0 { continue; }
                        let sp = src_img.get_pixel(px, py);
                        let dx = (paste_x + px as i64) as u32;
                        let dy = (paste_y + py as i64) as u32;
                        if dx >= dest_clone.width() || dy >= dest_clone.height() { continue; }
                        if mask_val == 255 {
                            dest_clone.put_pixel(dx, dy, sp);
                        } else {
                            let inv_alpha = 255u16 - mask_val as u16;
                            let dp = dest_clone.get_pixel(dx, dy);
                            let a = sp.0.get(3).copied().unwrap_or(255) as u16;
                            let da = dp.0.get(3).copied().unwrap_or(255) as u16;
                            let blended = image::Rgba([
                                ((sp[0] as u16 * mask_val as u16 + dp[0] as u16 * inv_alpha + 127) / 255) as u8,
                                ((sp[1] as u16 * mask_val as u16 + dp[1] as u16 * inv_alpha + 127) / 255) as u8,
                                ((sp[2] as u16 * mask_val as u16 + dp[2] as u16 * inv_alpha + 127) / 255) as u8,
                                ((a * mask_val as u16 + da * inv_alpha + 127) / 255) as u8,
                            ]);
                            dest_clone.put_pixel(dx, dy, blended);
                        }
                    }
                }
                Ok(preserve_mode(img, DynamicImage::ImageRgba8(dest_clone)))
            } else {
                let mut dest_clone = img.to_rgba8();
                image::imageops::overlay(
                    &mut dest_clone,
                    &src_img.to_rgba8(),
                    paste_x,
                    paste_y,
                );
                Ok(preserve_mode(img, DynamicImage::ImageRgba8(dest_clone)))
            }
        }
        PipelineOp::AlphaComposite { source, dest: _dest, src: _src } => {
            let src_img = source.materialize()?;
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
                    if out_a <= 0.0 { continue; }
                    let r = ((sp[0] as f64 * sa + dp[0] as f64 * da * (1.0 - sa)) / out_a).round().clamp(0.0, 255.0) as u8;
                    let g = ((sp[1] as f64 * sa + dp[1] as f64 * da * (1.0 - sa)) / out_a).round().clamp(0.0, 255.0) as u8;
                    let b = ((sp[2] as f64 * sa + dp[2] as f64 * da * (1.0 - sa)) / out_a).round().clamp(0.0, 255.0) as u8;
                    let a = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
                    dest_rgba.put_pixel(px, py, image::Rgba([r, g, b, a]));
                }
            }
            Ok(DynamicImage::ImageRgba8(dest_rgba))
        }

        // ── Module fns ──
        PipelineOp::Merge { mode, bands } => {
            // Merge bands into a multi-channel image.
            // The pipeline source is the first band; remaining bands are in `bands[1..]`.
            let _n_expected = match mode {
                ColorMode::RGB => 3,
                ColorMode::RGBA => 4,
                ColorMode::LA => 2,
                ColorMode::L | ColorMode::Mode1 => 1,
                _ => return Err(PilError::ValueError(format!("Unsupported merge mode: {:?}", mode))),
            };
            // Get pixel data from each band
            let mut band_pixels: Vec<Vec<u8>> = Vec::new();
            // First band is the current image
            let first_gray = img.to_luma8();
            let (w, h) = first_gray.dimensions();
            band_pixels.push(first_gray.into_raw());
            for band in bands.iter().skip(1) {
                let b_img = band.materialize()?;
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
                    let img = image::RgbImage::from_raw(w, h, rgb)
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
                    let img = image::RgbaImage::from_raw(w, h, rgba)
                        .ok_or_else(|| PilError::ValueError("merge: buffer error".into()))?;
                    Ok(DynamicImage::ImageRgba8(img))
                }
                ColorMode::LA => {
                    let mut la = vec![0u8; n * 2];
                    for i in 0..n {
                        la[i * 2] = band_pixels[0][i];
                        la[i * 2 + 1] = band_pixels[1][i];
                    }
                    let img = image::GrayAlphaImage::from_raw(w, h, la)
                        .ok_or_else(|| PilError::ValueError("merge: buffer error".into()))?;
                    Ok(DynamicImage::ImageLumaA8(img))
                }
                ColorMode::L | ColorMode::Mode1 => {
                    let img = image::GrayImage::from_raw(w, h, band_pixels.remove(0))
                        .ok_or_else(|| PilError::ValueError("merge: buffer error".into()))?;
                    Ok(DynamicImage::ImageLuma8(img))
                }
                _ => Err(PilError::ValueError("Unsupported merge mode".into())),
            }
        }
        PipelineOp::BlendModule { other, alpha } => {
            let other_img = other.materialize()?;
            let a = alpha.clamp(0.0, 1.0);
            let rgb1 = img.to_rgb8();
            let rgb2 = other_img.to_rgb8();
            let (w, h) = (rgb1.width().min(rgb2.width()), rgb1.height().min(rgb2.height()));
            let mut out = image::RgbImage::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    let p1 = rgb1.get_pixel(x, y);
                    let p2 = rgb2.get_pixel(x, y);
                    out.put_pixel(x, y, image::Rgb([
                        (p1[0] as f64 * (1.0 - a) + p2[0] as f64 * a) as u8,
                        (p1[1] as f64 * (1.0 - a) + p2[1] as f64 * a) as u8,
                        (p1[2] as f64 * (1.0 - a) + p2[2] as f64 * a) as u8,
                    ]));
                }
            }
            Ok(preserve_mode(img, DynamicImage::ImageRgb8(out)))
        }
        PipelineOp::CompositeModule { other, mask } => {
            let other_img = other.materialize()?;
            let mask_img = mask.materialize()?;
            let rgb1 = img.to_rgb8();
            let rgb2 = other_img.to_rgb8();
            let mask_gray = mask_img.to_luma8();
            let (w, h) = (rgb1.width().min(rgb2.width()).min(mask_gray.width()),
                          rgb1.height().min(rgb2.height()).min(mask_gray.height()));
            let mut out = image::RgbImage::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    let p1 = rgb1.get_pixel(x, y);
                    let p2 = rgb2.get_pixel(x, y);
                    let m = mask_gray.get_pixel(x, y)[0] as f64 / 255.0;
                    out.put_pixel(x, y, image::Rgb([
                        ((p1[0] as f64 * m + p2[0] as f64 * (1.0 - m)).round()) as u8,
                        ((p1[1] as f64 * m + p2[1] as f64 * (1.0 - m)).round()) as u8,
                        ((p1[2] as f64 * m + p2[2] as f64 * (1.0 - m)).round()) as u8,
                    ]));
                }
            }
            Ok(preserve_mode(img, DynamicImage::ImageRgb8(out)))
        }
        PipelineOp::Eval { lut } => {
            let n_bands = match img.color() {
                image::ColorType::L8 | image::ColorType::L16 => 1,
                image::ColorType::La8 | image::ColorType::La16 => 2,
                image::ColorType::Rgb8 | image::ColorType::Rgb16 => 3,
                _ => 4,
            };
            let band_luts: Vec<&[u8]> = if lut.len() >= 256 * n_bands {
                (0..n_bands).map(|b| &lut[b * 256..(b + 1) * 256]).collect()
            } else {
                vec![&lut[..]; n_bands]
            };
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            let mut out = image::RgbaImage::new(w, h);
            for (op, ip) in out.pixels_mut().zip(rgba.pixels()) {
                for b in 0..4 {
                    let idx = ip[b] as usize;
                    let band = b.min(band_luts.len() - 1);
                    op[b] = *band_luts[band].get(idx).unwrap_or(&ip[b]);
                }
            }
            Ok(preserve_mode(img, DynamicImage::ImageRgba8(out)))
        }
        PipelineOp::EffectNoise { sigma } => {
            let (w, h) = (img.width(), img.height());
            let mut rgb = image::RgbImage::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    let nx =
                        (x as f64 / w as f64).sin() * *sigma * 127.0;
                    let v = (128.0 + nx).round().clamp(0.0, 255.0) as u8;
                    rgb.put_pixel(x, y, image::Rgb([v, v, v]));
                }
            }
            Ok(preserve_mode(img, DynamicImage::ImageRgb8(rgb)))
        }

        // ── Point operations (lookup table) ──
        PipelineOp::PointOp { lut } => {
            let n_bands = match img.color() {
                image::ColorType::L8 | image::ColorType::L16 => 1,
                image::ColorType::La8 | image::ColorType::La16 => 2,
                image::ColorType::Rgb8 | image::ColorType::Rgb16 => 3,
                _ => 4,
            };
            // Per-band LUTs: if lut has 256*n_bands entries, split into per-band segments
            let band_luts: Vec<&[u8]> = if lut.len() >= 256 * n_bands {
                (0..n_bands).map(|b| &lut[b * 256..(b + 1) * 256]).collect()
            } else {
                // Single LUT: apply same to all bands
                vec![&lut[..]; n_bands]
            };
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            let mut out = image::RgbaImage::new(w, h);
            for (op, ip) in out.pixels_mut().zip(rgba.pixels()) {
                for b in 0..4 {
                    let idx = ip[b] as usize;
                    let band = b.min(band_luts.len() - 1);
                    op[b] = *band_luts[band].get(idx).unwrap_or(&ip[b]);
                }
            }
            Ok(preserve_mode(img, DynamicImage::ImageRgba8(out)))
        }
        PipelineOp::Transform { w, h, method, data, filter: _f, fill } => {
            match method {
                TransformMethod::Affine => {
                    if data.len() < 6 {
                        return Err(PilError::ValueError("Affine transform needs 6 coefficients".into()));
                    }
                    let (a, b, c, d, e, f) = (data[0], data[1], data[2], data[3], data[4], data[5]);
                    let fill_color = fill.unwrap_or((0, 0, 0, 255));
                    let src_rgba = img.to_rgba8();
                    let (sw, sh) = src_rgba.dimensions();
                    let mut out = image::RgbaImage::new(*w, *h);
                    for dy in 0..*h {
                        for dx in 0..*w {
                            let sx = a * dx as f64 + b * dy as f64 + c;
                            let sy = d * dx as f64 + e * dy as f64 + f;
                            if sx >= 0.0 && sx < sw as f64 - 1.0 && sy >= 0.0 && sy < sh as f64 - 1.0 {
                                let x0 = sx.floor() as u32;
                                let y0 = sy.floor() as u32;
                                let x1 = (x0 + 1).min(sw - 1);
                                let y1 = (y0 + 1).min(sh - 1);
                                let fx = sx - x0 as f64;
                                let fy = sy - y0 as f64;
                                let p00 = src_rgba.get_pixel(x0, y0);
                                let p10 = src_rgba.get_pixel(x1, y0);
                                let p01 = src_rgba.get_pixel(x0, y1);
                                let p11 = src_rgba.get_pixel(x1, y1);
                                out.put_pixel(dx, dy, image::Rgba([
                                    bilerp(p00[0], p10[0], p01[0], p11[0], fx, fy),
                                    bilerp(p00[1], p10[1], p01[1], p11[1], fx, fy),
                                    bilerp(p00[2], p10[2], p01[2], p11[2], fx, fy),
                                    bilerp(p00[3], p10[3], p01[3], p11[3], fx, fy),
                                ]));
                            } else {
                                out.put_pixel(dx, dy, image::Rgba([fill_color.0, fill_color.1, fill_color.2, fill_color.3]));
                            }
                        }
                    }
                    Ok(DynamicImage::ImageRgba8(out))
                }
                _ => Err(PilError::NotImplementedError(format!(
                    "Transform method {:?} not yet implemented", method
                ))),
            }
        }
    }
}
