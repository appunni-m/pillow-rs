//! Pillow-compatible `ImageDraw` primitives.
//!
//! [`Draw`] records drawing operations against an [`Image`] and keeps enough
//! mode metadata to convert the drawn result back to the original Pillow mode.
//! Coordinates are integer pixel coordinates. Colors are normalized RGBA byte
//! tuples before mode-specific drawing rules are applied.

use crate::compute::{Backend, ExecutionContext};
use crate::error::PilError;
use crate::image::{Image, ImageInfoValue, PaletteColorComponent};
use crate::raster::{DynamicImage, Mode};

mod color_input;

use color_input::{
    draw_single_band_color_error, is_double_band_draw_mode, is_single_band_draw_mode,
};

type DrawColor = (u8, u8, u8, u8);
type ShapeInks = (Option<DrawColor>, Option<DrawColor>);

/// Drawing context for Pillow-style image mutation.
///
/// This is the Rust equivalent of `ImageDraw.Draw(image)`. Methods queue or
/// apply drawing operations and [`Draw::image_clone`] returns the updated image
/// with the original mode restored where possible.
#[derive(Debug)]
pub struct Draw {
    image: Image,
    /// Original image mode used to restore the public image on `image_clone()`.
    orig_mode: Option<Mode>,
    /// Requested drawing mode, which may differ from `orig_mode` for Pillow's
    /// RGBA-on-RGB alpha-compositing context.
    draw_mode: Option<Mode>,
    /// Palette entries reserved by wrapper metadata during P color allocation.
    palette_reserved_indices: Vec<u32>,
    /// ImagePalette-style tuple keys retained after palette lookup, including
    /// keys whose byte conversion returned an error.
    palette_color_cache: Vec<(Vec<PaletteColorComponent>, u8)>,
}

fn image_with_draw_output(source: &Image, output: DynamicImage) -> Image {
    let mut image = Image::from_dynamic(output);
    if let Image::Loaded(data) = &mut image {
        data.palette = source.palette();
        data.palette_alpha = source.palette_alpha();
        data.source_format = source.source_format();
        data.info = source.image_info();
        data.exif = source.exif_metadata();
    }
    image
}

/// Host-neutral coordinate input for ImageDraw point-based primitives.
#[derive(Debug, Clone)]
pub enum DrawPointsInput {
    /// Flat `x0, y0, x1, y1, ...` values.
    Flat(Vec<i32>),
    /// Nested `(x, y)` point values.
    Nested(Vec<Vec<i32>>),
    /// A list/tuple sequence whose elements are not supported coordinates.
    InvalidSequence,
    /// A value that could not be represented as either supported sequence.
    Invalid,
}

/// Host-neutral center input for `ImageDraw.circle`.
#[derive(Debug, Clone)]
pub enum DrawCircleCenterInput {
    /// A sequence whose first two values are the center coordinates.
    Values(Vec<f64>),
    /// An integer is accepted by the host boundary but has no subscriptable
    /// center, matching Pillow's direct `xy[0]` diagnostic.
    Integer,
    /// Text reaches Pillow's arithmetic path after indexing the first
    /// character, so preserve its type-specific subtraction diagnostic.
    Text,
    /// A mapping is indexed by integer zero before Pillow performs arithmetic;
    /// an empty mapping therefore raises `KeyError(0)`.
    Mapping,
    /// A value that could not be represented as a numeric sequence.
    Invalid,
}

/// Host-neutral coordinate input for ImageDraw rectangle-like primitives.
#[derive(Debug, Clone)]
pub enum DrawBoxInput {
    /// Flat `x0, y0, x1, y1` values.
    Flat(Vec<i32>),
    /// Nested `((x0, y0), (x1, y1))` values.
    Nested(Vec<Vec<i32>>),
    /// A value that could not be represented as a supported sequence.
    Invalid,
}

