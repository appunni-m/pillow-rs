//! Image construction and encoded-byte opening methods for [`Image`].

use std::borrow::Cow;
use std::sync::Arc;

use image_slash_star::EncodedImage;

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::format::parse_format_str;
use crate::raster::DynamicImage;

use super::{Image, LoadedData, PalettedData, l16_uses_big_endian, materialization_cache};

const MODE1_BYTE_PIXELS: [[u8; 8]; 256] = {
    let mut table = [[0; 8]; 256];
    let mut value = 0;
    while value < 256 {
        let mut bit = 0;
        while bit < 8 {
            table[value][bit] = if value & (128 >> bit) == 0 { 0 } else { 255 };
            bit += 1;
        }
        value += 1;
    }
    table
};

/// Expand complete, validated mode-1 rows; input padding never becomes pixels.
pub(crate) fn unpack_mode1_rows(packed: &[u8], width: usize, pixels: &mut [u8]) {
    if width == 0 {
        return;
    }
    let row_bytes = width.div_ceil(8);
    debug_assert!(pixels.len().is_multiple_of(width));
    debug_assert!(packed.len() >= row_bytes * (pixels.len() / width));
    for (source, destination) in packed
        .chunks_exact(row_bytes)
        .zip(pixels.chunks_exact_mut(width))
    {
        let (blocks, tail) = destination.as_chunks_mut::<8>();
        for (block, &byte) in blocks.iter_mut().zip(source) {
            *block = MODE1_BYTE_PIXELS[usize::from(byte)];
        }
        if !tail.is_empty() {
            tail.copy_from_slice(
                &MODE1_BYTE_PIXELS[usize::from(source[blocks.len()])][..tail.len()],
            );
        }
    }
}

impl Image {
    /// Returns a logically independent image sharing immutable caches until mutation.
    pub fn copy(&self) -> Self {
        // Pillow's copy returns a fresh image whose `format` is None even when
        // the source came from a file; only the originally opened image
        // carries the container format.
        let mut copy = self.clone();
        match &mut copy {
            Image::Loaded(data) => data.source_format = None,
            Image::Paletted(data) => data.source_format = None,
            Image::Bytes { format, .. } => *format = None,
            Image::Pipeline { format, .. } => *format = None,
        }
        copy
    }

    /// Seeks to a frame in a multi-frame image.
    ///
    /// Multi-frame decoding is not implemented in core yet, so only frame `0`
    /// is accepted and other frame numbers raise [`PilError::EOFError`].
    pub fn seek(&self, frame: u32) -> Result<(), PilError> {
        if frame != self.tell() {
            return Err(PilError::EOFError("no more images in file".to_owned()));
        }
        Ok(())
    }

    /// Returns the current frame number.
    ///
    /// Multi-frame decoding is not implemented in core yet, so this always
    /// returns `0`.
    pub fn tell(&self) -> u32 {
        0
    }
}

/// Host-neutral values extracted for a public `Image.new` color argument.
#[derive(Debug, Clone)]
pub struct PythonNewColorInput {
    string: Option<String>,
    single: Option<u8>,
    rgb: Option<(u8, u8, u8)>,
    rgba: Option<(u8, u8, u8, u8)>,
    luma_alpha: Option<(u8, u8)>,
    integer: Option<i32>,
    float: Option<f64>,
    provided: bool,
}

impl PythonNewColorInput {
    /// Builds a constructor input from binding-level type extraction only.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        string: Option<String>,
        single: Option<u8>,
        rgb: Option<(u8, u8, u8)>,
        rgba: Option<(u8, u8, u8, u8)>,
        luma_alpha: Option<(u8, u8)>,
        integer: Option<i32>,
        float: Option<f64>,
        provided: bool,
    ) -> Self {
        Self {
            string,
            single,
            rgb,
            rgba,
            luma_alpha,
            integer,
            float,
            provided,
        }
    }
}

