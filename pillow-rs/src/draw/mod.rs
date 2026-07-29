//! Pillow-compatible `ImageDraw` primitives.
//!
//! [`Draw`] records drawing operations against an [`Image`] and keeps enough
//! mode metadata to convert the drawn result back to the original Pillow mode.
//! Coordinates are integer pixel coordinates. Colors are normalized RGBA byte
//! tuples before mode-specific drawing rules are applied.

use crate::raster::{DynamicImage, GenericImageView, Rgba, RgbaImage};

use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;

/// Drawing context for Pillow-style image mutation.
///
/// This is the Rust equivalent of `ImageDraw.Draw(image)`. Methods queue or
/// apply drawing operations and [`Draw::image_clone`] returns the updated image
/// with the original mode restored where possible.
#[derive(Debug)]
pub struct Draw {
    image: Image,
    /// Original mode before draw canvas created. Used to convert back on image_clone().
    orig_mode: Option<String>,
}

impl Draw {
    /// Creates a drawing context for `image`.
    ///
    /// `explicit_mode` is an optional PIL mode override for cases where the
    /// image's raw DynamicImage mode differs from the logical PIL mode
    /// (e.g. "P" stored as Luma8, "CMYK" stored as Rgba8).
    pub fn new(image: Image, explicit_mode: Option<String>) -> Self {
        let mode = explicit_mode.or_else(|| {
            let clone = image.clone();
            clone.mode().ok()
        });
        Draw {
            image,
            orig_mode: mode,
        }
    }

    /// Return the effective PIL mode for drawing operations.
    /// Uses the explicit mode if set, otherwise falls back to the image's mode.
    fn effective_mode(&self) -> String {
        self.orig_mode
            .clone()
            .or_else(|| self.image.mode().ok())
            .unwrap_or_else(|| "RGBA".to_string())
    }

    /// Returns the original Pillow mode of the drawing target.
    pub fn mode(&self) -> Option<&str> {
        self.orig_mode.as_deref()
    }

    fn shape_inks(
        &self,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
    ) -> (Option<(u8, u8, u8, u8)>, Option<(u8, u8, u8, u8)>) {
        if fill.is_none() && outline.is_none() && self.orig_mode.as_deref() == Some("PA") {
            (None, Some((255, 255, 255, 255)))
        } else {
            (fill, outline)
        }
    }

    /// Set the output image from a drawn RGBA canvas.
    /// image_clone() handles RGBA→native mode conversion for standard modes.
    /// Only F/I/CMYK need explicit_mode tagging (their RGBA data IS the final format).
    fn set_image(&mut self, canvas: RgbaImage) {
        let explicit = match self.orig_mode.as_deref() {
            Some("F") | Some("I") | Some("CMYK") => self.orig_mode.clone(),
            _ => None,
        };
        self.image = Image::from_dynamic(DynamicImage::ImageRgba8(canvas), explicit);
    }

