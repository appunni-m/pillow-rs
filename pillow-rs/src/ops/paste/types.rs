use crate::image::Image;

/// Source pixels for [`Image::paste`].
#[derive(Debug, Clone)]
pub enum PasteSource {
    /// Paste pixels from another image.
    Image(Box<Image>),
    /// Paste a scalar pixel value.
    Scalar(u8),
    /// Paste a two-band luma/alpha pixel value.
    LumaAlpha(u8, u8),
    /// Paste a three-band pixel value.
    Rgb(u8, u8, u8),
    /// Paste a four-band pixel value.
    Rgba(u8, u8, u8, u8),
    /// Paste an integer before mode-specific Pillow coercion.
    RawScalar(i64),
    /// Paste a floating-point scalar in an `F` destination.
    RawFloat(f64),
    /// Paste a two-component tuple before component clipping.
    RawLumaAlpha(i64, i64),
    /// Paste a three-component tuple before component clipping.
    RawRgb(i64, i64, i64),
    /// Paste a four-component tuple before component clipping.
    RawRgba(i64, i64, i64, i64),
}

/// Host-neutral source argument for Pillow-compatible `Image.paste`.
#[derive(Debug, Clone)]
pub enum PasteSourceInput {
    /// Another image object.
    Image(Box<Image>),
    /// A scalar color value before mode-specific validation.
    Scalar(i64),
    /// A floating-point scalar, accepted by Pillow for `F` images.
    Float(f64),
    /// A tuple/list color before mode-specific arity validation.
    Components(Vec<i64>),
    /// A Pillow color string before mode-specific conversion.
    String(String),
    /// A value that is neither an image nor a supported color value.
    Invalid,
}

/// Host-neutral box argument for Pillow-compatible `Image.paste`.
#[derive(Debug, Clone)]
pub enum PasteBoxInput {
    /// No second argument was supplied.
    None,
    /// The abbreviated `(image, mask)` form.
    Image(Box<Image>),
    /// A coordinate list/tuple.
    Values(Vec<i64>),
    /// A value that could not be interpreted as a coordinate sequence.
    Invalid {
        /// The sequence length when the host object exposed one.
        length: Option<usize>,
        /// The host type name for objects without a length.
        type_name: String,
    },
}

/// Host-neutral mask argument for Pillow-compatible `Image.paste`.
#[derive(Debug, Clone)]
pub enum PasteMaskInput {
    /// No mask was supplied.
    None,
    /// An image mask.
    Image(Box<Image>),
    /// A non-image object.
    Invalid(String),
}

/// Host-neutral coordinate input for public `Image.alpha_composite` calls.
#[derive(Debug, Clone)]
pub enum AlphaCompositeBox {
    /// A list/tuple represented as integer coordinates.
    Values(Vec<i64>),
    /// A value that was not an integer coordinate sequence.
    Invalid,
}

impl PasteSource {
    /// Builds a paste source from binding-normalized values.
    ///
    /// When `image` is present it takes priority. Otherwise the RGBA tuple is
    /// used as a solid color source.
    pub fn from_parts(image: Option<Image>, r: u8, g: u8, b: u8, a: u8) -> Self {
        if let Some(img) = image {
            PasteSource::Image(Box::new(img))
        } else {
            PasteSource::Rgba(r, g, b, a)
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum ResolvedPasteColor {
    Bytes((u8, u8, u8, u8)),
    Integer(i32),
    Float(f32),
    Luma16(u16),
}

#[derive(Debug, Clone, Copy)]
pub(super) enum PastePlacement {
    Position(i32, i32),
    Region(i32, i32, i32, i32),
}
