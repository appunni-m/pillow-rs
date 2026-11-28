use super::{Draw, DrawColorInput, DrawTextFillValue, Mode};
use crate::compute::ExecutionContext;
use crate::compute::registry::support::StrategySelection;
use crate::error::PilError;
use crate::font::FontTextRef;
use crate::font::text_layout::multiline_text_positions;

mod composition;

/// Select the mask mode used by the custom-font ImageDraw bbox path.
///
/// Pillow's Draw bbox wrapper checks bilevel drawing contexts before
/// embedded-color text, unlike the mask-rendering path.
#[doc(hidden)]
pub fn draw_text_bbox_font_mode(mode: Mode, embedded_color: bool) -> Mode {
    if matches!(
        mode,
        Mode::One | Mode::ImagePalette8 | Mode::ImageI32 | Mode::ImageF32
    ) {
        Mode::One
    } else if embedded_color {
        Mode::ImageRgba8
    } else {
        Mode::ImageLuma8
    }
}

/// Ordered host facts used to choose ImageDraw's direct FreeType text route.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawTextRouteCheck {
    /// The original input was a native string or bytes value.
    NativeTextInput,
    /// The wrapper is exactly the built-in ImageText.Text class.
    ExactTextWrapper,
    /// The stored text is exactly str or bytes.
    NativeTextValue,
    /// The stored Text mode is exactly a Pillow mode string.
    ExactTextModeString,
    /// The font is exactly the built-in FreeTypeFont class.
    ExactFreeTypeFont,
    /// Spacing is exactly an int.
    ExactIntegerSpacing,
    /// Spacing fits the native signed 32-bit path.
    SpacingFitsNativeRange,
    /// Stroke width is exactly an int.
    ExactIntegerStrokeWidth,
    /// Stroke width fits the native float conversion range.
    StrokeWidthFitsNativeRange,
    /// Embedded-color state is exactly a bool.
    ExactEmbeddedColorBoolean,
    /// Anchor is None or exactly a str.
    AnchorSupportedType,
    /// Alignment is exactly a str.
    ExactAlignmentString,
    /// Direction is None.
    DirectionIsNone,
    /// Features is None.
    FeaturesAreNone,
    /// Language is None.
    LanguageIsNone,
    /// Coordinates are exactly a tuple.
    ExactCoordinateTuple,
    /// Coordinate tuple length is two.
    CoordinateTupleLengthTwo,
    /// Both coordinates are exact built-ins whose truncation fits C-int.
    NativeCoordinates,
    /// No extra positional font-mask arguments were supplied.
    NoExtraArguments,
    /// No extra font-mask keyword arguments were supplied.
    NoExtraKeywords,
    /// The owner is built-in ImageDraw with its mode-derived font-mask mode.
    DefaultFontMode,
    /// ImageText and FreeTypeFont still expose the methods used by the built-in route.
    BuiltinTextAndFontMethods,
}

/// Callback facts for the ordered direct-font route selector.
#[doc(hidden)]
pub trait DrawTextRouteCallbacks {
    /// Error carrier for Python protocol and attribute failures.
    type Error;

    /// Prepare Pillow's ImageText type after the original input is accepted.
    fn begin(&mut self) -> Result<(), Self::Error>;
    /// Evaluate one host check when requested by the core selector.
    fn check(&mut self, check: DrawTextRouteCheck) -> Result<bool, Self::Error>;
    /// Return the exact stored Text mode, if the host value is recognized.
    fn text_mode(&mut self) -> Result<Option<Mode>, Self::Error>;
}

