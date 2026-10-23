use super::{Draw, pil_blend};
use crate::error::PilError;
use crate::image::Image;
use crate::raster::DynamicImage;

struct BitmapMask<'a> {
    mode: &'a str,
    width: u32,
    data: &'a [u8],
}

impl BitmapMask<'_> {
    #[inline]
    fn coverage(&self, x: u32, y: u32) -> u8 {
        let index = (y * self.width + x) as usize;
        match self.mode {
            "1" => u8::from(self.data[index] > 0) * 255,
            "L" => self.data[index],
            "RGBA" => self.data[index * 4 + 3],
            "RGBa" => 255,
            _ => unreachable!("bitmap mode was validated before mask iteration"),
        }
    }
}

impl Draw {
    /// Draws a bitmap mask at `(x, y)` using `fill`.
    ///
    /// Accepted mask modes are `1`, `L`, `RGBA`, and `RGBa`. Coverage and
    /// destination writes retain their mode-specific native raster paths.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when the bitmap mode is not a valid
    /// mask mode, and propagates mode, size, or pixel-data errors.
    pub fn bitmap(
        &mut self,
        x: i32,
        y: i32,
        bitmap: &Image,
        fill: Option<(u8, u8, u8, u8)>,
    ) -> Result<(), PilError> {
        let color = fill.unwrap_or((255, 255, 255, 255));
        let bmp_mode = bitmap.mode()?;
        if !matches!(bmp_mode.as_str(), "1" | "L" | "RGBA" | "RGBa") {
            return Err(PilError::ValueError("bad transparency mask".to_string()));
        }
        let (bmp_w, bmp_h) = bitmap.size()?;
        let mode = self.effective_mode();
        if self.try_native_rgb_bitmap(x, y, bitmap, &bmp_mode, bmp_w, bmp_h, color, &mode)? {
            return Ok(());
        }
        if self.try_native_cmyk_l_bitmap(x, y, bitmap, &bmp_mode, bmp_w, bmp_h, color, &mode)? {
            return Ok(());
        }

        let raw_data = bitmap.getdata(None)?;
        let mask = BitmapMask {
            mode: &bmp_mode,
            width: bmp_w,
            data: &raw_data,
        };
        match mode.as_str() {
            "RGB" | "RGBA" => self.bitmap_mode_rgba(x, y, bmp_w, bmp_h, &mask, color),
            "1" => self.bitmap_mode_1(x, y, bmp_w, bmp_h, &mask, color),
            "L" => self.bitmap_mode_l(x, y, bmp_w, bmp_h, &mask, color),
            "LA" => self.bitmap_mode_la(x, y, bmp_w, bmp_h, &mask, color),
            "CMYK" => self.bitmap_mode_cmyk(x, y, bmp_w, bmp_h, &mask, color),
            "P" => self.bitmap_mode_p(x, y, bmp_w, bmp_h, &mask, color),
            "I" | "F" => self.bitmap_mode_numeric(x, y, bmp_w, bmp_h, &mask, &mode, color),
            _ => self.bitmap_mode_fallback(x, y, bmp_w, bmp_h, &mask, &mode, color),
        }
    }
    fn try_native_rgb_bitmap(
        &mut self,
        x: i32,
        y: i32,
        bitmap: &Image,
        bmp_mode: &str,
        bmp_w: u32,
        bmp_h: u32,
        color: (u8, u8, u8, u8),
        mode: &str,
    ) -> Result<bool, PilError> {
        // Both an ordinary RGB context and an explicit RGBA context on an RGB
        // image ultimately write a three-byte RGB destination. The shared
        // path below widens that canvas, computes alpha, then discards it when
        // restoring the original RGB image. With RGB input alpha fixed at 255,
        // its visible channels are exactly the native coverage blend below.
        let rgb_target = (mode == "RGB" && self.orig_mode.as_deref() == Some("RGB"))
            || (mode == "RGBA" && self.alpha_blend_rgb());
        if rgb_target {
            let img = self.image.materialized_shared()?;
            let mask = bitmap.materialized_shared()?;
            let mask_layout = match (bmp_mode, mask.as_ref()) {
                ("1", DynamicImage::ImageLuma8(image)) => {
                    Some((image.as_raw().as_slice(), 1, 0, true, false))
                }
                ("L", DynamicImage::ImageLuma8(image)) => {
                    Some((image.as_raw().as_slice(), 1, 0, false, false))
                }
                ("RGBA", DynamicImage::ImageRgba8(image)) => {
                    Some((image.as_raw().as_slice(), 4, 3, false, false))
                }
                // Pillow passes RGBa as a fully opaque bitmap mask and ignores
                // its stored alpha byte in ImagingDrawBitmap.
                ("RGBa", DynamicImage::ImageRgba8(_)) => Some((&[][..], 0, 0, false, true)),
                _ => None,
            };
            if let (
                DynamicImage::ImageRgb8(source),
                Some((mask_bytes, mask_stride, mask_channel, binary_mask, opaque_mask)),
            ) = (img.as_ref(), mask_layout)
            {
                let (img_w, img_h) = source.dimensions();
                let mut canvas = source.clone();
                let row_stride = img_w as usize * 3;
                let destination = canvas.as_mut();
                let source_x_start = (-i64::from(x)).clamp(0, i64::from(bmp_w)) as u32;
                let source_x_end =
                    (i64::from(img_w) - i64::from(x)).clamp(0, i64::from(bmp_w)) as u32;
                let source_y_start = (-i64::from(y)).clamp(0, i64::from(bmp_h)) as u32;
                let source_y_end =
                    (i64::from(img_h) - i64::from(y)).clamp(0, i64::from(bmp_h)) as u32;

                for py in source_y_start..source_y_end {
                    let dy = i64::from(y) + i64::from(py);
                    let destination_row = dy as usize * row_stride;

                    for px in source_x_start..source_x_end {
                        let mask_index = (py as usize * bmp_w as usize + px as usize) * mask_stride
                            + mask_channel;
                        let m = if opaque_mask {
                            255
                        } else {
                            let value = mask_bytes[mask_index];
                            if binary_mask && value != 0 {
                                255
                            } else {
                                value
                            }
                        };
                        if m == 0 {
                            continue;
                        }
                        let dx = i64::from(x) + i64::from(px);
                        let destination_offset = destination_row + dx as usize * 3;
                        if m == 255 {
                            destination[destination_offset] = color.0;
                            destination[destination_offset + 1] = color.1;
                            destination[destination_offset + 2] = color.2;
                        } else {
                            destination[destination_offset] =
                                pil_blend(destination[destination_offset], color.0, m);
                            destination[destination_offset + 1] =
                                pil_blend(destination[destination_offset + 1], color.1, m);
                            destination[destination_offset + 2] =
                                pil_blend(destination[destination_offset + 2], color.2, m);
                        }
                    }
                }

                self.image = Image::from_dynamic(DynamicImage::ImageRgb8(canvas), None);
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn try_native_cmyk_l_bitmap(
        &mut self,
        x: i32,
        y: i32,
        bitmap: &Image,
        bmp_mode: &str,
        bmp_w: u32,
        bmp_h: u32,
        color: (u8, u8, u8, u8),
        mode: &str,
    ) -> Result<bool, PilError> {
        // CMYK pixels use the same four-byte carrier as RGBA in this image
        // model. Keep those bytes in place while drawing: materializing an
        // owned image and then calling `to_rgba8()` makes two full-frame
        // copies even though this path performs no color conversion.
        if mode == "CMYK" && self.orig_mode.as_deref() == Some("CMYK") && bmp_mode == "L" {
            let img = self.image.materialized_shared()?;
            let mask = bitmap.materialized_shared()?;

            if let (DynamicImage::ImageRgba8(source), DynamicImage::ImageLuma8(mask)) =
                (img.as_ref(), mask.as_ref())
            {
                let (img_w, img_h) = source.dimensions();
                let ink = [color.0, color.1, color.2, color.3];
                let mut canvas = source.clone();
                let row_stride = img_w as usize * 4;
                let destination = canvas.as_mut();
                let mask_bytes = mask.as_raw();
                let source_x_start = (-i64::from(x)).clamp(0, i64::from(bmp_w)) as u32;
                let source_x_end =
                    (i64::from(img_w) - i64::from(x)).clamp(0, i64::from(bmp_w)) as u32;
                let source_y_start = (-i64::from(y)).clamp(0, i64::from(bmp_h)) as u32;
                let source_y_end =
                    (i64::from(img_h) - i64::from(y)).clamp(0, i64::from(bmp_h)) as u32;
                let clipped_width = (source_x_end - source_x_start) as usize;
                let destination_x = (i64::from(x) + i64::from(source_x_start)) as usize;
                let destination_row_len = clipped_width * 4;

                for py in source_y_start..source_y_end {
                    let destination_row_offset =
                        (i64::from(y) + i64::from(py)) as usize * row_stride;
                    let destination_start = destination_row_offset + destination_x * 4;
                    let destination_row = &mut destination
                        [destination_start..destination_start + destination_row_len];
                    let mask_row_offset = py as usize * bmp_w as usize;
                    let mask_start = mask_row_offset + source_x_start as usize;
                    let mask_end = mask_row_offset + source_x_end as usize;
                    let mask_row = &mask_bytes[mask_start..mask_end];
                    for (pixel, coverage) in destination_row
                        .chunks_exact_mut(4)
                        .zip(mask_row.iter().copied())
                    {
                        if coverage == 0 {
                            continue;
                        }
                        if coverage == 255 {
                            pixel.copy_from_slice(&ink);
                        } else {
                            for channel in 0..4 {
                                pixel[channel] = pil_blend(pixel[channel], ink[channel], coverage);
                            }
                        }
                    }
                }

                self.image =
                    Image::from_dynamic(DynamicImage::ImageRgba8(canvas), Some("CMYK".to_string()));
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn bitmap_mode_rgba(
        &mut self,
        x: i32,
        y: i32,
        bmp_w: u32,
        bmp_h: u32,
        mask: &BitmapMask<'_>,
        color: (u8, u8, u8, u8),
    ) -> Result<(), PilError> {
        let img = self.image.materialize()?;
        let (img_w, img_h) = (img.width(), img.height());
        let mut canvas = img.to_rgba8();

        for py in 0..bmp_h {
            for px in 0..bmp_w {
                let m = mask.coverage(px, py);
                if m == 0 {
                    continue;
                }
                let dx = x + px as i32;
                let dy = y + py as i32;
                if dx >= 0 && dy >= 0 && (dx as u32) < img_w && (dy as u32) < img_h {
                    let existing = canvas.get_pixel(dx as u32, dy as u32);
                    // Pillow's RGBA bitmap path uses the tuple-ink
                    // paste rule: RGB channels are replaced by the
                    // ink when the destination is fully transparent,
                    // while partially opaque destinations use the
                    // ordinary channel blend.  Alpha is always
                    // blended by the mask.  This mirrors
                    // src/libImaging/ImagingPaste.c's RGBA/L path.
                    let blend_rgb = |background: u8, foreground: u8| {
                        if existing[3] == 0 {
                            foreground
                        } else {
                            pil_blend(background, foreground, m)
                        }
                    };
                    let r = blend_rgb(existing[0], color.0);
                    let g = blend_rgb(existing[1], color.1);
                    let b = blend_rgb(existing[2], color.2);
                    let a = pil_blend(existing[3], color.3, m);
                    canvas.put_pixel(dx as u32, dy as u32, crate::raster::Rgba([r, g, b, a]));
                }
            }
        }

        self.set_image(canvas);
        Ok(())
    }

    fn bitmap_mode_1(
        &mut self,
        x: i32,
        y: i32,
        bmp_w: u32,
        bmp_h: u32,
        mask: &BitmapMask<'_>,
        color: (u8, u8, u8, u8),
    ) -> Result<(), PilError> {
        let img = self.image.materialize()?;
        let (img_w, img_h) = (img.width(), img.height());
        let mut luma = img.to_luma8();
        let ink = color.0;
        for py in 0..bmp_h {
            for px in 0..bmp_w {
                let m = mask.coverage(px, py);
                if m == 0 {
                    continue;
                }
                let dx = x + px as i32;
                let dy = y + py as i32;
                if dx >= 0 && dy >= 0 && (dx as u32) < img_w && (dy as u32) < img_h {
                    let v = if m == 255 {
                        ink
                    } else {
                        pil_blend(luma.get_pixel(dx as u32, dy as u32)[0], ink, m)
                    };
                    luma.put_pixel(dx as u32, dy as u32, crate::raster::Luma([v]));
                }
            }
        }
        self.image = Image::from_dynamic(
            crate::raster::DynamicImage::ImageLuma8(luma),
            Some("1".to_string()),
        );
        Ok(())
    }

    fn bitmap_mode_l(
        &mut self,
        x: i32,
        y: i32,
        bmp_w: u32,
        bmp_h: u32,
        mask: &BitmapMask<'_>,
        color: (u8, u8, u8, u8),
    ) -> Result<(), PilError> {
        let img = self.image.materialize()?;
        let (img_w, img_h) = (img.width(), img.height());
        let mut luma = img.to_luma8();
        let ink = color.0;
        for py in 0..bmp_h {
            for px in 0..bmp_w {
                let m = mask.coverage(px, py);
                if m == 0 {
                    continue;
                }
                let dx = x + px as i32;
                let dy = y + py as i32;
                if dx >= 0 && dy >= 0 && (dx as u32) < img_w && (dy as u32) < img_h {
                    let v = if m == 255 {
                        ink
                    } else {
                        pil_blend(luma.get_pixel(dx as u32, dy as u32)[0], ink, m)
                    };
                    luma.put_pixel(dx as u32, dy as u32, crate::raster::Luma([v]));
                }
            }
        }
        self.image = Image::from_dynamic(crate::raster::DynamicImage::ImageLuma8(luma), None);
        Ok(())
    }

    fn bitmap_mode_la(
        &mut self,
        x: i32,
        y: i32,
        bmp_w: u32,
        bmp_h: u32,
        mask: &BitmapMask<'_>,
        color: (u8, u8, u8, u8),
    ) -> Result<(), PilError> {
        let img = self.image.materialize()?;
        let (img_w, img_h) = (img.width(), img.height());
        let mut la = img.to_luma_alpha8();
        let ink_l = color.0;
        let ink_a = color.3;
        for py in 0..bmp_h {
            for px in 0..bmp_w {
                let m = mask.coverage(px, py);
                if m == 0 {
                    continue;
                }
                let dx = x + px as i32;
                let dy = y + py as i32;
                if dx >= 0 && dy >= 0 && (dx as u32) < img_w && (dy as u32) < img_h {
                    let existing = la.get_pixel(dx as u32, dy as u32);
                    // Pillow's `fill_mask_L` adjusts the L-channel
                    // coverage against the destination alpha: with a
                    // fully transparent destination the L ink is
                    // written directly (`src/libImaging/Paste.c`).
                    let l_mask = if existing[1] == 0 { 255 } else { m };
                    let l = pil_blend(existing[0], ink_l, l_mask);
                    let a = pil_blend(existing[1], ink_a, m);
                    la.put_pixel(dx as u32, dy as u32, crate::raster::LumaA([l, a]));
                }
            }
        }
        self.image = Image::from_dynamic(
            crate::raster::DynamicImage::ImageLumaA8(la),
            Some("LA".to_string()),
        );
        Ok(())
    }

    fn bitmap_mode_cmyk(
        &mut self,
        x: i32,
        y: i32,
        bmp_w: u32,
        bmp_h: u32,
        mask: &BitmapMask<'_>,
        color: (u8, u8, u8, u8),
    ) -> Result<(), PilError> {
        let img = self.image.materialize()?;
        let (img_w, img_h) = (img.width(), img.height());
        let mut rgba = img.to_rgba8();
        let ink = [color.0, color.1, color.2, color.3];
        for py in 0..bmp_h {
            for px in 0..bmp_w {
                let m = mask.coverage(px, py);
                if m == 0 {
                    continue;
                }
                let dx = x + px as i32;
                let dy = y + py as i32;
                if dx >= 0 && dy >= 0 && (dx as u32) < img_w && (dy as u32) < img_h {
                    let existing = rgba.get_pixel(dx as u32, dy as u32);
                    let c = if m == 255 {
                        ink[0]
                    } else {
                        pil_blend(existing[0], ink[0], m)
                    };
                    let m_ch = if m == 255 {
                        ink[1]
                    } else {
                        pil_blend(existing[1], ink[1], m)
                    };
                    let y_ch = if m == 255 {
                        ink[2]
                    } else {
                        pil_blend(existing[2], ink[2], m)
                    };
                    let k = if m == 255 {
                        ink[3]
                    } else {
                        pil_blend(existing[3], ink[3], m)
                    };
                    rgba.put_pixel(
                        dx as u32,
                        dy as u32,
                        crate::raster::Rgba([c, m_ch, y_ch, k]),
                    );
                }
            }
        }
        self.image = Image::from_dynamic(
            crate::raster::DynamicImage::ImageRgba8(rgba),
            Some("CMYK".to_string()),
        );
        Ok(())
    }

    fn bitmap_mode_p(
        &mut self,
        x: i32,
        y: i32,
        bmp_w: u32,
        bmp_h: u32,
        mask: &BitmapMask<'_>,
        color: (u8, u8, u8, u8),
    ) -> Result<(), PilError> {
        // Raw P images use an empty retained palette to mean that no
        // palette has been attached yet; only a non-empty palette
        // needs the indexed reconstruction path below.
        if let Some(palette) = self.image.palette().filter(|palette| !palette.is_empty()) {
            // Pillow ImageDraw mutates the existing ImagingCore, so the
            // encoded format and pending `info` metadata stay attached.
            // Carry them across our immediate indexed-buffer rebuild.
            let palette_alpha = self.image.palette_alpha().unwrap_or_default();
            let source_format = self.image.source_format();
            let info = self.image.image_info();
            let img = self.image.materialize()?;
            let luma = img.to_luma8();
            let (img_w, img_h) = luma.dimensions();
            let mut indices = crate::raster::GrayImage::new(img_w, img_h);
            for (op, ip) in indices.pixels_mut().zip(luma.pixels()) {
                op[0] = ip[0];
            }
            let ink = color.0;
            for py in 0..bmp_h {
                for px in 0..bmp_w {
                    let m = mask.coverage(px, py);
                    if m == 0 {
                        continue;
                    }
                    let dx = x + px as i32;
                    let dy = y + py as i32;
                    if dx >= 0 && dy >= 0 && (dx as u32) < img_w && (dy as u32) < img_h {
                        let v = if m == 255 {
                            ink
                        } else {
                            pil_blend(indices.get_pixel(dx as u32, dy as u32)[0], ink, m)
                        };
                        indices.put_pixel(dx as u32, dy as u32, crate::raster::Luma([v]));
                    }
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
        } else {
            let img = self.image.materialize()?;
            let (img_w, img_h) = (img.width(), img.height());
            let mut luma = img.to_luma8();
            let ink = color.0;
            for py in 0..bmp_h {
                for px in 0..bmp_w {
                    let m = mask.coverage(px, py);
                    if m == 0 {
                        continue;
                    }
                    let dx = x + px as i32;
                    let dy = y + py as i32;
                    if dx >= 0 && dy >= 0 && (dx as u32) < img_w && (dy as u32) < img_h {
                        let v = if m == 255 {
                            ink
                        } else {
                            pil_blend(luma.get_pixel(dx as u32, dy as u32)[0], ink, m)
                        };
                        luma.put_pixel(dx as u32, dy as u32, crate::raster::Luma([v]));
                    }
                }
            }
            self.image = Image::from_dynamic(
                crate::raster::DynamicImage::ImageLuma8(luma),
                Some("P".to_string()),
            );
        }
        Ok(())
    }

    fn bitmap_mode_numeric(
        &mut self,
        x: i32,
        y: i32,
        bmp_w: u32,
        bmp_h: u32,
        mask: &BitmapMask<'_>,
        mode: &str,
        color: (u8, u8, u8, u8),
    ) -> Result<(), PilError> {
        let img = self.image.materialize()?;
        let (img_w, img_h) = (img.width(), img.height());
        let mut rgba = img.to_rgba8();
        // Write all 4 bytes of the LE representation produced by the
        // mode-aware color normalization above.
        let ink = [color.0, color.1, color.2, color.3];
        for py in 0..bmp_h {
            for px in 0..bmp_w {
                let m = mask.coverage(px, py);
                if m == 0 {
                    continue;
                }
                let dx = x + px as i32;
                let dy = y + py as i32;
                if dx >= 0 && dy >= 0 && (dx as u32) < img_w && (dy as u32) < img_h {
                    let existing = rgba.get_pixel(dx as u32, dy as u32);
                    let b0 = if m == 255 {
                        ink[0]
                    } else {
                        pil_blend(existing[0], ink[0], m)
                    };
                    let b1 = if m == 255 {
                        ink[1]
                    } else {
                        pil_blend(existing[1], ink[1], m)
                    };
                    let b2 = if m == 255 {
                        ink[2]
                    } else {
                        pil_blend(existing[2], ink[2], m)
                    };
                    let b3 = if m == 255 {
                        ink[3]
                    } else {
                        pil_blend(existing[3], ink[3], m)
                    };
                    rgba.put_pixel(dx as u32, dy as u32, crate::raster::Rgba([b0, b1, b2, b3]));
                }
            }
        }
        self.image = Image::from_dynamic(
            crate::raster::DynamicImage::ImageRgba8(rgba),
            Some(mode.to_string()),
        );
        Ok(())
    }

    fn bitmap_mode_fallback(
        &mut self,
        x: i32,
        y: i32,
        bmp_w: u32,
        bmp_h: u32,
        mask: &BitmapMask<'_>,
        mode: &str,
        color: (u8, u8, u8, u8),
    ) -> Result<(), PilError> {
        // Fallback: RGBA pipeline
        let img = self.image.materialize()?;
        let (img_w, img_h) = (img.width(), img.height());
        let mut canvas = img.to_rgba8();
        for py in 0..bmp_h {
            for px in 0..bmp_w {
                let m = mask.coverage(px, py);
                if m == 0 {
                    continue;
                }
                let dx = x + px as i32;
                let dy = y + py as i32;
                if dx >= 0 && dy >= 0 && (dx as u32) < img_w && (dy as u32) < img_h {
                    let existing = canvas.get_pixel(dx as u32, dy as u32);
                    let r = if m == 255 {
                        color.0
                    } else {
                        pil_blend(existing[0], color.0, m)
                    };
                    let g = if m == 255 {
                        color.1
                    } else {
                        pil_blend(existing[1], color.1, m)
                    };
                    let b = if m == 255 {
                        color.2
                    } else {
                        pil_blend(existing[2], color.2, m)
                    };
                    let a = if m == 255 {
                        color.3
                    } else {
                        pil_blend(existing[3], color.3, m)
                    };
                    canvas.put_pixel(dx as u32, dy as u32, crate::raster::Rgba([r, g, b, a]));
                }
            }
        }
        // YCbCr/HSV and other 3-band fallback modes store no alpha;
        // rebuild the canvas as RGB tagged with the source mode so
        // the resulting image keeps Pillow's 3-byte pixel layout.
        self.image = Image::from_dynamic(
            crate::raster::DynamicImage::ImageRgb8(
                crate::raster::DynamicImage::ImageRgba8(canvas).to_rgb8(),
            ),
            Some(mode.to_string()),
        );
        Ok(())
    }
}
