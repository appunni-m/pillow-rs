//! Pillow's Arrow image view over materialized raster storage.

use std::sync::Arc;

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::{
    DynamicImage, GrayAlphaImage, GrayImage, ImageBuffer, Luma, Mode, RgbImage, RgbaImage,
};

use super::Image;

/// Rust-owned image storage exposed by the Python Arrow C-data adapter.
///
/// This binding-facing type keeps materialized raster storage alive for the
/// lifetime of an Arrow array. Compact 8-bit carriers are expanded only when
/// Pillow's four-byte multiband Arrow layout requires their physical lanes.
#[doc(hidden)]
#[derive(Clone)]
pub struct ArrowExportBuffer {
    image: Arc<DynamicImage>,
    expanded: Option<Arc<[u8]>>,
    mode: Mode,
    width: u32,
    height: u32,
    pixel_count: usize,
    format: &'static str,
    name: &'static str,
    bands: &'static [&'static str],
}

#[derive(Clone, Copy)]
struct ArrowLayout {
    format: &'static str,
    name: &'static str,
    bands: &'static [&'static str],
}

const ONE_BAND_ONE: &[&str] = &["1"];
const ONE_BAND_L: &[&str] = &["L"];
const ONE_BAND_P: &[&str] = &["P"];
const ONE_BAND_I: &[&str] = &["I"];
const ONE_BAND_F: &[&str] = &["F"];
const TWO_BAND_LA: &[&str] = &["L", "X", "X", "A"];
const TWO_BAND_LA_PREMULTIPLIED: &[&str] = &["L", "X", "X", "a"];
const TWO_BAND_PA: &[&str] = &["P", "X", "X", "A"];
const THREE_BAND_RGB: &[&str] = &["R", "G", "B", "X"];
const FOUR_BAND_YCBCR: &[&str] = &["Y", "Cb", "Cr", "X"];
const FOUR_BAND_LAB: &[&str] = &["L", "a", "b", "X"];
const FOUR_BAND_HSV: &[&str] = &["H", "S", "V", "X"];
const FOUR_BAND_RGBA: &[&str] = &["R", "G", "B", "A"];
const FOUR_BAND_RGB_PREMULTIPLIED: &[&str] = &["R", "G", "B", "a"];
const FOUR_BAND_RGBX: &[&str] = &["R", "G", "B", "X"];
const FOUR_BAND_CMYK: &[&str] = &["C", "M", "Y", "K"];

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn layout_for_mode(mode: Mode) -> Result<ArrowLayout, PilError> {
    let layout = match mode {
        Mode::One => ArrowLayout {
            format: "C",
            name: "1",
            bands: ONE_BAND_ONE,
        },
        Mode::ImageLuma8 => ArrowLayout {
            format: "C",
            name: "L",
            bands: ONE_BAND_L,
        },
        Mode::ImagePalette8 => ArrowLayout {
            format: "C",
            name: "P",
            bands: ONE_BAND_P,
        },
        Mode::ImageI32 => ArrowLayout {
            format: "i",
            name: "I",
            bands: ONE_BAND_I,
        },
        Mode::ImageF32 => ArrowLayout {
            format: "f",
            name: "F",
            bands: ONE_BAND_F,
        },
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => ArrowLayout {
            format: "s",
            name: "I",
            bands: ONE_BAND_I,
        },
        Mode::ImageLumaA8 => ArrowLayout {
            format: "C",
            name: "LA",
            bands: TWO_BAND_LA,
        },
        Mode::ImageLaPremultiplied8 => ArrowLayout {
            format: "C",
            name: "La",
            bands: TWO_BAND_LA_PREMULTIPLIED,
        },
        Mode::ImagePaletteAlpha8 => ArrowLayout {
            format: "C",
            name: "PA",
            bands: TWO_BAND_PA,
        },
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => ArrowLayout {
            format: "C",
            name: match mode {
                Mode::ImageYCbCr8 => "YCbCr",
                Mode::ImageLab8 => "LAB",
                Mode::ImageHsv8 => "HSV",
                _ => "RGB",
            },
            bands: match mode {
                Mode::ImageYCbCr8 => FOUR_BAND_YCBCR,
                Mode::ImageLab8 => FOUR_BAND_LAB,
                Mode::ImageHsv8 => FOUR_BAND_HSV,
                _ => THREE_BAND_RGB,
            },
        },
        Mode::ImageRgba8 => ArrowLayout {
            format: "C",
            name: "RGBA",
            bands: FOUR_BAND_RGBA,
        },
        Mode::ImageRgbPremultiplied8 => ArrowLayout {
            format: "C",
            name: "RGBa",
            bands: FOUR_BAND_RGB_PREMULTIPLIED,
        },
        Mode::ImageRgbx8 => ArrowLayout {
            format: "C",
            name: "RGBX",
            bands: FOUR_BAND_RGBX,
        },
        Mode::ImageCmyk8 => ArrowLayout {
            format: "C",
            name: "CMYK",
            bands: FOUR_BAND_CMYK,
        },
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => {
            // NO_PILLOW_SUPPORT: these Rust raster modes have no Pillow mode
            // identity and therefore no established Arrow representation.
            return Err(PilError::ValueError(
                "Incompatible Pillow mode for Arrow array".into(),
            ));
        }
    };
    Ok(layout)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn expanded_multiband_bytes(
    image: &DynamicImage,
    mode: Mode,
) -> Result<Option<Arc<[u8]>>, PilError> {
    let Some(carrier_width) = mode.byte_carrier_pixel_width() else {
        return Ok(None);
    };
    if carrier_width == 4 {
        return Ok(None);
    }
    let pixel_count = (image.width() as usize)
        .checked_mul(image.height() as usize)
        .ok_or_else(|| PilError::DimensionError("Arrow pixel count overflows".into()))?;
    let expected_input = pixel_count
        .checked_mul(carrier_width)
        .ok_or_else(|| PilError::DimensionError("Arrow input size overflows".into()))?;
    let source = image.as_bytes();
    if source.len() != expected_input {
        return Err(PilError::InternalError(
            "Arrow carrier length does not match its mode".into(),
        ));
    }
    let output_len = pixel_count
        .checked_mul(4)
        .ok_or_else(|| PilError::DimensionError("Arrow output size overflows".into()))?;
    let mut expanded = Vec::new();
    expanded
        .try_reserve_exact(output_len)
        .map_err(|error| PilError::MemoryError(error.to_string()))?;
    expanded.resize(output_len, 0);
    let hidden = image.hidden_pixel_lanes();
    validate_hidden_lanes(hidden, mode, pixel_count)?;
    copy_visible_lanes(source, &mut expanded, mode, carrier_width, pixel_count)?;
    copy_hidden_lanes(hidden, &mut expanded, mode, pixel_count)?;
    Ok(Some(Arc::from(expanded)))
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn validate_hidden_lanes(
    hidden: Option<&HiddenPixelLanes>,
    mode: Mode,
    pixel_count: usize,
) -> Result<(), PilError> {
    if let Some(hidden) = hidden
        && (Some(hidden.lane_count()) != mode.hidden_pixel_lane_count()
            || hidden.pixel_count() != pixel_count)
    {
        return Err(PilError::InternalError(
            "Arrow hidden lane storage does not match its mode".into(),
        ));
    }
    Ok(())
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn copy_visible_lanes(
    source: &[u8],
    expanded: &mut [u8],
    mode: Mode,
    carrier_width: usize,
    pixel_count: usize,
) -> Result<(), PilError> {
    for pixel in 0..pixel_count {
        for lane in 0..carrier_width {
            let physical_lane = mode.byte_carrier_physical_offset(lane).ok_or_else(|| {
                PilError::InternalError("Arrow mode has an invalid byte carrier".into())
            })?;
            expanded[pixel * 4 + physical_lane] = source[pixel * carrier_width + lane];
        }
    }
    Ok(())
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn copy_hidden_lanes(
    hidden: Option<&HiddenPixelLanes>,
    expanded: &mut [u8],
    mode: Mode,
    pixel_count: usize,
) -> Result<(), PilError> {
    let Some(lane_count) = mode.hidden_pixel_lane_count() else {
        return Ok(());
    };
    for pixel in 0..pixel_count {
        let lanes = hidden.and_then(|lanes| lanes.for_pixel(pixel));
        for lane in 0..lane_count {
            let physical_lane = mode.hidden_pixel_lane_offset(lane).ok_or_else(|| {
                PilError::InternalError("Arrow mode has an invalid hidden lane".into())
            })?;
            expanded[pixel * 4 + physical_lane] = lanes.map_or(0, |values| values[lane]);
        }
    }
    Ok(())
}

impl ArrowExportBuffer {
    /// Exact mode identity captured from the materialized raster.
    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
    #[must_use]
    pub const fn mode(&self) -> Mode {
        self.mode
    }

    /// Image width in pixels.
    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Image height in pixels.
    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Arrow C Data Interface format for one scalar sample.
    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
    #[must_use]
    pub const fn format(&self) -> &'static str {
        self.format
    }

    /// Arrow field name matching the Pillow mode.
    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Physical Pillow band labels represented by the exported sample.
    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
    #[must_use]
    pub const fn bands(&self) -> &'static [&'static str] {
        self.bands
    }

    /// Number of image pixels, or fixed-size-list entries, in the array.
    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
    #[must_use]
    pub fn len(&self) -> usize {
        self.pixel_count
    }

    /// Whether the exported Arrow array contains no image pixels.
    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pixel_count == 0
    }

    /// Whether the schema is a four-element fixed-size list.
    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
    #[must_use]
    pub fn is_multiband(&self) -> bool {
        self.bands.len() == 4 && self.mode.band_count() > 1
    }

    /// Immutable contiguous bytes retained for the Arrow array lifetime.
    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.expanded
            .as_deref()
            .unwrap_or_else(|| self.image.as_bytes())
    }
}