/// Host-neutral mode input for the Python `Image.open` wrapper.
#[derive(Debug, Clone)]
pub enum PythonOpenModeInput {
    /// No mode override was supplied.
    None,
    /// A mode value extracted from the host.
    Name(String),
    /// A non-string host value.
    Invalid(String),
}

/// Host-neutral format allow-list input for the Python `Image.open` wrapper.
#[derive(Debug, Clone)]
pub enum PythonOpenFormatsInput {
    /// No format allow-list was supplied.
    None,
    /// A list or tuple of format names.
    Names(Vec<String>),
    /// A value that is not a list or tuple.
    Invalid(String),
}

/// Validates the Python `Image.open` mode and format arguments.
pub fn validate_python_open_inputs(
    mode: PythonOpenModeInput,
    formats: PythonOpenFormatsInput,
) -> Result<Option<Vec<String>>, PilError> {
    match mode {
        PythonOpenModeInput::None => {}
        PythonOpenModeInput::Name(name) if name == "r" => {}
        PythonOpenModeInput::Name(name) => {
            return Err(PilError::ValueError(format!("bad mode '{name}'")));
        }
        PythonOpenModeInput::Invalid(value) => {
            // Pillow formats a non-string mode with its value directly; the
            // quotes are reserved for an invalid string mode.
            return Err(PilError::ValueError(format!("bad mode {value}")));
        }
    }
    match formats {
        PythonOpenFormatsInput::None => Ok(None),
        PythonOpenFormatsInput::Names(names) => Ok(Some(names)),
        PythonOpenFormatsInput::Invalid(_) => Err(PilError::TypeError(
            "formats must be a list or tuple".to_owned(),
        )),
    }
}