/// Select the direct FreeType route, keeping host checks in Pillow order.
#[doc(hidden)]
// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub fn draw_uses_direct_text_route<C: DrawTextRouteCallbacks>(
    draw_mode: Mode,
    callbacks: &mut C,
) -> Result<bool, C::Error> {
    if !callbacks.check(DrawTextRouteCheck::NativeTextInput)? {
        return Ok(false);
    }
    callbacks.begin()?;
    let before_mode = [
        DrawTextRouteCheck::ExactTextWrapper,
        DrawTextRouteCheck::NativeTextValue,
        DrawTextRouteCheck::ExactTextModeString,
    ];
    for check in before_mode {
        if !callbacks.check(check)? {
            return Ok(false);
        }
    }
    if callbacks.text_mode()? != Some(draw_mode) {
        return Ok(false);
    }
    let after_mode = [
        DrawTextRouteCheck::ExactFreeTypeFont,
        DrawTextRouteCheck::ExactIntegerSpacing,
        DrawTextRouteCheck::SpacingFitsNativeRange,
        DrawTextRouteCheck::ExactIntegerStrokeWidth,
        DrawTextRouteCheck::StrokeWidthFitsNativeRange,
        DrawTextRouteCheck::ExactEmbeddedColorBoolean,
        DrawTextRouteCheck::AnchorSupportedType,
        DrawTextRouteCheck::ExactAlignmentString,
        DrawTextRouteCheck::DirectionIsNone,
        DrawTextRouteCheck::FeaturesAreNone,
        DrawTextRouteCheck::LanguageIsNone,
        DrawTextRouteCheck::ExactCoordinateTuple,
        DrawTextRouteCheck::CoordinateTupleLengthTwo,
        DrawTextRouteCheck::NativeCoordinates,
        DrawTextRouteCheck::NoExtraArguments,
        DrawTextRouteCheck::NoExtraKeywords,
        DrawTextRouteCheck::DefaultFontMode,
        DrawTextRouteCheck::BuiltinTextAndFontMethods,
    ];
    for check in after_mode {
        if !callbacks.check(check)? {
            return Ok(false);
        }
    }
    Ok(true)
}

impl Draw {
    /// Convert the resolved ink to the native value accepted by a font mask.
    #[doc(hidden)]
    pub fn text_fill_from_ink(&self, ink: i64) -> Result<DrawTextFillValue, PilError> {
        if self.effective_mode()? != Mode::ImageF32 {
            return Ok(DrawTextFillValue::Integer(ink));
        }
        let packed = i32::try_from(ink).map_err(|_| {
            PilError::InternalError("float text ink did not fit its packed i32 carrier".into())
        })?;
        Ok(DrawTextFillValue::Float(f32::from_le_bytes(
            packed.to_le_bytes(),
        )))
    }

