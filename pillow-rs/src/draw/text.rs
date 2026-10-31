use super::{Draw, Mode};
use crate::compute::ExecutionContext;
use crate::compute::registry::support::StrategySelection;
use crate::error::PilError;

mod composition;

impl Draw {
    /// Draws text at `(x, y)` using a loaded font.
    ///
    /// For RGB and RGBA modes, uses the standard RGBA compositing pipeline.
    /// For other modes (1, L, LA, CMYK, P, I, F), renders directly in the
    /// mode's native pixel format, matching PIL's `draw_bitmap` behavior:
    /// - Integer fill values go to the first channel only; other channels get 0.
    /// - Binary modes (1, P, I, F) use PIL's fontmode="1": binary glyphs (coverage >= 128 → 255).
    /// - Anti-aliased modes (L, LA, CMYK) use PIL's BLEND (truncation) per channel.
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
        let anchor = options.anchor.as_deref().unwrap_or("la");
        if anchor.chars().count() != 2 {
            return Err(PilError::ValueError(
                if options.anchor_invalid_length_error {
                    "anchor must be a 2 character string".into()
                } else {
                    format!("bad anchor specified: {anchor}")
                },
            ));
        }
        if matches!(anchor.as_bytes()[1], b't' | b'b')
            && options.direction.as_deref() != Some("ttb")
        {
            return Err(PilError::ValueError(
                "anchor not supported for multiline text".into(),
            ));
        }

        let mut line_bbox_options = options.clone();
        line_bbox_options.anchor = Some("la".into());
        let line_spacing = f64::from(font.getbbox_with_options("A", &line_bbox_options)?.3)
            + f64::from(options.stroke_width)
            + spacing;
        let mut line_y = match anchor.as_bytes()[1] {
            b'm' => y - (lines.len().saturating_sub(1) as f64 * line_spacing / 2.0),
            b'd' => y - (lines.len().saturating_sub(1) as f64 * line_spacing),
            _ => y,
        };
        let widths = lines
            .iter()
            .map(|line| font.getlength_with_options(line, options))
            .collect::<Result<Vec<_>, _>>()?;
        let max_width = widths.iter().copied().fold(0.0_f32, f32::max) as f64;
        // Pillow applies special positioning only to its known alignment names;
        // an unknown name follows the ordinary left-aligned path.
        let align = options.align.as_deref().unwrap_or("left");

        for (index, line) in lines.iter().enumerate() {
            let width_difference = max_width - f64::from(widths[index]);
            let mut line_x = match align {
                "center" => x + width_difference / 2.0,
                "right" => x + width_difference,
                _ => x,
            };
            if align == "justify" && width_difference != 0.0 && index + 1 != lines.len() {
                let words = line.split(' ').collect::<Vec<_>>();
                if words.len() > 1 {
                    if anchor.as_bytes()[0] == b'm' {
                        line_x -= max_width / 2.0;
                    } else if anchor.as_bytes()[0] == b'r' {
                        line_x -= max_width;
                    }
                    let word_widths = words
                        .iter()
                        .map(|word| font.getlength_with_options(word, options))
                        .collect::<Result<Vec<_>, _>>()?;
                    let extra = (max_width
                        - word_widths
                            .iter()
                            .map(|width| f64::from(*width))
                            .sum::<f64>())
                        / (words.len() - 1) as f64;
                    let mut word_options = options.clone();
                    word_options.anchor = Some(format!("l{}", anchor.as_bytes()[1] as char));
                    for (word, width) in words.iter().zip(word_widths) {
                        self.draw_text_at(line_x, line_y, word, font, fill, &word_options)?;
                        line_x += f64::from(width) + extra;
                    }
                    line_y += line_spacing;
                    continue;
                }
            }
            if anchor.as_bytes()[0] == b'm' {
                line_x -= width_difference / 2.0;
            } else if anchor.as_bytes()[0] == b'r' {
                line_x -= width_difference;
            }
            let mut line_options = options.clone();
            line_options.anchor = Some(anchor.to_owned());
            self.draw_text_at(line_x, line_y, line, font, fill, &line_options)?;
            line_y += line_spacing;
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
        let result =
            self.text_with_options_inner_selected(x, y, text, font, fill, options, &context);
        match result {
            Ok((dimensions, selection)) => {
                let output_bytes = self
                    .image
                    .materialized_shared()
                    .map_or(0, |image| image.as_bytes().len());
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
            let (_, stroke_selection) = self.text_render_and_compose(
                x,
                y,
                text,
                font,
                stroke_fill,
                options.stroke_fill_is_integer,
                &stroke_options,
                false,
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
                true,
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
            false,
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
        blend_rgba_overlay: bool,
        context: &ExecutionContext,
    ) -> Result<(u32, u32, Option<StrategySelection>), PilError> {
        let mode = self.effective_mode();
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

        // ImageFont rendering always uses alpha=255 so glyph coverage is preserved.
        let (w, h, mask, offset) = font.getmask2_with_options(text, &options)?;
        if w == 0 || h == 0 {
            return Ok((w, h, None));
        }
        let support = crate::compute::pool_cpu::ops::draw::ops::SUPPORT.for_mode(raster_mode);
        let palette = if raster_mode == Mode::ImagePalette8 && self.image.has_nonempty_palette() {
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
                pixels: std::borrow::Cow::Borrowed(&mask),
                mask_kind,
                fill,
                fill_is_integer,
                blend_rgba_overlay,
                palette,
            },
        )?;
        self.apply_text_output(execution_mode, palette, output)?;
        Ok((w, h, Some(selection)))
    }
}
