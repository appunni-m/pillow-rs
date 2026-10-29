use super::{Draw, Image, PilError, Rgba, blend_u8, pil_blend};
impl Draw {
    pub(super) fn pack_text_ink(fill: (u8, u8, u8, u8)) -> i64 {
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
    pub(super) fn text_compose_rgb_native(
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
    pub(super) fn text_compose_rgba(
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
        blend_rgba_overlay: bool,
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
                let (r, g, b) =
                    if mode == "RGBA" && !rgba_blend_rgb && !color_mask && !blend_rgba_overlay {
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
                crate::image::mode_boundary::parse_pillow_mode(&mode),
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
    pub(super) fn text_compose_direct(
        &mut self,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        pixels: &[u8],
        mode: &str,
        fill: (u8, u8, u8, u8),
        fill_is_integer: bool,
    ) -> Result<(), PilError> {
        let img = self.image.materialize()?;
        match mode {
            "1" => self.text_compose_mode_1(img, x, y, w, h, pixels, fill),
            "L" => self.text_compose_mode_l(img, x, y, w, h, pixels, fill),
            "LA" => self.text_compose_mode_la(img, x, y, w, h, pixels, fill),
            "CMYK" => self.text_compose_mode_cmyk(img, x, y, w, h, pixels, fill, fill_is_integer),
            "P" => self.text_compose_mode_p(img, x, y, w, h, pixels, fill),
            "I" | "F" => self.text_compose_mode_numeric(img, x, y, w, h, pixels, mode, fill),
            _ => self.text_compose_rgba(x, y, w, h, pixels, fill, false, false, false),
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
            Some(crate::raster::Mode::One),
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
            Some(crate::raster::Mode::ImageLumaA8),
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
        fill_is_integer: bool,
    ) -> Result<(), PilError> {
        let ink = if fill_is_integer {
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
            Some(crate::raster::Mode::ImageCmyk8),
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
            indices: crate::raster::DynamicImage::ImagePalette8(indices),
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
            Some(crate::raster::Mode::ImagePalette8),
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
            crate::image::mode_boundary::parse_pillow_mode(&mode),
        );
        Ok(())
    }
}
