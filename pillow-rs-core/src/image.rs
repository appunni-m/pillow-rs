use image::{DynamicImage, GenericImage, GenericImageView, ImageFormat};
use std::path::PathBuf;

use crate::color::color_type_to_mode;
use crate::error::PilError;
use crate::format::parse_format_str;
use crate::lazy::LazyImage;

#[derive(Clone)]
pub struct Image {
    pub(crate) inner: LazyImage,
    pub(crate) format: Option<ImageFormat>,
}

impl Image {
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
            _ => {
                return Err(PilError::ValueError(format!(
                    "Unsupported mode: {}",
                    mode
                )))
            }
        };
        Ok(Image {
            inner: LazyImage::Loaded(img),
            format: None,
        })
    }

    pub fn open_path(path: &str) -> Result<Self, PilError> {
        let path_buf = PathBuf::from(path);
        let format = ImageFormat::from_path(&path_buf).ok();
        Ok(Image {
            inner: LazyImage::Path {
                path: path_buf,
                format,
            },
            format,
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
        Ok(Image {
            inner: LazyImage::Bytes { data, format },
            format,
        })
    }

    pub fn ensure_loaded(&mut self) -> Result<&DynamicImage, PilError> {
        self.inner.ensure_loaded()
    }

    pub fn size(&mut self) -> Result<(u32, u32), PilError> {
        let img = self.ensure_loaded()?;
        Ok((img.width(), img.height()))
    }

    pub fn mode(&mut self) -> Result<String, PilError> {
        let img = self.ensure_loaded()?;
        Ok(color_type_to_mode(img.color()).to_string())
    }

    pub fn format_name(&self) -> Option<String> {
        self.format.map(|f| format!("{:?}", f).to_uppercase())
    }

    /// Get a single pixel's RGBA value. Returns (r, g, b, a) for color images,
    /// or (l, a) for grayscale+alpha. Mode-aware.
    pub fn getpixel(&mut self, x: u32, y: u32) -> Result<(u8, u8, u8, u8), PilError> {
        let img = self.ensure_loaded()?;
        if x >= img.width() || y >= img.height() {
            return Err(PilError::ValueError(format!(
                "pixel ({},{}) out of bounds ({}x{})",
                x,
                y,
                img.width(),
                img.height()
            )));
        }
        let px = img.get_pixel(x, y);
        let rgba = px.0;
        Ok((
            rgba[0],
            rgba.get(1).copied().unwrap_or(0),
            rgba.get(2).copied().unwrap_or(0),
            rgba.get(3).copied().unwrap_or(255),
        ))
    }

    /// Set a single pixel. Mutates self in-place.
    pub fn putpixel(
        &mut self,
        x: u32,
        y: u32,
        r: u8,
        g: u8,
        b: u8,
        a: u8,
    ) -> Result<(), PilError> {
        let img = self.ensure_loaded()?;
        if x >= img.width() || y >= img.height() {
            return Err(PilError::ValueError(format!(
                "pixel ({},{}) out of bounds ({}x{})",
                x,
                y,
                img.width(),
                img.height()
            )));
        }
        let mut clone = img.clone();
        clone.put_pixel(x, y, image::Rgba([r, g, b, a]));
        self.inner = crate::lazy::LazyImage::Loaded(clone);
        Ok(())
    }

    /// Extract a single channel as an L-mode image.
    pub fn getchannel(&mut self, channel: i32) -> Result<Image, PilError> {
        let img = self.ensure_loaded()?;
        let bands = img.color().channel_count();
        let ch = if channel < 0 { (bands as i32 + channel) as usize } else { channel as usize };
        if ch >= bands as usize {
            return Err(PilError::ValueError(format!("Channel {} out of range (0-{})", channel, bands - 1)));
        }
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let mut gray = image::GrayImage::new(w, h);
        for (gp, rp) in gray.pixels_mut().zip(rgba.pixels()) {
            gp[0] = rp[ch.min(3)];
        }
        Ok(Image {
            inner: crate::lazy::LazyImage::Loaded(image::DynamicImage::ImageLuma8(gray)),
            format: self.format,
        })
    }

    /// Load pixel data (no-op in Rust — data is always loaded). Returns Ok.
    pub fn load(&mut self) -> Result<(), PilError> {
        self.ensure_loaded()?;
        Ok(())
    }

    /// Set/replace alpha channel.
    pub fn putalpha(&mut self, alpha: u8) -> Result<(), PilError> {
        let img = self.ensure_loaded()?;
        let mut rgba = img.to_rgba8();
        for p in rgba.pixels_mut() {
            p[3] = alpha;
        }
        self.inner = crate::lazy::LazyImage::Loaded(image::DynamicImage::ImageRgba8(rgba));
        Ok(())
    }

    /// Reduce image by integer factor.
    pub fn reduce(&self, factor: u32) -> Result<Image, PilError> {
        if factor < 2 {
            return Ok(self.clone());
        }
        let mut clone = self.clone();
        let img = clone.ensure_loaded()?;
        let (w, h) = (img.width(), img.height());
        let new_w = w / factor;
        let new_h = h / factor;
        let small = img.resize_exact(new_w, new_h, image::imageops::FilterType::Nearest);
        Ok(Image {
            inner: crate::lazy::LazyImage::Loaded(small),
            format: self.format,
        })
    }

    /// Alpha composite another image over this one.
    pub fn alpha_composite(&mut self, src: &Image, dest: (i32, i32), source: (i32, i32)) -> Result<(), PilError> {
        let mut src_clone = src.clone();
        let src_img = src_clone.ensure_loaded()?;
        let dst_img = self.ensure_loaded()?;
        let mut dst_rgba = dst_img.to_rgba8();
        let src_rgba = src_img.to_rgba8();
        let (dx, dy) = (dest.0.max(0) as u32, dest.1.max(0) as u32);
        let (sx, sy) = (source.0.max(0) as u32, source.1.max(0) as u32);

        for py in 0..src_rgba.height().min(dst_rgba.height().saturating_sub(dy)) {
            for px in 0..src_rgba.width().min(dst_rgba.width().saturating_sub(dx)) {
                let sp = src_rgba.get_pixel(sx + px, sy + py);
                let dp = dst_rgba.get_pixel(dx + px, dy + py);
                let sa = sp[3] as f64 / 255.0;
                let da = dp[3] as f64 / 255.0;
                let out_a = sa + da * (1.0 - sa);
                if out_a > 0.0 {
                    let r = ((sp[0] as f64 * sa + dp[0] as f64 * da * (1.0 - sa)) / out_a).round() as u8;
                    let g = ((sp[1] as f64 * sa + dp[1] as f64 * da * (1.0 - sa)) / out_a).round() as u8;
                    let b = ((sp[2] as f64 * sa + dp[2] as f64 * da * (1.0 - sa)) / out_a).round() as u8;
                    dst_rgba.put_pixel(dx + px, dy + py, image::Rgba([r, g, b, (out_a * 255.0).round() as u8]));
                }
            }
        }
        self.inner = crate::lazy::LazyImage::Loaded(image::DynamicImage::ImageRgba8(dst_rgba));
        Ok(())
    }

    /// Get unique colors and their counts.
    pub fn getcolors(&mut self, maxcolors: u32) -> Result<Option<Vec<(u32, Vec<u8>)>>, PilError> {
        let img = self.ensure_loaded()?;
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
    pub fn entropy(&mut self) -> Result<f64, PilError> {
        let img = self.ensure_loaded()?;
        let gray = img.to_luma8();
        let mut hist = [0u32; 256];
        for &p in gray.iter() { hist[p as usize] += 1; }
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

    /// Get pixel data as sequence. Returns per-channel values in display order.
    /// For RGB: [R,G,B,R,G,B,...]; for L: [L,L,...]; for RGBA: [R,G,B,A,...].
    pub fn getdata(&mut self, band: Option<i32>) -> Result<Vec<u8>, PilError> {
        let img = self.ensure_loaded()?;
        let band = band.unwrap_or(-1);
        if band >= 0 {
            let rgba = img.to_rgba8();
            let b = band.min(3) as usize;
            return Ok(rgba.pixels().map(|p| p[b]).collect());
        }
        // Return in native format order
        match img.color() {
            image::ColorType::L8 | image::ColorType::L16 => {
                let gray = img.to_luma8();
                Ok(gray.into_raw())
            }
            image::ColorType::La8 | image::ColorType::La16 => {
                let ga = img.to_luma_alpha8();
                // Interleave L,A
                let mut out = Vec::with_capacity((ga.width() * ga.height() * 2) as usize);
                for p in ga.pixels() { out.push(p[0]); out.push(p[1]); }
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

    /// Get horizontal and vertical projections.
    pub fn getprojection(&mut self) -> Result<(Vec<u32>, Vec<u32>), PilError> {
        let img = self.ensure_loaded()?;
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

    /// Seek to frame in multi-frame image. Stub for now (no multi-frame support).
    pub fn seek(&mut self, _frame: u32) -> Result<(), PilError> {
        Ok(())
    }

    /// Return current frame number.
    pub fn tell(&self) -> u32 {
        0
    }

    /// Apply a lookup table or function to each pixel channel.
    /// Preserves input image mode (L stays L, RGB stays RGB).
    pub fn point(&mut self, lut: &[u8]) -> Result<Image, PilError> {
        let img = self.ensure_loaded()?;
        let is_luma = matches!(img.color(), image::ColorType::L8 | image::ColorType::La8);
        if is_luma {
            let gray = img.to_luma8();
            let (w, h) = gray.dimensions();
            let mut out = image::GrayImage::new(w, h);
            for (op, ip) in out.pixels_mut().zip(gray.pixels()) {
                op[0] = *lut.get(ip[0] as usize).unwrap_or(&ip[0]);
            }
            Ok(Image {
                inner: crate::lazy::LazyImage::Loaded(image::DynamicImage::ImageLuma8(out)),
                format: self.format,
            })
        } else {
            let rgb = img.to_rgb8();
            let (w, h) = rgb.dimensions();
            let mut out = image::RgbImage::new(w, h);
            for (op, ip) in out.pixels_mut().zip(rgb.pixels()) {
                op[0] = *lut.get(ip[0] as usize).unwrap_or(&ip[0]);
                op[1] = *lut.get(ip[1] as usize).unwrap_or(&ip[1]);
                op[2] = *lut.get(ip[2] as usize).unwrap_or(&ip[2]);
            }
            Ok(Image {
                inner: crate::lazy::LazyImage::Loaded(image::DynamicImage::ImageRgb8(out)),
                format: self.format,
            })
        }
    }

    /// Simple spread/blur effect by averaging 3x3 neighborhood.
    pub fn effect_spread(&self, distance: u32) -> Result<Image, PilError> {
        let d = distance.min(10);
        if d == 0 { return Ok(self.clone()); }
        let mut clone = self.clone();
        let img = clone.ensure_loaded()?;
        let result = img.blur(d as f32);
        Ok(Image {
            inner: crate::lazy::LazyImage::Loaded(result),
            format: self.format,
        })
    }

    pub fn to_bytes(&mut self) -> Result<Vec<u8>, PilError> {
        let img = self.ensure_loaded()?;
        Ok(img.as_bytes().to_vec())
    }

    pub fn copy(&self) -> Self {
        self.clone()
    }

    pub fn save(&mut self, path: &str, format: Option<&str>) -> Result<(), PilError> {
        let img = self.ensure_loaded()?;
        let save_format = if let Some(fmt) = format {
            parse_format_str(fmt)?
        } else {
            ImageFormat::from_path(path).map_err(|_| {
                PilError::UnknownFormat("Cannot determine format from path".into())
            })?
        };
        img.save_with_format(path, save_format)
            .map_err(|e| PilError::ImageError(e))
    }

    pub fn thumbnail(
        &mut self,
        size: (u32, u32),
        filter: Option<&str>,
    ) -> Result<(), PilError> {
        let (w, h) = {
            let img = self.ensure_loaded()?;
            (img.width(), img.height())
        };
        let (max_w, max_h) = size;
        if max_w == 0 || max_h == 0 {
            return Err(PilError::ValueError("thumbnail size must be > 0".into()));
        }
        let scale = (max_w as f64 / w as f64).min(max_h as f64 / h as f64);
        let new_w = (w as f64 * scale) as u32;
        let new_h = (h as f64 * scale) as u32;
        // resize returns a new Image — replace self.inner with it
        let resized = self.resize((new_w, new_h), filter)?;
        self.inner = resized.inner;
        self.format = resized.format;
        Ok(())
    }
}