    /// Resolve an ImageDraw text color to the native ink passed to font masks.
    ///
    /// Single-band and I;16 modes retain their scalar ink values. Multiband
    /// byte modes use the same little-endian channel order as RGBA font masks.
    pub fn text_ink_with_input(&mut self, input: DrawColorInput) -> Result<i64, PilError> {
        let fill = self.resolve_text_color_with_input(input)?;
        Ok(match self.effective_mode()? {
            Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8 => i64::from(fill.0),
            Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
                i64::from(u16::from_le_bytes([fill.0, fill.1]))
            }
            _ => Self::pack_text_ink(fill),
        })
    }

    /// Resolve a Pillow ImageDraw ink to the signed 32-bit value accepted by
    /// its drawing primitives.
    #[doc(hidden)]
    pub fn draw_ink_with_input(&mut self, input: DrawColorInput) -> Result<i32, PilError> {
        i32::try_from(self.text_ink_with_input(input)?).map_err(|_| {
            PilError::InternalError("draw ink did not fit its packed i32 carrier".into())
        })
    }

    /// Decode Pillow's already-packed drawing ink into this crate's channel
    /// carrier without interpreting it again as a user color.
    #[doc(hidden)]
    pub fn color_from_draw_ink(&self, ink: i32) -> Result<(u8, u8, u8, u8), PilError> {
        let [red, green, blue, alpha] = ink.to_le_bytes();
        Ok(match self.effective_mode()? {
            Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8 => (red, red, red, 255),
            Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
                (red, green, 0, 255)
            }
            Mode::ImageI32 | Mode::ImageF32 => (red, green, blue, alpha),
            Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => {
                (red, green, blue, alpha)
            }
            Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => {
                (red, green, blue, 255)
            }
            Mode::ImageRgba8
            | Mode::ImageRgbPremultiplied8
            | Mode::ImageRgbx8
            | Mode::ImageCmyk8
            | Mode::ImageLumaA16
            | Mode::ImageRgb16
            | Mode::ImageRgba16
            | Mode::ImageRgb32F
            | Mode::ImageRgba32F => (red, green, blue, alpha),
        })
    }

    /// Paste a custom font's RGBA glyph colors through the existing Paste
    /// mode strategy, using the mask alpha as coverage.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    pub fn paste_text_color_mask(
        &mut self,
        position: (i32, i32),
        color: &crate::image::Image,
        coverage: &crate::image::Image,
    ) -> Result<(), PilError> {
        Self::validate_text_color_mask(color, coverage)?;
        let (requested_backend, context) = self.cpu_execution_context()?;
        self.execute_text_color_mask_with_context(
            position,
            color,
            coverage,
            requested_backend,
            context,
        )
    }

    /// Paste a custom font's RGBA glyph colors with the parent Draw strategy's
    /// already selected backend and architecture.
    #[doc(hidden)]
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    pub fn paste_text_color_mask_with_context(
        &mut self,
        position: (i32, i32),
        color: &crate::image::Image,
        coverage: &crate::image::Image,
        requested_backend: Option<crate::compute::Backend>,
        context: ExecutionContext,
    ) -> Result<(), PilError> {
        Self::validate_text_color_mask(color, coverage)?;
        self.execute_text_color_mask_with_context(
            position,
            color,
            coverage,
            requested_backend,
            context,
        )
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn validate_text_color_mask(
        color: &crate::image::Image,
        coverage: &crate::image::Image,
    ) -> Result<(), PilError> {
        if color.mode_identity()? != Mode::ImageRgba8 {
            return Err(PilError::ValueError("bad transparency mask".into()));
        }
        if !matches!(coverage.mode_identity()?, Mode::One | Mode::ImageLuma8) {
            return Err(PilError::ValueError("bad transparency mask".into()));
        }
        Ok(())
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn execute_text_color_mask_with_context(
        &mut self,
        position: (i32, i32),
        color: &crate::image::Image,
        coverage: &crate::image::Image,
        requested_backend: Option<crate::compute::Backend>,
        context: ExecutionContext,
    ) -> Result<(), PilError> {
        let output = self.image.execute_paste_at_with_context(
            crate::ops::paste::PasteSource::Image(Box::new(color.clone())),
            position,
            coverage,
            requested_backend,
            context,
        )?;
        if let Some(output) = output {
            self.replace_draw_output(output);
        }
        Ok(())
    }

    /// Converts an RGBA glyph mask into Pillow's color-source and coverage
    /// images, composes them through the destination mode's Paste strategy,
    /// and restores eager Draw mutation semantics.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn paste_embedded_color_mask(
        &mut self,
        position: (i32, i32),
        size: (u32, u32),
        mut rgba_mask: Vec<u8>,
        ink_alpha: u8,
        requested_backend: Option<crate::compute::Backend>,
        context: &ExecutionContext,
    ) -> Result<(), PilError> {
        let expected_bytes =
            crate::checked_dims::CheckedDims::new(size.0, size.1, 4)?.total_bytes();
        if rgba_mask.len() != expected_bytes {
            return Err(PilError::InternalError(
                "embedded-color font mask did not match its RGBA dimensions".into(),
            ));
        }
        let mut coverage = Vec::with_capacity(expected_bytes / 4);
        for pixel in rgba_mask.chunks_exact_mut(4) {
            coverage.push(pixel[3]);
            pixel[3] = ink_alpha;
        }
        let color = crate::image::Image::frombytes_owned(Mode::ImageRgba8, size, rgba_mask)?;
        let coverage = crate::image::Image::frombytes_owned(Mode::ImageLuma8, size, coverage)?;
        self.paste_text_color_mask_with_context(
            position,
            &color,
            &coverage,
            requested_backend,
            *context,
        )
    }

    /// Read fill opacity from its semantic lane, using opaque alpha for modes
    /// whose fourth stored byte is color, padding, or a numeric sample.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn embedded_color_ink_alpha(mode: Mode, ink: Option<i64>, fill: (u8, u8, u8, u8)) -> u8 {
        match mode {
            Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => {
                ink.map_or(fill.1, |value| value.to_le_bytes()[1])
            }
            Mode::ImageRgba8 | Mode::ImageRgbPremultiplied8 => {
                ink.map_or(fill.3, |value| value.to_le_bytes()[3])
            }
            _ => 255,
        }
    }

    /// Selects the declared child route for embedded-color Text composition.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn embedded_color_paste_route_selected(
        raster_mode: Mode,
        context_mode: Mode,
        embedded_color: bool,
        stroke_width: f32,
        size: (u32, u32),
    ) -> Result<bool, PilError> {
        let route_inputs = crate::compute::registry::support::OperationRouteInputs {
            requested_backend: None,
            width: size.0,
            height: size.1,
            loaded_image: None,
            byte_storage_length_matches_pixel_count: None,
            reducing_gap_pre_reduction_selected: None,
            reducing_gap_fused_resize_selected: None,
            draw_context_mode: Some(context_mode),
            bitmap_mask_mode: None,
            draw_embedded_color: Some(embedded_color),
            draw_text_stroke_width_zero: Some(stroke_width == 0.0),
        };
        let child = crate::compute::pool_cpu::ops::draw::ops::SUPPORT
            .conditional_pipeline_child(raster_mode, route_inputs);
        match child {
            Some(crate::compute::registry::support::OperationId::Paste) => Ok(true),
            Some(_) => Err(PilError::InternalError(
                "Draw Text selected an unexpected composed child".into(),
            )),
            None if embedded_color && stroke_width == 0.0 => Err(PilError::InternalError(
                "Draw Text support omitted its embedded-color Paste child".into(),
            )),
            None => Ok(false),
        }
    }

    fn pack_text_ink(fill: (u8, u8, u8, u8)) -> i64 {
        i64::from(i32::from_le_bytes([fill.0, fill.1, fill.2, fill.3]))
    }
}
impl Draw {
    /// Draws text at `(x, y)` using a loaded font.
    ///
    /// Dispatches the native glyph mask and resolved ink to the exact-mode
    /// Draw strategy. Binary and indexed modes request Pillow's binary font
    /// mask; other supported modes use coverage masks, with alpha-bearing and
    /// packed sample behavior handled by their mode owners.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when mode detection, font rendering, or destination
    /// materialization fails.
    pub fn text(
        &mut self,
        x: i32,
        y: i32,
        text: &str,
        font: &crate::font::FreeTypeFont,
        fill: (u8, u8, u8, u8),
    ) -> Result<(), PilError> {
        self.text_with_options_inner(
            x,
            y,
            FontTextRef::String(text),
            font,
            fill,
            &crate::font::ImageFontTextOptions::default(),
        )
        .map(|_| ())
    }