impl Image {
    /// Prepare a contiguous Arrow view while retaining all Pillow physical
    /// byte lanes for multiband modes.
    ///
    /// Scalar storage and existing four-byte pixels are shared. Compact `LA`,
    /// `La`, `PA`, `RGB`, `YCbCr`, `LAB`, and `HSV` carriers expand into their
    /// Arrow four-byte form and retain omitted physical lanes when present.
    ///
    /// # Errors
    ///
    /// Returns materialization, memory, or invalid mode/storage errors.
    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
    pub fn arrow_export_buffer(&self) -> Result<ArrowExportBuffer, PilError> {
        let mode = self.mode_identity()?;
        let layout = layout_for_mode(mode)?;
        let image = self.materialized_shared()?;
        if image.mode() != mode {
            return Err(PilError::InternalError(
                "Arrow mode changed during materialization".into(),
            ));
        }
        let pixels = u64::from(image.width())
            .checked_mul(u64::from(image.height()))
            .and_then(|pixels| usize::try_from(pixels).ok())
            .ok_or_else(|| PilError::DimensionError("Arrow pixel count overflows".into()))?;
        let expanded = if layout.bands.len() > 1 {
            expanded_multiband_bytes(&image, mode)?
        } else {
            None
        };
        Ok(ArrowExportBuffer {
            width: image.width(),
            height: image.height(),
            image,
            expanded,
            mode,
            format: layout.format,
            name: layout.name,
            bands: layout.bands,
            pixel_count: pixels,
        })
    }

