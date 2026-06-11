use image::{DynamicImage, GenericImage, GenericImageView, ImageFormat};
use std::path::PathBuf;
use std::sync::Arc;

use crate::color::color_type_to_mode;
use crate::error::PilError;
use crate::format::parse_format_str;
use crate::pipeline::{
    ColorMode, PipelineOp, ResampleFilter, TransposeMethod,
};

#[derive(Debug, Clone)]
pub enum Image {
    /// Fully decoded, ready to process or save.
    Loaded(DynamicImage),
    /// Path not yet decoded — lazy.
    Path {
        path: PathBuf,
        format: Option<ImageFormat>,
    },
    /// Byte buffer not yet decoded — lazy.
    Bytes {
        data: Arc<Vec<u8>>,
        format: Option<ImageFormat>,
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
            _ => {
                return Err(PilError::ValueError(format!(
                    "Unsupported mode: {}",
                    mode
                )))
            }
        };
        Ok(Image::Loaded(img))
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
            _ => DynamicImage::new_rgba8(w, h),
        };
        Ok(Image::Loaded(img))
    }

    pub fn open(path: &str, format: Option<&str>) -> Result<Self, PilError> {
        let fmt = format
            .and_then(|f| parse_format_str(f).ok())
            .or_else(|| ImageFormat::from_path(PathBuf::from(path)).ok());
        Ok(Image::Path {
            path: PathBuf::from(path),
            format: fmt,
        })
    }

    pub fn open_bytes(data: Vec<u8>) -> Result<Self, PilError> {
        let format = {
            let cursor = std::io::Cursor::new(&data);
            image::ImageReader::new(cursor)
                .with_guessed_format()
                .ok()
                .and_then(|r| r.format())
        };
        Ok(Image::Bytes {
            data: Arc::new(data),
            format,
        })
    }

    // ── Materialize ──

    /// Execute the pipeline chain and return a decoded DynamicImage.
    /// This is where all the lazy work gets done.
    pub fn materialize(&self) -> Result<DynamicImage, PilError> {
        match self {
            Image::Loaded(img) => Ok(img.clone()),
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

    /// Materialize and return a mutable reference to the decoded image.
    /// Used by methods that need to mutate pixel data (putpixel, putdata, etc).
    /// This forces full materialization of any pipeline.
    fn materialize_mut(&mut self) -> Result<&mut DynamicImage, PilError> {
        // Convert non-Loaded variants to Loaded
        let loaded = match self {
            Image::Loaded(_) => return Ok(match self {
                Image::Loaded(ref mut img) => img,
                _ => unreachable!(),
            }),
            Image::Path { path, .. } => {
                let img = image::open(path).map_err(PilError::ImageError)?;
                *self = Image::Loaded(img);
            }
            Image::Bytes { data, .. } => {
                let cursor = std::io::Cursor::new(data.as_ref());
                let reader = image::ImageReader::new(cursor)
                    .with_guessed_format()
                    .map_err(PilError::Io)?;
                let img = reader.decode().map_err(PilError::ImageError)?;
                *self = Image::Loaded(img);
            }
            Image::Pipeline { source, ops, .. } => {
                let mut img = source.materialize()?;
                for op in ops {
                    img = execute_op(&img, op)?;
                }
                *self = Image::Loaded(img);
            }
        };
        let _ = loaded;
        match self {
            Image::Loaded(ref mut img) => Ok(img),
            _ => unreachable!(),
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
        let img = self.materialize_mut()?;
        if x >= img.width() || y >= img.height() {
            return Err(PilError::ValueError(format!(
                "pixel ({},{}) out of bounds ({}x{})",
                x,
                y,
                img.width(),
                img.height()
            )));
        }
        img.put_pixel(x, y, image::Rgba([r, g, b, a]));
        Ok(())
    }

    pub fn getbands(&self) -> Result<Vec<String>, PilError> {
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
            .map_err(|e| PilError::ImageError(e))?;
        Ok(buf.into_inner())
    }

    pub fn size(&self) -> Result<(u32, u32), PilError> {
        let img = self.materialize()?;
        Ok((img.width(), img.height()))
    }

    pub fn mode(&self) -> Result<String, PilError> {
        let img = self.materialize()?;
        Ok(color_type_to_mode(img.color()).to_string())
    }

    pub fn format_name(&self) -> Option<String> {
        match self {
            Image::Loaded(_) => None,
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
    pub fn putdata(&mut self, data: &[u8]) -> Result<(), PilError> {
        let img = self.materialize_mut()?;
        let (w, h) = (img.width() as usize, img.height() as usize);
        let expected = match img.color() {
            image::ColorType::L8 | image::ColorType::L16 => w * h,
            image::ColorType::La8 | image::ColorType::La16 => w * h * 2,
            image::ColorType::Rgb8 | image::ColorType::Rgb16 => w * h * 3,
            _ => w * h * 4,
        };
        if data.len() < expected {
            return Err(PilError::ValueError(format!(
                "putdata: expected {} bytes, got {}",
                expected,
                data.len()
            )));
        }
        match img.color() {
            image::ColorType::Rgb8 | image::ColorType::Rgb16 => {
                let copy = data[..expected].to_vec();
                let rgb = image::RgbImage::from_raw(w as u32, h as u32, copy)
                    .ok_or_else(|| PilError::ValueError("putdata: buffer error".into()))?;
                *img = DynamicImage::ImageRgb8(rgb);
            }
            image::ColorType::L8 | image::ColorType::L16 => {
                let copy = data[..expected].to_vec();
                let gray = image::GrayImage::from_raw(w as u32, h as u32, copy)
                    .ok_or_else(|| PilError::ValueError("putdata: buffer error".into()))?;
                *img = DynamicImage::ImageLuma8(gray);
            }
            _ => {
                let copy = data[..expected].to_vec();
                let rgba = image::RgbaImage::from_raw(w as u32, h as u32, copy)
                    .ok_or_else(|| PilError::ValueError("putdata: buffer error".into()))?;
                *img = DynamicImage::ImageRgba8(rgba);
            }
        }
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
        Ok(Image::Loaded(DynamicImage::ImageLuma8(gray)))
    }

    /// Set/replace alpha channel.
    pub fn putalpha(&mut self, alpha: u8) -> Result<(), PilError> {
        let img = self.materialize_mut()?;
        let mut rgba = img.to_rgba8();
        for p in rgba.pixels_mut() {
            p[3] = alpha;
        }
        *img = DynamicImage::ImageRgba8(rgba);
        Ok(())
    }

    /// Get unique colors and their counts.
    pub fn getcolors(&self, maxcolors: u32) -> Result<Option<Vec<(u32, Vec<u8>)>>, PilError> {
        let img = self.materialize()?;
        let rgb = img.to_rgb8();
        let mut counts: std::collections::HashMap<Vec<u8>, u32> = std::collections::HashMap::new();
        for p in rgb.pixels() {
            let key = vec![p[0], p[1], p[2]];
            *counts.entry(key).or_insert(0) += 1;
        }
        if counts.len() > maxcolors as usize {
            return Ok(None);
        }
        let result: Vec<_> = counts.into_iter().map(|(k, v)| (v, k)).collect();
        Ok(Some(result))
    }

    /// Get entropy of the image.
    pub fn entropy(&self) -> Result<f64, PilError> {
        let img = self.materialize()?;
        let gray = img.to_luma8();
        let mut hist = [0u32; 256];
        for &p in gray.iter() {
            hist[p as usize] += 1;
        }
        let total = gray.len() as f64;
        let mut entropy = 0.0f64;
        for &h in &hist {
            if h > 0 {
                let p = h as f64 / total;
                entropy -= p * p.log2();
            }
        }
        Ok(entropy)
    }

    /// Get horizontal and vertical projections.
    pub fn getprojection(&self) -> Result<(Vec<u32>, Vec<u32>), PilError> {
        let img = self.materialize()?;
        let gray = img.to_luma8();
        let (w, h) = (gray.width() as usize, gray.height() as usize);
        let mut h_proj = vec![0u32; w];
        let mut v_proj = vec![0u32; h];
        for y in 0..h {
            for x in 0..w {
                let v = gray.get_pixel(x as u32, y as u32)[0] as u32;
                h_proj[x] += v;
                v_proj[y] += v;
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
        Ok(Image::Loaded(DynamicImage::ImageRgb8(out)))
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
    Ok(DynamicImage::ImageRgb8(out))
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
            Ok(image::imageops::resize(img, *w, *h, f).into())
        }
        PipelineOp::Crop { left, top, right, bottom } => {
            let w = right.saturating_sub(*left);
            let h = bottom.saturating_sub(*top);
            Ok(img.crop_imm(*left, *top, w, h))
        }
        PipelineOp::Rotate { angle, expand: _e, fill: _f } => {
            // Round to nearest multiple of 90 for discrete rotation
            // We use the DynamicImage rotate90/180/270 methods
            let deg = (angle.to_degrees().round() as i32).rem_euclid(360);
            let result = if (deg - 90).abs() < 2 || (deg - 90).abs() >= 358 {
                img.rotate90()
            } else if (deg - 180).abs() < 2 {
                img.rotate180()
            } else if (deg - 270).abs() < 2 || (deg - 270).abs() >= 358 {
                img.rotate270()
            } else {
                // For non-90-degree rotations, return the original (not yet implemented)
                img.clone()
            };
            Ok(result)
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
            Ok(image::imageops::resize(img, new_w.max(1), new_h.max(1), f).into())
        }
        PipelineOp::Reduce { factor } => {
            if *factor < 2 {
                return Ok(img.clone());
            }
            let new_w = img.width() / factor;
            let new_h = img.height() / factor;
            Ok(image::imageops::resize(img, new_w.max(1), new_h.max(1), image::imageops::FilterType::Nearest).into())
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
            _ => Err(PilError::NotImplementedError(format!(
                "Convert to {:?} not yet implemented",
                mode
            ))),
        },
        PipelineOp::Quantize { .. } => Err(PilError::NotImplementedError(
            "Quantize not yet implemented".into(),
        )),
        PipelineOp::RemapPalette { dest_map } => {
            let rgb = img.to_rgb8();
            let (w, h) = rgb.dimensions();
            let mut out = image::RgbImage::new(w, h);
            for (op, ip) in out.pixels_mut().zip(rgb.pixels()) {
                op[0] = *dest_map.get(ip[0] as usize).unwrap_or(&ip[0]);
                op[1] = *dest_map.get(ip[1] as usize).unwrap_or(&ip[1]);
                op[2] = *dest_map.get(ip[2] as usize).unwrap_or(&ip[2]);
            }
            Ok(DynamicImage::ImageRgb8(out))
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
            Ok(DynamicImage::ImageRgb8(out))
        }
        PipelineOp::GaussianBlur { sigma } => Ok(img.blur(*sigma)),
        PipelineOp::BoxBlur { radius } => Ok(img.blur(*radius as f32)),
        PipelineOp::MedianFilter { .. } => Err(PilError::NotImplementedError(
            "MedianFilter not yet implemented".into(),
        )),
        PipelineOp::MaxFilter { .. } => Err(PilError::NotImplementedError(
            "MaxFilter not yet implemented".into(),
        )),
        PipelineOp::MinFilter { .. } => Err(PilError::NotImplementedError(
            "MinFilter not yet implemented".into(),
        )),
        PipelineOp::RankFilter { .. } => Err(PilError::NotImplementedError(
            "RankFilter not yet implemented".into(),
        )),

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
            Ok(DynamicImage::ImageRgb8(rgb))
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
            Ok(DynamicImage::ImageRgb8(rgb))
        }
        PipelineOp::Invert => {
            let mut rgb = img.to_rgb8();
            for p in rgb.pixels_mut() {
                for c in 0..3 {
                    p[c] = 255 - p[c];
                }
            }
            Ok(DynamicImage::ImageRgb8(rgb))
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
            Ok(DynamicImage::ImageRgb8(rgb))
        }
        PipelineOp::Solarize { threshold } => {
            let t = *threshold;
            let mut rgb = img.to_rgb8();
            for p in rgb.pixels_mut() {
                for c in 0..3 {
                    if p[c] > t {
                        p[c] = 255 - p[c];
                    }
                }
            }
            Ok(DynamicImage::ImageRgb8(rgb))
        }
        PipelineOp::Grayscale => {
            Ok(DynamicImage::ImageLuma8(crate::color::pil_grayscale(img)))
        }
        PipelineOp::Colorize { .. } => Err(PilError::NotImplementedError(
            "Colorize not yet implemented".into(),
        )),
        PipelineOp::Contain { .. } => Err(PilError::NotImplementedError(
            "Contain not yet implemented".into(),
        )),
        PipelineOp::Cover { .. } => Err(PilError::NotImplementedError(
            "Cover not yet implemented".into(),
        )),
        PipelineOp::Fit { .. } => Err(PilError::NotImplementedError(
            "Fit not yet implemented".into(),
        )),
        PipelineOp::Pad { .. } => Err(PilError::NotImplementedError(
            "Pad not yet implemented".into(),
        )),
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
            Ok(image::imageops::resize(img, new_w.max(1), new_h.max(1), f).into())
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
            Ok(expanded)
        }
        PipelineOp::CropBorder { .. } => Err(PilError::NotImplementedError(
            "CropBorder not yet implemented".into(),
        )),

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
            ((a as f64 * b as f64) / 255.0).round() as u8
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
            let b = base as f64 / 255.0;
            let bl = blend as f64 / 255.0;
            let r = if b < 0.5 {
                2.0 * b * bl
            } else {
                1.0 - 2.0 * (1.0 - b) * (1.0 - bl)
            };
            (r * 255.0).round() as u8
        }),
        PipelineOp::HardLight { other } => channel_op_binary(img, other, |base, blend| {
            let bl = blend as f64 / 255.0;
            if bl < 0.5 {
                ((2.0 * base as f64 * bl) / 255.0).round() as u8
            } else {
                255 - ((2.0 * (255.0 - base as f64) * (1.0 - bl)) / 255.0).round() as u8
            }
        }),
        PipelineOp::SoftLight { other } => channel_op_binary(img, other, |base, blend| {
            let b = base as f64 / 255.0;
            let bl = blend as f64 / 255.0;
            let r = if bl < 0.5 {
                b - (1.0 - 2.0 * bl) * b * (1.0 - b)
            } else {
                b + (2.0 * bl - 1.0)
                    * ((if b <= 0.25 {
                        ((16.0 * b - 12.0) * b + 4.0) * b
                    } else {
                        b.sqrt()
                    }) - b)
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
            let (w, h) = (img.width(), img.height());
            let mut out = image::RgbImage::new(w, h);
            for p in out.pixels_mut() {
                p[0] = *value;
                p[1] = *value;
                p[2] = *value;
            }
            Ok(DynamicImage::ImageRgb8(out))
        }
        PipelineOp::Offset { x, y } => {
            let (w, h) = (img.width(), img.height());
            let mut result = DynamicImage::new_rgba8(w, h);
            let src_rgba = img.to_rgba8();
            for py in 0..h {
                for px in 0..w {
                    let sx = (px as i32 + x).rem_euclid(w as i32) as u32;
                    let sy = (py as i32 + y).rem_euclid(h as i32) as u32;
                    result.put_pixel(px, py, *src_rgba.get_pixel(sx, sy));
                }
            }
            Ok(result)
        }
        PipelineOp::Blend { .. } => Err(PilError::NotImplementedError(
            "Blend not yet implemented".into(),
        )),
        PipelineOp::Composite { .. } => Err(PilError::NotImplementedError(
            "Composite not yet implemented".into(),
        )),
        PipelineOp::Duplicate => Ok(img.clone()),
        PipelineOp::InvertChops => {
            let mut rgb = img.to_rgb8();
            for p in rgb.pixels_mut() {
                for c in 0..3 {
                    p[c] = 255 - p[c];
                }
            }
            Ok(DynamicImage::ImageRgb8(rgb))
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
            Ok(DynamicImage::ImageRgb8(rgb))
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
        PipelineOp::ColorSaturation { .. } => Err(PilError::NotImplementedError(
            "ColorSaturation not yet implemented".into(),
        )),
        PipelineOp::Sharpness { .. } => Err(PilError::NotImplementedError(
            "Sharpness not yet implemented".into(),
        )),

        // ── Effects ──
        PipelineOp::EffectSpread { distance } => {
            if *distance == 0 {
                return Ok(img.clone());
            }
            Ok(img.blur(*distance as f32))
        }
        PipelineOp::Paste { .. } => Err(PilError::NotImplementedError(
            "Paste not yet implemented in pipeline execute_op".into(),
        )),
        PipelineOp::AlphaComposite { .. } => Err(PilError::NotImplementedError(
            "AlphaComposite not yet implemented in pipeline execute_op".into(),
        )),

        // ── Module fns ──
        PipelineOp::Merge { .. } => Err(PilError::NotImplementedError(
            "Merge not yet implemented".into(),
        )),
        PipelineOp::BlendModule { .. } => Err(PilError::NotImplementedError(
            "BlendModule not yet implemented".into(),
        )),
        PipelineOp::CompositeModule { .. } => Err(PilError::NotImplementedError(
            "CompositeModule not yet implemented".into(),
        )),
        PipelineOp::Eval { lut } => {
            let is_luma =
                matches!(img.color(), image::ColorType::L8 | image::ColorType::La8);
            if is_luma {
                let gray = img.to_luma8();
                let (w, h) = gray.dimensions();
                let mut out = image::GrayImage::new(w, h);
                for (op, ip) in out.pixels_mut().zip(gray.pixels()) {
                    op[0] = *lut.get(ip[0] as usize).unwrap_or(&ip[0]);
                }
                Ok(DynamicImage::ImageLuma8(out))
            } else {
                let rgb = img.to_rgb8();
                let (w, h) = rgb.dimensions();
                let mut out = image::RgbImage::new(w, h);
                for (op, ip) in out.pixels_mut().zip(rgb.pixels()) {
                    op[0] = *lut.get(ip[0] as usize).unwrap_or(&ip[0]);
                    op[1] = *lut.get(ip[1] as usize).unwrap_or(&ip[1]);
                    op[2] = *lut.get(ip[2] as usize).unwrap_or(&ip[2]);
                }
                Ok(DynamicImage::ImageRgb8(out))
            }
        }
        PipelineOp::EffectNoise { sigma } => {
            let (w, h) = (img.width(), img.height());
            let mut rgb = image::RgbImage::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    let nx =
                        (x as f64 / w as f64).sin() * *sigma as f64 * 127.0;
                    let v = (128.0 + nx).round().clamp(0.0, 255.0) as u8;
                    rgb.put_pixel(x, y, image::Rgb([v, v, v]));
                }
            }
            Ok(DynamicImage::ImageRgb8(rgb))
        }

        // ── Point operations (lookup table) ──
        PipelineOp::PointOp { lut } => {
            let is_luma =
                matches!(img.color(), image::ColorType::L8 | image::ColorType::La8);
            if is_luma {
                let gray = img.to_luma8();
                let (w, h) = gray.dimensions();
                let mut out = image::GrayImage::new(w, h);
                for (op, ip) in out.pixels_mut().zip(gray.pixels()) {
                    op[0] = *lut.get(ip[0] as usize).unwrap_or(&ip[0]);
                }
                Ok(DynamicImage::ImageLuma8(out))
            } else {
                let rgb = img.to_rgb8();
                let (w, h) = rgb.dimensions();
                let mut out = image::RgbImage::new(w, h);
                for (op, ip) in out.pixels_mut().zip(rgb.pixels()) {
                    op[0] = *lut.get(ip[0] as usize).unwrap_or(&ip[0]);
                    op[1] = *lut.get(ip[1] as usize).unwrap_or(&ip[1]);
                    op[2] = *lut.get(ip[2] as usize).unwrap_or(&ip[2]);
                }
                Ok(DynamicImage::ImageRgb8(out))
            }
        }
        PipelineOp::Transform { .. } => Err(PilError::NotImplementedError(
            "Transform not yet implemented".into(),
        )),
    }
}