    /// Draws text at `(x, y)` using Pillow-compatible text options.
    ///
    /// Libraqm-dependent options (`direction`, `features`, `language`) are
    /// validated by the `ImageFont` adapter and return
    /// [`PilError::UnsupportedLibraqm`] in no-libraqm builds.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when text option validation, font rendering, or
    /// destination materialization fails.
    // TODO(pillow-rs#brg0): rerun the exact CPU `text.mode.cmyk` and
    // `text.nuanced.stroked-rgba` output cases after CPU source closure.
    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
    pub fn text_with_options(
        &mut self,
        x: i32,
        y: i32,
        text: &str,
        font: &crate::font::FreeTypeFont,
        fill: (u8, u8, u8, u8),
        options: &crate::font::ImageFontTextOptions,
    ) -> Result<(), PilError> {
        self.text_with_options_inner(x, y, FontTextRef::String(text), font, fill, options)
            .map(|_| ())
    }

    /// Draw text after applying Pillow's host-neutral text input rules.
    pub fn text_with_options_input(
        &mut self,
        x: i32,
        y: i32,
        text: crate::font::ImageFontTextInput,
        font: &crate::font::FreeTypeFont,
        fill: (u8, u8, u8, u8),
        options: &crate::font::ImageFontTextOptions,
    ) -> Result<(), PilError> {
        let text = text.into_font_text()?;
        self.text_with_options_inner(x, y, text.as_ref(), font, fill, options)
            .map(|_| ())
    }