    /// Builds a materialized image from a borrowed physical Arrow pixel slice.
    ///
    /// The binding validates and consumes the Arrow C Data Interface capsules;
    /// this operation applies the exact mode's physical-byte layout and keeps
    /// storage and copies omitted lanes into the raster sidecar before the
    /// producer's Arrow capsule can be released.
    #[doc(hidden)]
    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
    pub fn from_arrow_buffer(
        mode: Mode,
        size: (i32, i32),
        layout: ArrowImportLayout,
        bytes: &[u8],
    ) -> Result<Self, PilError> {
        let size = validate_arrow_import_size(mode, size)?;
        validate_arrow_import_layout(mode, layout)?;
        let physical_width = mode
            .pillow_pixel_size()
            .ok_or_else(incompatible_arrow_mode)?;
        let pixel_count = checked_arrow_pixel_count(size)?;
        let expected_bytes = pixel_count
            .checked_mul(physical_width)
            .ok_or_else(|| PilError::DimensionError("Arrow image data is too large".into()))?;
        if bytes.len() != expected_bytes {
            return Err(invalid_arrow_array());
        }

        let (raster, hidden) = if is_native_u16_mode(mode) {
            (raster_from_arrow_u16(mode, size, bytes)?, None)
        } else {
            let (carrier, hidden) = unpack_arrow_carrier(mode, pixel_count, bytes)?;
            (raster_from_arrow_carrier(mode, size, carrier)?, hidden)
        };
        let raster = attach_arrow_hidden_lanes(raster, mode, hidden, pixel_count)?;
        if mode == Mode::ImagePalette8 {
            Self::from_generated_dynamic(raster, mode)
        } else {
            Ok(Self::from_dynamic(raster))
        }
    }
}

/// Validates Arrow import dimensions before reading or allocating pixel storage.
#[doc(hidden)]
// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub fn validate_arrow_import_size(mode: Mode, size: (i32, i32)) -> Result<(u32, u32), PilError> {
    let size = checked_arrow_size(size)?;
    let channels = mode
        .pillow_pixel_size()
        .ok_or_else(incompatible_arrow_mode)?;
    if size.0 != 0 && size.1 != 0 {
        CheckedDims::new(size.0, size.1, channels as u8)?;
    }
    Ok(size)
}

