use super::{Draw, DrawColorInput, Mode};
use crate::compute::ExecutionContext;
use crate::compute::registry::support::StrategySelection;
use crate::error::PilError;

mod composition;

impl Draw {
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

    /// Paste a custom font's RGBA glyph colors through the existing Paste
    /// mode strategy, using the mask alpha as coverage.
    pub fn paste_text_color_mask(
        &mut self,
        position: (i32, i32),
        color: &crate::image::Image,
        coverage: &crate::image::Image,
    ) -> Result<(), PilError> {
        if color.mode_identity()? != Mode::ImageRgba8 {
            return Err(PilError::ValueError("bad transparency mask".into()));
        }
        if !matches!(coverage.mode_identity()?, Mode::One | Mode::ImageLuma8) {
            return Err(PilError::ValueError("bad transparency mask".into()));
        }
        self.image.paste_at(
            crate::ops::paste::PasteSource::Image(Box::new(color.clone())),
            Some(position),
            Some(coverage),
        )
    }

    fn pack_text_ink(fill: (u8, u8, u8, u8)) -> i64 {
        i64::from(i32::from_le_bytes([fill.0, fill.1, fill.2, fill.3]))
    }
}

struct PositionedTextLine<'a> {
    x: f64,
    y: f64,
    text: &'a str,
    anchor: String,
}

struct MultilineAnchor {
    value: String,
    horizontal: char,
    vertical: char,
    vertical_direction: bool,
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
            text,
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
    pub fn text_with_options(
        &mut self,
        x: i32,
        y: i32,
        text: &str,
        font: &crate::font::FreeTypeFont,
        fill: (u8, u8, u8, u8),
        options: &crate::font::ImageFontTextOptions,
    ) -> Result<(), PilError> {
        self.text_with_options_inner(x, y, text, font, fill, options)
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
        let text = text.into_text();
        self.text_with_options(x, y, &text, font, fill, options)
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
        let lines = text.split('\n').collect::<Vec<_>>();
        let positioned = multiline_text_positions(
            x,
            y,
            &lines,
            font,
            spacing,
            options,
            self.effective_mode()?,
        )?;
        for line in positioned {
            let mut line_options = options.clone();
            line_options.anchor = Some(line.anchor);
            self.draw_text_at(line.x, line.y, line.text, font, fill, &line_options)?;
        }
        Ok(())
    }

    fn draw_text_at(
        &mut self,
        x: f64,
        y: f64,
        text: &str,
        font: &crate::font::FreeTypeFont,
        fill: (u8, u8, u8, u8),
        options: &crate::font::ImageFontTextOptions,
    ) -> Result<(), PilError> {
        let mut options = options.clone();
        options.start = Some((x.fract(), y.fract()));
        self.text_with_options_inner(x as i32, y as i32, text, font, fill, &options)?;
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
            let text = text.into_text();
            self.multiline_text_with_options(x, y, &text, font, fill, spacing, options)
        })
    }

    fn text_with_options_inner(
        &mut self,
        x: i32,
        y: i32,
        text: &str,
        font: &crate::font::FreeTypeFont,
        fill: (u8, u8, u8, u8),
        options: &crate::font::ImageFontTextOptions,
    ) -> Result<(u32, u32), PilError> {
        self.validate_text_options(options)?;
        let (requested_backend, context) = self.cpu_execution_context()?;
        let telemetry =
            crate::compute::begin_eager_cpu_operation_telemetry("DrawText", requested_backend);
        let mut operation_receipts =
            crate::compute::PipelineOperationTelemetryCapture::begin_preserving_current();
        let result =
            self.text_with_options_inner_selected(x, y, text, font, fill, options, &context);
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

    fn text_with_options_inner_selected(
        &mut self,
        x: i32,
        y: i32,
        text: &str,
        font: &crate::font::FreeTypeFont,
        fill: (u8, u8, u8, u8),
        options: &crate::font::ImageFontTextOptions,
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
                options.stroke_fill_is_integer,
                &stroke_options,
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
                options.text_fill_is_integer,
                &fill_options,
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
            options.text_fill_is_integer,
            options,
            context,
        )?;
        Ok(((width, height), selection))
    }

    fn text_render_and_compose(
        &mut self,
        x: i32,
        y: i32,
        text: &str,
        font: &crate::font::FreeTypeFont,
        fill: (u8, u8, u8, u8),
        fill_is_integer: bool,
        options: &crate::font::ImageFontTextOptions,
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
            options.mode = Some("1".to_string());
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
        let (w, h, mask, offset) = font.getmask2_with_options(text, &options)?;
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
            fill_is_integer,
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

// TODO(pillow-rs#brg0): verify anchored, font-size, and spacing Text outputs in CPU parity.
fn multiline_text_positions<'a>(
    x: f64,
    y: f64,
    lines: &[&'a str],
    font: &crate::font::FreeTypeFont,
    spacing: f64,
    options: &crate::font::ImageFontTextOptions,
    mode: Mode,
) -> Result<Vec<PositionedTextLine<'a>>, PilError> {
    let anchor = resolve_multiline_anchor(lines.len(), options)?;
    if lines.len() == 1 {
        return Ok(vec![PositionedTextLine {
            x,
            y,
            text: lines[0],
            anchor: anchor.value,
        }]);
    }
    let font_mode = text_layout_font_mode(mode, options.embedded_color);
    let line_spacing = multiline_line_spacing(font, options, font_mode, spacing)?;
    if anchor.vertical_direction {
        return Ok(vertical_text_positions(x, y, lines, line_spacing, &anchor));
    }
    horizontal_text_positions(x, y, lines, font, options, font_mode, line_spacing, &anchor)
}

