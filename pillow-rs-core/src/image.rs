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

