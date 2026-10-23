use super::{Draw, blend_u8, pil_blend};
use crate::error::PilError;
use crate::image::Image;
use crate::raster::Rgba;

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
        let mut line_y = y;
        let can_reuse_default_line_height = options.uses_default_mask()
            && !matches!(self.effective_mode().as_str(), "1" | "P" | "I" | "F");
        for line in text.split('\n') {
            if line.is_empty() {
                line_y += spacing + 10.0;
                continue;
            }
            let (_, rendered_height) =
                self.text_with_options_inner(x as i32, line_y as i32, line, font, fill, options)?;
            let height = if can_reuse_default_line_height {
                // The default mask's measured height is the same bbox height
                // used for line stepping; reuse it instead of laying out each
                // glyph a second time through `font.text_bbox`.
                rendered_height
            } else {
                // Stroke, anchors, and other mask options can change rendered
                // bounds while Pillow's multiline line step still uses the
                // plain font bbox.
                font.text_bbox(line)?.1
            };
            line_y += height as f64 + spacing;
        }
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
            ),
            _ => self.text_compose_direct(draw_x, draw_y, w, h, &pixels, &mode, fill),
        }?;
        Ok((w, h))
    }

    fn pack_text_ink(fill: (u8, u8, u8, u8)) -> i64 {
        i64::from(fill.0) | (i64::from(fill.1) << 8) | (i64::from(fill.2) << 16)
    }

    /// Blend text coverage directly into an exact logical RGB raster.
    ///
    /// `DynamicImage::ImageRgb8` owns three bytes per pixel, whereas
    /// `ImageRgba8` can also represent CMYK or an explicit RGBA drawing
    /// context. Require both the caller's RGB mode gate and this concrete
    /// storage variant before writing native bytes. `mask` is one coverage
    /// byte per pixel for ordinary glyphs and RGBA source pixels for color
    /// glyphs.
    fn text_compose_rgb_native(
        &mut self,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        mask: &[u8],
        fill: (u8, u8, u8, u8),
        color_mask: bool,
    ) -> Result<bool, PilError> {
        let mut canvas = self.image.materialize()?;
        let crate::raster::DynamicImage::ImageRgb8(rgb) = &mut canvas else {
            return Ok(false);
        };
        let (img_w, img_h) = (rgb.width(), rgb.height());
        if img_w == 0 || img_h == 0 {
            return Ok(true);
        }
        let raw = rgb.as_mut();
        for py in 0..h {
            for px in 0..w {
                let pixel_index = (py * w + px) as usize;
                let source_offset = if color_mask {
                    pixel_index * 4
                } else {
                    pixel_index
                };
                let coverage = if color_mask {
                    mask[source_offset + 3]
                } else {
                    mask[source_offset]
                };
                if coverage == 0 {
                    continue;
                }

                // Text is clipped in signed space. Casting a negative anchor
                // before the bounds check would wrap and address unrelated
                // pixels.
                let dx = i64::from(x) + i64::from(px);
                let dy = i64::from(y) + i64::from(py);
                if dx < 0 || dy < 0 || dx >= i64::from(img_w) || dy >= i64::from(img_h) {
                    continue;
                }

                let destination_offset = (dy as usize * img_w as usize + dx as usize) * 3;
                let inverse = 255u16 - u16::from(coverage);
                let source_rgb = if color_mask {
                    (
                        mask[source_offset],
                        mask[source_offset + 1],
                        mask[source_offset + 2],
                    )
                } else {
                    (fill.0, fill.1, fill.2)
                };
                raw[destination_offset] =
                    blend_u8(source_rgb.0, raw[destination_offset], coverage, inverse);
                raw[destination_offset + 1] =
                    blend_u8(source_rgb.1, raw[destination_offset + 1], coverage, inverse);
                raw[destination_offset + 2] =
                    blend_u8(source_rgb.2, raw[destination_offset + 2], coverage, inverse);
            }
        }
        self.image = Image::from_dynamic(canvas, None);
        Ok(true)
    }

    /// RGBA compositing for text (used for RGB and RGBA modes).
    ///
    /// Pixels from the font renderer have the glyph coverage in the alpha channel
    /// and the fill color in the RGB channels. Pillow's mask paste path blends
    /// each stored channel by glyph coverage. RGB output keeps alpha fixed at
    /// 255; RGBA output blends the alpha channel too.
    fn text_compose_rgba(
        &mut self,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        pixels: &[u8],
        fill: (u8, u8, u8, u8),
        // Whether the requested RGBA context is compositing onto an RGB
        // destination rather than preserving an RGBA destination.
        rgba_blend_rgb: bool,
        color_mask: bool,
    ) -> Result<(), PilError> {
        let img = self.image.materialize()?;
        let mut canvas = img.to_rgba8();
        let (img_w, img_h) = (canvas.width(), canvas.height());
        if img_w == 0 || img_h == 0 {
            return Ok(());
        }
        let mode = self.effective_mode();
        // Every font mask constructor returns exactly `w * h * 4` bytes for
        // this drawing path (grayscale masks are expanded before composing,
        // and color masks are produced as RGBA). Index the validated raster
        // directly in the hot loop instead of carrying a defensive length
        // branch that cannot occur for a core-owned mask.
        for py in 0..h {
            for px in 0..w {
                let off = ((py * w + px) * 4) as usize;
                let sa = pixels[off + 3];
                if sa == 0 {
                    continue;
                }
                // Pillow's ImageDraw text compositor clips glyph pixels
                // outside the destination. Keep the coordinate signed until
                // bounds are checked; casting a negative anchor or stroke
                // offset to u32 previously wrapped and panicked.
                let dx_signed = i64::from(x) + i64::from(px);
                let dy_signed = i64::from(y) + i64::from(py);
                if dx_signed < 0
                    || dy_signed < 0
                    || dx_signed >= i64::from(img_w)
                    || dy_signed >= i64::from(img_h)
                {
                    continue;
                }
                let dx = dx_signed as u32;
                let dy = dy_signed as u32;
                let dp = canvas.get_pixel(dx, dy);
                // Pillow's `src/libImaging/Paste.c` RGB mask path uses
                // glyph coverage as the RGB blend mask. The alpha byte
                // of an RGBA ink is intentionally ignored on an RGB
                // destination; RGBA destinations blend it separately.
                let channel_alpha = sa;
                let inv = 255u16 - u16::from(channel_alpha);
                let alpha = if mode == "RGBA" && !rgba_blend_rgb {
                    blend_u8(fill.3, dp[3], sa, inv)
                } else {
                    255
                };
                let source_rgb = if color_mask {
                    (pixels[off], pixels[off + 1], pixels[off + 2])
                } else {
                    (fill.0, fill.1, fill.2)
                };
                let (r, g, b) = if mode == "RGBA" && !rgba_blend_rgb && !color_mask {
                    // Pillow's RGBA text writes the fill RGB directly and
                    // blends only the alpha channel with glyph coverage.
                    (fill.0, fill.1, fill.2)
                } else {
                    (
                        blend_u8(source_rgb.0, dp[0], channel_alpha, inv),
                        blend_u8(source_rgb.1, dp[1], channel_alpha, inv),
                        blend_u8(source_rgb.2, dp[2], channel_alpha, inv),
                    )
                };
                canvas.put_pixel(dx, dy, Rgba([r, g, b, alpha]));
            }
        }
        if matches!(mode.as_str(), "YCbCr" | "HSV") {
            // Three-band canvases store no alpha; rebuild as RGB tagged with
            // the source mode so the result keeps Pillow's 3-byte layout.
            self.image = Image::from_dynamic(
                crate::raster::DynamicImage::ImageRgb8(
                    crate::raster::DynamicImage::ImageRgba8(canvas).to_rgb8(),
                ),
                Some(mode),
            );
        } else {
            self.set_image(canvas);
        }
        Ok(())
    }

    /// Direct per-pixel text compositing for non-standard modes.
    ///
    /// Matches PIL's `fill_mask_1` (binary) and `fill_mask_L` (anti-aliased)
    /// behavior from Paste.c. Integer fill values go to the first channel;
    /// other channels are zeroed.
    fn text_compose_direct(
        &mut self,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        pixels: &[u8],
        mode: &str,
        fill: (u8, u8, u8, u8),
    ) -> Result<(), PilError> {
        let img = self.image.materialize()?;
        match mode {
            "1" => self.text_compose_mode_1(img, x, y, w, h, pixels, fill),
            "L" => self.text_compose_mode_l(img, x, y, w, h, pixels, fill),
            "LA" => self.text_compose_mode_la(img, x, y, w, h, pixels, fill),
            "CMYK" => self.text_compose_mode_cmyk(img, x, y, w, h, pixels, fill),
            "P" => self.text_compose_mode_p(img, x, y, w, h, pixels, fill),
            "I" | "F" => self.text_compose_mode_numeric(img, x, y, w, h, pixels, mode, fill),
            _ => self.text_compose_rgba(x, y, w, h, pixels, fill, false, false),
        }
    }

    fn text_compose_mode_1(
        &mut self,
        img: crate::raster::DynamicImage,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        pixels: &[u8],
        fill: (u8, u8, u8, u8),
    ) -> Result<(), PilError> {
        let mut luma = img.to_luma8();
        let (img_w, img_h) = luma.dimensions();
        let ink = if fill.0 > 0 { 255u8 } else { 0u8 };
        for py in 0..h {
            for px in 0..w {
                let off = ((py * w + px) * 4) as usize;
                if pixels[off + 3] > 0 {
                    let dx = x as i64 + px as i64;
                    let dy = y as i64 + py as i64;
                    if dx < 0 || dy < 0 || dx >= i64::from(img_w) || dy >= i64::from(img_h) {
                        continue;
                    }
                    luma.put_pixel(dx as u32, dy as u32, crate::raster::Luma([ink]));
                }
            }
        }
        self.image = Image::from_dynamic(
            crate::raster::DynamicImage::ImageLuma8(luma),
            Some("1".to_string()),
        );
        Ok(())
    }

    fn text_compose_mode_l(
        &mut self,
        img: crate::raster::DynamicImage,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        pixels: &[u8],
        fill: (u8, u8, u8, u8),
    ) -> Result<(), PilError> {
        let mut luma = img.to_luma8();
        let (img_w, img_h) = luma.dimensions();
        let ink = fill.0;
        for py in 0..h {
            for px in 0..w {
                let off = ((py * w + px) * 4) as usize;
                let coverage = pixels[off + 3];
                if coverage == 0 {
                    continue;
                }
                let dx = x as i64 + px as i64;
                let dy = y as i64 + py as i64;
                if dx < 0 || dy < 0 || dx >= i64::from(img_w) || dy >= i64::from(img_h) {
                    continue;
                }
                let (dx, dy) = (dx as u32, dy as u32);
                let background = luma.get_pixel(dx, dy)[0];
                luma.put_pixel(
                    dx,
                    dy,
                    crate::raster::Luma([pil_blend(ink, background, coverage)]),
                );
            }
        }
        self.image = Image::from_dynamic(crate::raster::DynamicImage::ImageLuma8(luma), None);
        Ok(())
    }

    fn text_compose_mode_la(
        &mut self,
        img: crate::raster::DynamicImage,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        pixels: &[u8],
        fill: (u8, u8, u8, u8),
    ) -> Result<(), PilError> {
        let mut la = img.to_luma_alpha8();
        let (img_w, img_h) = la.dimensions();
        for py in 0..h {
            for px in 0..w {
                let off = ((py * w + px) * 4) as usize;
                let coverage = pixels[off + 3];
                if coverage == 0 {
                    continue;
                }
                let dx = x as i64 + px as i64;
                let dy = y as i64 + py as i64;
                if dx < 0 || dy < 0 || dx >= i64::from(img_w) || dy >= i64::from(img_h) {
                    continue;
                }
                let (dx, dy) = (dx as u32, dy as u32);
                let background = la.get_pixel(dx, dy);
                let luma_coverage = if background[1] == 0 { 255 } else { coverage };
                let new_luma = pil_blend(fill.0, background[0], luma_coverage);
                let new_alpha = pil_blend(fill.3, background[1], coverage);
                la.put_pixel(dx, dy, crate::raster::LumaA([new_luma, new_alpha]));
            }
        }
        self.image = Image::from_dynamic(
            crate::raster::DynamicImage::ImageLumaA8(la),
            Some("LA".to_string()),
        );
        Ok(())
    }

    fn text_compose_mode_cmyk(
        &mut self,
        img: crate::raster::DynamicImage,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        pixels: &[u8],
        fill: (u8, u8, u8, u8),
    ) -> Result<(), PilError> {
        let is_int_fill = fill.0 == fill.1 && fill.0 == fill.2 && fill.3 == 255;
        let ink = if is_int_fill {
            [fill.0, 0u8, 0u8, 0u8]
        } else {
            [fill.0, fill.1, fill.2, fill.3]
        };
        let mut rgba = img.to_rgba8();
        let (img_w, img_h) = rgba.dimensions();
        for py in 0..h {
            for px in 0..w {
                let off = ((py * w + px) * 4) as usize;
                let coverage = pixels[off + 3];
                if coverage == 0 {
                    continue;
                }
                let dx = x as i64 + px as i64;
                let dy = y as i64 + py as i64;
                if dx < 0 || dy < 0 || dx >= i64::from(img_w) || dy >= i64::from(img_h) {
                    continue;
                }
                let (dx, dy) = (dx as u32, dy as u32);
                let background = rgba.get_pixel(dx, dy);
                let pixel = if coverage == 255 {
                    Rgba(ink)
                } else {
                    Rgba([
                        pil_blend(ink[0], background[0], coverage),
                        pil_blend(ink[1], background[1], coverage),
                        pil_blend(ink[2], background[2], coverage),
                        pil_blend(ink[3], background[3], coverage),
                    ])
                };
                rgba.put_pixel(dx, dy, pixel);
            }
        }
        self.image = Image::from_dynamic(
            crate::raster::DynamicImage::ImageRgba8(rgba),
            Some("CMYK".to_string()),
        );
        Ok(())
    }

    fn text_compose_mode_p(
        &mut self,
        img: crate::raster::DynamicImage,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        pixels: &[u8],
        fill: (u8, u8, u8, u8),
    ) -> Result<(), PilError> {
        if let Some(palette) = self.image.palette().filter(|palette| !palette.is_empty()) {
            self.text_compose_paletted(img, x, y, w, h, pixels, fill.0, palette)
        } else {
            self.text_compose_unpaletted(img, x, y, w, h, pixels, fill.0)
        }
    }

    fn text_compose_paletted(
        &mut self,
        img: crate::raster::DynamicImage,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        pixels: &[u8],
        ink: u8,
        palette: Vec<u8>,
    ) -> Result<(), PilError> {
        let palette_alpha = self.image.palette_alpha().unwrap_or_default();
        let source_format = self.image.source_format();
        let info = self.image.image_info();
        let img_loaded = img.to_luma8();
        let (img_w, img_h) = img_loaded.dimensions();
        let mut indices = crate::raster::GrayImage::new(img_w, img_h);
        for (out_pixel, in_pixel) in indices.pixels_mut().zip(img_loaded.pixels()) {
            out_pixel[0] = in_pixel[0];
        }
        for py in 0..h.min(img_h) {
            for px in 0..w.min(img_w) {
                let off = ((py * w + px) * 4) as usize;
                if pixels[off + 3] == 0 {
                    continue;
                }
                let dx = x as i64 + px as i64;
                let dy = y as i64 + py as i64;
                if dx < 0 || dy < 0 || dx >= i64::from(img_w) || dy >= i64::from(img_h) {
                    continue;
                }
                indices.put_pixel(dx as u32, dy as u32, crate::raster::Luma([ink]));
            }
        }
        self.image = Image::Paletted(crate::image::PalettedData {
            indices,
            palette,
            palette_alpha,
            source_format,
            info,
            exif: self.image.exif_metadata(),
            materialized: crate::image::materialization_cache(),
        });
        Ok(())
    }

    fn text_compose_unpaletted(
        &mut self,
        img: crate::raster::DynamicImage,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        pixels: &[u8],
        ink: u8,
    ) -> Result<(), PilError> {
        let (img_w, img_h) = (img.width(), img.height());
        let mut luma = img.to_luma8();
        for py in 0..h {
            for px in 0..w {
                let off = ((py * w + px) * 4) as usize;
                if pixels[off + 3] == 0 {
                    continue;
                }
                let dx = x as i64 + px as i64;
                let dy = y as i64 + py as i64;
                if dx < 0 || dy < 0 || dx >= i64::from(img_w) || dy >= i64::from(img_h) {
                    continue;
                }
                luma.put_pixel(dx as u32, dy as u32, crate::raster::Luma([ink]));
            }
        }
        self.image = Image::from_dynamic(
            crate::raster::DynamicImage::ImageLuma8(luma),
            Some("P".to_string()),
        );
        Ok(())
    }

    fn text_compose_mode_numeric(
        &mut self,
        img: crate::raster::DynamicImage,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        pixels: &[u8],
        mode: &str,
        fill: (u8, u8, u8, u8),
    ) -> Result<(), PilError> {
        let mut rgba = img.to_rgba8();
        let (img_w, img_h) = rgba.dimensions();
        let ink = [fill.0, fill.1, fill.2, fill.3];
        for py in 0..h {
            for px in 0..w {
                let off = ((py * w + px) * 4) as usize;
                if pixels[off + 3] == 0 {
                    continue;
                }
                let dx = x as i64 + px as i64;
                let dy = y as i64 + py as i64;
                if dx < 0 || dy < 0 || dx >= i64::from(img_w) || dy >= i64::from(img_h) {
                    continue;
                }
                rgba.put_pixel(dx as u32, dy as u32, Rgba(ink));
            }
        }
        self.image = Image::from_dynamic(
            crate::raster::DynamicImage::ImageRgba8(rgba),
            Some(mode.to_string()),
        );
        Ok(())
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
