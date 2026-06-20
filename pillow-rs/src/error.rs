use thiserror::Error;

#[derive(Error, Debug)]
pub enum PilError {
    #[error("{0}")]
    IOError(String),

    #[error("{0}")]
    OsError(String),

    #[error("{0}")]
    AssertionError(String),

    #[error("{0}")]
    IndexError(String),

    #[error("cannot identify image file '{0}'")]
    UnidentifiedImageError(String),

    #[error("{0}")]
    ValueError(String),

    #[error("{0}")]
    TypeError(String),

    #[error("image processing error: {0}")]
    ImageError(#[from] pillow_rs_image::ImageError),

    #[error("{0}")]
    NotImplementedError(String),

    #[error("unknown format: {0}")]
    UnknownFormat(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
