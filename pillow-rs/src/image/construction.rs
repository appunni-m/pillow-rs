//! Image construction and encoded-byte opening methods for [`Image`].

use std::borrow::Cow;
use std::sync::Arc;

use image_slash_star::EncodedImage;

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::format::parse_format_str;
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::{DynamicImage, Mode};

mod empty;
mod internal_samples;

use super::{Image, LoadedData, PalettedData, materialization_cache};

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

fn initialize_new_hidden_lanes(
    image: &mut DynamicImage,
    mode: Mode,
    color: (u8, u8, u8, u8),
    color_provided: bool,
) -> Result<(), PilError> {
    let (lane_count, values) = match mode {
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => (
            2,
            if color_provided {
                [color.1, color.2]
            } else {
                [0, 0]
            },
        ),
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => {
            (1, if color_provided { [color.3, 0] } else { [0, 0] })
        }
        _ => return Ok(()),
    };
    let pixel_count = (image.width() as usize)
        .checked_mul(image.height() as usize)
        .ok_or_else(|| PilError::DimensionError("physical pixel lane count overflows".into()))?;
    let lanes = HiddenPixelLanes::constant(lane_count, pixel_count, values)
        .ok_or_else(|| PilError::InternalError("invalid Pillow physical lane layout".into()))?;
    let attached = match (mode, image) {
        (Mode::ImageLumaA8, DynamicImage::ImageLumaA8(buffer))
        | (Mode::ImageLaPremultiplied8, DynamicImage::ImageLaPremultiplied8(buffer))
        | (Mode::ImagePaletteAlpha8, DynamicImage::ImagePaletteAlpha8(buffer)) => {
            buffer.replace_hidden_pixel_lanes(Some(lanes))
        }
        (Mode::ImageRgb8, DynamicImage::ImageRgb8(buffer))
        | (Mode::ImageYCbCr8, DynamicImage::ImageYCbCr8(buffer))
        | (Mode::ImageLab8, DynamicImage::ImageLab8(buffer))
        | (Mode::ImageHsv8, DynamicImage::ImageHsv8(buffer)) => {
            buffer.replace_hidden_pixel_lanes(Some(lanes))
        }
        _ => false,
    };
    if !attached {
        return Err(PilError::InternalError(
            "new image mode did not match its physical pixel layout".into(),
        ));
    }
    Ok(())
}

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
            Image::Pipeline { format, ops, .. } => {
                *format = None;
                ops.suppress_source_format();
            }
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
pub struct NewColorInput {
    string: Option<String>,
    single: Option<u8>,
    rgb: Option<(u8, u8, u8)>,
    rgba: Option<(u8, u8, u8, u8)>,
    luma_alpha: Option<(u8, u8)>,
    integer: Option<i32>,
    float: Option<f64>,
    provided: bool,
}

