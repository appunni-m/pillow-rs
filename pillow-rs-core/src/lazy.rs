use image::{DynamicImage, ImageFormat};
use std::io::Cursor;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum LazyImage {
    Loaded(DynamicImage),
    Path {
        path: PathBuf,
        format: Option<ImageFormat>,
    },
    Bytes {
        data: Vec<u8>,
        format: Option<ImageFormat>,
    },
}

impl LazyImage {
    pub fn ensure_loaded(&mut self) -> Result<&DynamicImage, crate::error::PilError> {
        match self {
            LazyImage::Loaded(img, _) => Ok(img),
            LazyImage::Path { path, format: _ } => {
                let img = image::open(path).map_err(crate::error::PilError::ImageError)?;
                *self = LazyImage::Loaded(img, None);
                match self {
                    LazyImage::Loaded(img, _) => Ok(img),
                    _ => unreachable!(),
                }
            }
            LazyImage::Bytes { data, format: _ } => {
                let cursor = Cursor::new(data);
                let reader = image::ImageReader::new(cursor)
                    .with_guessed_format()
                    .map_err(crate::error::PilError::Io)?;
                let img = reader
                    .decode()
                    .map_err(crate::error::PilError::ImageError)?;
                *self = LazyImage::Loaded(img, None);
                match self {
                    LazyImage::Loaded(img, _) => Ok(img),
                    _ => unreachable!(),
                }
            }
        }
    }
}
