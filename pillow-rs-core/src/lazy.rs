use image::{DynamicImage, ImageFormat};
use std::io::Cursor;
use std::path::PathBuf;

#[derive(Clone)]
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
            LazyImage::Loaded(img) => Ok(img),
            LazyImage::Path { path, format: _ } => {
                let img = image::open(path).map_err(|e| crate::error::PilError::ImageError(e))?;
                *self = LazyImage::Loaded(img);
                match self {
                    LazyImage::Loaded(img) => Ok(img),
                    _ => unreachable!(),
                }
            }
            LazyImage::Bytes { data, format: _ } => {
                let cursor = Cursor::new(data);
                let reader = image::ImageReader::new(cursor)
                    .with_guessed_format()
                    .map_err(|e| crate::error::PilError::Io(e))?;
                let img = reader
                    .decode()
                    .map_err(|e| crate::error::PilError::ImageError(e))?;
                *self = LazyImage::Loaded(img);
                match self {
                    LazyImage::Loaded(img) => Ok(img),
                    _ => unreachable!(),
                }
            }
        }
    }
}