/// Physical Arrow representation selected by the producer's schema.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrowImportLayout {
    /// One scalar byte per pixel.
    ScalarU8,
    /// One native-endian signed 32-bit sample per pixel.
    ScalarI32,
    /// One native-endian 32-bit float sample per pixel.
    ScalarF32,
    /// One native-endian 16-bit sample per pixel.
    ScalarI16,
    /// Four uint8 child lanes in a fixed-size list per pixel.
    FixedSizeByteList4,
    /// Four interleaved uint8 values per pixel in a flat array.
    InterleavedByte4,
    /// One int32 element per four-byte multiband pixel.
    PackedI32Multiband,
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
fn checked_arrow_size(size: (i32, i32)) -> Result<(u32, u32), PilError> {
    let (width, height) = size;
    if width < 0 || height < 0 {
        return Err(PilError::ValueError("bad image size".into()));
    }
    Ok((width as u32, height as u32))
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
fn checked_arrow_pixel_count(size: (u32, u32)) -> Result<usize, PilError> {
    usize::try_from(u64::from(size.0) * u64::from(size.1))
        .map_err(|_| PilError::DimensionError("Arrow pixel count is too large".into()))
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
fn incompatible_arrow_mode() -> PilError {
    PilError::ValueError("Incompatible Pillow mode for Arrow array".into())
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
fn invalid_arrow_array() -> PilError {
    PilError::ValueError("Invalid Arrow array mode or size mismatch".into())
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
fn validate_arrow_import_layout(mode: Mode, layout: ArrowImportLayout) -> Result<(), PilError> {
    let valid = match layout {
        ArrowImportLayout::ScalarU8 => {
            matches!(mode, Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8)
        }
        ArrowImportLayout::ScalarI32 => mode == Mode::ImageI32,
        ArrowImportLayout::ScalarF32 => mode == Mode::ImageF32,
        ArrowImportLayout::ScalarI16 => matches!(
            mode,
            Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N
        ),
        ArrowImportLayout::FixedSizeByteList4 | ArrowImportLayout::InterleavedByte4 => {
            mode.band_count() > 1 && mode.pillow_pixel_size() == Some(4)
        }
        ArrowImportLayout::PackedI32Multiband => {
            mode.band_count() > 1 && mode.pillow_pixel_size() == Some(4)
        }
    };
    if valid {
        Ok(())
    } else {
        Err(invalid_arrow_array())
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
fn is_native_u16_mode(mode: Mode) -> bool {
    matches!(
        mode,
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N
    )
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
fn unpack_arrow_carrier(
    mode: Mode,
    pixel_count: usize,
    bytes: &[u8],
) -> Result<(Vec<u8>, Option<HiddenPixelLanes>), PilError> {
    let physical_width = mode
        .pillow_pixel_size()
        .ok_or_else(incompatible_arrow_mode)?;
    let carrier_width = mode.byte_carrier_pixel_width();
    if carrier_width == Some(physical_width) {
        let mut carrier = Vec::new();
        carrier
            .try_reserve_exact(bytes.len())
            .map_err(|error| PilError::MemoryError(error.to_string()))?;
        carrier.extend_from_slice(bytes);
        return Ok((carrier, None));
    }
    let carrier_width = carrier_width.ok_or_else(incompatible_arrow_mode)?;
    unpack_compact_arrow_carrier(mode, pixel_count, carrier_width, bytes)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
fn unpack_compact_arrow_carrier(
    mode: Mode,
    pixel_count: usize,
    carrier_width: usize,
    bytes: &[u8],
) -> Result<(Vec<u8>, Option<HiddenPixelLanes>), PilError> {
    let compact_len = pixel_count
        .checked_mul(carrier_width)
        .ok_or_else(|| PilError::DimensionError("Arrow image data is too large".into()))?;
    let hidden_width = mode
        .hidden_pixel_lane_count()
        .ok_or_else(incompatible_arrow_mode)?;
    let hidden_len = pixel_count
        .checked_mul(hidden_width)
        .ok_or_else(|| PilError::DimensionError("Arrow hidden lanes are too large".into()))?;
    let mut compact = Vec::new();
    compact
        .try_reserve_exact(compact_len)
        .map_err(|error| PilError::MemoryError(error.to_string()))?;
    let mut hidden = Vec::new();
    hidden
        .try_reserve_exact(hidden_len)
        .map_err(|error| PilError::MemoryError(error.to_string()))?;
    for pixel in bytes.chunks_exact(4) {
        for lane in 0..carrier_width {
            let offset = mode
                .byte_carrier_physical_offset(lane)
                .ok_or_else(incompatible_arrow_mode)?;
            compact.push(pixel[offset]);
        }
        for lane in 0..hidden_width {
            let offset = mode
                .hidden_pixel_lane_offset(lane)
                .ok_or_else(incompatible_arrow_mode)?;
            hidden.push(pixel[offset]);
        }
    }
    let hidden = HiddenPixelLanes::per_pixel(hidden_width, pixel_count, hidden)
        .ok_or_else(|| PilError::InternalError("invalid Arrow hidden lane layout".into()))?;
    Ok((compact, Some(hidden)))
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn raster_from_arrow_carrier(
    mode: Mode,
    (width, height): (u32, u32),
    carrier: Vec<u8>,
) -> Result<DynamicImage, PilError> {
    let raster = match mode {
        Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8 => {
            let image = GrayImage::from_raw(width, height, carrier).ok_or_else(|| {
                PilError::ValueError("Arrow image buffer has an invalid shape".into())
            })?;
            DynamicImage::ImageLuma8(image)
        }
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => {
            let image = GrayAlphaImage::from_raw(width, height, carrier).ok_or_else(|| {
                PilError::ValueError("Arrow image buffer has an invalid shape".into())
            })?;
            DynamicImage::ImageLumaA8(image)
        }
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => {
            let image = RgbImage::from_raw(width, height, carrier).ok_or_else(|| {
                PilError::ValueError("Arrow image buffer has an invalid shape".into())
            })?;
            DynamicImage::ImageRgb8(image)
        }
        Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageI32
        | Mode::ImageF32 => {
            let image = RgbaImage::from_raw(width, height, carrier).ok_or_else(|| {
                PilError::ValueError("Arrow image buffer has an invalid shape".into())
            })?;
            DynamicImage::ImageRgba8(image)
        }
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            return Err(incompatible_arrow_mode());
        }
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => return Err(incompatible_arrow_mode()),
    };
    raster
        .with_mode(mode)
        .map_err(|_| incompatible_arrow_mode())
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn raster_from_arrow_u16(
    mode: Mode,
    (width, height): (u32, u32),
    bytes: &[u8],
) -> Result<DynamicImage, PilError> {
    let mut samples = Vec::new();
    samples
        .try_reserve_exact(bytes.len() / 2)
        .map_err(|error| PilError::MemoryError(error.to_string()))?;
    samples.extend(
        bytes
            .chunks_exact(2)
            .map(|bytes| u16::from_ne_bytes([bytes[0], bytes[1]])),
    );
    let raster = DynamicImage::ImageI16(
        ImageBuffer::<Luma<u16>, Vec<u16>>::from_raw(width, height, samples)
            .ok_or_else(invalid_arrow_array)?,
    );
    raster
        .with_mode(mode)
        .map_err(|_| incompatible_arrow_mode())
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
fn attach_arrow_hidden_lanes(
    mut raster: DynamicImage,
    mode: Mode,
    hidden: Option<HiddenPixelLanes>,
    pixel_count: usize,
) -> Result<DynamicImage, PilError> {
    let Some(hidden) = hidden else {
        return Ok(raster);
    };
    if hidden.pixel_count() != pixel_count
        || Some(hidden.lane_count()) != mode.hidden_pixel_lane_count()
    {
        return Err(PilError::InternalError(
            "invalid Arrow hidden lane dimensions".into(),
        ));
    }
    let attached = match (mode, &mut raster) {
        (Mode::ImageLumaA8, DynamicImage::ImageLumaA8(image)) => {
            image.replace_hidden_pixel_lanes(Some(hidden))
        }
        (Mode::ImageLaPremultiplied8, DynamicImage::ImageLaPremultiplied8(image)) => {
            image.replace_hidden_pixel_lanes(Some(hidden))
        }
        (Mode::ImagePaletteAlpha8, DynamicImage::ImagePaletteAlpha8(image)) => {
            image.replace_hidden_pixel_lanes(Some(hidden))
        }
        (Mode::ImageRgb8, DynamicImage::ImageRgb8(image)) => {
            image.replace_hidden_pixel_lanes(Some(hidden))
        }
        (Mode::ImageYCbCr8, DynamicImage::ImageYCbCr8(image)) => {
            image.replace_hidden_pixel_lanes(Some(hidden))
        }
        (Mode::ImageLab8, DynamicImage::ImageLab8(image)) => {
            image.replace_hidden_pixel_lanes(Some(hidden))
        }
        (Mode::ImageHsv8, DynamicImage::ImageHsv8(image)) => {
            image.replace_hidden_pixel_lanes(Some(hidden))
        }
        _ => false,
    };
    if attached {
        Ok(raster)
    } else {
        Err(PilError::InternalError(
            "Arrow mode did not match its raster carrier".into(),
        ))
    }
}
