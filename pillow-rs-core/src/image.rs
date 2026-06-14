use image::{DynamicImage, GenericImage, GenericImageView, ImageFormat};
use std::path::PathBuf;
use std::sync::Arc;

use crate::color::{color_type_to_mode, pil_grayscale};
use crate::error::PilError;
use crate::format::parse_format_str;
use crate::pipeline::{
    ColorMode, DitherMethod, PipelineOp, ResampleFilter, TransformMethod, TransposeMethod,
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
    /// explicit_mode: PIL mode override (e.g. "1", "P") preserved from source.
    Pipeline {
        source: Arc<Image>,
        ops: Vec<PipelineOp>,
        format: Option<ImageFormat>,
        explicit_mode: Option<String>,
    },
}

/// PIL-compatible statistics result. Scalars for single-band, Vecs for multi-band.
#[derive(Debug, Clone)]
pub struct StatResult {
    pub count: StatValue,
    pub sum: StatValue,
    pub sum2: StatValue,
    pub mean: StatValue,
    pub median: StatValue,
    pub rms: StatValue,
    pub var: StatValue,
    pub stddev: StatValue,
    pub extrema: StatValue,
}

#[derive(Debug, Clone)]
pub enum StatValue {
    Int(i64),
    Float(f64),
    IntList(Vec<i64>),
    FloatList(Vec<f64>),
    ExtremaSingle((i64, i64)),
    ExtremaList(Vec<(i64, i64)>),
}