/// Host-neutral color input for ImageDraw operations.
#[derive(Debug, Clone)]
pub enum DrawColorInput {
    /// No fill or outline was supplied.
    None,
    /// A Pillow color name or hexadecimal color string.
    String(String),
    /// An integer color value.
    Integer(i64),
    /// An arbitrary-size scalar Python integer, retained until the drawing
    /// mode decides whether Pillow parses it as a long long or a float.
    WideInteger(PaletteColorComponent),
    /// A floating-point color value, used by `F` mode.
    Float(f64),
    /// A component sequence supplied by a Rust caller.
    Components(Vec<i64>),
    /// Tuple-like integer components, retained because P mode resolves colors
    /// through its palette.
    TupleComponents(Vec<i64>),
    /// Tuple-like components whose values are all floating-point numbers.
    TupleFloatComponents(Vec<f64>),
    /// Tuple-like components mixing integer and floating-point values.
    TupleMixedComponents(Vec<PaletteColorComponent>),
    /// Integer tuple with at least one component outside i64.
    ///
    /// Retained until the mode selects long-long, C-int, float, or palette-key
    /// handling for each component.
    TupleComponentsWithWideInteger(Vec<PaletteColorComponent>),
    /// Python list components, which do not trigger P mode palette allocation.
    ListComponents(Vec<i64>),
    /// A Python tuple whose values could not be represented as integer components.
    InvalidTuple,
    /// A Python list whose values could not be represented as integer components.
    InvalidList,
    /// A value that is not a supported color form.
    Invalid,
}

/// Normalize Pillow's optional draw width at the core boundary.
pub fn normalize_draw_width(width: Option<u32>) -> u32 {
    // An omitted width defaults to one, but an explicit zero is preserved.
    // Pillow forwards that distinction to the C primitive: line-like paths
    // handle zero as their own default while outline paths treat it as a
    // no-op.  Collapsing `Some(0)` here makes those public contracts
    // indistinguishable before the backend can apply the operation-specific
    // rule.
    width.unwrap_or(1)
}

/// Normalizes a flat or nested ImageDraw bounding box.
pub fn normalize_draw_box(input: DrawBoxInput) -> Result<(i32, i32, i32, i32), PilError> {
    let error = || PilError::TypeError("coordinate list must contain exactly 2 coordinates".into());
    match input {
        DrawBoxInput::Flat(values) if values.len() == 4 => {
            Ok((values[0], values[1], values[2], values[3]))
        }
        DrawBoxInput::Nested(values)
            if values.len() == 2 && values.iter().all(|point| point.len() == 2) =>
        {
            Ok((values[0][0], values[0][1], values[1][0], values[1][1]))
        }
        // Pillow's nested coordinate parser reports a malformed point as a
        // value error, while a flat sequence with the wrong number of values
        // keeps the generic coordinate-list type error.
        DrawBoxInput::Nested(values)
            if values.len() == 2 && values.iter().any(|point| point.len() != 2) =>
        {
            Err(PilError::ValueError("incorrect coordinate type".into()))
        }
        // Pillow's three-value flat parser reports an arity mismatch as a
        // value error; other flat forms and nested sequences retain the
        // generic type diagnostic used by its coordinate unpacker.
        DrawBoxInput::Flat(values) if values.len() == 3 => {
            Err(PilError::ValueError("wrong number of coordinates".into()))
        }
        DrawBoxInput::Flat(_) | DrawBoxInput::Nested(_) | DrawBoxInput::Invalid => Err(error()),
    }
}

/// Normalizes the rounded-rectangle box, preserving its two-scalar Pillow
/// diagnostic before falling back to the shared box validation.
pub fn normalize_rounded_rectangle_box(
    input: DrawBoxInput,
) -> Result<(i32, i32, i32, i32), PilError> {
    if matches!(&input, DrawBoxInput::Flat(values) if values.len() == 2) {
        return Err(PilError::ValueError(
            "not enough values to unpack (expected 4, got 2)".into(),
        ));
    }
    normalize_draw_box(input)
}