impl NewColorInput {
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

/// Host-neutral mode argument for Pillow-compatible image opening.
#[derive(Debug, Clone)]
pub enum OpenModeInput {
    /// No mode override was supplied.
    None,
    /// A mode value extracted from the host.
    Name(String),
    /// A non-string host value.
    Invalid(String),
}

/// Host-neutral format allow-list argument for Pillow-compatible image opening.
#[derive(Debug, Clone)]
pub enum OpenFormatsInput {
    /// No format allow-list was supplied.
    None,
    /// A list or tuple of format names.
    Names(Vec<String>),
    /// A value that is not a list or tuple.
    Invalid(String),
}

/// Validates Pillow-compatible image-open mode and format arguments.
pub fn validate_open_inputs(
    mode: OpenModeInput,
    formats: OpenFormatsInput,
) -> Result<Option<Vec<String>>, PilError> {
    match mode {
        OpenModeInput::None => {}
        OpenModeInput::Name(name) if name == "r" => {}
        OpenModeInput::Name(name) => {
            return Err(PilError::ValueError(format!("bad mode '{name}'")));
        }
        OpenModeInput::Invalid(value) => {
            // Pillow formats a non-string mode with its value directly; the
            // quotes are reserved for an invalid string mode.
            return Err(PilError::ValueError(format!("bad mode {value}")));
        }
    }
    match formats {
        OpenFormatsInput::None => Ok(None),
        OpenFormatsInput::Names(names) => Ok(Some(names)),
        OpenFormatsInput::Invalid(_) => Err(PilError::TypeError(
            "formats must be a list or tuple".to_owned(),
        )),
    }
}

/// Validates byte paths before the binding translates decode failures.
pub fn validate_open_source_bytes(data: &[u8]) -> Result<(), PilError> {
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

impl FromBytesMode {
    fn from_mode(mode: Mode) -> Result<Self, PilError> {
        match mode {
            Mode::ImageLuma8 => Ok(Self::L),
            Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => {
                Ok(Self::LA)
            }
            Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => Ok(Self::L16),
            Mode::ImageRgb8 => Ok(Self::Rgb),
            Mode::ImageLab8 => Ok(Self::Lab),
            Mode::ImageRgba8 | Mode::ImageRgbPremultiplied8 | Mode::ImageRgbx8 => Ok(Self::Rgba),
            Mode::ImageCmyk8 => Ok(Self::Cmyk),
            Mode::ImageHsv8 => Ok(Self::Hsv),
            Mode::ImageYCbCr8 => Ok(Self::YCbCr),
            Mode::ImageI32 => Ok(Self::I),
            Mode::ImageF32 => Ok(Self::F),
            Mode::ImagePalette8 => Ok(Self::P),
            Mode::One => Ok(Self::Mode1),
            Mode::ImageLumaA16
            | Mode::ImageRgb16
            | Mode::ImageRgba16
            | Mode::ImageRgb32F
            | Mode::ImageRgba32F => Err(PilError::ValueError("unrecognized image mode".into())),
        }
    }
}

const fn mode_uses_big_endian_samples(mode: Mode) -> bool {
    match mode {
        Mode::ImageI16B => true,
        Mode::ImageI16L => false,
        Mode::ImageI16 | Mode::ImageI16N => cfg!(target_endian = "big"),
        _ => false,
    }
}

impl Image {
    /// Wraps operation-created pixels without claiming an encoded source.
    /// A requested identity is written into the `DynamicImage` variant; it is
    /// never retained beside the sample buffer.
    pub(crate) fn from_dynamic(image: DynamicImage) -> Self {
        Self::Loaded(LoadedData {
            image: Arc::new(image),
            known_zero_fill: false,
            palette: None,
            palette_alpha: None,
            source_format: None,
            info: None,
            exif: None,
        })
    }

    /// Assigns a requested mode identity to an existing compatible carrier.
    ///
    /// The caller certifies that the existing samples already have the
    /// requested meaning; this operation never converts sample values.
    /// Passing `None` preserves the variant already carried by `image`, while
    /// an incompatible carrier is rejected.
    pub(crate) fn from_dynamic_with_mode(
        image: DynamicImage,
        mode: Option<Mode>,
    ) -> Result<Self, PilError> {
        let image = match mode {
            Some(mode) => image.with_mode(mode).map_err(|image| {
                PilError::InternalError(format!(
                    "dynamic {:?} samples cannot represent requested mode {mode:?}",
                    image.mode()
                ))
            })?,
            None => image,
        };
        Ok(Self::from_dynamic(image))
    }

    /// Wraps an eager generator result while preserving Pillow's logical mode.
    ///
    /// Indexed gradients store their generated bytes as palette indices, not
    /// as a decoded L image carrying a P tag. Other generated modes keep their
    /// target identity in the resulting `DynamicImage` variant.
    pub(crate) fn from_generated_dynamic(
        image: DynamicImage,
        mode: Mode,
    ) -> Result<Self, PilError> {
        let image = image.with_mode(mode).map_err(|image| {
            PilError::ValueError(format!(
                "generated {:?} storage cannot represent {mode:?} mode",
                image.mode()
            ))
        })?;
        if mode == Mode::ImagePalette8 {
            let DynamicImage::ImagePalette8(indices) = image else {
                return Err(PilError::InternalError(
                    "palette mode tagging returned a non-index raster".into(),
                ));
            };
            return Ok(Self::Paletted(PalettedData {
                indices: DynamicImage::ImagePalette8(indices),
                palette: Vec::new(),
                palette_alpha: Vec::new(),
                source_format: None,
                info: None,
                exif: None,
                materialized: materialization_cache(),
            }));
        }
        Ok(Self::from_dynamic(image))
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
        Ok(Self::from_dynamic(DynamicImage::ImageLuma8(image)))
    }

    // ── Constructors ──

    #[allow(clippy::too_many_arguments)]
    /// Creates a new image with the requested mode, dimensions, and fill color.
    ///
    /// The mode is supplied as a typed [`Mode`] identity. Host adapters parse
    /// Pillow's string spelling before calling this constructor.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when dimensions are invalid or the mode is not
    /// supported by core image construction.
    pub fn new(
        width: u32,
        height: u32,
        mode: Mode,
        color: (u8, u8, u8, u8),
    ) -> Result<Self, PilError> {
        Self::new_with_color_presence(width, height, mode, color, true)
    }

    fn new_with_color_presence(
        width: u32,
        height: u32,
        mode: Mode,
        color: (u8, u8, u8, u8),
        color_provided: bool,
    ) -> Result<Self, PilError> {
        if mode == Mode::ImagePalette8 {
            // A tuple color allocates palette entry zero. Scalar P fills use
            // `new_palette_index` because this resolved tuple no longer carries
            // the argument's scalar-versus-tuple distinction.
            return Ok(Image::Paletted(PalettedData {
                indices: DynamicImage::ImagePalette8(crate::raster::GrayImage::from_pixel(
                    width,
                    height,
                    crate::raster::Luma([0u8]),
                )),
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

        let img = Self::new_dynamic_image(width, height, mode, color, color_provided)?;
        crate::compute::record_pipeline_allocation(img.as_bytes().len());
        let known_zero_fill = match mode {
            Mode::ImageRgb8 => color.0 == 0 && color.1 == 0 && color.2 == 0,
            Mode::ImageCmyk8 => color == (0, 0, 0, 0),
            _ => false,
        };
        let mut image = Image::from_dynamic(img);
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
        mode: Mode,
        color: (u8, u8, u8, u8),
        color_provided: bool,
    ) -> Result<DynamicImage, PilError> {
        let mut image = match mode {
            Mode::ImageRgb8 => DynamicImage::ImageRgb8(crate::raster::RgbImage::from_pixel(
                width,
                height,
                crate::raster::Rgb([color.0, color.1, color.2]),
            )),
            Mode::ImageRgba8 => DynamicImage::ImageRgba8(crate::raster::RgbaImage::from_pixel(
                width,
                height,
                crate::raster::Rgba([color.0, color.1, color.2, color.3]),
            )),
            Mode::ImageRgbPremultiplied8 => {
                DynamicImage::ImageRgbPremultiplied8(crate::raster::RgbaImage::from_pixel(
                    width,
                    height,
                    crate::raster::Rgba([color.0, color.1, color.2, color.3]),
                ))
            }
            Mode::ImageRgbx8 => DynamicImage::ImageRgbx8(crate::raster::RgbaImage::from_pixel(
                width,
                height,
                crate::raster::Rgba([color.0, color.1, color.2, color.3]),
            )),
            Mode::ImageLuma8 => DynamicImage::ImageLuma8(crate::raster::GrayImage::from_pixel(
                width,
                height,
                crate::raster::Luma([color.0]),
            )),
            Mode::One => DynamicImage::One(crate::raster::GrayImage::from_pixel(
                width,
                height,
                // Pillow stores the supplied Image.new color byte verbatim;
                // this constructor does not threshold it to a bit value.
                crate::raster::Luma([color.0]),
            )),
            Mode::ImageLumaA8 => {
                DynamicImage::ImageLumaA8(crate::raster::GrayAlphaImage::from_pixel(
                    width,
                    height,
                    crate::raster::LumaA([color.0, color.3]),
                ))
            }
            Mode::ImageLaPremultiplied8 => {
                DynamicImage::ImageLaPremultiplied8(crate::raster::GrayAlphaImage::from_pixel(
                    width,
                    height,
                    crate::raster::LumaA([color.0, color.3]),
                ))
            }
            // The binding packs the low 16 bits of an Image.new scalar into
            // color.0/color.1 in little-endian order. Keep the logical sample
            // native in the raster and apply the requested raw byte order only
            // when the public byte representation is requested.
            Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
                Self::new_i16_dynamic_image(width, height, mode, color)
            }
            Mode::ImagePaletteAlpha8 => {
                DynamicImage::ImagePaletteAlpha8(crate::raster::GrayAlphaImage::from_pixel(
                    width,
                    height,
                    crate::raster::LumaA([color.0, color.3]),
                ))
            }
            Mode::ImageCmyk8 => DynamicImage::ImageCmyk8(crate::raster::RgbaImage::from_pixel(
                width,
                height,
                crate::raster::Rgba([color.0, color.1, color.2, color.3]),
            )),
            Mode::ImageYCbCr8 => DynamicImage::ImageYCbCr8(crate::raster::RgbImage::from_pixel(
                width,
                height,
                crate::raster::Rgb([color.0, color.1, color.2]),
            )),
            Mode::ImageHsv8 => DynamicImage::ImageHsv8(crate::raster::RgbImage::from_pixel(
                width,
                height,
                crate::raster::Rgb([color.0, color.1, color.2]),
            )),
            Mode::ImageLab8 => Self::new_lab8_dynamic_image(width, height, color),
            Mode::ImageI32 => DynamicImage::ImageI32(crate::raster::RgbaImage::from_pixel(
                width,
                height,
                crate::raster::Rgba([color.0, color.1, color.2, color.3]),
            )),
            Mode::ImageF32 => DynamicImage::ImageF32(crate::raster::RgbaImage::from_pixel(
                width,
                height,
                crate::raster::Rgba([color.0, color.1, color.2, color.3]),
            )),
            Mode::ImagePalette8 => unreachable!("palette mode is handled by Image::new"),
            Mode::ImageLumaA16
            | Mode::ImageRgb16
            | Mode::ImageRgba16
            | Mode::ImageRgb32F
            | Mode::ImageRgba32F => {
                // NO_PILLOW_SUPPORT: these internal storage identities have
                // no Pillow Image.new spelling or scalar-fill contract.
                return Err(PilError::ValueError(
                    "unsupported image mode for Image.new".into(),
                ));
            }
        };
        initialize_new_hidden_lanes(&mut image, mode, color, color_provided)?;
        Ok(image)
    }

    fn new_i16_dynamic_image(
        width: u32,
        height: u32,
        mode: Mode,
        color: (u8, u8, u8, u8),
    ) -> DynamicImage {
        let image = crate::raster::ImageBuffer::from_pixel(
            width,
            height,
            crate::raster::Luma([u16::from_le_bytes([color.0, color.1])]),
        );
        match mode {
            Mode::ImageI16 => DynamicImage::ImageI16(image),
            Mode::ImageI16L => DynamicImage::ImageI16L(image),
            Mode::ImageI16B => DynamicImage::ImageI16B(image),
            Mode::ImageI16N => DynamicImage::ImageI16N(image),
            _ => unreachable!("matched 16-bit luma mode"),
        }
    }

    fn new_lab8_dynamic_image(width: u32, height: u32, color: (u8, u8, u8, u8)) -> DynamicImage {
        DynamicImage::ImageLab8(crate::raster::RgbImage::from_pixel(
            width,
            height,
            crate::raster::Rgb([
                color.0,
                color.1.wrapping_add(128),
                color.2.wrapping_add(128),
            ]),
        ))
    }

    /// Creates an image using Pillow's host-facing `Image.new` color rules.
    ///
    /// Bindings extract host values into [`NewColorInput`]; this method
    /// owns mode-specific color resolution and the distinction between scalar
    /// and tuple-created palette images.
    pub fn new_with_input(
        width: u32,
        height: u32,
        mode: Mode,
        input: NewColorInput,
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
        if mode == Mode::ImagePalette8 {
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
        Self::new_with_color_presence(width, height, mode, color, input.provided)
    }

    /// Creates a `P` image filled with one raw palette index and no palette.
    ///
    /// Pillow distinguishes a scalar `Image.new("P", ..., index)` argument
    /// from a tuple color: the scalar is stored directly in every pixel while
    /// the image retains an empty palette.
    pub fn new_palette_index(width: u32, height: u32, index: u8) -> Self {
        Self::new_palette_index_with_metadata(
            width,
            height,
            index,
            Vec::new(),
            Vec::new(),
            None,
            None,
        )
    }

    /// Creates an indexed Transform destination with its source palette and
    /// public image metadata.
    pub(crate) fn new_palette_index_from_source(
        source: &Image,
        width: u32,
        height: u32,
        index: u8,
    ) -> Self {
        Self::new_palette_index_with_metadata(
            width,
            height,
            index,
            source.palette().unwrap_or_default(),
            source.palette_alpha().unwrap_or_default(),
            source.image_info(),
            source.exif_metadata(),
        )
    }

    fn new_palette_index_with_metadata(
        width: u32,
        height: u32,
        index: u8,
        palette: Vec<u8>,
        palette_alpha: Vec<u8>,
        info: Option<image_slash_star::ImageInfo>,
        exif: Option<Vec<u8>>,
    ) -> Self {
        Image::Paletted(PalettedData {
            indices: DynamicImage::ImagePalette8(crate::raster::GrayImage::from_pixel(
                width,
                height,
                crate::raster::Luma([index]),
            )),
            palette,
            palette_alpha,
            source_format: None,
            info,
            exif,
            materialized: materialization_cache(),
        })
    }

    /// Creates an image from tightly packed raw bytes.
    ///
    /// The typed `mode` selects the Pillow raw byte layout. Modes `L`, `LA`,
    /// `RGB`, `LAB`, `RGBA`, `CMYK`, `HSV`, `YCbCr`, `I`, `F`, and `P` expect one full pixel after
    /// another.
    /// The unsigned 16-bit luma raw modes `I;16`, `I;16L`, `I;16B`, and
    /// `I;16N` consume two bytes per sample with the mode's declared byte
    /// order.
    /// `ImageLumaA16`, `ImageRgb16`, `ImageRgba16`, `ImageRgb32F`, and
    /// `ImageRgba32F` consume native-endian samples. These internal
    /// raster identities are Rust extensions without Pillow mode spellings.
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
    pub fn frombytes(mode: Mode, size: (u32, u32), data: &[u8]) -> Result<Self, PilError> {
        Self::frombytes_with_data(mode, size, Cow::Borrowed(data))
    }

    /// Builds an image from bytes already extracted into an owned buffer.
    /// Raw byte modes transfer the vector into raster storage without copying.
    pub(crate) fn frombytes_owned(
        mode: Mode,
        size: (u32, u32),
        data: Vec<u8>,
    ) -> Result<Self, PilError> {
        Self::frombytes_with_data(mode, size, Cow::Owned(data))
    }

    fn frombytes_with_data(
        mode: Mode,
        size: (u32, u32),
        data: Cow<'_, [u8]>,
    ) -> Result<Self, PilError> {
        let (w, h) = size;
        // NO_PILLOW_SUPPORT: decode native sample layouts for internal modes.
        if let Some(image) = internal_samples::decode(mode, size, data.as_ref())? {
            return Ok(Self::from_dynamic(image));
        }
        // Pillow's Unpack.c PA/PA entry uses unpackLA for byte pairs. Keep
        // the physical LA layout without interpreting indices as luma.
        let frombytes_mode = FromBytesMode::from_mode(mode)?;
        if w == 0 || h == 0 {
            // Pillow accepts empty frombytes images and ignores the payload
            // because there are no samples to decode. Reuse the established
            // empty-image constructors while keeping CheckedDims strict for
            // all non-empty allocations.
            if matches!(frombytes_mode, FromBytesMode::P) {
                return Ok(Image::Paletted(PalettedData {
                    indices: DynamicImage::ImagePalette8(crate::raster::GrayImage::from_pixel(
                        w,
                        h,
                        crate::raster::Luma([0u8]),
                    )),
                    palette: Vec::new(),
                    palette_alpha: Vec::new(),
                    source_format: None,
                    info: None,
                    exif: None,
                    materialized: materialization_cache(),
                }));
            }
            if matches!(frombytes_mode, FromBytesMode::L16) {
                return Self::from_dynamic_with_mode(
                    DynamicImage::ImageI16(crate::raster::ImageBuffer::from_pixel(
                        w,
                        h,
                        crate::raster::Luma([0u16]),
                    )),
                    Some(mode),
                );
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
        mode: Mode,
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
                        if mode_uses_big_endian_samples(mode) {
                            u16::from_be_bytes([sample[0], sample[1]])
                        } else {
                            u16::from_le_bytes([sample[0], sample[1]])
                        }
                    })
                    .collect();
                DynamicImage::ImageI16(
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
                    indices: DynamicImage::ImagePalette8(
                        crate::raster::GrayImage::from_raw(w, h, data.into_owned()).ok_or_else(
                            || PilError::ValueError("frombytes: buffer error".into()),
                        )?,
                    ),
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
        let mut image = img.with_mode(mode).map_err(|image| {
            PilError::InternalError(format!(
                "frombytes {:?} samples cannot represent requested mode {mode:?}",
                image.mode()
            ))
        })?;
        super::codec_adapter::attach_decoded_hidden_lanes(&mut image, mode)?;
        Ok(Self::from_dynamic(image))
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
        // EncodedImage::raster_mode reads the inspected header cache. Translate
        // it once to Pillow's operation mode while keeping pixels lazy.
        let header_mode = super::codec_adapter::raster_mode_from_codec_mode(source.raster_mode());
        if requested
            .as_ref()
            .is_some_and(|requested| !requested.contains(&info.format))
        {
            return Err(PilError::UnidentifiedImageError("memory".to_owned()));
        }
        Ok(Image::Bytes {
            source,
            header_mode,
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
