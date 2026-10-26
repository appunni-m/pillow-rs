use super::{Draw, blend_u8, pil_blend};
use crate::error::PilError;
use crate::image::Image;
use crate::raster::Rgba;

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
        if options.stroke_width != 0.0
            && let Some(stroke_fill) = options.stroke_fill
            && stroke_fill != fill
        {
            let mut stroke_options = options.clone();
            stroke_options.ink = Some(Self::pack_text_ink(stroke_fill));
            self.text_render_and_compose(
                x,
                y,
                text,
                font,
                stroke_fill,
                options.stroke_fill_is_integer,
                &stroke_options,
                false,
            )?;

            let mut fill_options = options.clone();
            fill_options.stroke_width = 0.0;
            fill_options.stroke_filled = false;
            fill_options.stroke_fill = None;
            fill_options.ink = Some(Self::pack_text_ink(fill));
            return self.text_render_and_compose(
                x,
                y,
                text,
                font,
                fill,
                options.text_fill_is_integer,
                &fill_options,
                true,
            );
        }
        self.text_render_and_compose(
            x,
            y,
            text,
            font,
            fill,
            options.text_fill_is_integer,
            options,
            false,
        )
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
    ) -> Result<(u32, u32), PilError> {
        let mode = self.effective_mode();
        self.validate_text_options(options)?;
        let binary = matches!(mode.as_str(), "1" | "P" | "I" | "F");
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
        // Mode-specific alpha handling (e.g., LA alpha=0 for int fills) is done
        // in text_compose_direct / text_compose_rgba.
        let render_fill = (fill.0, fill.1, fill.2, 255u8);
        let (w, h, mask, offset) = font.getmask2_with_options(text, &options)?;
        if w == 0 || h == 0 {
            return Ok((w, h));
        }
        let draw_x = x.saturating_add(offset.0);
        let draw_y = y.saturating_add(offset.1);

        // Keep RGB text on its native three-byte canvas, including an explicit
        // RGBA draw context over an RGB image. Pillow uses glyph coverage to
        // blend RGB in both cases and ignores the fill alpha on an RGB target;
        // expanding the full image and grayscale mask to RGBA only to narrow
        // them again adds work without changing those semantics.
        let rgba_blend_rgb = self.alpha_blend_rgb();
        let rgb_target = (mode == "RGB" && !rgba_blend_rgb) || (mode == "RGBA" && rgba_blend_rgb);
        if rgb_target
            && self.text_compose_rgb_native(draw_x, draw_y, w, h, &mask, fill, color_mask)?
        {
            return Ok((w, h));
        }

        let pixels = if color_mask {
            mask
        } else {
            text_mask_to_rgba(mask, render_fill)
        };

        match mode.as_str() {
            "RGB" | "RGBA" => self.text_compose_rgba(
                draw_x,
                draw_y,
                w,
                h,
                &pixels,
                fill,
                self.alpha_blend_rgb(),
                color_mask,
                blend_rgba_overlay,
            ),
            _ => self.text_compose_direct(
                draw_x,
                draw_y,
                w,
                h,
                &pixels,
                &mode,
                fill,
                fill_is_integer,
            ),
        }?;
        Ok((w, h))
    }
}

fn text_mask_to_rgba(mask: Vec<u8>, fill: (u8, u8, u8, u8)) -> Vec<u8> {
    let mut pixels = vec![0u8; mask.len() * 4];
    for (index, coverage) in mask.into_iter().enumerate() {
        if coverage == 0 {
            continue;
        }
        let offset = index * 4;
        pixels[offset] = fill.0;
        pixels[offset + 1] = fill.1;
        pixels[offset + 2] = fill.2;
        pixels[offset + 3] = coverage;
    }
    pixels
}