/// Validates byte paths before the binding translates decode failures.
pub fn validate_python_open_source_bytes(data: &[u8]) -> Result<(), PilError> {
    if data.contains(&0) {
        return Err(PilError::ValueError("embedded null byte".to_owned()));
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum FromBytesMode {
    L,
    LA,
    Lab,
    L16,
    Rgb,
    Rgba,
    Cmyk,
    Hsv,
    YCbCr,
    I,
    F,
    P,
    Mode1,
}

impl Image {
    /// Wraps operation-created pixels without claiming an encoded source.
    pub(crate) fn from_dynamic(image: DynamicImage, explicit_mode: Option<String>) -> Self {
        let decoded_mode = image.color().into();
        Self::Loaded(LoadedData {
            image: Arc::new(image),
            known_zero_fill: false,
            explicit_mode,
            decoded_mode,
            palette: None,
            palette_alpha: None,
            source_format: None,
            info: None,
            exif: None,
        })
    }

    /// Wraps an eager generator result while preserving Pillow's logical mode.
    ///
    /// Indexed gradients store their generated bytes as palette indices, not
    /// as a decoded L image carrying a P tag.  The other non-byte gradient
    /// modes use the native four-byte buffer with an explicit mode override.
    pub(crate) fn from_generated_dynamic(image: DynamicImage, mode: &str) -> Self {
        if mode == "P" {
            let indices = match image {
                DynamicImage::ImageLuma8(indices) => indices,
                other => other.to_luma8(),
            };
            return Self::Paletted(PalettedData {
                indices,
                palette: Vec::new(),
                palette_alpha: Vec::new(),
                source_format: None,
                info: None,
                exif: None,
                materialized: materialization_cache(),
            });
        }
        let explicit_mode = matches!(mode, "1" | "I" | "F").then(|| mode.to_owned());
        Self::from_dynamic(image, explicit_mode)
    }

    /// Creates an internal L-mode mask, including Pillow's valid empty
    /// `(0, 0)` glyph-mask representation.
    ///
    /// Unlike public [`Image::frombytes`], this constructor is intended for
    /// algorithm-produced mask storage and therefore accepts zero dimensions.
    pub fn from_luma_mask(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, PilError> {
        let expected = (width as usize)
            .checked_mul(height as usize)
            .ok_or_else(|| PilError::ValueError("mask dimensions overflow".into()))?;
        if pixels.len() != expected {
            return Err(PilError::ValueError(format!(
                "mask byte length mismatch: expected {expected}, got {}",
                pixels.len()
            )));
        }
        let image = crate::raster::GrayImage::from_raw(width, height, pixels)
            .ok_or_else(|| PilError::InternalError("failed to construct L mask".into()))?;
        Ok(Self::from_dynamic(DynamicImage::ImageLuma8(image), None))
    }

    // ── Constructors ──

    #[allow(clippy::too_many_arguments)]
    /// Creates a new image with the requested Pillow mode, dimensions, and fill color.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when dimensions are invalid or the mode is not
    /// supported by core image construction.
    pub fn new(
        width: u32,
        height: u32,
        mode: &str,
        color: (u8, u8, u8, u8),
    ) -> Result<Self, PilError> {
        if mode == "P" {
            // A tuple color allocates palette entry zero. Scalar P fills use
            // `new_palette_index` because this resolved tuple no longer carries
            // the Python argument's scalar-versus-tuple distinction.
            return Ok(Image::Paletted(PalettedData {
                indices: crate::raster::GrayImage::from_pixel(
                    width,
                    height,
                    crate::raster::Luma([0u8]),
                ),
                // Pillow retains one structural RGB entry here. Keeping 768
                // zero-padded bytes would make getpalette() invent trailing
                // black entries now that palette length is exact.
                palette: vec![color.0, color.1, color.2],
                palette_alpha: Vec::new(),
                source_format: None,
                info: None,
                exif: None,
                materialized: materialization_cache(),
            }));
        }

        let img = Self::new_dynamic_image(width, height, mode, color)?;
        crate::compute::record_pipeline_allocation(img.as_bytes().len());
        let explicit = if matches!(
            mode,
            "CMYK"
                | "YCbCr"
                | "LAB"
                | "HSV"
                | "I"
                | "F"
                | "RGBa"
                | "La"
                | "RGBX"
                | "PA"
                | "1"
                | "I;16"
                | "I;16L"
                | "I;16B"
                | "I;16N"
        ) {
            Some(mode.to_string())
        } else {
            None
        };
        let known_zero_fill = match mode {
            "RGB" => color.0 == 0 && color.1 == 0 && color.2 == 0,
            "CMYK" => color == (0, 0, 0, 0),
            _ => false,
        };
        let mut image = Image::from_dynamic(img, explicit);
        if known_zero_fill {
            let Image::Loaded(data) = &mut image else {
                unreachable!("RGB and CMYK constructors produce loaded images")
            };
            data.known_zero_fill = true;
        }
        Ok(image)
    }

    fn new_dynamic_image(
        width: u32,
        height: u32,
        mode: &str,
        color: (u8, u8, u8, u8),
    ) -> Result<DynamicImage, PilError> {
        let image = match mode {
            "RGB" => DynamicImage::ImageRgb8(crate::raster::RgbImage::from_pixel(
                width,
                height,
                crate::raster::Rgb([color.0, color.1, color.2]),
            )),
            // Pillow's RGBa and RGBX keep an explicit mode tag over the same
            // four-byte storage layout as RGBA; preserve that tag at the
            // Image boundary.
            "RGBA" | "RGBa" | "RGBX" => {
                DynamicImage::ImageRgba8(crate::raster::RgbaImage::from_pixel(
                    width,
                    height,
                    crate::raster::Rgba([color.0, color.1, color.2, color.3]),
                ))
            }
            "L" => DynamicImage::ImageLuma8(crate::raster::GrayImage::from_pixel(
                width,
                height,
                crate::raster::Luma([color.0]),
            )),
            "LA" | "La" => DynamicImage::ImageLumaA8(crate::raster::GrayAlphaImage::from_pixel(
                width,
                height,
                crate::raster::LumaA([color.0, color.3]),
            )),
            // The binding packs the low 16 bits of an Image.new scalar into
            // color.0/color.1 in little-endian order. Keep the logical sample
            // native in the raster and apply the requested raw byte order only
            // when the public byte representation is requested.
            "I;16" | "I;16L" | "I;16B" | "I;16N" => {
                DynamicImage::ImageLuma16(crate::raster::ImageBuffer::from_pixel(
                    width,
                    height,
                    crate::raster::Luma([u16::from_le_bytes([color.0, color.1])]),
                ))
            }
            "PA" => DynamicImage::ImageLumaA8(crate::raster::GrayAlphaImage::from_pixel(
                width,
                height,
                crate::raster::LumaA([color.0, color.3]),
            )),
            "1" => DynamicImage::ImageLuma8(crate::raster::GrayImage::from_pixel(
                width,
                height,
                // PIL: stores the exact pixel value (0 or 1 or 255).
                // PIL's new("1") stores the raw color value as-is.
                crate::raster::Luma([color.0]),
            )),
            "CMYK" => DynamicImage::ImageRgba8(crate::raster::RgbaImage::from_pixel(
                width,
                height,
                crate::raster::Rgba([color.0, color.1, color.2, color.3]),
            )),
            // These logical modes keep Pillow's public tag over the native
            // three-byte RGB raster storage.
            "YCbCr" | "HSV" => DynamicImage::ImageRgb8(crate::raster::RgbImage::from_pixel(
                width,
                height,
                crate::raster::Rgb([color.0, color.1, color.2]),
            )),
            // Pillow exposes signed A/B samples but stores them in the shared
            // unsigned raster with a +128 bias.
            "LAB" => DynamicImage::ImageRgb8(crate::raster::RgbImage::from_pixel(
                width,
                height,
                crate::raster::Rgb([
                    color.0,
                    color.1.wrapping_add(128),
                    color.2.wrapping_add(128),
                ]),
            )),
            // I and F modes store all four resolved int32/float32 LE bytes.
            "I" | "F" => DynamicImage::ImageRgba8(crate::raster::RgbaImage::from_pixel(
                width,
                height,
                crate::raster::Rgba([color.0, color.1, color.2, color.3]),
            )),
            _ => return Err(PilError::ValueError("unrecognized image mode".into())),
        };
        Ok(image)
    }

    /// Creates an image using Pillow's host-facing `Image.new` color rules.
    ///
    /// Bindings extract host values into [`PythonNewColorInput`]; this method
    /// owns mode-specific color resolution and the distinction between scalar
    /// and tuple-created palette images.
    pub fn new_with_input(
        width: u32,
        height: u32,
        mode: &str,
        input: PythonNewColorInput,
    ) -> Result<Self, PilError> {
        let color = crate::color::resolve_new_color(
            mode,
            input.string.as_deref(),
            input.single,
            input.rgb,
            input.rgba,
            input.luma_alpha,
            input.integer,
            input.float,
        )?;
        if mode == "P" {
            if let Some(index) = input.single {
                return Ok(Self::new_palette_index(width, height, index));
            }
            if !input.provided {
                return Ok(Self::new_palette_index(width, height, 0));
            }
            let tuple_color = if let Some((r, g, b, a)) = input.rgba {
                if a != 255 {
                    return Err(PilError::ValueError(
                        "cannot add non-opaque RGBA color to RGB palette".to_owned(),
                    ));
                }
                (r, g, b, a)
            } else if let Some((r, g, b)) = input.rgb {
                (r, g, b, 255)
            } else {
                color
            };
            return Self::new(width, height, mode, tuple_color);
        }
        Self::new(width, height, mode, color)
    }

    /// Creates a `P` image filled with one raw palette index and no palette.
    ///
    /// Pillow distinguishes a scalar `Image.new("P", ..., index)` argument
    /// from a tuple color: the scalar is stored directly in every pixel while
    /// the image retains an empty palette.
    pub fn new_palette_index(width: u32, height: u32, index: u8) -> Self {
        Image::Paletted(PalettedData {
            indices: crate::raster::GrayImage::from_pixel(
                width,
                height,
                crate::raster::Luma([index]),
            ),
            palette: Vec::new(),
            palette_alpha: Vec::new(),
            source_format: None,
            info: None,
            exif: None,
            materialized: materialization_cache(),
        })
    }

    /// Creates an image from tightly packed raw bytes.
    ///
    /// `mode` uses Pillow mode names. Modes `L`, `LA`, `RGB`, `LAB`, `RGBA`,
    /// `CMYK`, `HSV`, `YCbCr`, `I`, `F`, and `P` expect one full pixel after
    /// another.
    /// The unsigned 16-bit luma raw modes `I;16`, `I;16L`, `I;16B`, and
    /// `I;16N` consume two bytes per sample with the mode's declared byte
    /// order.
    /// Mode `"1"` expects Pillow's packed bitmap layout: eight pixels per byte,
    /// most-significant bit first, with each row padded to a byte boundary.
    ///
    /// Extra bytes are ignored, matching Pillow's permissive `frombytes`
    /// behavior. Too few bytes is an error.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when allocation checks fail, the mode is
    /// unsupported, or `data` is shorter than the required mode layout.
    pub fn frombytes(mode: &str, size: (u32, u32), data: &[u8]) -> Result<Self, PilError> {
        Self::frombytes_with_data(mode, size, Cow::Borrowed(data))
    }

    /// Builds an image from bytes already extracted into an owned buffer.
    /// Raw byte modes transfer the vector into raster storage without copying.
    pub(crate) fn frombytes_owned(
        mode: &str,
        size: (u32, u32),
        data: Vec<u8>,
    ) -> Result<Self, PilError> {
        Self::frombytes_with_data(mode, size, Cow::Owned(data))
    }

    fn frombytes_with_data(
        mode: &str,
        size: (u32, u32),
        data: Cow<'_, [u8]>,
    ) -> Result<Self, PilError> {
        let (w, h) = size;
        let frombytes_mode = match mode {
            "L" => FromBytesMode::L,
            // Pillow's Unpack.c PA/PA entry uses unpackLA for byte pairs. Keep
            // the physical LA layout without interpreting indices as luma.
            "LA" | "La" | "PA" => FromBytesMode::LA,
            "I;16" | "I;16L" | "I;16B" | "I;16N" => FromBytesMode::L16,
            "RGB" => FromBytesMode::Rgb,
            "LAB" => FromBytesMode::Lab,
            "RGBA" | "RGBa" | "RGBX" => FromBytesMode::Rgba,
            "CMYK" => FromBytesMode::Cmyk,
            "HSV" => FromBytesMode::Hsv,
            "YCbCr" => FromBytesMode::YCbCr,
            "I" => FromBytesMode::I,
            "F" => FromBytesMode::F,
            "P" => FromBytesMode::P,
            "1" => FromBytesMode::Mode1,
            _ => return Err(PilError::ValueError("unrecognized image mode".into())),
        };
        if w == 0 || h == 0 {
            // Pillow accepts empty frombytes images and ignores the payload
            // because there are no samples to decode. Reuse the established
            // empty-image constructors while keeping CheckedDims strict for
            // all non-empty allocations.
            if matches!(frombytes_mode, FromBytesMode::P) {
                return Ok(Image::Paletted(PalettedData {
                    indices: crate::raster::GrayImage::from_pixel(w, h, crate::raster::Luma([0u8])),
                    palette: Vec::new(),
                    palette_alpha: Vec::new(),
                    source_format: None,
                    info: None,
                    exif: None,
                    materialized: materialization_cache(),
                }));
            }
            if matches!(frombytes_mode, FromBytesMode::L16) {
                return Ok(Self::from_dynamic(
                    DynamicImage::ImageLuma16(crate::raster::ImageBuffer::from_pixel(
                        w,
                        h,
                        crate::raster::Luma([0u16]),
                    )),
                    Some(mode.to_owned()),
                ));
            }
            return Self::new(w, h, mode, (0, 0, 0, 0));
        }
        // Pillow checks the supplied decoder payload before its global pixel
        // limit. Keep this size-only calculation separate from
        // `CheckedDims::new`, so a short buffer reports "not enough image
        // data" even when the requested dimensions also exceed MAX_PIXELS.
        let channels = match frombytes_mode {
            FromBytesMode::L | FromBytesMode::P | FromBytesMode::Mode1 => 1,
            FromBytesMode::LA | FromBytesMode::L16 => 2,
            FromBytesMode::Rgb | FromBytesMode::Lab | FromBytesMode::Hsv | FromBytesMode::YCbCr => {
                3
            }
            FromBytesMode::Rgba | FromBytesMode::Cmyk | FromBytesMode::I | FromBytesMode::F => 4,
        };
        let expected = if matches!(frombytes_mode, FromBytesMode::Mode1) {
            (w as usize)
                .div_ceil(8)
                .checked_mul(h as usize)
                .ok_or_else(|| PilError::DimensionError("image buffer size overflow".into()))?
        } else {
            (w as usize)
                .checked_mul(h as usize)
                .and_then(|pixels| pixels.checked_mul(channels))
                .ok_or_else(|| PilError::DimensionError("image buffer size overflow".into()))?
        };
        if data.len() < expected {
            return Err(PilError::ValueError("not enough image data".into()));
        }
        // Only validate allocation limits after the decoder has established
        // that the input buffer is complete.
        let _dimensions = CheckedDims::new(w, h, channels as u8)?;
        let data = match data {
            Cow::Borrowed(data) => Cow::Borrowed(&data[..expected]),
            Cow::Owned(data) => {
                if data.len() == expected {
                    Cow::Owned(data)
                } else {
                    Cow::Owned(data[..expected].to_vec())
                }
            }
        };
        Self::decode_frombytes_pixels(mode, frombytes_mode, size, data)
    }

    fn decode_frombytes_pixels(
        mode: &str,
        frombytes_mode: FromBytesMode,
        (w, h): (u32, u32),
        data: Cow<'_, [u8]>,
    ) -> Result<Self, PilError> {
        let img = match frombytes_mode {
            FromBytesMode::L => DynamicImage::ImageLuma8(
                crate::raster::GrayImage::from_raw(w, h, data.into_owned())
                    .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
            ),
            FromBytesMode::L16 => {
                let pixels = data
                    .chunks_exact(2)
                    .map(|sample| {
                        if l16_uses_big_endian(mode) {
                            u16::from_be_bytes([sample[0], sample[1]])
                        } else {
                            u16::from_le_bytes([sample[0], sample[1]])
                        }
                    })
                    .collect();
                DynamicImage::ImageLuma16(
                    crate::raster::ImageBuffer::from_raw(w, h, pixels)
                        .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
                )
            }
            FromBytesMode::Rgb => DynamicImage::ImageRgb8(
                crate::raster::RgbImage::from_raw(w, h, data.into_owned())
                    .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
            ),
            FromBytesMode::Rgba => DynamicImage::ImageRgba8(
                crate::raster::RgbaImage::from_raw(w, h, data.into_owned())
                    .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
            ),
            FromBytesMode::LA => DynamicImage::ImageLumaA8(
                crate::raster::GrayAlphaImage::from_raw(w, h, data.into_owned())
                    .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
            ),
            FromBytesMode::P => {
                return Ok(Image::Paletted(PalettedData {
                    indices: crate::raster::GrayImage::from_raw(w, h, data.into_owned())
                        .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
                    // Raw P bytes are indices only. Pillow does not synthesize
                    // palette entries until a palette is explicitly attached.
                    palette: Vec::new(),
                    palette_alpha: Vec::new(),
                    source_format: None,
                    info: None,
                    exif: None,
                    materialized: materialization_cache(),
                }));
            }
            FromBytesMode::Cmyk | FromBytesMode::I | FromBytesMode::F => DynamicImage::ImageRgba8(
                crate::raster::RgbaImage::from_raw(w, h, data.into_owned())
                    .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
            ),
            FromBytesMode::Lab | FromBytesMode::Hsv | FromBytesMode::YCbCr => {
                DynamicImage::ImageRgb8(
                    crate::raster::RgbImage::from_raw(w, h, data.into_owned())
                        .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
                )
            }
            FromBytesMode::Mode1 => {
                let mut pixels = CheckedDims::new(w, h, 1)?.alloc_buffer();
                unpack_mode1_rows(data.as_ref(), w as usize, &mut pixels);
                DynamicImage::ImageLuma8(
                    crate::raster::GrayImage::from_raw(w, h, pixels)
                        .ok_or_else(|| PilError::ValueError("frombytes: buffer error".into()))?,
                )
            }
        };
        let explicit_mode = match frombytes_mode {
            FromBytesMode::Mode1
            | FromBytesMode::Cmyk
            | FromBytesMode::Lab
            | FromBytesMode::Hsv
            | FromBytesMode::YCbCr
            | FromBytesMode::I
            | FromBytesMode::F => Some(mode.to_string()),
            FromBytesMode::Rgba if matches!(mode, "RGBa" | "RGBX") => Some(mode.to_string()),
            FromBytesMode::LA if matches!(mode, "La" | "PA") => Some(mode.to_string()),
            FromBytesMode::L16 => Some(mode.to_string()),
            FromBytesMode::L
            | FromBytesMode::LA
            | FromBytesMode::Rgb
            | FromBytesMode::Rgba
            | FromBytesMode::P => None,
        };
        Ok(Image::from_dynamic(img, explicit_mode))
    }

    /// Creates a lazy image from encoded image bytes with an optional format hint.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when the format string is unknown, the encoded
    /// header is unknown or malformed, or the hint does not match the detected
    /// format.
    pub fn open_bytes_with_format(data: Vec<u8>, format: Option<&str>) -> Result<Self, PilError> {
        let formats = format.map(|format| vec![format]);
        Self::open_bytes_with_formats(data, formats.as_deref())
    }

    /// Creates a lazy image from encoded bytes while restricting accepted formats.
    ///
    /// Pillow's ``Image.open(formats=...)`` argument is an allow-list, rather
    /// than a decoder selection hint. The header is still detected first; the
    /// image is rejected when its detected format is not present in the list.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when a format name is unknown, the encoded header
    /// is unknown or malformed, or no requested format matches the header.
    pub fn open_bytes_with_formats(
        data: Vec<u8>,
        formats: Option<&[&str]>,
    ) -> Result<Self, PilError> {
        let requested = formats
            .map(|formats| {
                formats
                    .iter()
                    .map(|format| parse_format_str(format))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;
        let data: Arc<[u8]> = data.into();
        // Image.open() reports every decoder rejection, including malformed
        // payloads and an allow-list mismatch, as UnidentifiedImageError.
        // Deferred load/verify paths retain their more specific codec errors.
        let source = EncodedImage::new(Arc::clone(&data))
            .map_err(|_| PilError::UnidentifiedImageError("memory".to_owned()))?;
        let info = source.info().clone();
        if requested
            .as_ref()
            .is_some_and(|requested| !requested.contains(&info.format))
        {
            return Err(PilError::UnidentifiedImageError("memory".to_owned()));
        }
        Ok(Image::Bytes {
            source,
            format: Some(info.format),
            info: Some(info),
            materialized: materialization_cache(),
        })
    }

    /// Creates a lazy image from encoded image bytes.
    ///
    /// Format and metadata are detected from encoded headers without decoding
    /// pixel payloads.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when the encoded header is unknown or malformed.
    pub fn open_bytes(data: Vec<u8>) -> Result<Self, PilError> {
        Self::open_bytes_with_format(data, None)
    }
}