    /// Draws a line from `(x0, y0)` to `(x1, y1)`.
    ///
    /// `fill` is an RGBA byte tuple and `width` is the stroke width in pixels.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn line(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        fill: (u8, u8, u8, u8),
        width: u32,
    ) -> Result<(), PilError> {
        self.image = Image::push_op(
            &self.image,
            PipelineOp::DrawLine {
                x0,
                y0,
                x1,
                y1,
                fill,
                width,
            },
        );
        Ok(())
    }

    /// Draws consecutive line segments through `points`.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when fewer than two points are given.
    /// Deferred pipeline execution reports materialization failures later.
    pub fn polyline(
        &mut self,
        points: &[(i32, i32)],
        fill: (u8, u8, u8, u8),
        width: u32,
    ) -> Result<(), PilError> {
        if points.len() < 2 {
            return Err(PilError::ValueError(
                "wrong number of coordinates".to_owned(),
            ));
        }
        for segment in points.windows(2) {
            self.line(
                segment[0].0,
                segment[0].1,
                segment[1].0,
                segment[1].1,
                fill,
                width,
            )?;
        }
        Ok(())
    }

    /// Draws a rectangle bounded by `(x0, y0, x1, y1)`.
    ///
    /// `fill` paints the interior when present. `outline` paints the border
    /// when present. `width` controls outline thickness in pixels.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn rectangle(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        width: u32,
    ) -> Result<(), PilError> {
        let (fill, outline) = self.shape_inks(fill, outline);
        self.image = Image::push_op(
            &self.image,
            PipelineOp::DrawRectangle {
                x0,
                y0,
                x1,
                y1,
                fill,
                outline,
                width,
            },
        );
        Ok(())
    }

    /// Draws an ellipse inside `(x0, y0, x1, y1)`.
    ///
    /// Fill, outline, and width follow Pillow `ImageDraw.ellipse` semantics.
    /// The backend uses Pillow's Bresenham-style quarter-ellipse algorithm.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn ellipse(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        _width: u32,
    ) -> Result<(), PilError> {
        let (fill, outline) = self.shape_inks(fill, outline);
        self.image = Image::push_op(
            &self.image,
            PipelineOp::DrawEllipse {
                x0,
                y0,
                x1,
                y1,
                fill,
                outline,
                width: _width,
            },
        );
        Ok(())
    }

    /// Draws a polygon from integer vertices.
    ///
    /// Fewer than three points is a no-op. `fill` paints the interior and
    /// `outline` paints the boundary when present.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn polygon(
        &mut self,
        points: &[(i32, i32)],
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        _width: u32,
    ) -> Result<(), PilError> {
        let (fill, outline) = self.shape_inks(fill, outline);
        if points.len() < 3 {
            return Ok(());
        }
        self.image = Image::push_op(
            &self.image,
            PipelineOp::DrawPolygon {
                points: points.to_vec(),
                fill,
                outline,
                width: _width,
            },
        );
        Ok(())
    }

    /// Fills a closed outline using Pillow's `ImageDraw.shape` ink order.
    ///
    /// Pillow draws `fill` first and `outline` last, but its outline primitive
    /// fills the complete path. Therefore `outline`, when present, is the
    /// effective color for the whole shape.
    pub fn shape(
        &mut self,
        points: &[(i32, i32)],
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
    ) -> Result<(), PilError> {
        let Some(ink) = outline.or(fill) else {
            return Ok(());
        };
        self.polygon(points, Some(ink), None, 1)
    }

    /// Draws one or more individual points.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn point(&mut self, points: &[(i32, i32)], fill: (u8, u8, u8, u8)) -> Result<(), PilError> {
        self.image = Image::push_op(
            &self.image,
            PipelineOp::DrawPoint {
                points: points.to_vec(),
                fill,
            },
        );
        Ok(())
    }

    /// Draws a bitmap mask at `(x, y)` using `fill`.
    ///
    /// The bitmap acts as a transparency mask. Valid bitmap modes:
    /// - "1": binary mask (non-zero → fill)
    /// - "L": alpha mask (0-255 opacity)
    /// - "RGBA"/"RGBa": alpha channel at byte offset +3
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when the bitmap mode is not a valid mask
    /// mode. Returns other [`PilError`] values when mode, size, or data lookup
    /// fails.
    pub fn bitmap(
        &mut self,
        x: i32,
        y: i32,
        bitmap: &Image,
        fill: Option<(u8, u8, u8, u8)>,
    ) -> Result<(), PilError> {
        let color = fill.unwrap_or((255, 255, 255, 255));
        let bmp_mode = bitmap.mode()?;
        // Validate mask mode — PIL only accepts "1", "L", "RGBA", "RGBa"
        let is_valid_mask = matches!(bmp_mode.as_str(), "1" | "L" | "RGBA" | "RGBa");
        if !is_valid_mask {
            return Err(PilError::ValueError("bad transparency mask".to_string()));
        }
        let (bmp_w, bmp_h) = bitmap.size()?;
        let raw_data = bitmap.getdata(None)?;
        let bmp_stride: usize = match bmp_mode.as_str() {
            "1" | "L" => 1,
            "RGBA" | "RGBa" => 4,
            _ => unreachable!(),
        };

        // Helper: get mask value (alpha) at pixel (px, py)
        let mask_val = |px: u32, py: u32, data: &[u8]| -> u8 {
            let idx = (py * bmp_w + px) as usize;
            match bmp_mode.as_str() {
                "1" => {
                    if idx < data.len() && data[idx] > 0 {
                        255
                    } else {
                        0
                    }
                }
                "L" => {
                    if idx < data.len() {
                        data[idx]
                    } else {
                        0
                    }
                }
                "RGBA" | "RGBa" => {
                    let pixel_idx = idx * bmp_stride;
                    if pixel_idx + 3 < data.len() {
                        data[pixel_idx + 3]
                    } else {
                        0
                    }
                }
                _ => 0,
            }
        };

        // PIL's BLEND: DIV255(a * (255 - mask) + b * mask)
        let pil_blend = |bg: u8, fg: u8, m: u8| -> u8 {
            if m == 0 {
                return bg;
            }
            if m == 255 {
                return fg;
            }
            ((bg as u16 * (255u16 - m as u16) + fg as u16 * m as u16 + 127u16) / 255u16) as u8
        };

        let mode = self.effective_mode();

        match mode.as_str() {
            "RGB" | "RGBA" => {
                let img = self.image.materialize()?;
                let (img_w, img_h) = (img.width(), img.height());
                let mut canvas = img.to_rgba8();

                for py in 0..bmp_h {
                    for px in 0..bmp_w {
                        let m = mask_val(px, py, &raw_data);
                        if m == 0 {
                            continue;
                        }
                        let dx = x + px as i32;
                        let dy = y + py as i32;
                        if dx >= 0 && dy >= 0 && (dx as u32) < img_w && (dy as u32) < img_h {
                            let existing = canvas.get_pixel(dx as u32, dy as u32);
                            let r = pil_blend(existing[0], color.0, m);
                            let g = pil_blend(existing[1], color.1, m);
                            let b = pil_blend(existing[2], color.2, m);
                            let a = pil_blend(existing[3], color.3, m);
                            canvas.put_pixel(
                                dx as u32,
                                dy as u32,
                                crate::raster::Rgba([r, g, b, a]),
                            );
                        }
                    }
                }

                self.set_image(canvas);
                Ok(())
            }
            "1" => {
                let img = self.image.materialize()?;
                let (img_w, img_h) = (img.width(), img.height());
                let mut luma = img.to_luma8();
                let ink = color.0;
                for py in 0..bmp_h {
                    for px in 0..bmp_w {
                        let m = mask_val(px, py, &raw_data);
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
            "L" => {
                let img = self.image.materialize()?;
                let (img_w, img_h) = (img.width(), img.height());
                let mut luma = img.to_luma8();
                let ink = color.0;
                for py in 0..bmp_h {
                    for px in 0..bmp_w {
                        let m = mask_val(px, py, &raw_data);
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
                self.image =
                    Image::from_dynamic(crate::raster::DynamicImage::ImageLuma8(luma), None);
                Ok(())
            }
            "LA" => {
                let img = self.image.materialize()?;
                let (img_w, img_h) = (img.width(), img.height());
                let mut la = img.to_luma_alpha8();
                let ink_l = color.0;
                let ink_a = 255u8;
                for py in 0..bmp_h {
                    for px in 0..bmp_w {
                        let m = mask_val(px, py, &raw_data);
                        if m == 0 {
                            continue;
                        }
                        let dx = x + px as i32;
                        let dy = y + py as i32;
                        if dx >= 0 && dy >= 0 && (dx as u32) < img_w && (dy as u32) < img_h {
                            let existing = la.get_pixel(dx as u32, dy as u32);
                            let l = if m == 255 {
                                ink_l
                            } else {
                                pil_blend(existing[0], ink_l, m)
                            };
                            let a = if m == 255 {
                                ink_a
                            } else {
                                pil_blend(existing[1], ink_a, m)
                            };
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
            "CMYK" => {
                let img = self.image.materialize()?;
                let (img_w, img_h) = (img.width(), img.height());
                let mut rgba = img.to_rgba8();
                let ink = [color.0, 0u8, 0u8, 0u8];
                for py in 0..bmp_h {
                    for px in 0..bmp_w {
                        let m = mask_val(px, py, &raw_data);
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
            "P" => {
                if let Some(palette) = self.image.palette() {
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
                            let m = mask_val(px, py, &raw_data);
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
                        materialized: crate::image::materialization_cache(),
                    });
                } else {
                    let img = self.image.materialize()?;
                    let (img_w, img_h) = (img.width(), img.height());
                    let mut luma = img.to_luma8();
                    let ink = color.0;
                    for py in 0..bmp_h {
                        for px in 0..bmp_w {
                            let m = mask_val(px, py, &raw_data);
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
            "I" | "F" => {
                let img = self.image.materialize()?;
                let (img_w, img_h) = (img.width(), img.height());
                let mut rgba = img.to_rgba8();
                // Write all 4 bytes of the LE representation (parse_draw_color
                // already packed I as i32→LE bytes, F as f32→LE bytes)
                let ink = [color.0, color.1, color.2, color.3];
                for py in 0..bmp_h {
                    for px in 0..bmp_w {
                        let m = mask_val(px, py, &raw_data);
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
                            rgba.put_pixel(
                                dx as u32,
                                dy as u32,
                                crate::raster::Rgba([b0, b1, b2, b3]),
                            );
                        }
                    }
                }
                self.image = Image::from_dynamic(
                    crate::raster::DynamicImage::ImageRgba8(rgba),
                    Some(mode.to_string()),
                );
                Ok(())
            }
            _ => {
                // Fallback: RGBA pipeline
                let img = self.image.materialize()?;
                let (img_w, img_h) = (img.width(), img.height());
                let mut canvas = img.to_rgba8();
                for py in 0..bmp_h {
                    for px in 0..bmp_w {
                        let m = mask_val(px, py, &raw_data);
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
                            canvas.put_pixel(
                                dx as u32,
                                dy as u32,
                                crate::raster::Rgba([r, g, b, a]),
                            );
                        }
                    }
                }
                self.set_image(canvas);
                Ok(())
            }
        }
    }

    /// Returns the current drawn image with original mode semantics restored.
    ///
    /// Standard modes are converted from the internal RGBA drawing canvas back
    /// to their original layout. `P` mode attempts palette-index restoration
    /// using the carried palette.
    pub fn image_clone(&self) -> Result<Image, PilError> {
        let img = self.image.clone();
        let source_format = img.source_format();
        let info = img.image_info();
        if let Some(ref orig) = self.orig_mode {
            if let Ok(current) = img.mode() {
                if current != *orig || matches!(orig.as_str(), "RGBA" | "CMYK") {
                    // Convert RGBA back to original mode
                    if let Ok(img_loaded) = img.materialize() {
                        let converted = match orig.as_str() {
                            "RGB" => DynamicImage::ImageRgb8(img_loaded.to_rgb8()),
                            "L" => {
                                DynamicImage::ImageLuma8(crate::color::pil_grayscale(&img_loaded)?)
                            }
                            "1" => {
                                // No dither: just threshold at 128 (matching PIL's fill behavior)
                                let gray = crate::color::pil_grayscale_truncate(&img_loaded)?;
                                let (w, h) = gray.dimensions();
                                let mut out = crate::raster::GrayImage::new(w, h);
                                for (op, gp) in out.pixels_mut().zip(gray.pixels()) {
                                    op[0] = if gp[0] >= 128 { 255 } else { 0 };
                                }
                                DynamicImage::ImageLuma8(out)
                            }
                            "LA" => {
                                // Use alpha from the RGBA image directly (PIL int fill
                                // writes A=0, which comes from fill=(*,*,*,0) in our
                                // draw pipeline, preserving the RGBA alpha channel)
                                let gray = crate::color::pil_grayscale(&img_loaded)?;
                                let (w, h) = gray.dimensions();
                                let mut ga = crate::raster::GrayAlphaImage::new(w, h);
                                let rgba = img_loaded.to_rgba8();
                                for ((gap, gp), rp) in
                                    ga.pixels_mut().zip(gray.pixels()).zip(rgba.pixels())
                                {
                                    gap[0] = gp[0];
                                    gap[1] = rp[3];
                                }
                                DynamicImage::ImageLumaA8(ga)
                            }
                            "P" => {
                                // Map RGBA pixels back to palette indices using the
                                // original palette if available, else fall back to grayscale.
                                if let Some(pal) = self.image.palette() {
                                    let (w, h) = img_loaded.dimensions();
                                    let rgba = img_loaded.to_rgba8();
                                    let mut indices = crate::raster::GrayImage::new(w, h);
                                    for (op, rp) in indices.pixels_mut().zip(rgba.pixels()) {
                                        let idx = pal
                                            .chunks_exact(3)
                                            .position(|p| {
                                                p[0] == rp[0] && p[1] == rp[1] && p[2] == rp[2]
                                            })
                                            .unwrap_or(0)
                                            .min(255)
                                            as u8;
                                        op[0] = idx;
                                    }
                                    let palette = self
                                        .image
                                        .palette()
                                        .unwrap_or_else(crate::image::default_palette);
                                    // Pillow keeps format/info on its in-place
                                    // ImageDraw mutation; retain that provenance
                                    // when restoring our indexed representation.
                                    return Ok(Image::Paletted(crate::image::PalettedData {
                                        indices,
                                        palette,
                                        palette_alpha: self
                                            .image
                                            .palette_alpha()
                                            .unwrap_or_default(),
                                        source_format,
                                        info,
                                        materialized: crate::image::materialization_cache(),
                                    }));
                                }
                                // Fallback: grayscale approximation
                                DynamicImage::ImageLuma8(crate::color::pil_grayscale(&img_loaded)?)
                            }
                            "CMYK" => {
                                // Identity: RGBA pixel values ARE CMYK pixel values
                                // (C→R, M→G, Y→B, K→A). Just tag the buffer as CMYK.
                                return Ok(Image::from_dynamic(
                                    img_loaded,
                                    Some("CMYK".to_string()),
                                ));
                            }
                            "RGBA" => {
                                // Identity: RGBA pixel values stay RGBA.
                                // Tag with explicit mode so mode() always reports "RGBA".
                                return Ok(Image::from_dynamic(
                                    DynamicImage::ImageRgba8(img_loaded.to_rgba8()),
                                    Some("RGBA".to_string()),
                                ));
                            }
                            _ => img_loaded,
                        };
                        let explicit = match orig.as_str() {
                            "P" => Some("P".to_string()),
                            "1" => Some("1".to_string()),
                            _ => None,
                        };
                        return Ok(Image::from_dynamic(converted, explicit));
                    }
                }
            }
        }
        Ok(img)
    }

    /// Draws an arc along an ellipse boundary.
    ///
    /// Angles are in degrees, following Pillow's coordinate convention.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn arc(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        start: f64,
        end: f64,
        fill: (u8, u8, u8, u8),
        _width: u32,
    ) -> Result<(), PilError> {
        self.image = Image::push_op(
            &self.image,
            PipelineOp::DrawArc {
                x0,
                y0,
                x1,
                y1,
                start,
                end,
                fill: Some(fill),
                width: _width,
            },
        );
        Ok(())
    }

    /// Draws a chord inside an ellipse bounding box.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn chord(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        start: f64,
        end: f64,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        _width: u32,
    ) -> Result<(), PilError> {
        let (fill, outline) = self.shape_inks(fill, outline);
        self.image = Image::push_op(
            &self.image,
            PipelineOp::DrawChord {
                x0,
                y0,
                x1,
                y1,
                start,
                end,
                fill,
                outline,
                width: _width,
            },
        );
        Ok(())
    }

    /// Draws a pieslice inside an ellipse bounding box.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn pieslice(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        start: f64,
        end: f64,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        _width: u32,
    ) -> Result<(), PilError> {
        let (fill, outline) = self.shape_inks(fill, outline);
        self.image = Image::push_op(
            &self.image,
            PipelineOp::DrawPieslice {
                x0,
                y0,
                x1,
                y1,
                start,
                end,
                fill,
                outline,
                width: _width,
            },
        );
        Ok(())
    }

    /// Draws a circle centered at `(cx, cy)`.
    ///
    /// `radius` is rounded to an integer pixel radius for pipeline execution.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn circle(
        &mut self,
        cx: i32,
        cy: i32,
        radius: f64,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        _width: u32,
    ) -> Result<(), PilError> {
        let (fill, outline) = self.shape_inks(fill, outline);
        self.image = Image::push_op(
            &self.image,
            PipelineOp::DrawCircle {
                cx,
                cy,
                radius: radius as i32,
                fill,
                outline,
                width: _width,
            },
        );
        Ok(())
    }

    /// Draws a rounded rectangle.
    ///
    /// `radius` is rounded to pixels. Non-positive radii or degenerate boxes
    /// fall back to a normal rectangle.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn rounded_rectangle(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        radius: f64,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        _width: u32,
    ) -> Result<(), PilError> {
        let (fill, outline) = self.shape_inks(fill, outline);
        let r = radius.round() as i32;
        let d = r * 2;
        if d <= 0 || x1 <= x0 + 1 || y1 <= y0 + 1 {
            // No corner curve, just draw rectangle
            self.image = Image::push_op(
                &self.image,
                PipelineOp::DrawRectangle {
                    x0,
                    y0,
                    x1,
                    y1,
                    fill,
                    outline,
                    width: 1,
                },
            );
            return Ok(());
        }

        self.image = Image::push_op(
            &self.image,
            PipelineOp::DrawRoundedRect {
                x0,
                y0,
                x1,
                y1,
                radius,
                fill,
                outline,
                width: _width,
            },
        );
        Ok(())
    }

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
            false,
        )
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
        self.text_with_options_inner(x, y, text, font, fill, options, true)
    }

    fn text_with_options_inner(
        &mut self,
        x: i32,
        y: i32,
        text: &str,
        font: &crate::font::FreeTypeFont,
        fill: (u8, u8, u8, u8),
        options: &crate::font::ImageFontTextOptions,
        rgba_blend_rgb: bool,
    ) -> Result<(), PilError> {
        let mode = self.effective_mode();
        let binary = matches!(mode.as_str(), "1" | "P" | "I" | "F");
        let mut options = options.clone();
        if binary && options.mode.is_none() {
            options.mode = Some("1".to_string());
        }

        // ImageFont rendering always uses alpha=255 so glyph coverage is preserved.
        // Mode-specific alpha handling (e.g., LA alpha=0 for int fills) is done
        // in text_compose_direct / text_compose_rgba.
        let render_fill = (fill.0, fill.1, fill.2, 255u8);
        let (w, h, mask, offset) = font.getmask2_with_options(text, &options)?;
        let pixels = text_mask_to_rgba(mask, render_fill);
        if w == 0 || h == 0 {
            return Ok(());
        }
        let draw_x = x.saturating_add(offset.0);
        let draw_y = y.saturating_add(offset.1);

        match mode.as_str() {
            "RGB" | "RGBA" => {
                self.text_compose_rgba(draw_x, draw_y, w, h, &pixels, fill, rgba_blend_rgb)
            }
            _ => self.text_compose_direct(draw_x, draw_y, w, h, &pixels, &mode, fill),
        }
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
        rgba_blend_rgb: bool,
    ) -> Result<(), PilError> {
        let img = self.image.materialize()?;
        let mut canvas = img.to_rgba8();
        let (img_w, img_h) = (canvas.width(), canvas.height());
        let mode = self.effective_mode();
        for py in 0..h {
            for px in 0..w {
                let off = ((py * w + px) * 4) as usize;
                if off + 3 < pixels.len() {
                    let sa = pixels[off + 3];
                    if sa == 0 {
                        continue;
                    }
                    let dx = (x as u32 + px).min(img_w - 1);
                    let dy = (y as u32 + py).min(img_h - 1);
                    let dp = canvas.get_pixel(dx, dy);
                    let inv = 255u16 - sa as u16;
                    let alpha = if mode == "RGBA" {
                        blend_u8(fill.3, dp[3], sa, inv)
                    } else {
                        255
                    };
                    let (r, g, b) = if mode == "RGBA" && !rgba_blend_rgb {
                        (fill.0, fill.1, fill.2)
                    } else {
                        (
                            blend_u8(fill.0, dp[0], sa, inv),
                            blend_u8(fill.1, dp[1], sa, inv),
                            blend_u8(fill.2, dp[2], sa, inv),
                        )
                    };
                    canvas.put_pixel(dx, dy, Rgba([r, g, b, alpha]));
                }
            }
        }
        self.set_image(canvas);
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
        let (img_w, img_h) = (img.width(), img.height());

        // For integer fills (all channels equal and alpha=255), treat as
        // single-channel: fill.0 goes to first channel, others get 0.
        // For tuple fills, use channel values directly.
        let is_int_fill = fill.0 == fill.1 && fill.0 == fill.2 && fill.3 == 255;

        match mode {
            "1" => {
                // Binary: write 255 where coverage > 0. PIL thresholds non-zero to 255.
                let mut luma = img.to_luma8();
                let ink = if fill.0 > 0 { 255u8 } else { 0u8 };
                for py in 0..h {
                    for px in 0..w {
                        let off = ((py * w + px) * 4) as usize;
                        if off + 3 < pixels.len() && pixels[off + 3] > 0 {
                            let dx = (x as u32 + px).min(img_w - 1);
                            let dy = (y as u32 + py).min(img_h - 1);
                            luma.put_pixel(dx, dy, crate::raster::Luma([ink]));
                        }
                    }
                }
                self.image = Image::from_dynamic(
                    crate::raster::DynamicImage::ImageLuma8(luma),
                    Some("1".to_string()),
                );
                Ok(())
            }
            "L" => {
                // Anti-aliased: blend fill.0 with background using coverage.
                // Uses PIL's signed truncation: bg + (fg - bg) * cov / 255
                let mut luma = img.to_luma8();
                let ink = fill.0;
                for py in 0..h {
                    for px in 0..w {
                        let off = ((py * w + px) * 4) as usize;
                        if off + 3 >= pixels.len() {
                            continue;
                        }
                        let cov = pixels[off + 3];
                        if cov == 0 {
                            continue;
                        }
                        let dx = (x as u32 + px).min(img_w - 1);
                        let dy = (y as u32 + py).min(img_h - 1);
                        let bg = luma.get_pixel(dx, dy)[0];
                        let result = pil_blend(ink, bg, cov);
                        luma.put_pixel(dx, dy, crate::raster::Luma([result]));
                    }
                }
                self.image =
                    Image::from_dynamic(crate::raster::DynamicImage::ImageLuma8(luma), None);
                Ok(())
            }
            "LA" => {
                // Anti-aliased per channel: L channel gets fill.0, A channel gets 0
                // (for integer fill) or the tuple's alpha.
                // Uses PIL's signed truncation: bg + (fg - bg) * cov / 255
                let mut la = img.to_luma_alpha8();
                let ink_l = fill.0;
                let ink_a = if is_int_fill { 0u8 } else { fill.3 };
                for py in 0..h {
                    for px in 0..w {
                        let off = ((py * w + px) * 4) as usize;
                        if off + 3 >= pixels.len() {
                            continue;
                        }
                        let cov = pixels[off + 3];
                        if cov == 0 {
                            continue;
                        }
                        let dx = (x as u32 + px).min(img_w - 1);
                        let dy = (y as u32 + py).min(img_h - 1);
                        let bg = la.get_pixel(dx, dy);
                        let new_l = pil_blend(ink_l, bg[0], cov);
                        let new_a = pil_blend(ink_a, bg[1], cov);
                        la.put_pixel(dx, dy, crate::raster::LumaA([new_l, new_a]));
                    }
                }
                self.image = Image::from_dynamic(
                    crate::raster::DynamicImage::ImageLumaA8(la),
                    Some("LA".to_string()),
                );
                Ok(())
            }
            "CMYK" => {
                // Anti-aliased per channel:
                //   C channel = fill.0 or tuple C, M=tuple M, Y=tuple Y, K=tuple K.
                //   For integer fill: C=fill.0, M=Y=K=0.
                // Uses PIL's signed truncation: bg + (fg - bg) * cov / 255
                let mut rgba = img.to_rgba8(); // CMYK stored as Rgba8 internally
                let ink = if is_int_fill {
                    [fill.0, 0u8, 0u8, 0u8]
                } else {
                    [fill.0, fill.1, fill.2, fill.3]
                };
                for py in 0..h {
                    for px in 0..w {
                        let off = ((py * w + px) * 4) as usize;
                        if off + 3 >= pixels.len() {
                            continue;
                        }
                        let cov = pixels[off + 3];
                        if cov == 0 {
                            continue;
                        }
                        let dx = (x as u32 + px).min(img_w - 1);
                        let dy = (y as u32 + py).min(img_h - 1);
                        let bg = rgba.get_pixel(dx, dy);
                        let new_pix = if cov == 255 {
                            Rgba(ink)
                        } else {
                            Rgba([
                                pil_blend(ink[0], bg[0], cov),
                                pil_blend(ink[1], bg[1], cov),
                                pil_blend(ink[2], bg[2], cov),
                                pil_blend(ink[3], bg[3], cov),
                            ])
                        };
                        rgba.put_pixel(dx, dy, new_pix);
                    }
                }
                self.image = Image::from_dynamic(
                    crate::raster::DynamicImage::ImageRgba8(rgba),
                    Some("CMYK".to_string()),
                );
                Ok(())
            }
            "P" => {
                // Binary: write palette index where coverage > 0. fontmode="1".
                if let Some(palette) = self.image.palette() {
                    // Pillow's in-place text draw preserves format/info. The
                    // Rust path rebuilds PalettedData, so copy both explicitly.
                    let palette_alpha = self.image.palette_alpha().unwrap_or_default();
                    let source_format = self.image.source_format();
                    let info = self.image.image_info();
                    let img_loaded = img.to_luma8();
                    let (w_i, h_i) = img_loaded.dimensions();
                    let mut indices = crate::raster::GrayImage::new(w_i, h_i);
                    for (op, ip) in indices.pixels_mut().zip(img_loaded.pixels()) {
                        op[0] = ip[0];
                    }
                    let ink = fill.0; // palette index
                    for py in 0..h.min(h_i) {
                        for px in 0..w.min(w_i) {
                            let off = ((py * w + px) * 4) as usize;
                            if off + 3 < pixels.len() && pixels[off + 3] > 0 {
                                let dx = (x as u32 + px).min(img_w - 1);
                                let dy = (y as u32 + py).min(img_h - 1);
                                indices.put_pixel(dx, dy, crate::raster::Luma([ink]));
                            }
                        }
                    }
                    self.image = Image::Paletted(crate::image::PalettedData {
                        indices,
                        palette,
                        palette_alpha,
                        source_format,
                        info,
                        materialized: crate::image::materialization_cache(),
                    });
                } else {
                    // Fallback: just modify luma8
                    let mut luma = img.to_luma8();
                    let ink = fill.0;
                    for py in 0..h {
                        for px in 0..w {
                            let off = ((py * w + px) * 4) as usize;
                            if off + 3 < pixels.len() && pixels[off + 3] > 0 {
                                let dx = (x as u32 + px).min(img_w - 1);
                                let dy = (y as u32 + py).min(img_h - 1);
                                luma.put_pixel(dx, dy, crate::raster::Luma([ink]));
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
            "I" | "F" => {
                // Binary: write full 4-byte LE representation. fontmode="1".
                // Stored internally as Rgba8 with explicit mode.
                let mut rgba = img.to_rgba8();
                let ink = [fill.0, fill.1, fill.2, fill.3];
                for py in 0..h {
                    for px in 0..w {
                        let off = ((py * w + px) * 4) as usize;
                        if off + 3 < pixels.len() && pixels[off + 3] > 0 {
                            let dx = (x as u32 + px).min(img_w - 1);
                            let dy = (y as u32 + py).min(img_h - 1);
                            rgba.put_pixel(dx, dy, Rgba(ink));
                        }
                    }
                }
                self.image = Image::from_dynamic(
                    crate::raster::DynamicImage::ImageRgba8(rgba),
                    Some(mode.to_string()),
                );
                Ok(())
            }
            _ => {
                // Fallback: RGBA pipeline
                self.text_compose_rgba(x, y, w, h, pixels, fill, false)
            }
        }
    }

    /// Consume the drawing context and return the modified image.
    pub fn into_image(self) -> Image {
        self.image
    }
}

// ── Drawing primitives ──────────────────────────────────────────────

/// Minimal native-pixel canvas used by the shared rasterizers.
///
/// Implementations retain their original storage layout; drawing code never
/// needs to convert an `L`, `LA`, `RGB`, or indexed buffer through `RGBA`.
pub(crate) trait DrawCanvas {
    /// Canvas width in pixels.
    fn width(&self) -> u32;
    /// Canvas height in pixels.
    fn height(&self) -> u32;
    /// Writes one normalized RGBA color using the canvas's native channels.
    fn put_rgba(&mut self, x: u32, y: u32, color: [u8; 4]);
}

impl DrawCanvas for RgbaImage {
    fn width(&self) -> u32 {
        self.width()
    }

    fn height(&self) -> u32 {
        self.height()
    }

    fn put_rgba(&mut self, x: u32, y: u32, color: [u8; 4]) {
        self.put_pixel(x, y, Rgba(color));
    }
}

/// Bresenham's line algorithm with clamping.
pub(crate) fn bresenham_line<C: DrawCanvas>(
    canvas: &mut C,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: (u8, u8, u8, u8),
    w: u32,
    h: u32,
    raw: bool,
) {
    // Match Pillow src/libImaging/Draw.c::{line8,line32,line32rgba}.
    // The C primitive omits its final endpoint because draw_lines adds it
    // once after the segment chain; this helper represents one complete
    // high-level segment, so it appends that endpoint below.
    let mut x = i64::from(x0);
    let mut y = i64::from(y0);
    let target_x = i64::from(x1);
    let target_y = i64::from(y1);
    let mut dx = target_x - x;
    let step_x = if dx < 0 {
        dx = -dx;
        -1
    } else {
        1
    };
    let mut dy = target_y - y;
    let step_y = if dy < 0 {
        dy = -dy;
        -1
    } else {
        1
    };

    if dx == 0 {
        for _ in 0..dy {
            plot(canvas, x as i32, y as i32, color, w, h, raw);
            y += step_y;
        }
    } else if dy == 0 {
        for _ in 0..dx {
            plot(canvas, x as i32, y as i32, color, w, h, raw);
            x += step_x;
        }
    } else if dx > dy {
        let steps = dx;
        dy += dy;
        let mut error = dy - dx;
        dx += dx;
        for _ in 0..steps {
            plot(canvas, x as i32, y as i32, color, w, h, raw);
            if error >= 0 {
                y += step_y;
                error -= dx;
            }
            error += dy;
            x += step_x;
        }
    } else {
        let steps = dy;
        dx += dx;
        let mut error = dx - dy;
        dy += dy;
        for _ in 0..steps {
            plot(canvas, x as i32, y as i32, color, w, h, raw);
            if error >= 0 {
                x += step_x;
                error -= dy;
            }
            error += dx;
            y += step_y;
        }
    }
    plot(canvas, x1, y1, color, w, h, raw);
}

/// Plot a single pixel with bounds checking.
///
/// When `raw` is true (F/I mode), writes the 4 bytes directly as-is without any
/// alpha blending — the 4-byte chunk represents a raw float32 or int32 LE value.
///
/// When `raw` is false, writes the normalized color directly. Pillow's default
/// drawing context uses the native-mode `point8`/`point32` primitives; alpha is
/// a stored channel, not an implicit blend factor. RGBA-on-RGB drawing uses a
/// separate explicit blend mode at the binding/API layer.
#[inline]
pub(crate) fn plot<C: DrawCanvas>(
    canvas: &mut C,
    x: i32,
    y: i32,
    color: (u8, u8, u8, u8),
    w: u32,
    h: u32,
    raw: bool,
) {
    if x < 0 || y < 0 || x as u32 >= w || y as u32 >= h {
        return;
    }
    let (x, y) = (x as u32, y as u32);
    let _ = raw;
    canvas.put_rgba(x, y, [color.0, color.1, color.2, color.3]);
}

#[inline]
pub(crate) fn blend_u8(src: u8, dst: u8, alpha: u8, inv_alpha: u16) -> u8 {
    let a = alpha as u16;
    (((src as u16 * a) + (dst as u16 * inv_alpha) + 127) / 255) as u8
}

/// PIL-style single-channel blend:
///   BLEND(mask, dst, src) = DIV255(dst * (255 - mask) + src * mask)
/// where DIV255(x) = (x + 127) / 255  (round-to-nearest via +127 before /255 truncation)
///
/// This exactly matches PIL's ImagingFill2 → fill_mask_L C implementation.
/// Using the simpler unsigned formula (fg*cov + bg*(255-cov))/255 truncates,
/// which differs by 1 from PIL's rounded result for some cov values.
#[inline]
pub(crate) fn pil_blend(fg: u8, bg: u8, cov: u8) -> u8 {
    let x = (bg as u32) * (255u32 - cov as u32) + (fg as u32) * (cov as u32);
    // DIV255 with rounding: (x + 127) / 255
    // Note: `(x + 127 + (x >> 8)) >> 8` is NOT used — it is an approximation
    // that differs from the exact /255 for some inputs (e.g., x=37104 gives 145 vs 146).
    ((x + 127) / 255) as u8
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

/// Compute cubic Bezier curve subdivision points.
/// Returns a flat list of (x, y) integer pairs for the curve from t=1..steps.
/// `control_points` must have at least 8 elements: [x0, y0, x1, y1, x2, y2, x3, y3].
/// Matches PIL's Outline.curve() algorithm exactly.
pub fn outline_curve_points(control_points: &[f64], steps: u32) -> Vec<(i32, i32)> {
    if control_points.len() < 8 || steps == 0 {
        return vec![];
    }
    let x0 = control_points[0];
    let y0 = control_points[1];
    let x1 = control_points[2];
    let y1 = control_points[3];
    let x2 = control_points[4];
    let y2 = control_points[5];
    let x3 = control_points[6];
    let y3 = control_points[7];

    let mut points = Vec::with_capacity(steps as usize);
    for i in 1..=steps {
        let t = i as f64 / steps as f64;
        let u = 1.0 - t;
        let x = u * u * u * x0 + 3.0 * u * u * t * x1 + 3.0 * u * t * t * x2 + t * t * t * x3;
        let y = u * u * u * y0 + 3.0 * u * u * t * y1 + 3.0 * u * t * t * y2 + t * t * t * y3;
        points.push((x.round() as i32, y.round() as i32));
    }
    points
}

/// PIL-style ROUND_UP: away-from-zero rounding at 0.5.
pub(crate) fn round_up(f: f64) -> i32 {
    if f >= 0.0 {
        (f + 0.5).floor() as i32
    } else {
        -((-f) + 0.5).floor() as i32
    }
}

/// PIL-style ROUND_DOWN: toward-zero rounding at 0.5.
pub(crate) fn round_down(f: f64) -> i32 {
    if f >= 0.0 {
        (f - 0.5).ceil() as i32
    } else {
        -((-f) - 0.5).ceil() as i32
    }
}

/// PIL-identical scanline polygon fill.
///
/// Uses PIL's edge-table / scanline algorithm from Draw.c:
/// 1. Build edges with inverse slope (dx = Δx/Δy)
/// 2. For each scanline, compute x-intersections from active edges
/// 3. Sort intersections and fill between pairs using ROUND_UP/ROUND_DOWN
/// 4. Horizontal edges drawn directly as filled lines
pub(crate) fn scanline_polygon_fill<C: DrawCanvas>(
    canvas: &mut C,
    points: &[(i32, i32)],
    color: (u8, u8, u8, u8),
    img_w: u32,
    img_h: u32,
    _raw: bool,
) {
    let n = points.len();
    if n < 3 {
        return;
    }

    // Edge descriptor matching PIL's Edge struct
    #[derive(Clone, Copy)]
    struct ScanEdge {
        x0: i32,
        y0: i32,
        xmin: i32,
        xmax: i32,
        ymin: i32,
        ymax: i32,
        dx: f32,
    }

    let make_edge = |x0: i32, y0: i32, x1: i32, y1: i32| ScanEdge {
        x0,
        y0,
        xmin: x0.min(x1),
        xmax: x0.max(x1),
        ymin: y0.min(y1),
        ymax: y0.max(y1),
        dx: if y0 == y1 {
            0.0
        } else {
            (x1 - x0) as f32 / (y1 - y0) as f32
        },
    };

    // Build Pillow's edge list, including its consecutive-horizontal-edge
    // coalescing. That detail affects vertex parity on scanlines that touch a
    // run of collinear polygon points.
    let mut edges: Vec<ScanEdge> = Vec::with_capacity(n);
    for i in 0..n.saturating_sub(1) {
        let (x0, y0) = points[i];
        let (x1, y1) = points[i + 1];
        if y0 == y1 && i != 0 && y0 == points[i - 1].1 {
            let previous_x = points[i - 1].0;
            if let Some(last) = edges.last_mut() {
                if x1 > x0 && x0 > previous_x {
                    last.xmax = x1;
                    continue;
                }
                if x1 < x0 && x0 < previous_x {
                    last.xmin = x1;
                    continue;
                }
            }
        }
        edges.push(make_edge(x0, y0, x1, y1));
    }
    if points[n - 1] != points[0] {
        let (x0, y0) = points[n - 1];
        let (x1, y1) = points[0];
        edges.push(make_edge(x0, y0, x1, y1));
    }

    if edges.is_empty() {
        return;
    }

    // Draw horizontal edges immediately (matching PIL's hline in non-alpha mode)
    let iw = img_w as i32;
    let ih = img_h as i32;
    let rgba = [color.0, color.1, color.2, color.3];
    for e in &edges {
        if e.ymin == e.ymax && e.ymin >= 0 && e.ymin < ih {
            let x_start = e.xmin.max(0);
            let x_end = e.xmax.min(iw - 1);
            for x in x_start..=x_end {
                canvas.put_rgba(x as u32, e.ymin as u32, rgba);
            }
        }
    }

    // Find global y bounds
    let mut global_ymin = i32::MAX;
    let mut global_ymax = i32::MIN;
    for e in &edges {
        global_ymin = global_ymin.min(e.ymin);
        global_ymax = global_ymax.max(e.ymax);
    }
    global_ymin = global_ymin.max(0);
    global_ymax = global_ymax.min(ih - 1);
    if global_ymin > global_ymax {
        return;
    }

    // Edge table: only non-horizontal edges (matching PIL's edge_table)
    let edge_table: Vec<&ScanEdge> = edges.iter().filter(|e| e.ymin != e.ymax).collect();
    if edge_table.is_empty() {
        return;
    }

    // Pre-allocate x-intersection array
    let mut xx: Vec<f32> = Vec::with_capacity(edge_table.len() * 2);

    // Scanline sweep
    for y in global_ymin..=global_ymax {
        xx.clear();
        let yf = y as f32;

        for (edge_index, edge) in edge_table.iter().enumerate() {
            if y >= edge.ymin && y <= edge.ymax {
                let mut x = (yf - edge.y0 as f32) * edge.dx + edge.x0 as f32;
                xx.push(x);

                // PIL duplicate at ymax (vertex parity)
                if y == edge.ymax && y < global_ymax {
                    xx.push(x);
                } else if (y == edge.ymin || y == edge.ymax) && edge.dx != 0.0 {
                    // Pillow connects discontiguous corners by looking one row
                    // into the two incident edges and nudging the shared
                    // intersection when both edges leave in the same direction.
                    for other in edge_table.iter().take(edge_index) {
                        if (y != other.ymin && y != other.ymax) || other.dx == 0.0 {
                            continue;
                        }
                        let other_x = (yf - other.y0 as f32) * other.dx + other.x0 as f32;
                        if x.round() != other_x.round() {
                            continue;
                        }
                        let offset = if y == edge.ymax { -1 } else { 1 };
                        let adjacent_x = ((y + offset - edge.y0) as f32) * edge.dx + edge.x0 as f32;
                        if y + offset < other.ymin || y + offset > other.ymax {
                            continue;
                        }
                        let adjacent_other_x =
                            ((y + offset - other.y0) as f32) * other.dx + other.x0 as f32;
                        if x > adjacent_x + 1.0 && x > adjacent_other_x + 1.0 {
                            x = adjacent_x.max(adjacent_other_x).round() + 1.0;
                        } else if x < adjacent_x - 1.0 && x < adjacent_other_x - 1.0 {
                            x = adjacent_x.min(adjacent_other_x).round() - 1.0;
                        }
                        if let Some(current) = xx.last_mut() {
                            *current = x;
                        }
                        break;
                    }
                }
            }
        }

        if xx.is_empty() {
            continue;
        }

        xx.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        // Fill pairs (0-1, 2-3, ...) matching PIL's pair fill
        let mut i = 1;
        while i < xx.len() {
            let x_start = round_up(f64::from(xx[i - 1]));
            let x_end = round_down(f64::from(xx[i]));
            if x_end < 0 || x_start >= iw {
                i += 2;
                continue;
            }
            let x_start = x_start.max(0);
            let x_end = x_end.min(iw - 1);
            if x_start <= x_end {
                for x in x_start..=x_end {
                    canvas.put_rgba(x as u32, y as u32, rgba);
                }
            }
            i += 2;
        }
    }
}