fn normalize_draw_points(
    input: DrawPointsInput,
    allow_short: bool,
) -> Result<Vec<(i32, i32)>, PilError> {
    // Pillow's _imaging.c coordinate parser distinguishes an odd flat list
    // ("wrong number of coordinates") from a malformed nested point
    // ("incorrect coordinate type"); preserve that public split here.
    let too_few =
        || PilError::TypeError("coordinate list must contain at least 2 coordinates".into());
    let invalid_input = || PilError::TypeError("argument must be sequence".into());
    let wrong_number = || PilError::ValueError("wrong number of coordinates".into());
    let wrong_point = || PilError::ValueError("incorrect coordinate type".into());
    match input {
        DrawPointsInput::Flat(values) => {
            if values.len() % 2 != 0 {
                return Err(wrong_number());
            }
            if allow_short && values.len() < 4 {
                return Ok(Vec::new());
            }
            if values.len() < 4 {
                return Err(too_few());
            }
            Ok(values
                .chunks_exact(2)
                .map(|point| (point[0], point[1]))
                .collect())
        }
        DrawPointsInput::Nested(values) => {
            if values.iter().any(|point| point.len() != 2) {
                return Err(wrong_point());
            }
            if values.len() < 2 {
                if allow_short {
                    return Ok(Vec::new());
                }
                return Err(too_few());
            }
            Ok(values
                .into_iter()
                .map(|point| (point[0], point[1]))
                .collect())
        }
        DrawPointsInput::InvalidSequence => Err(wrong_point()),
        DrawPointsInput::Invalid => Err(invalid_input()),
    }
}

fn normalize_draw_point_input(input: DrawPointsInput) -> Result<Vec<(i32, i32)>, PilError> {
    let invalid_input = || PilError::TypeError("argument must be sequence".into());
    let wrong_number = || PilError::ValueError("wrong number of coordinates".into());
    let wrong_point = || PilError::ValueError("incorrect coordinate type".into());
    match input {
        DrawPointsInput::Flat(values) => {
            if values.is_empty() {
                return Ok(Vec::new());
            }
            if values.len() % 2 != 0 {
                return Err(wrong_number());
            }
            Ok(values
                .chunks_exact(2)
                .map(|point| (point[0], point[1]))
                .collect())
        }
        DrawPointsInput::Nested(values) => {
            if values.iter().any(|point| point.len() != 2) {
                return Err(wrong_point());
            }
            Ok(values
                .into_iter()
                .map(|point| (point[0], point[1]))
                .collect())
        }
        DrawPointsInput::InvalidSequence => Err(wrong_point()),
        DrawPointsInput::Invalid => Err(invalid_input()),
    }
}

/// Normalizes the center accepted by Pillow's circle wrapper.
pub fn normalize_draw_circle_center(input: DrawCircleCenterInput) -> Result<(f64, f64), PilError> {
    match input {
        DrawCircleCenterInput::Values(values) if values.len() >= 2 => Ok((values[0], values[1])),
        // Pillow's circle wrapper indexes the first two sequence elements
        // directly; a short numeric sequence therefore raises IndexError
        // rather than the TypeError used by its other coordinate parsers.
        DrawCircleCenterInput::Values(_) => {
            Err(PilError::IndexError("tuple index out of range".into()))
        }
        DrawCircleCenterInput::Integer => Err(PilError::TypeError(
            "'int' object is not subscriptable".into(),
        )),
        DrawCircleCenterInput::Text => Err(PilError::TypeError(
            "unsupported operand type(s) for -: 'str' and 'int'".into(),
        )),
        DrawCircleCenterInput::Mapping => Err(PilError::KeyErrorInt(0)),
        DrawCircleCenterInput::Invalid => Err(PilError::TypeError(
            "'NoneType' object is not subscriptable".into(),
        )),
    }
}

/// Host-neutral `ImageDraw.regular_polygon` bounding-circle input.
#[derive(Debug, Clone, Copy)]
pub enum RegularPolygonCircle {
    /// `(x, y, radius)` form.
    Flat(f64, f64, f64),
    /// `((x, y), radius)` form.
    Nested(f64, f64, f64),
    /// A value that was not one of the accepted forms.
    Invalid,
}

/// Host-neutral side-count input for `ImageDraw.regular_polygon`.
#[derive(Debug, Clone, Copy)]
pub enum RegularPolygonSides {
    /// The integer supplied by the caller.
    Value(i64),
    /// A non-integer value.
    Invalid,
}

