use image::{DynamicImage, ImageFormat};
use std::path::PathBuf;

use crate::color::color_type_to_mode;
use crate::error::PilError;
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

    pub fn to_bytes(&mut self) -> Result<Vec<u8>, PilError> {
        let img = self.ensure_loaded()?;
        Ok(img.as_bytes().to_vec())
    }

    pub fn copy(&self) -> Self {
        self.clone()
    }

    pub fn save(&mut self, _path: &str, _format: Option<&str>) -> Result<(), PilError> {
        Err(PilError::NotImplementedError("Image.save".into()))
    }

    pub fn thumbnail(
        &mut self,
        _size: (u32, u32),
        _filter: Option<&str>,
    ) -> Result<(), PilError> {
        Err(PilError::NotImplementedError("Image.thumbnail".into()))
    }
}