impl StatResult {
    fn from_bands(bands: &[Vec<f64>]) -> Self {
        let n = bands.len();
        let single = n == 1;
        let fi = |idx: usize| -> StatValue {
            if single { StatValue::Int(bands[0][idx] as i64) }
            else { StatValue::IntList(bands.iter().map(|b| b[idx] as i64).collect()) }
        };
        let ff = |idx: usize| -> StatValue {
            if single { StatValue::Float(bands[0][idx]) }
            else { StatValue::FloatList(bands.iter().map(|b| b[idx]).collect()) }
        };
        let extrema = |min_idx, max_idx| -> StatValue {
            // Always use list format for extrema: [[min, max]] for single, [[min,max], ...] for multi
            StatValue::ExtremaList(bands.iter().map(|b| (b[min_idx] as i64, b[max_idx] as i64)).collect())
        };
        StatResult {
            count: fi(0),
            sum: ff(1),
            sum2: ff(2),
            mean: ff(3),
            median: fi(4),
            rms: ff(5),
            var: ff(6),
            stddev: ff(7),
            extrema: extrema(8, 9),
        }
    }
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
            "RGB" | "HSV" | "YCbCr" => (w * h * 3) as usize,
            "RGBA" | "CMYK" | "I" | "F" => (w * h * 4) as usize,
            "P" => (w * h) as usize,
            "1" => ((w as usize + 7) / 8 * h as usize),
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
            "P" => DynamicImage::ImageLuma8(
                image::GrayImage::from_raw(w, h, data[..expected].to_vec())
                    .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
            ),
            "CMYK" | "I" | "F" => DynamicImage::ImageRgba8(
                image::RgbaImage::from_raw(w, h, data[..expected].to_vec())
                    .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
            ),
            "HSV" | "YCbCr" => DynamicImage::ImageRgb8(
                image::RgbImage::from_raw(w, h, data[..expected].to_vec())
                    .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
            ),
            "1" => {
                // PIL packs 8 pixels per byte, MSB first, rows padded to byte boundary
                let row_bytes = (w as usize + 7) / 8;
                let mut pixels = vec![0u8; (w * h) as usize];
                for y in 0..h as usize {
                    for x in 0..w as usize {
                        let byte_idx = y * row_bytes + x / 8;
                        let bit_idx = 7 - (x % 8); // MSB first
                        let val = if byte_idx < data.len() && (data[byte_idx] >> bit_idx) & 1 != 0 { 255 } else { 0 };
                        pixels[y * w as usize + x] = val;
                    }
                }
                DynamicImage::ImageLuma8(
                    image::GrayImage::from_raw(w, h, pixels)
                        .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
                )
            }
            _ => DynamicImage::new_rgba8(w, h),
        };
        let explicit_mode = match mode {
            "1" | "P" | "CMYK" | "HSV" | "YCbCr" | "I" | "F" => Some(mode.to_string()),
            _ => None,
        };
        Ok(Image::Loaded(img, explicit_mode))
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
        let explicit_mode = source.explicit_mode().map(|s| s.to_string());
        match source {
            Image::Pipeline { source, ops, format, .. } => {
                let mut new_ops = ops.clone();
                new_ops.push(op);
                Image::Pipeline {
                    source: Arc::clone(source),
                    ops: new_ops,
                    format: *format,
                    explicit_mode,
                }
            }
            other => {
                let fmt = match other {
                    Image::Pipeline { format, .. } => *format,
                    _ => None,
                };
                Image::Pipeline {
                    source: Arc::new(other.clone()),
                    ops: vec![op],
                    format: fmt,
                    explicit_mode,
                }
            }
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

    /// PIL-compatible statistics result. Single-band: scalars. Multi-band: vectors.
    pub fn stat_formatted(&self) -> Result<StatResult, PilError> {
        let bands = self.stat()?;
        Ok(StatResult::from_bands(&bands))
    }

    /// Compute per-band statistics: count, sum, sum2, mean, rms, var, stddev, extrema.
    /// Returns vectors indexed by band: [band0_stats, band1_stats, ...].
    /// Each band is: [count, sum, sum2, mean, median, rms, var, stddev, min, max]
    pub fn stat(&self) -> Result<Vec<Vec<f64>>, PilError> {
        let img = self.materialize()?;
        let rgba = img.to_rgba8();
        let (w, h) = (rgba.width() as usize, rgba.height() as usize);
        let n_pixels = w * h;
        let n_bands = img.color().channel_count() as usize;
        let mut bands: Vec<Vec<u8>> = vec![Vec::with_capacity(n_pixels); n_bands];

        for px in rgba.pixels() {
            for b in 0..n_bands {
                bands[b].push(px[b]);
            }
        }
        for b in bands.iter_mut() {
            b.sort_unstable();
        }

        let mut results = Vec::with_capacity(n_bands);
        for band in &bands {
            let count = band.len() as f64;
            if count == 0.0 {
                results.push(vec![0.0; 10]);
                continue;
            }
            let sum: f64 = band.iter().map(|&x| x as f64).sum();
            let sum2: f64 = band.iter().map(|&x| (x as f64) * (x as f64)).sum();
            let mean = sum / count;
            let rms = (sum2 / count).sqrt();
            // PIL computes variance as: (sum2 - sum*sum/count) / count — avoids rms rounding
            let var = (sum2 - sum * sum / count) / count;
            let var = if var < 0.0 { 0.0 } else { var };
            let stddev = var.sqrt();
            let min = band[0] as f64;
            let max = band[band.len() - 1] as f64;
            let median = band[band.len() / 2] as f64;
            results.push(vec![count, sum, sum2, mean, median, rms, var, stddev, min, max]);
        }
        Ok(results)
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
        let img = self.materialize()?;
        // For mode "1" images, pack 8 pixels per byte (MSB first) matching PIL.
        // Only when the materialized image is still grayscale (not after convert etc.)
        if let Some(mode) = self.explicit_mode() {
            if mode == "1" && img.color() == image::ColorType::L8 {
                let gray = img.to_luma8();
                let (w, h) = gray.dimensions();
                let row_bytes = ((w + 7) / 8) as usize;
                let mut packed = vec![0u8; row_bytes * h as usize];
                for y in 0..h as usize {
                    for x in 0..w as usize {
                        let pixel = gray.get_pixel(x as u32, y as u32)[0];
                        if pixel >= 128 {
                            let byte_idx = y * row_bytes + x / 8;
                            let bit_idx = 7 - (x % 8);
                            packed[byte_idx] |= 1 << bit_idx;
                        }
                    }
                }
                return Ok(packed);
            }
        }
        Ok(img.as_bytes().to_vec())
    }

    /// Return the explicit mode override if set (e.g. "1", "P")
    pub fn explicit_mode(&self) -> Option<&str> {
        match self {
            Image::Loaded(_, Some(m)) => Some(m.as_str()),
            Image::Pipeline { explicit_mode: Some(m), .. } => Some(m.as_str()),
            _ => None,
        }
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
        if let Image::Pipeline { explicit_mode: Some(m), .. } = self {
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
        let mut result: Vec<_> = counts.into_iter().map(|(k, v)| (v, k)).collect();
        // PIL sorts by color value ascending
        result.sort_by(|a, b| a.1.cmp(&b.1));
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
        let mut bits = vec![0u8; row_bytes * h as usize];
        for y in 0..h {
            for x in 0..w {
                let v = gray.get_pixel(x, y)[0];
                if v >= 128 {
                    // PIL XBM: 1 = white, 0 = black; LSB = leftmost pixel
                    let byte_idx = (x / 8) as usize;
                    let bit_idx = x % 8;
                    bits[(y as usize) * row_bytes + byte_idx] |= 1u8 << bit_idx;
                }
            }
        }
        // PIL tobitmap format: XBM C source, 15 hex values per line
        let mut xbm = String::new();
        xbm.push_str(&format!("#define image_width {}\n", w));
        xbm.push_str(&format!("#define image_height {}\n", h));
        xbm.push_str("static char image_bits[] = {\n");
        let hexes: Vec<String> = bits.iter().map(|b| format!("0x{:02x}", b)).collect();
        let total = hexes.len();
        for (i, chunk) in hexes.chunks(15).enumerate() {
            let start = i * 15;
            let end = (start + chunk.len()).min(total);
            let is_last = end >= total;
            if is_last {
                // Last line: no trailing comma
                xbm.push_str(&chunk.join(","));
            } else {
                // Full line: trailing comma
                xbm.push_str(&chunk.join(","));
                xbm.push(',');
            }
            xbm.push('\n');
        }
        xbm.push_str("};");
        Ok(xbm.into_bytes())
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

/// Per-channel binary operation using a precomputed 256×256 lookup table.
/// The LUT is indexed as LUT[base * 256 + blend] for each channel.
fn channel_op_binary_lut(
    img: &DynamicImage,
    other: &Arc<Image>,
    lut: &[u8; 65536],
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
            out.put_pixel(x, y, image::Rgb([
                lut[pa[0] as usize * 256 + pb[0] as usize],
                lut[pa[1] as usize * 256 + pb[1] as usize],
                lut[pa[2] as usize * 256 + pb[2] as usize],
            ]));
        }
    }
    Ok(preserve_mode(img, DynamicImage::ImageRgb8(out)))
}

// ── Blend mode lookup tables (generated from PIL C implementation) ──

static OVERLAY_LUT: [u8; 65536] = {
    let bytes = include_bytes!("ops/lut_overlay.bin");
    let mut arr = [0u8; 65536];
    let mut i = 0;
    while i < 65536 {
        arr[i] = bytes[i];
        i += 1;
    }
    arr
};

static HARD_LIGHT_LUT: [u8; 65536] = {
    let bytes = include_bytes!("ops/lut_hardlight.bin");
    let mut arr = [0u8; 65536];
    let mut i = 0;
    while i < 65536 {
        arr[i] = bytes[i];
        i += 1;
    }
    arr
};

static SOFT_LIGHT_LUT: [u8; 65536] = {
    let bytes = include_bytes!("ops/lut_softlight.bin");
    let mut arr = [0u8; 65536];
    let mut i = 0;
    while i < 65536 {
        arr[i] = bytes[i];
        i += 1;
    }
    arr
};

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
pub fn preserve_mode(original: &DynamicImage, result: DynamicImage) -> DynamicImage {
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
/// PIL uses clamping for border pixels.
fn rank_filter_impl(img: &DynamicImage, size: u32, rank: u32) -> Result<DynamicImage, PilError> {
    let rgb = img.to_rgb8();
    let (w, h) = (rgb.width() as i32, rgb.height() as i32);
    let mut out = image::RgbImage::new(w as u32, h as u32);
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
                    let sx = (x + dx).clamp(0, w - 1) as u32;
                    let sy = (y + dy).clamp(0, h - 1) as u32;
                    let p = rgb.get_pixel(sx, sy);
                    r_vals.push(p[0]);
                    g_vals.push(p[1]);
                    b_vals.push(p[2]);
                }
            }
            r_vals.sort_unstable();
            g_vals.sort_unstable();
            b_vals.sort_unstable();
            out.put_pixel(x as u32, y as u32, image::Rgb([r_vals[rank], g_vals[rank], b_vals[rank]]));
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

/// PIL's clip8: truncating cast to u8, clamping at 0 and 255.
/// Matches PIL ImagingFilter's clip8(): `return ss <= 0.0 ? 0 : ss >= 255.0 ? 255 : (UINT8)ss`
fn clip8_filter(v: f32) -> u8 {
    if v <= 0.0 {
        0
    } else if v >= 255.0 {
        255
    } else {
        v as u8
    }
}

/// PIL-style box blur with fractional radius support.
/// Uses sliding-window accumulator with fixed-point (24-bit) arithmetic.
/// Repeats `passes` times (horizontal + vertical per pass).
pub fn pil_box_blur(img: &DynamicImage, radius: f32, passes: u32) -> Result<DynamicImage, PilError> {
    if radius <= 0.0 {
        return Ok(img.clone());
    }
    let rgb = img.to_rgb8();
    let (w_u32, h_u32) = rgb.dimensions();
    let w = w_u32 as i32;
    let h = h_u32 as i32;

    // Integer part of radius (PIL: (int)floatRadius)
    let r_int = radius as i32;
    // Number of pixels in the integer window
    let window_pixels = (2 * r_int + 1) as u32;
    // Fixed-point weight (PIL: ww = (UINT32)((1 << 24) / (floatRadius * 2 + 1)))
    let ww = ((1u64 << 24) as f64 / (radius as f64 * 2.0 + 1.0)) as u32;
    // Fractional edge weight (PIL: fw = ((1 << 24) - window_pixels * ww) / 2)
    let fw = ((1u64 << 24) - window_pixels as u64 * ww as u64) as u32 / 2;
    let bias = 1u32 << 23;

    let mut work = rgb.clone();

    for _pass in 0..passes {
        // Horizontal blur: process each row
        let mut hpass = image::RgbImage::new(w_u32, h_u32);
        for y in 0..h {
            for x in 0..w {
                // Accumulate pixels in the integer window: [x-r_int, x+r_int]
                let mut acc_r = 0u64;
                let mut acc_g = 0u64;
                let mut acc_b = 0u64;
                for dx in -r_int..=r_int {
                    let sx = (x + dx).clamp(0, w - 1) as u32;
                    let p = work.get_pixel(sx, y as u32);
                    acc_r += p[0] as u64;
                    acc_g += p[1] as u64;
                    acc_b += p[2] as u64;
                }
                // Fringe pixels for fractional weight
                let left_x = (x - r_int - 1).clamp(0, w - 1) as u32;
                let right_x = (x + r_int + 1).clamp(0, w - 1) as u32;
                let lp = work.get_pixel(left_x, y as u32);
                let rp = work.get_pixel(right_x, y as u32);
                // bulk = acc * ww + (left + right) * fw, then round with bias
                let bulk_r = acc_r * ww as u64 + (lp[0] as u64 + rp[0] as u64) * fw as u64 + bias as u64;
                let bulk_g = acc_g * ww as u64 + (lp[1] as u64 + rp[1] as u64) * fw as u64 + bias as u64;
                let bulk_b = acc_b * ww as u64 + (lp[2] as u64 + rp[2] as u64) * fw as u64 + bias as u64;
                hpass.put_pixel(x as u32, y as u32, image::Rgb([
                    (bulk_r >> 24) as u8,
                    (bulk_g >> 24) as u8,
                    (bulk_b >> 24) as u8,
                ]));
            }
        }
        work = hpass;

        // Vertical blur: process each column
        let mut vpass = image::RgbImage::new(w_u32, h_u32);
        for x in 0..w {
            for y in 0..h {
                let mut acc_r = 0u64;
                let mut acc_g = 0u64;
                let mut acc_b = 0u64;
                for dy in -r_int..=r_int {
                    let sy = (y + dy).clamp(0, h - 1) as u32;
                    let p = work.get_pixel(x as u32, sy);
                    acc_r += p[0] as u64;
                    acc_g += p[1] as u64;
                    acc_b += p[2] as u64;
                }
                let top_y = (y - r_int - 1).clamp(0, h - 1) as u32;
                let bot_y = (y + r_int + 1).clamp(0, h - 1) as u32;
                let tp = work.get_pixel(x as u32, top_y);
                let bp = work.get_pixel(x as u32, bot_y);
                let bulk_r = acc_r * ww as u64 + (tp[0] as u64 + bp[0] as u64) * fw as u64 + bias as u64;
                let bulk_g = acc_g * ww as u64 + (tp[1] as u64 + bp[1] as u64) * fw as u64 + bias as u64;
                let bulk_b = acc_b * ww as u64 + (tp[2] as u64 + bp[2] as u64) * fw as u64 + bias as u64;
                vpass.put_pixel(x as u32, y as u32, image::Rgb([
                    (bulk_r >> 24) as u8,
                    (bulk_g >> 24) as u8,
                    (bulk_b >> 24) as u8,
                ]));
            }
        }
        work = vpass;
    }
    Ok(preserve_mode(img, DynamicImage::ImageRgb8(work)))
}

/// Convert ResampleFilter to image crate's FilterType.
fn to_image_filter(f: &ResampleFilter) -> image::imageops::FilterType {
    match f {
        ResampleFilter::Nearest => image::imageops::FilterType::Nearest,
        ResampleFilter::Bilinear => image::imageops::FilterType::Triangle,
        ResampleFilter::Bicubic => image::imageops::FilterType::CatmullRom,
        ResampleFilter::Lanczos => image::imageops::FilterType::Lanczos3,
        ResampleFilter::Box => image::imageops::FilterType::Gaussian,
        ResampleFilter::Hamming => image::imageops::FilterType::Lanczos3,
    }
}

/// Execute a single PipelineOp against a DynamicImage.
/// Each op borrows the input, allocates and returns the output.
pub fn execute_op(img: &DynamicImage, op: &PipelineOp) -> Result<DynamicImage, PilError> {
    match op {
        // ── Geometry ──
        PipelineOp::Resize { w, h, filter } => {
            let f = to_image_filter(filter);
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
            let f = to_image_filter(filter);
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
            // PIL reduce: average each factor×factor block (no overlap, no resampling)
            let f = *factor;
            let rgb = img.to_rgb8();
            let (w, h) = (rgb.width(), rgb.height());
            let new_w = w / f;
            let new_h = h / f;
            let mut out = image::RgbImage::new(new_w, new_h);
            for y in 0..new_h {
                for x in 0..new_w {
                    let mut sum_r: u32 = 0;
                    let mut sum_g: u32 = 0;
                    let mut sum_b: u32 = 0;
                    let mut count: u32 = 0;
                    for dy in 0..f {
                        for dx in 0..f {
                            let px = x * f + dx;
                            let py = y * f + dy;
                            if px < w && py < h {
                                let p = rgb.get_pixel(px, py);
                                sum_r += p[0] as u32;
                                sum_g += p[1] as u32;
                                sum_b += p[2] as u32;
                                count += 1;
                            }
                        }
                    }
                    if count > 0 {
                        // PIL reduce uses rounding: (sum + count/2) / count
                        let half = count / 2;
                        out.put_pixel(x, y, image::Rgb([
                            ((sum_r + half) / count) as u8,
                            ((sum_g + half) / count) as u8,
                            ((sum_b + half) / count) as u8,
                        ]));
                    }
                }
            }
            Ok(preserve_mode(img, DynamicImage::ImageRgb8(out)))
        }

        // ── Color/Convert ──
        PipelineOp::Convert { mode, matrix: _, dither } => match mode {
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
                // PIL uses the SAME rounded-L conversion for convert("1") as for convert("L")
                let gray = crate::color::pil_grayscale(img);
                let (w, h) = gray.dimensions();
                let mut out = image::GrayImage::new(w, h);
                match dither {
                    Some(DitherMethod::None) => {
                        // Threshold at 128 (no dither)
                        for (op, gp) in out.pixels_mut().zip(gray.pixels()) {
                            op[0] = if gp[0] >= 128 { 255 } else { 0 };
                        }
                    }
                    _ => {
                        // Floyd-Steinberg error diffusion dithering (PIL default)
                        // Convert to Vec for mutable access
                        let mut pixels: Vec<u16> = gray.pixels().map(|p| p[0] as u16).collect();
                        for y in 0..h {
                            for x in 0..w {
                                let idx = (y * w + x) as usize;
                                let old = pixels[idx];
                                let new = if old >= 128 { 255u16 } else { 0u16 };
                                pixels[idx] = new;
                                let err = old as i32 - new as i32;
                                // Distribute error to neighbors (Floyd-Steinberg weights)
                                if x + 1 < w {
                                    let right = pixels[(y * w + x + 1) as usize] as i32 + err * 7 / 16;
                                    pixels[(y * w + x + 1) as usize] = right.max(0).min(255) as u16;
                                }
                                if y + 1 < h {
                                    if x > 0 {
                                        let down_left = pixels[((y + 1) * w + x - 1) as usize] as i32 + err * 3 / 16;
                                        pixels[((y + 1) * w + x - 1) as usize] = down_left.max(0).min(255) as u16;
                                    }
                                    let down = pixels[((y + 1) * w + x) as usize] as i32 + err * 5 / 16;
                                    pixels[((y + 1) * w + x) as usize] = down.max(0).min(255) as u16;
                                    if x + 1 < w {
                                        let down_right = pixels[((y + 1) * w + x + 1) as usize] as i32 + err * 1 / 16;
                                        pixels[((y + 1) * w + x + 1) as usize] = down_right.max(0).min(255) as u16;
                                    }
                                }
                            }
                        }
                        for (op, gp) in out.pixels_mut().zip(pixels.iter()) {
                            op[0] = if *gp >= 128 { 255 } else { 0 };
                        }
                    }
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
            // PIL C binding pre-divides kernel by scale in f32, then passes to
            // ImagingFilter3x3 which adds offset+0.5 and accumulates with f32.
            // CRITICAL: PIL's KERNEL1x3 macro groups 3 pixels into one expression,
            // so each row (bottom→center→top) is computed with fewer intermediate
            // rounding steps than per-pixel accumulation.
            // We must match PIL's grouping exactly for bit-exact results.
            let rgb = img.to_rgb8();
            let (w, h) = (rgb.width() as i32, rgb.height() as i32);
            // Pre-divide kernel by scale in f32 (matching PIL's _filter binding)
            let k0 = kernel[0] / scale;
            let k1 = kernel[1] / scale;
            let k2 = kernel[2] / scale;
            let k3 = kernel[3] / scale;
            let k4 = kernel[4] / scale;
            let k5 = kernel[5] / scale;
            let k6 = kernel[6] / scale;
            let k7 = kernel[7] / scale;
            let k8 = kernel[8] / scale;
            let rounding_bias = *offset as f32 + 0.5;
            let mut out = rgb.clone();
            for y in 1..h - 1 {
                for x in 1..w - 1 {
                    // bottom row (y+1): kernel[0..2]
                    let bp = rgb.get_pixel((x - 1) as u32, (y + 1) as u32);
                    let cp = rgb.get_pixel(x as u32, (y + 1) as u32);
                    let ap = rgb.get_pixel((x + 1) as u32, (y + 1) as u32);
                    let row_b_r = bp[0] as f32 * k0 + cp[0] as f32 * k1 + ap[0] as f32 * k2;
                    let row_b_g = bp[1] as f32 * k0 + cp[1] as f32 * k1 + ap[1] as f32 * k2;
                    let row_b_b = bp[2] as f32 * k0 + cp[2] as f32 * k1 + ap[2] as f32 * k2;
                    // center row (y): kernel[3..5]
                    let bp = rgb.get_pixel((x - 1) as u32, y as u32);
                    let cp = rgb.get_pixel(x as u32, y as u32);
                    let ap = rgb.get_pixel((x + 1) as u32, y as u32);
                    let row_c_r = bp[0] as f32 * k3 + cp[0] as f32 * k4 + ap[0] as f32 * k5;
                    let row_c_g = bp[1] as f32 * k3 + cp[1] as f32 * k4 + ap[1] as f32 * k5;
                    let row_c_b = bp[2] as f32 * k3 + cp[2] as f32 * k4 + ap[2] as f32 * k5;
                    // top row (y-1): kernel[6..8]
                    let bp = rgb.get_pixel((x - 1) as u32, (y - 1) as u32);
                    let cp = rgb.get_pixel(x as u32, (y - 1) as u32);
                    let ap = rgb.get_pixel((x + 1) as u32, (y - 1) as u32);
                    let row_t_r = bp[0] as f32 * k6 + cp[0] as f32 * k7 + ap[0] as f32 * k8;
                    let row_t_g = bp[1] as f32 * k6 + cp[1] as f32 * k7 + ap[1] as f32 * k8;
                    let row_t_b = bp[2] as f32 * k6 + cp[2] as f32 * k7 + ap[2] as f32 * k8;
                    // Accumulate in PIL's exact order: start with offset+0.5, then add
                    // row bottom → center → top (each as a grouped expression).
                    // f32 addition is NOT associative, so order matters for bit-exactness.
                    let mut r = rounding_bias;
                    let mut g = rounding_bias;
                    let mut b = rounding_bias;
                    r = r + row_b_r; g = g + row_b_g; b = b + row_b_b;
                    r = r + row_c_r; g = g + row_c_g; b = b + row_c_b;
                    r = r + row_t_r; g = g + row_t_g; b = b + row_t_b;
                    out.put_pixel(x as u32, y as u32, image::Rgb([
                        clip8_filter(r),
                        clip8_filter(g),
                        clip8_filter(b),
                    ]));
                }
            }
            Ok(preserve_mode(img, DynamicImage::ImageRgb8(out)))
        }
        PipelineOp::Filter5x5 {
            kernel,
            scale,
            offset,
        } => {
            // PIL C binding pre-divides kernel by scale in f32, then passes to
            // ImagingFilter5x5 which adds offset+0.5 and accumulates with f32.
            // Use row-grouped expressions matching PIL's KERNEL1x5 macro pattern.
            let rgb = img.to_rgb8();
            let (w, h) = (rgb.width() as i32, rgb.height() as i32);
            let k00 = kernel[0] / scale;  let k01 = kernel[1] / scale;
            let k02 = kernel[2] / scale;  let k03 = kernel[3] / scale;
            let k04 = kernel[4] / scale;
            let k10 = kernel[5] / scale;  let k11 = kernel[6] / scale;
            let k12 = kernel[7] / scale;  let k13 = kernel[8] / scale;
            let k14 = kernel[9] / scale;
            let k20 = kernel[10] / scale; let k21 = kernel[11] / scale;
            let k22 = kernel[12] / scale; let k23 = kernel[13] / scale;
            let k24 = kernel[14] / scale;
            let k30 = kernel[15] / scale; let k31 = kernel[16] / scale;
            let k32 = kernel[17] / scale; let k33 = kernel[18] / scale;
            let k34 = kernel[19] / scale;
            let k40 = kernel[20] / scale; let k41 = kernel[21] / scale;
            let k42 = kernel[22] / scale; let k43 = kernel[23] / scale;
            let k44 = kernel[24] / scale;
            let rounding_bias = *offset as f32 + 0.5;
            let mut out = rgb.clone();
            for y in 2..h - 2 {
                for x in 2..w - 2 {
                    let xm2 = (x - 2) as u32;
                    let xm1 = (x - 1) as u32;
                    let x0 = x as u32;
                    let xp1 = (x + 1) as u32;
                    let xp2 = (x + 2) as u32;
                    let yp2 = (y + 2) as u32;
                    let yp1 = (y + 1) as u32;
                    let y0 = y as u32;
                    let ym1 = (y - 1) as u32;
                    let ym2 = (y - 2) as u32;
                    // Row 0 (bottom-1, y+2): 5 pixels × kernel[0..4]
                    let p0 = rgb.get_pixel(xm2, yp2);
                    let p1 = rgb.get_pixel(xm1, yp2);
                    let p2 = rgb.get_pixel(x0, yp2);
                    let p3 = rgb.get_pixel(xp1, yp2);
                    let p4 = rgb.get_pixel(xp2, yp2);
                    let row0_r = p0[0] as f32 * k00 + p1[0] as f32 * k01 + p2[0] as f32 * k02 + p3[0] as f32 * k03 + p4[0] as f32 * k04;
                    let row0_g = p0[1] as f32 * k00 + p1[1] as f32 * k01 + p2[1] as f32 * k02 + p3[1] as f32 * k03 + p4[1] as f32 * k04;
                    let row0_b = p0[2] as f32 * k00 + p1[2] as f32 * k01 + p2[2] as f32 * k02 + p3[2] as f32 * k03 + p4[2] as f32 * k04;
                    let mut ss_r = rounding_bias;
                    let mut ss_g = rounding_bias;
                    let mut ss_b = rounding_bias;
                    ss_r = ss_r + row0_r; ss_g = ss_g + row0_g; ss_b = ss_b + row0_b;
                    // Row 1 (bottom, y+1): kernel[5..9]
                    let p0 = rgb.get_pixel(xm2, yp1);
                    let p1 = rgb.get_pixel(xm1, yp1);
                    let p2 = rgb.get_pixel(x0, yp1);
                    let p3 = rgb.get_pixel(xp1, yp1);
                    let p4 = rgb.get_pixel(xp2, yp1);
                    let row1_r = p0[0] as f32 * k10 + p1[0] as f32 * k11 + p2[0] as f32 * k12 + p3[0] as f32 * k13 + p4[0] as f32 * k14;
                    let row1_g = p0[1] as f32 * k10 + p1[1] as f32 * k11 + p2[1] as f32 * k12 + p3[1] as f32 * k13 + p4[1] as f32 * k14;
                    let row1_b = p0[2] as f32 * k10 + p1[2] as f32 * k11 + p2[2] as f32 * k12 + p3[2] as f32 * k13 + p4[2] as f32 * k14;
                    ss_r = ss_r + row1_r; ss_g = ss_g + row1_g; ss_b = ss_b + row1_b;
                    // Row 2 (center, y): kernel[10..14]
                    let p0 = rgb.get_pixel(xm2, y0);
                    let p1 = rgb.get_pixel(xm1, y0);
                    let p2 = rgb.get_pixel(x0, y0);
                    let p3 = rgb.get_pixel(xp1, y0);
                    let p4 = rgb.get_pixel(xp2, y0);
                    let row2_r = p0[0] as f32 * k20 + p1[0] as f32 * k21 + p2[0] as f32 * k22 + p3[0] as f32 * k23 + p4[0] as f32 * k24;
                    let row2_g = p0[1] as f32 * k20 + p1[1] as f32 * k21 + p2[1] as f32 * k22 + p3[1] as f32 * k23 + p4[1] as f32 * k24;
                    let row2_b = p0[2] as f32 * k20 + p1[2] as f32 * k21 + p2[2] as f32 * k22 + p3[2] as f32 * k23 + p4[2] as f32 * k24;
                    ss_r = ss_r + row2_r; ss_g = ss_g + row2_g; ss_b = ss_b + row2_b;
                    // Row 3 (top, y-1): kernel[15..19]
                    let p0 = rgb.get_pixel(xm2, ym1);
                    let p1 = rgb.get_pixel(xm1, ym1);
                    let p2 = rgb.get_pixel(x0, ym1);
                    let p3 = rgb.get_pixel(xp1, ym1);
                    let p4 = rgb.get_pixel(xp2, ym1);
                    let row3_r = p0[0] as f32 * k30 + p1[0] as f32 * k31 + p2[0] as f32 * k32 + p3[0] as f32 * k33 + p4[0] as f32 * k34;
                    let row3_g = p0[1] as f32 * k30 + p1[1] as f32 * k31 + p2[1] as f32 * k32 + p3[1] as f32 * k33 + p4[1] as f32 * k34;
                    let row3_b = p0[2] as f32 * k30 + p1[2] as f32 * k31 + p2[2] as f32 * k32 + p3[2] as f32 * k33 + p4[2] as f32 * k34;
                    ss_r = ss_r + row3_r; ss_g = ss_g + row3_g; ss_b = ss_b + row3_b;
                    // Row 4 (top+1, y-2): kernel[20..24]
                    let p0 = rgb.get_pixel(xm2, ym2);
                    let p1 = rgb.get_pixel(xm1, ym2);
                    let p2 = rgb.get_pixel(x0, ym2);
                    let p3 = rgb.get_pixel(xp1, ym2);
                    let p4 = rgb.get_pixel(xp2, ym2);
                    let row4_r = p0[0] as f32 * k40 + p1[0] as f32 * k41 + p2[0] as f32 * k42 + p3[0] as f32 * k43 + p4[0] as f32 * k44;
                    let row4_g = p0[1] as f32 * k40 + p1[1] as f32 * k41 + p2[1] as f32 * k42 + p3[1] as f32 * k43 + p4[1] as f32 * k44;
                    let row4_b = p0[2] as f32 * k40 + p1[2] as f32 * k41 + p2[2] as f32 * k42 + p3[2] as f32 * k43 + p4[2] as f32 * k44;
                    ss_r = ss_r + row4_r; ss_g = ss_g + row4_g; ss_b = ss_b + row4_b;
                    out.put_pixel(x as u32, y as u32, image::Rgb([
                        clip8_filter(ss_r),
                        clip8_filter(ss_g),
                        clip8_filter(ss_b),
                    ]));
                }
            }
            Ok(preserve_mode(img, DynamicImage::ImageRgb8(out)))
        }
        PipelineOp::GaussianBlur { sigma } => {
            // PIL GaussianBlur: 3 passes of BoxBlur with computed fractional radius.
            // Uses the "From Box Blur to Gaussian Blur" algorithm (Gwosdek et al. 2011).
            // PIL's ImagingGaussianBlur uses f32 parameters but f64 in sqrt/promotion.
            if *sigma <= 0.0 {
                return Ok(img.clone());
            }
            let passes = 3.0f64;
            let sigma2 = *sigma as f64 * *sigma as f64 / passes;
            let l_val = ((12.0 * sigma2 + 1.0).sqrt() - 1.0) / 2.0;
            let l = l_val.floor();
            let l1 = l + 1.0;
            let a_num = (2.0 * l + 1.0) * (l * l1 - 3.0 * sigma2);
            let a_den = 6.0 * (sigma2 - l1 * l1);
            let a = if a_den.abs() > 1e-10 { a_num / a_den } else { 0.0 };
            // Assign back to f32 (PIL: result is float)
            let blur_radius = (l + a) as f32;
            pil_box_blur(img, blur_radius, 3)
        }
        PipelineOp::BoxBlur { radius } => {
            // PIL BoxBlur: separable 2-pass with 24-bit fixed-point arithmetic
            // Uses clamping at borders (repeating edge pixels)
            let r = *radius as i32;
            if r <= 0 { return Ok(img.clone()); }
            let rgb = img.to_rgb8();
            let (w, h) = (rgb.width() as u32, rgb.height() as u32);
            let window = (2 * r + 1) as u32;
            // Fixed-point: ww = 2^24 / window_size, bias = 2^23
            let ww: u32 = ((1u64 << 24) / window as u64) as u32;
            let bias: u32 = 1u32 << 23;

            // Horizontal pass
            let mut hpass = image::RgbImage::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    let mut acc_r: u64 = 0;
                    let mut acc_g: u64 = 0;
                    let mut acc_b: u64 = 0;
                    for dx in -r..=r {
                        let sx = (x as i32 + dx).clamp(0, w as i32 - 1) as u32;
                        let p = rgb.get_pixel(sx, y);
                        acc_r += p[0] as u64;
                        acc_g += p[1] as u64;
                        acc_b += p[2] as u64;
                    }
                    hpass.put_pixel(x, y, image::Rgb([
                        ((acc_r * ww as u64 + bias as u64) >> 24) as u8,
                        ((acc_g * ww as u64 + bias as u64) >> 24) as u8,
                        ((acc_b * ww as u64 + bias as u64) >> 24) as u8,
                    ]));
                }
            }
            // Vertical pass
            let mut out = image::RgbImage::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    let mut acc_r: u64 = 0;
                    let mut acc_g: u64 = 0;
                    let mut acc_b: u64 = 0;
                    for dy in -r..=r {
                        let sy = (y as i32 + dy).clamp(0, h as i32 - 1) as u32;
                        let p = hpass.get_pixel(x, sy);
                        acc_r += p[0] as u64;
                        acc_g += p[1] as u64;
                        acc_b += p[2] as u64;
                    }
                    out.put_pixel(x, y, image::Rgb([
                        ((acc_r * ww as u64 + bias as u64) >> 24) as u8,
                        ((acc_g * ww as u64 + bias as u64) >> 24) as u8,
                        ((acc_b * ww as u64 + bias as u64) >> 24) as u8,
                    ]));
                }
            }
            Ok(preserve_mode(img, DynamicImage::ImageRgb8(out)))
        }
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
            // PIL 12 equalize: build LUT from non-zero histogram bins
            // step = (sum(non_zero_bins) - last_bin_count) / 255
            // lut[i] = floor(accumulator / step) where accumulator tracks step/2 + cumulative hist
            let rgb = img.to_rgb8();
            let (w, h) = rgb.dimensions();
            let mut out = image::RgbImage::new(w, h);
            for ch in 0..3 {
                let mut hist = [0u32; 256];
                for px in rgb.pixels() { hist[px[ch] as usize] += 1; }
                // Collect non-zero bins
                let nonzero: Vec<u32> = hist.iter().filter(|&&c| c > 0).copied().collect();
                if nonzero.len() <= 1 {
                    // Identity LUT
                    continue; // out already has original pixels from the RgbImage
                }
                let total: u32 = nonzero.iter().sum();
                let step = (total - nonzero[nonzero.len() - 1]) / 255;
                if step == 0 {
                    continue; // Identity LUT
                }
                let mut n = step / 2;
                let mut lut = [0u8; 256];
                for i in 0..256 {
                    lut[i] = (n / step).min(255) as u8;
                    n += hist[i];
                }
                for (opx, ipx) in out.pixels_mut().zip(rgb.pixels()) {
                    opx[ch] = lut[ipx[ch] as usize];
                }
            }
            Ok(preserve_mode(img, DynamicImage::ImageRgb8(out)))
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
            // Colorize always outputs RGB (PIL behavior)
            Ok(DynamicImage::ImageRgb8(out))
        }
        PipelineOp::Contain { w, h, filter } => {
            let f = to_image_filter(filter);
            let (w, h) = (*w, *h);
            let (iw, ih) = (img.width(), img.height());
            let ratio = (w as f64 / iw as f64).min(h as f64 / ih as f64);
            let nw = (iw as f64 * ratio) as u32;
            let nh = (ih as f64 * ratio) as u32;
            let result = DynamicImage::from(image::imageops::resize(img, nw.max(1), nh.max(1), f));
            Ok(preserve_mode(img, result))
        }
        PipelineOp::Cover { w, h, filter } => {
            let f = to_image_filter(filter);
            let (w, h) = (*w, *h);
            let (iw, ih) = (img.width(), img.height());
            let ratio = (w as f64 / iw as f64).max(h as f64 / ih as f64);
            let nw = (iw as f64 * ratio) as u32;
            let nh = (ih as f64 * ratio) as u32;
            let resized = DynamicImage::from(image::imageops::resize(img, nw.max(1), nh.max(1), f));
            let x = (nw.saturating_sub(w)) / 2;
            let y = (nh.saturating_sub(h)) / 2;
            Ok(preserve_mode(img, resized.crop_imm(x, y, w, h)))
        }
        PipelineOp::Fit { w, h, filter, bleed, centering } => {
            let f = to_image_filter(filter);
            let (w, h) = (*w, *h);
            let (iw, ih) = (img.width(), img.height());
            let bleed = *bleed;
            let centering = *centering;
            // PIL's fit algorithm: apply bleed, compute ratio, resize, crop with centering
            let eff_w = w as f64 / (1.0 + 2.0 * bleed);
            let eff_h = h as f64 / (1.0 + 2.0 * bleed);
            let ratio = (eff_w / iw as f64).min(eff_h / ih as f64);
            let nw = (iw as f64 * ratio) as u32;
            let nh = (ih as f64 * ratio) as u32;
            let resized = DynamicImage::from(image::imageops::resize(img, nw.max(1), nh.max(1), f));
            let crop_x = ((nw as f64 - w as f64) * centering.0) as u32;
            let crop_y = ((nh as f64 - h as f64) * centering.1) as u32;
            Ok(preserve_mode(img, resized.crop_imm(crop_x.min(nw.saturating_sub(1)), crop_y.min(nh.saturating_sub(1)), w.min(nw), h.min(nh))))
        }
        PipelineOp::Pad { w, h, filter, color, centering } => {
            let f = to_image_filter(filter);
            let (w, h) = (*w, *h);
            let fill = color.unwrap_or((0, 0, 0, 255));
            let (iw, ih) = (img.width(), img.height());
            // Step 1: contain (resize to fit within target, preserving aspect ratio)
            let ratio = (w as f64 / iw as f64).min(h as f64 / ih as f64);
            let nw = (iw as f64 * ratio) as u32;
            let nh = (ih as f64 * ratio) as u32;
            let resized = DynamicImage::from(image::imageops::resize(img, nw.max(1), nh.max(1), f));
            // Step 2: pad to target size
            let mut padded = DynamicImage::new_rgba8(w, h);
            for py in 0..h { for px in 0..w { padded.put_pixel(px, py, image::Rgba([fill.0, fill.1, fill.2, fill.3])); } }
            let centering = *centering;
            let x = ((w as f64 - nw as f64) * centering.0) as i64;
            let y = ((h as f64 - nh as f64) * centering.1) as i64;
            image::imageops::overlay(&mut padded, &resized.to_rgba8(), x, y);
            Ok(preserve_mode(img, padded))
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
            let f = to_image_filter(filter);
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
        PipelineOp::Overlay { other } => channel_op_binary_lut(img, other, &OVERLAY_LUT),
        PipelineOp::HardLight { other } => channel_op_binary_lut(img, other, &HARD_LIGHT_LUT),
        PipelineOp::SoftLight { other } => channel_op_binary_lut(img, other, &SOFT_LIGHT_LUT),
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
            // PIL: convert to L, compute rounded mean, create uniform gray degenerate,
            // then blend: degenerate * (1-factor) + original * factor
            let gray = pil_grayscale(img);
            let pixels: Vec<u8> = gray.pixels().map(|p| p[0]).collect();
            let n = pixels.len() as u64;
            let mean = if n > 0 {
                let sum: u64 = pixels.iter().map(|&p| p as u64).sum();
                // int(mean + 0.5) matching PIL's ImageStat
                ((sum as f64 / n as f64) + 0.5) as u8
            } else {
                0
            };
            let m = mean as f64;
            let f = *factor;
            let mut rgb = img.to_rgb8();
            for p in rgb.pixels_mut() {
                for c in 0..3 {
                    p[c] = (m * (1.0 - f) + p[c] as f64 * f).clamp(0.0, 255.0) as u8;
                }
            }
            Ok(preserve_mode(img, DynamicImage::ImageRgb8(rgb)))
        }
        PipelineOp::ColorSaturation { factor } => {
            // Use PIL's rounded grayscale conversion (to_luma8 truncates)
            let gray = pil_grayscale(img);
            let mut rgb = img.to_rgb8();
            let f = *factor;
            for (px, gp) in rgb.pixels_mut().zip(gray.pixels()) {
                let g = gp[0] as f64;
                // blend formula: gray * (1-factor) + original * factor
                px[0] = ((g + f * (px[0] as f64 - g)).clamp(0.0, 255.0)) as u8;
                px[1] = ((g + f * (px[1] as f64 - g)).clamp(0.0, 255.0)) as u8;
                px[2] = ((g + f * (px[2] as f64 - g)).clamp(0.0, 255.0)) as u8;
            }
            Ok(preserve_mode(img, DynamicImage::ImageRgb8(rgb)))
        }
        PipelineOp::Sharpness { factor } => {
            // PIL: apply SMOOTH filter (3x3 kernel [1,1,1; 1,5,1; 1,1,1] / 13, offset 0),
            // then blend: smoothed * (1-factor) + original * factor
            let f = *factor;
            let rgb = img.to_rgb8();
            let (w, h) = (rgb.width() as i32, rgb.height() as i32);
            // Pre-divided kernel values matching PIL's layout
            // kernel: [1,1,1, 1,5,1, 1,1,1], scale=13
            let inv_scale = 1.0f32 / 13.0f32;
            let k = inv_scale;       // edges = 1/13
            let kc = 5.0f32 * inv_scale;  // center = 5/13
            let rounding_bias = 0.5f32;  // offset=0 => 0+0.5
            let mut blurred = rgb.clone();
            for y in 1..h - 1 {
                for x in 1..w - 1 {
                    // bottom row (y+1): kernel[0..2] = 1,1,1
                    let bp = rgb.get_pixel((x - 1) as u32, (y + 1) as u32);
                    let cp = rgb.get_pixel(x as u32, (y + 1) as u32);
                    let ap = rgb.get_pixel((x + 1) as u32, (y + 1) as u32);
                    let row_b_r = bp[0] as f32 * k + cp[0] as f32 * k + ap[0] as f32 * k;
                    let row_b_g = bp[1] as f32 * k + cp[1] as f32 * k + ap[1] as f32 * k;
                    let row_b_b = bp[2] as f32 * k + cp[2] as f32 * k + ap[2] as f32 * k;
                    // center row (y): kernel[3..5] = 1,5,1
                    let bp = rgb.get_pixel((x - 1) as u32, y as u32);
                    let cp = rgb.get_pixel(x as u32, y as u32);
                    let ap = rgb.get_pixel((x + 1) as u32, y as u32);
                    let row_c_r = bp[0] as f32 * k + cp[0] as f32 * kc + ap[0] as f32 * k;
                    let row_c_g = bp[1] as f32 * k + cp[1] as f32 * kc + ap[1] as f32 * k;
                    let row_c_b = bp[2] as f32 * k + cp[2] as f32 * kc + ap[2] as f32 * k;
                    // top row (y-1): kernel[6..8] = 1,1,1
                    let bp = rgb.get_pixel((x - 1) as u32, (y - 1) as u32);
                    let cp = rgb.get_pixel(x as u32, (y - 1) as u32);
                    let ap = rgb.get_pixel((x + 1) as u32, (y - 1) as u32);
                    let row_t_r = bp[0] as f32 * k + cp[0] as f32 * k + ap[0] as f32 * k;
                    let row_t_g = bp[1] as f32 * k + cp[1] as f32 * k + ap[1] as f32 * k;
                    let row_t_b = bp[2] as f32 * k + cp[2] as f32 * k + ap[2] as f32 * k;
                    // Accumulate: start with rounding_bias, then add each row group
                    let mut r = rounding_bias;
                    let mut g = rounding_bias;
                    let mut b = rounding_bias;
                    r += row_b_r; g += row_b_g; b += row_b_b;
                    r += row_c_r; g += row_c_g; b += row_c_b;
                    r += row_t_r; g += row_t_g; b += row_t_b;
                    blurred.put_pixel(x as u32, y as u32, image::Rgb([
                        r.clamp(0.0, 255.0) as u8,
                        g.clamp(0.0, 255.0) as u8,
                        b.clamp(0.0, 255.0) as u8,
                    ]));
                }
            }
            // blend: blurred * (1-f) + original * f   (matching PIL's Image.blend)
            for y in 0..h {
                for x in 0..w {
                    let op = rgb.get_pixel(x as u32, y as u32);
                    let bp = blurred.get_pixel(x as u32, y as u32);
                    blurred.put_pixel(x as u32, y as u32, image::Rgb([
                        (bp[0] as f64 * (1.0 - f) + op[0] as f64 * f).clamp(0.0, 255.0) as u8,
                        (bp[1] as f64 * (1.0 - f) + op[1] as f64 * f).clamp(0.0, 255.0) as u8,
                        (bp[2] as f64 * (1.0 - f) + op[2] as f64 * f).clamp(0.0, 255.0) as u8,
                    ]));
                }
            }
            Ok(preserve_mode(img, DynamicImage::ImageRgb8(blurred)))
        }

        // ── Effects ──
        PipelineOp::EffectSpread { distance } => {
            // PIL's ImagingEffectSpread:
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
            if *distance == 0 {
                return Ok(img.clone());
            }
            let d = *distance as i32;
            let half_d = d / 2;
            // Determine pixel stride based on color type (PIL uses 1-byte image8 for L, 4-byte image32 for RGB/RGBA)
            let (pixels, w, h, stride) = match img.color() {
                image::ColorType::L8 => {
                    let luma = img.to_luma8();
                    let (w, h) = luma.dimensions();
                    (luma.into_raw(), w as i32, h as i32, 1usize)
                }
                image::ColorType::Rgb8 => {
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

            // PIL uses C rand() WITHOUT calling srand(). We call libc's rand()
            // directly via FFI, matching PIL's behavior on glibc.
            #[cfg(not(target_arch = "wasm32"))]
            {
                extern "C" {
                    fn rand() -> i32;
                }
                for y in 0..h {
                    for x in 0..w {
                        let src_idx = (y * w + x) as usize;
                        let src_base = src_idx * stride;
                        unsafe {
                            let xx = x + (rand() % d) - half_d;
                            let yy = y + (rand() % d) - half_d;
                            if xx >= 0 && xx < w && yy >= 0 && yy < h {
                                let dst_idx = (yy * w + xx) as usize;
                                let dst_base = dst_idx * stride;
                                // Read from INPUT (never modified), write to OUTPUT
                                for c in 0..stride {
                                    out_pixels[dst_base + c] = input_pixels[src_base + c];
                                    out_pixels[src_base + c] = input_pixels[dst_base + c];
                                }
                            } else {
                                // Copy pixel as-is
                                for c in 0..stride {
                                    out_pixels[src_base + c] = input_pixels[src_base + c];
                                }
                            }
                        }
                    }
                }
            }
            #[cfg(target_arch = "wasm32")]
            {
                // WASM fallback: simple copy
                out_pixels.copy_from_slice(&input_pixels);
                let _ = (d, half_d);
            }
            // Reconstruct DynamicImage from the output pixel data
            let result = match stride {
                1 => DynamicImage::ImageLuma8(
                    image::GrayImage::from_raw(w as u32, h as u32, out_pixels)
                        .ok_or_else(|| PilError::ImageError(image::ImageError::from(std::io::Error::new(std::io::ErrorKind::InvalidData, "effect_spread buffer error"))))?,
                ),
                3 => DynamicImage::ImageRgb8(
                    image::RgbImage::from_raw(w as u32, h as u32, out_pixels)
                        .ok_or_else(|| PilError::ImageError(image::ImageError::from(std::io::Error::new(std::io::ErrorKind::InvalidData, "effect_spread buffer error"))))?,
                ),
                _ => DynamicImage::ImageRgba8(
                    image::RgbaImage::from_raw(w as u32, h as u32, out_pixels)
                        .ok_or_else(|| PilError::ImageError(image::ImageError::from(std::io::Error::new(std::io::ErrorKind::InvalidData, "effect_spread buffer error"))))?,
                ),
            };
            Ok(result)
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
            // PIL's ImagingEffectNoise: Box-Muller polar transform (gaussian noise).
            // Always produces L mode output. Uses libc rand().
            let (w, h) = (img.width(), img.height());
            let mut out = image::GrayImage::new(w, h);
            let mut nextok = false;
            let mut next_val = 0.0f64;
            #[cfg(not(target_arch = "wasm32"))]
            {
                extern "C" {
                    fn rand() -> i32;
                }
                for pixel in out.pixels_mut() {
                    let this_val = if nextok {
                        nextok = false;
                        next_val
                    } else {
                        let (v1, v2, radius) = loop {
                            unsafe {
                                let v1 = rand() as f64 * (2.0 / 2147483647.0) - 1.0;
                                let v2 = rand() as f64 * (2.0 / 2147483647.0) - 1.0;
                                let radius = v1 * v1 + v2 * v2;
                                if radius < 1.0 {
                                    break (v1, v2, radius);
                                }
                                // RAND_MAX = 2147483647 on glibc (the max value of rand())
                            }
                        };
                        let factor = (-2.0 * radius.ln() / radius).sqrt();
                        // this = factor * v1; next = factor * v2 (cache it)
                        next_val = factor * v2;
                        nextok = true;
                        factor * v1
                    };
                    let v = (128.0 + sigma * this_val).round().clamp(0.0, 255.0) as u8;
                    pixel[0] = v;
                }
            }
            #[cfg(target_arch = "wasm32")]
            {
                // WASM fallback: pure sin-based noise
                for (i, pixel) in out.pixels_mut().enumerate() {
                    let x = (i as u32) % w;
                    let nx = (x as f64 / w as f64).sin() * *sigma * 127.0;
                    pixel[0] = (128.0 + nx).round().clamp(0.0, 255.0) as u8;
                }
            }
            Ok(DynamicImage::ImageLuma8(out))
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