impl Draw {
    /// Creates a drawing context for `image`.
    ///
    /// `explicit_mode` may select Pillow's RGBA drawing context over an RGB image.
    pub fn new(image: Image, explicit_mode: Option<Mode>) -> Self {
        // Keep Draw construction lazy and infallible. If the source mode is
        // unresolved, operations retry through effective_mode() and propagate
        // the error instead of selecting an invented default.
        let original_mode = image.mode_identity().ok();
        let draw_mode = explicit_mode.or(original_mode);
        let palette_reserved_indices =
            if original_mode == Some(Mode::ImagePalette8) && image.has_attached_palette() {
                image
                    .compatibility_info()
                    .into_iter()
                    .filter_map(|(name, value)| {
                        if !matches!(name.as_str(), "background" | "transparency") {
                            return None;
                        }
                        match value {
                            ImageInfoValue::Integer(index) => u32::try_from(index)
                                .ok()
                                .filter(|index| *index < 256),
                            _ => None,
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            };
        Draw {
            image,
            orig_mode: original_mode,
            draw_mode,
            palette_reserved_indices,
            palette_color_cache: Vec::new(),
        }
    }

    /// Supplies metadata indices that a P palette allocator must not reuse.
    ///
    /// Python's `Image.info` is wrapper-owned and can be mutated after the
    /// Rust image was created, so the binding refreshes these values before
    /// each drawing operation that may resolve a palette color.
    #[doc(hidden)]
    pub fn set_palette_reserved_indices(&mut self, indices: &[u32]) {
        self.palette_reserved_indices.clear();
        self.palette_reserved_indices.extend_from_slice(indices);
    }

    /// Replaces the indexed palette after a Python-owned `ImagePalette`
    /// resolves a tuple color.
    #[doc(hidden)]
    pub fn set_palette_data(&mut self, data: &[u8], mode: &str) -> Result<(), PilError> {
        self.palette_color_cache.clear();
        self.image.replace_indexed_palette_from_draw(data, mode)
    }

    /// Resolves eager drawing through CPU while preserving SIMD's architecture request.
    pub(crate) fn cpu_execution_context(
        &self,
    ) -> Result<(Option<Backend>, ExecutionContext), PilError> {
        self.image.cpu_arch_execution_route()
    }

    /// Replaces PA samples while keeping palette and source metadata attached.
    pub(super) fn replace_palette_alpha_output(
        &mut self,
        image: DynamicImage,
    ) -> Result<(), PilError> {
        if !matches!(&image, DynamicImage::ImagePaletteAlpha8(_)) {
            return Err(PilError::InternalError(
                "Draw PA strategy returned another raster mode".into(),
            ));
        }
        self.replace_draw_output(image);
        Ok(())
    }

    /// Replaces eagerly drawn pixels while retaining their Image metadata.
    pub(super) fn replace_draw_output(&mut self, image: DynamicImage) {
        self.image = image_with_draw_output(&self.image, image);
    }

    /// Validate Pillow's optional draw-mode override against the destination.
    ///
    /// RGB images may be drawn through an RGBA context for alpha blending. All
    /// other explicit modes must match the destination image mode exactly.
    pub fn validate_mode(&self) -> Result<(), PilError> {
        let Some(requested) = self.draw_mode else {
            return Ok(());
        };
        let actual = self.image.mode_identity()?;
        if requested != actual
            && !(requested == Mode::ImageRgba8 && actual == Mode::ImageRgb8)
        {
            return Err(PilError::ValueError("mode mismatch".to_owned()));
        }
        Ok(())
    }

    /// Return the effective PIL mode without inventing a fallback identity.
    fn effective_mode(&self) -> Result<Mode, PilError> {
        match self.draw_mode.or(self.orig_mode) {
            Some(mode) => Ok(mode),
            None => self.image.mode_identity(),
        }
    }

    /// Whether Pillow requested an RGBA drawing context over an RGB image.
    fn alpha_blend_rgb(&self) -> bool {
        self.draw_mode == Some(Mode::ImageRgba8)
            && self.orig_mode == Some(Mode::ImageRgb8)
    }

    /// Validate text options against the destination's Pillow mode.
    pub fn validate_text_options(
        &self,
        options: &crate::font::ImageFontTextOptions,
    ) -> Result<(), PilError> {
        if options.embedded_color
            && !matches!(
                self.effective_mode()?,
                Mode::ImageRgb8 | Mode::ImageRgba8
            )
        {
            return Err(PilError::ValueError(
                "Embedded color supported only in RGB and RGBA modes".into(),
            ));
        }
        Ok(())
    }

    /// Validates a bitmap fill and delegates the complete operation to core.
    pub fn bitmap_with_input(
        &mut self,
        x: i32,
        y: i32,
        bitmap: &Image,
        input: DrawColorInput,
    ) -> Result<(), PilError> {
        let fill = self.resolve_bitmap_fill_with_input(input)?;
        self.bitmap(x, y, bitmap, fill)
    }

    /// Validate and resolve a Bitmap fill before the wrapper parses `xy`.
    ///
    /// # Errors
    ///
    /// Returns the mode-specific fill error that Bitmap reports before
    /// coordinate or mask validation.
    pub fn resolve_bitmap_fill_with_input(
        &mut self,
        input: DrawColorInput,
    ) -> Result<Option<(u8, u8, u8, u8)>, PilError> {
        if matches!(input, DrawColorInput::None) {
            return Ok(None);
        }
        let mode = self.effective_mode()?;
        if let DrawColorInput::Components(values) | DrawColorInput::TupleComponents(values) = &input {
            if is_single_band_draw_mode(mode) && mode != Mode::ImagePalette8 && values.len() != 1 {
                if mode == Mode::ImageF32 {
                    return Err(PilError::TypeError(
                        "must be real number, not tuple".to_owned(),
                    ));
                }
                return Err(PilError::TypeError(
                    "color must be int or single-element tuple".to_owned(),
                ));
            }
            if is_double_band_draw_mode(mode) && !matches!(values.len(), 1 | 2) {
                return Err(PilError::TypeError(
                    "color must be int, or tuple of one or two elements".to_owned(),
                ));
            }
        }
        let color = self.resolve_draw_color_with_input(input)?;
        Ok(Some(color))
    }

    /// Returns the original Pillow mode of the drawing target.
    pub fn mode(&self) -> Option<Mode> {
        self.orig_mode
    }

    fn shape_inks(
        &self,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
    ) -> Result<ShapeInks, PilError> {
        if fill.is_none() && outline.is_none() {
            if self.effective_mode()? == Mode::ImagePaletteAlpha8 {
                return Ok((None, Some((255, 255, 255, 255))));
            }
            // ImageDraw geometric primitives default to the context's outline
            // ink, not a filled interior.  Rectangle/ellipse/polygon and
            // their relatives therefore retain a one-pixel white outline on
            // RGB-family images when both color arguments are omitted.
            return Ok((None, self.default_shape_ink()?));
        }
        Ok((fill, outline))
    }

    /// Return the ink Pillow's experimental `Outline` API uses when both
    /// `fill` and `outline` are omitted.
    fn default_shape_ink(&self) -> Result<Option<(u8, u8, u8, u8)>, PilError> {
        let ink = match self.effective_mode()? {
            // ImageDraw._getink() starts an RGB-family outline with
            // draw_ink(-1), which resolves to an all-255 sample.
            Mode::ImageRgb8
            | Mode::ImageRgba8
            | Mode::ImageRgbx8
            | Mode::ImageRgbPremultiplied8
            | Mode::ImageLaPremultiplied8
            | Mode::ImageLab8
            | Mode::ImageLumaA8
            | Mode::ImagePaletteAlpha8
            | Mode::ImageCmyk8
            | Mode::ImageYCbCr8
            | Mode::ImageHsv8 => {
                Some((255, 255, 255, 255))
            }
            // I/F contexts initialize their ink with draw_ink(1). These
            // tuples are the native little-endian representations carried by
            // the explicit-mode RGBA canvas used by this crate.
            Mode::ImageI32 => Some((1, 0, 0, 0)),
            // I;16* uses Pillow's draw_ink(-1) default. The native canvas
            // consumes the low two packed bytes as the full 65535 sample.
            Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
                Some((255, 255, 255, 255))
            }
            Mode::ImageF32 => Some((0, 0, 128, 63)),
            // Pillow's draw_ink(-1) is not a valid ink for these single-byte
            // and palette-indexed modes, so shape() remains a no-op.
            Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8 => None,
            _ => None,
        };
        Ok(ink)
    }

    /// Returns the current drawn image with original mode semantics restored.
    ///
    /// The only public path that changes the stored mode during drawing is an
    /// explicit RGBA context over an RGB image; this method restores RGB.
    pub fn image_clone(&self) -> Result<Image, PilError> {
        let img = self.image.clone();
        if self.orig_mode != Some(Mode::ImageRgb8)
            || img.mode_identity()? == Mode::ImageRgb8
        {
            return Ok(img);
        }

        // The RGBA draw context over RGB is the one public path whose backing
        // storage intentionally differs from the logical destination mode.
        let img_loaded = img.materialize()?;
        Ok(image_with_draw_output(
            &img,
            DynamicImage::ImageRgb8(img_loaded.to_rgb8()),
        ))
    }

    /// Consume the drawing context and return the modified image.
    pub fn into_image(self) -> Image {
        self.image
    }
}

mod bitmap;
mod curves;
mod geometry;
mod primitives;
mod text;

pub use geometry::outline_curve_points;
pub(crate) use geometry::{
    DrawCanvas, blend_u8, bresenham_line, for_each_bresenham_point, for_each_polygon_fill_span,
    pil_blend, plot, scanline_polygon_fill, wide_line_polygon_points,
};

#[cfg(test)]
mod tests {
    use super::{Draw, for_each_polygon_fill_span, wide_line_polygon_points};
    use crate::image::{FormattedPixelValue, Image};

    #[test]
    fn rounded_rectangle_rejects_reversed_axes_in_pillow_order() {
        let image = Image::new(8, 8, crate::raster::Mode::ImageLuma8, (0, 0, 0, 255)).expect("test image");
        let mut draw = Draw::new(image, None);

        let error = draw
            .rounded_rectangle(1, 5, 5, 1, 0.0, None, None, 1)
            .expect_err("reversed y axis must fail");
        assert_eq!(error.to_string(), "y1 must be greater than or equal to y0");

        let error = draw
            .rounded_rectangle(5, 5, 1, 1, 0.0, None, None, 1)
            .expect_err("reversed x axis must fail");
        assert_eq!(error.to_string(), "x1 must be greater than or equal to x0");

        draw.rounded_rectangle(1, 1, 5, 5, 0.0, None, None, 1)
            .expect("a non-reversed box remains valid");
    }

    #[test]
    fn i16_geometry_uses_native_samples_and_declared_byte_order() {
        for mode in ["I;16", "I;16L", "I;16B", "I;16N"] {
            let image = Image::new(
                4,
                4,
                crate::raster::Mode::from_pillow_name(mode).expect("known test mode"),
                (0, 0, 0, 0),
            )
            .expect("test image");
            let mut draw = Draw::new(image, None);
            draw.shape(&[(0, 0), (2, 0), (0, 2)], None, None)
                .expect("default shape ink");
            let image = draw.into_image();
            assert_eq!(image.mode().expect("mode"), mode);
            assert_eq!(
                image.getpixel_formatted(0, 0).expect("shape pixel"),
                FormattedPixelValue::Integer(65535)
            );

            let image = Image::new(
                1,
                1,
                crate::raster::Mode::from_pillow_name(mode).expect("known test mode"),
                (0, 0, 0, 0),
            )
            .expect("test image");
            let mut draw = Draw::new(image, None);
            draw.point(&[(0, 0)], (1, 0, 0, 255))
                .expect("packed point ink");
            let image = draw.into_image();
            let expected = if mode == "I;16B" { 256 } else { 1 };
            assert_eq!(
                image.getpixel_formatted(0, 0).expect("point pixel"),
                FormattedPixelValue::Integer(expected)
            );
        }
    }

    #[test]
    fn wide_line_bottom_edge_keeps_pillow_corner_intersection() {
        let points = wide_line_polygon_points(0, 5, 7, 0, 2).expect("non-zero line");
        let mut spans = Vec::new();
        for_each_polygon_fill_span(&points, 8, 6, |x_start, x_end, y| {
            spans.push((x_start, x_end, y));
        });

        assert!(spans.contains(&(0, 1, 5)));
    }
}