    /// Draw text using Pillow's optional-font and `font_size` rules.
    pub fn text_with_optional_font(
        &mut self,
        x: i32,
        y: i32,
        text: &str,
        font: Option<&crate::font::FreeTypeFont>,
        fill: (u8, u8, u8, u8),
        size: Option<f32>,
        options: &crate::font::ImageFontTextOptions,
    ) -> Result<(), PilError> {
        crate::font::with_text_font(font, size, |font| {
            self.text_with_options(x, y, text, font, fill, options)
        })
    }

    /// Draw text with optional-font rules and host-neutral text input.
    pub fn text_with_optional_font_input(
        &mut self,
        x: i32,
        y: i32,
        text: crate::font::ImageFontTextInput,
        font: Option<&crate::font::FreeTypeFont>,
        fill: (u8, u8, u8, u8),
        size: Option<f32>,
        options: &crate::font::ImageFontTextOptions,
    ) -> Result<(), PilError> {
        crate::font::with_text_font(font, size, |font| {
            self.text_with_options_input(x, y, text, font, fill, options)
        })
    }

    /// Draws multiline text using the same line stepping as Pillow's public
    /// `ImageDraw.multiline_text` path.
    // TODO(pillow-rs#brg0): rerun the exact CPU centered-anchor, explicit-font-
    // size, three-line-spacing, and CMYK multiline output cases after source
    // closure; retain frozen case IDs and output bytes.
    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
    pub fn multiline_text_with_options(
        &mut self,
        x: f64,
        y: f64,
        text: &str,
        font: &crate::font::FreeTypeFont,
        fill: (u8, u8, u8, u8),
        spacing: f64,
        options: &crate::font::ImageFontTextOptions,
    ) -> Result<(), PilError> {
        self.multiline_text_with_options_ref(
            x,
            y,
            FontTextRef::String(text),
            font,
            fill,
            spacing,
            options,
        )
    }

    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
    fn multiline_text_with_options_ref(
        &mut self,
        x: f64,
        y: f64,
        text: FontTextRef<'_>,
        font: &crate::font::FreeTypeFont,
        fill: (u8, u8, u8, u8),
        spacing: f64,
        options: &crate::font::ImageFontTextOptions,
    ) -> Result<(), PilError> {
        let lines = text.split('\n');
        let positioned =
            multiline_text_positions(x, y, &lines, font, spacing, options, self.effective_mode()?)?;
        self.validate_text_options(options)?;
        let (requested_backend, context) = self.cpu_execution_context()?;
        self.draw_positioned_text_lines(
            positioned,
            font,
            fill,
            options,
            requested_backend,
            &context,
        )
    }

    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
    fn multiline_text_with_options_ref_with_context(
        &mut self,
        x: f64,
        y: f64,
        text: FontTextRef<'_>,
        font: &crate::font::FreeTypeFont,
        fill: (u8, u8, u8, u8),
        spacing: f64,
        options: &crate::font::ImageFontTextOptions,
        requested_backend: Option<crate::compute::Backend>,
        context: &ExecutionContext,
    ) -> Result<(), PilError> {
        let lines = text.split('\n');
        let positioned =
            multiline_text_positions(x, y, &lines, font, spacing, options, self.effective_mode()?)?;
        self.validate_text_options(options)?;
        self.draw_positioned_text_lines(positioned, font, fill, options, requested_backend, context)
    }

    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
    fn draw_positioned_text_lines<'a>(
        &mut self,
        positioned: Vec<crate::font::text_layout::PositionedTextLine<'a>>,
        font: &crate::font::FreeTypeFont,
        fill: (u8, u8, u8, u8),
        options: &crate::font::ImageFontTextOptions,
        requested_backend: Option<crate::compute::Backend>,
        context: &ExecutionContext,
    ) -> Result<(), PilError> {
        for line in positioned {
            let mut line_options = options.clone();
            line_options.anchor = Some(line.anchor);
            self.draw_text_at(
                line.x,
                line.y,
                line.text,
                font,
                fill,
                &line_options,
                requested_backend,
                context,
            )?;
        }
        Ok(())
    }

    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
    fn draw_text_at(
        &mut self,
        x: f64,
        y: f64,
        text: FontTextRef<'_>,
        font: &crate::font::FreeTypeFont,
        fill: (u8, u8, u8, u8),
        options: &crate::font::ImageFontTextOptions,
        requested_backend: Option<crate::compute::Backend>,
        context: &ExecutionContext,
    ) -> Result<(), PilError> {
        let mut options = options.clone();
        options.start = Some((x.fract(), y.fract()));
        self.text_with_options_inner_with_context(
            x as i32,
            y as i32,
            text,
            font,
            fill,
            &options,
            requested_backend,
            context,
        )?;
        Ok(())
    }

    /// Draw multiline text using Pillow's optional-font and `font_size` rules.
    pub fn multiline_text_with_optional_font(
        &mut self,
        x: f64,
        y: f64,
        text: &str,
        font: Option<&crate::font::FreeTypeFont>,
        fill: (u8, u8, u8, u8),
        spacing: f64,
        size: Option<f32>,
        options: &crate::font::ImageFontTextOptions,
    ) -> Result<(), PilError> {
        crate::font::with_text_font(font, size, |font| {
            self.multiline_text_with_options(x, y, text, font, fill, spacing, options)
        })
    }

    /// Draw multiline text with optional-font rules and host-neutral text
    /// input. Byte text is interpreted as one Latin-1 code point per byte,
    /// preserving Pillow's public font contract before line splitting.
    pub fn multiline_text_with_optional_font_input(
        &mut self,
        x: f64,
        y: f64,
        text: crate::font::ImageFontTextInput,
        font: Option<&crate::font::FreeTypeFont>,
        fill: (u8, u8, u8, u8),
        spacing: f64,
        size: Option<f32>,
        options: &crate::font::ImageFontTextOptions,
    ) -> Result<(), PilError> {
        crate::font::with_text_font(font, size, |font| {
            let text = text.into_font_text()?;
            self.multiline_text_with_options_ref(x, y, text.as_ref(), font, fill, spacing, options)
        })
    }

    /// Draw multiline text with an execution route selected by its parent.
    #[doc(hidden)]
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[allow(clippy::too_many_arguments)]
    pub fn multiline_text_with_optional_font_input_with_context(
        &mut self,
        x: f64,
        y: f64,
        text: crate::font::ImageFontTextInput,
        font: Option<&crate::font::FreeTypeFont>,
        fill: (u8, u8, u8, u8),
        spacing: f64,
        size: Option<f32>,
        options: &crate::font::ImageFontTextOptions,
        requested_backend: Option<crate::compute::Backend>,
        context: &ExecutionContext,
    ) -> Result<(), PilError> {
        crate::font::with_text_font(font, size, |font| {
            let text = text.into_font_text()?;
            self.multiline_text_with_options_ref_with_context(
                x,
                y,
                text.as_ref(),
                font,
                fill,
                spacing,
                options,
                requested_backend,
                context,
            )
        })
    }

    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
    fn text_with_options_inner(
        &mut self,
        x: i32,
        y: i32,
        text: FontTextRef<'_>,
        font: &crate::font::FreeTypeFont,
        fill: (u8, u8, u8, u8),
        options: &crate::font::ImageFontTextOptions,
    ) -> Result<(u32, u32), PilError> {
        self.validate_text_options(options)?;
        let (requested_backend, context) = self.cpu_execution_context()?;
        self.text_with_options_inner_with_context(
            x,
            y,
            text,
            font,
            fill,
            options,
            requested_backend,
            &context,
        )
    }

    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
    fn text_with_options_inner_with_context(
        &mut self,
        x: i32,
        y: i32,
        text: FontTextRef<'_>,
        font: &crate::font::FreeTypeFont,
        fill: (u8, u8, u8, u8),
        options: &crate::font::ImageFontTextOptions,
        requested_backend: Option<crate::compute::Backend>,
        context: &ExecutionContext,
    ) -> Result<(u32, u32), PilError> {
        let telemetry =
            crate::compute::begin_eager_cpu_operation_telemetry("DrawText", requested_backend);
        let mut operation_receipts =
            crate::compute::PipelineOperationTelemetryCapture::begin_preserving_current();
        let result = self.text_with_options_inner_selected(
            x,
            y,
            text,
            font,
            fill,
            options,
            requested_backend,
            context,
        );
        let output_bytes = if result.is_ok() {
            self.image
                .materialized_shared()
                .map_or(0, |image| image.as_bytes().len())
        } else {
            0
        };
        operation_receipts.capture();
        operation_receipts.restore();
        match result {
            Ok((dimensions, selection)) => {
                if let Some(selection) = selection {
                    crate::compute::finish_eager_cpu_operation_telemetry_with_selection(
                        telemetry,
                        output_bytes,
                        "DrawText",
                        selection,
                    );
                } else {
                    crate::compute::finish_eager_cpu_operation_telemetry(telemetry, output_bytes);
                }
                Ok(dimensions)
            }
            Err(error) => {
                crate::compute::cancel_eager_cpu_operation_telemetry(telemetry);
                Err(error)
            }
        }
    }

    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
    fn text_with_options_inner_selected(
        &mut self,
        x: i32,
        y: i32,
        text: FontTextRef<'_>,
        font: &crate::font::FreeTypeFont,
        fill: (u8, u8, u8, u8),
        options: &crate::font::ImageFontTextOptions,
        requested_backend: Option<crate::compute::Backend>,
        context: &ExecutionContext,
    ) -> Result<((u32, u32), Option<StrategySelection>), PilError> {
        if options.stroke_width != 0.0
            && let Some(stroke_fill) = options.stroke_fill
            && stroke_fill != fill
        {
            let mut stroke_options = options.clone();
            stroke_options.ink = Some(Self::pack_text_ink(stroke_fill));
            let (_, _, stroke_selection) = self.text_render_and_compose(
                x,
                y,
                text,
                font,
                stroke_fill,
                &stroke_options,
                requested_backend,
                context,
            )?;

            let mut fill_options = options.clone();
            fill_options.stroke_width = 0.0;
            fill_options.stroke_filled = false;
            fill_options.stroke_fill = None;
            fill_options.ink = Some(Self::pack_text_ink(fill));
            let (width, height, fill_selection) = self.text_render_and_compose(
                x,
                y,
                text,
                font,
                fill,
                &fill_options,
                requested_backend,
                context,
            )?;
            return Ok(((width, height), fill_selection.or(stroke_selection)));
        }
        let (width, height, selection) = self.text_render_and_compose(
            x,
            y,
            text,
            font,
            fill,
            options,
            requested_backend,
            context,
        )?;
        Ok(((width, height), selection))
    }

    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
    fn text_render_and_compose(
        &mut self,
        x: i32,
        y: i32,
        text: FontTextRef<'_>,
        font: &crate::font::FreeTypeFont,
        fill: (u8, u8, u8, u8),
        options: &crate::font::ImageFontTextOptions,
        requested_backend: Option<crate::compute::Backend>,
        context: &ExecutionContext,
    ) -> Result<(u32, u32, Option<StrategySelection>), PilError> {
        let mode = self.effective_mode()?;
        // Read the current typed raster identity, including pending outputs.
        let raster_mode = self.image.mode_identity()?;
        self.validate_text_options(options)?;
        let binary = matches!(
            raster_mode,
            Mode::One | Mode::ImagePalette8 | Mode::ImageI32 | Mode::ImageF32
        );
        let mut options = options.clone();
        if binary && options.mode.is_none() {
            options.mode = Some(Mode::One);
        }
        if options.stroke_width != 0.0 {
            // Pillow's ImageDraw.text path requests the outside-border
            // variant of the FreeType stroke helper. Keep this policy in the
            // Rust draw core so Python/JS bindings only forward public text
            // arguments and cannot silently select the getmask2 default.
            options.stroke_filled = true;
        }
        let color_mask = options.uses_color_mask();
        if color_mask && options.ink.is_none() {
            // Pillow's RGBA font mask stores the draw fill in the mask's RGB
            // channels and reserves alpha for glyph coverage. Keep this
            // packing in the Rust core so Python only forwards the fill.
            options.ink = Some(Self::pack_text_ink(fill));
        }

        // The mask alpha carries glyph coverage; RGB carries the foreground or
        // embedded glyph color for the RGBA-mask strategy.
        let (w, h, mask, offset) = font.getmask2_text_with_options(text, &options)?;
        if w == 0 || h == 0 {
            return Ok((w, h, None));
        }
        let support = crate::compute::pool_cpu::ops::draw::ops::SUPPORT.for_mode(raster_mode);
        let palette = if matches!(raster_mode, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8)
            && self.image.has_nonempty_palette()
        {
            crate::compute::registry::support::PaletteKnowledge::Present
        } else {
            crate::compute::registry::support::PaletteKnowledge::Absent
        };
        let mask_kind = if color_mask {
            crate::compute::registry::support::DrawTextMaskKind::Rgba
        } else {
            crate::compute::registry::support::DrawTextMaskKind::Coverage
        };
        let selection = crate::compute::pool_cpu::ops::draw::ops::select_text_strategy(
            support,
            context,
            raster_mode,
            mode,
            options.embedded_color,
            mask_kind,
            options.ink,
            palette,
        )?;
        crate::compute::pool_cpu::ops::draw::ops::validate_direct_selection(
            selection,
            raster_mode,
        )?;
        let execution_mode = selection.implementation.mode;
        let draw_x = x.saturating_add(offset.0);
        let draw_y = y.saturating_add(offset.1);
        if Self::embedded_color_paste_route_selected(
            raster_mode,
            mode,
            options.embedded_color,
            options.stroke_width,
            (w, h),
        )? {
            // The explicit RGBA drawing context may differ from RGB storage;
            // it owns the semantic ink lane used by the composed Paste.
            let ink_alpha = Self::embedded_color_ink_alpha(mode, options.ink, fill);
            self.paste_embedded_color_mask(
                (draw_x, draw_y),
                (w, h),
                mask,
                ink_alpha,
                requested_backend,
                context,
            )?;
            return Ok((w, h, Some(selection)));
        }
        let output = crate::compute::pool_cpu::ops::draw::ops::apply_text(
            selection,
            crate::compute::pool_cpu::ops::draw::TextComposeInput {
                image: self.image.materialize()?,
                x: draw_x,
                y: draw_y,
                width: w,
                height: h,
                pixels: &mask,
                mask_kind,
                fill,
                palette,
            },
        )?;
        self.apply_text_output(execution_mode, palette, output)?;
        Ok((w, h, Some(selection)))
    }
}