fn resolve_multiline_anchor(
    line_count: usize,
    options: &crate::font::ImageFontTextOptions,
) -> Result<MultilineAnchor, PilError> {
    let vertical_direction = options.direction.as_deref() == Some("ttb");
    let default_anchor = if vertical_direction { "lt" } else { "la" };
    let value = options.anchor.as_deref().unwrap_or(default_anchor);
    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() != 2 {
        return Err(PilError::ValueError(
            if options.anchor_invalid_length_error {
                "anchor must be a 2 character string".into()
            } else {
                format!("bad anchor specified: {value}")
            },
        ));
    }
    if line_count > 1 && matches!(chars[1], 't' | 'b') && !vertical_direction {
        return Err(PilError::ValueError(
            "anchor not supported for multiline text".into(),
        ));
    }
    Ok(MultilineAnchor {
        value: value.to_owned(),
        horizontal: chars[0],
        vertical: chars[1],
        vertical_direction,
    })
}

fn text_layout_font_mode(mode: Mode, embedded_color: bool) -> &'static str {
    if embedded_color {
        "RGBA"
    } else if matches!(mode, Mode::One | Mode::ImagePalette8 | Mode::ImageI32 | Mode::ImageF32) {
        "1"
    } else {
        "L"
    }
}

fn multiline_line_spacing(
    font: &crate::font::FreeTypeFont,
    options: &crate::font::ImageFontTextOptions,
    font_mode: &str,
    spacing: f64,
) -> Result<f64, PilError> {
    let mut bbox_options = options.clone();
    bbox_options.mode = Some(font_mode.to_owned());
    bbox_options.direction = None;
    bbox_options.anchor = None;
    Ok(f64::from(font.getbbox_with_options("A", &bbox_options)?.3)
        + f64::from(options.stroke_width)
        + spacing)
}

fn vertical_text_positions<'a>(
    x: f64,
    y: f64,
    lines: &[&'a str],
    line_spacing: f64,
    anchor: &MultilineAnchor,
) -> Vec<PositionedTextLine<'a>> {
    lines
        .iter()
        .enumerate()
        .map(|(index, &text)| PositionedTextLine {
            x: x + index as f64 * line_spacing,
            y,
            text,
            anchor: anchor.value.clone(),
        })
        .collect()
}

fn horizontal_text_positions<'a>(
    x: f64,
    y: f64,
    lines: &[&'a str],
    font: &crate::font::FreeTypeFont,
    options: &crate::font::ImageFontTextOptions,
    font_mode: &str,
    line_spacing: f64,
    anchor: &MultilineAnchor,
) -> Result<Vec<PositionedTextLine<'a>>, PilError> {
    let mut width_options = options.clone();
    width_options.mode = Some(font_mode.to_owned());
    width_options.stroke_width = 0.0;
    width_options.anchor = None;
    let widths = lines
        .iter()
        .map(|line| font.getlength_with_options(line, &width_options))
        .collect::<Result<Vec<_>, _>>()?;
    let max_width = widths.iter().copied().fold(0.0_f32, f32::max) as f64;
    let align = options.align.as_deref().unwrap_or("left");
    if !matches!(align, "left" | "center" | "right" | "justify") {
        return Err(PilError::ValueError(
            "align must be \"left\", \"center\", \"right\" or \"justify\"".into(),
        ));
    }

    let mut top = y;
    if anchor.vertical == 'm' {
        top -= (lines.len() - 1) as f64 * line_spacing / 2.0;
    } else if anchor.vertical == 'd' {
        top -= (lines.len() - 1) as f64 * line_spacing;
    }
    let mut positioned = Vec::new();
    for (index, &line) in lines.iter().enumerate() {
        let width_difference = max_width - f64::from(widths[index]);
        let mut left = match align {
            "center" => x + width_difference / 2.0,
            "right" => x + width_difference,
            _ => x,
        };
        if align == "justify" && width_difference != 0.0 && index + 1 != lines.len() {
            let words = line.split(' ').collect::<Vec<_>>();
            if words.len() > 1 {
                if anchor.horizontal == 'm' {
                    left -= max_width / 2.0;
                } else if anchor.horizontal == 'r' {
                    left -= max_width;
                }
                let word_widths = words
                    .iter()
                    .map(|word| font.getlength_with_options(word, &width_options))
                    .collect::<Result<Vec<_>, _>>()?;
                let extra = (max_width
                    - word_widths.iter().map(|width| f64::from(*width)).sum::<f64>())
                    / (words.len() - 1) as f64;
                let word_anchor = format!("l{}", anchor.vertical);
                for (word, width) in words.into_iter().zip(word_widths) {
                    positioned.push(PositionedTextLine {
                        x: left,
                        y: top,
                        text: word,
                        anchor: word_anchor.clone(),
                    });
                    left += f64::from(width) + extra;
                }
                top += line_spacing;
                continue;
            }
        }
        if anchor.horizontal == 'm' {
            left -= width_difference / 2.0;
        } else if anchor.horizontal == 'r' {
            left -= width_difference;
        }
        positioned.push(PositionedTextLine {
            x: left,
            y: top,
            text: line,
            anchor: anchor.value.clone(),
        });
        top += line_spacing;
    }
    Ok(positioned)
}
