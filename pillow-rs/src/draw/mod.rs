//! ImageDraw — drawing primitives on images.
//! Implements line, rectangle, ellipse, polygon, point, text.
//! Uses Bresenham-style algorithms for pixel-perfect rendering.

use pillow_rs_image::{DynamicImage, GenericImageView, Rgba, RgbaImage};

use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;

/// Drawing context wrapping an Image.
/// PIL: `draw = ImageDraw.Draw(image)` then `draw.line(...)`, `draw.rectangle(...)`, etc.
#[derive(Debug)]
pub struct Draw {
    image: Image,
    /// Original mode before draw canvas created. Used to convert back on image_clone().
    orig_mode: Option<String>,
}

impl Draw {
    /// Create a new drawing context.
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

    /// Return the original PIL mode of the drawing target.
    pub fn mode(&self) -> Option<&str> {
        self.orig_mode.as_deref()
    }

    /// Set the output image from a drawn RGBA canvas.
    /// image_clone() handles RGBA→native mode conversion for standard modes.
    /// Only F/I/CMYK need explicit_mode tagging (their RGBA data IS the final format).
    fn set_image(&mut self, canvas: RgbaImage) {
        let explicit = match self.orig_mode.as_deref() {
            Some("F") | Some("I") | Some("CMYK") => self.orig_mode.clone(),
            _ => None,
        };
        self.image = Image::Loaded(DynamicImage::ImageRgba8(canvas), explicit);
    }

    /// Draw a line from (x0,y0) to (x1,y1). Bresenham's algorithm.
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

    /// Draw a rectangle. Filled if fill is provided, outline otherwise.
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

    /// Draw an ellipse within the bounding box.
    /// Uses PIL's exact Bresenham quarter-ellipse algorithm with step-2 coordinate system.
    /// Matches PIL pixel-for-pixel by using the same `a=x1-x0, b=y1-y0` scaling.
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

    /// Draw a polygon. Filled if fill provided.
    pub fn polygon(
        &mut self,
        points: &[(i32, i32)],
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        _width: u32,
    ) -> Result<(), PilError> {
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

    /// Draw a single point.
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

    /// Draw a bitmap image at position (x, y) with fill color.
    ///
    /// The bitmap acts as a transparency mask. Valid bitmap modes:
    /// - "1": binary mask (non-zero → fill)
    /// - "L": alpha mask (0-255 opacity)
    /// - "RGBA"/"RGBa": alpha channel at byte offset +3
    ///
    /// Matching PIL's `ImagingFill2` behavior exactly.
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
                                pillow_rs_image::Rgba([r, g, b, a]),
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
                            luma.put_pixel(dx as u32, dy as u32, pillow_rs_image::Luma([v]));
                        }
                    }
                }
                self.image = Image::Loaded(
                    pillow_rs_image::DynamicImage::ImageLuma8(luma),
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
                            luma.put_pixel(dx as u32, dy as u32, pillow_rs_image::Luma([v]));
                        }
                    }
                }
                self.image = Image::Loaded(pillow_rs_image::DynamicImage::ImageLuma8(luma), None);
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
                            la.put_pixel(dx as u32, dy as u32, pillow_rs_image::LumaA([l, a]));
                        }
                    }
                }
                self.image = Image::Loaded(
                    pillow_rs_image::DynamicImage::ImageLumaA8(la),
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
                                pillow_rs_image::Rgba([c, m_ch, y_ch, k]),
                            );
                        }
                    }
                }
                self.image = Image::Loaded(
                    pillow_rs_image::DynamicImage::ImageRgba8(rgba),
                    Some("CMYK".to_string()),
                );
                Ok(())
            }
            "P" => {
                if let Some(palette) = self.image.palette() {
                    let img = self.image.materialize()?;
                    let luma = img.to_luma8();
                    let (img_w, img_h) = luma.dimensions();
                    let mut indices = pillow_rs_image::GrayImage::new(img_w, img_h);
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
                                indices.put_pixel(dx as u32, dy as u32, pillow_rs_image::Luma([v]));
                            }
                        }
                    }
                    self.image = Image::Paletted(crate::image::PalettedData { indices, palette });
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
                                luma.put_pixel(dx as u32, dy as u32, pillow_rs_image::Luma([v]));
                            }
                        }
                    }
                    self.image = Image::Loaded(
                        pillow_rs_image::DynamicImage::ImageLuma8(luma),
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
                                pillow_rs_image::Rgba([b0, b1, b2, b3]),
                            );
                        }
                    }
                }
                self.image = Image::Loaded(
                    pillow_rs_image::DynamicImage::ImageRgba8(rgba),
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
                                pillow_rs_image::Rgba([r, g, b, a]),
                            );
                        }
                    }
                }
                self.set_image(canvas);
                Ok(())
            }
        }
    }

    /// Return a clone of the current image state, converted back to original mode.
    pub fn image_clone(&self) -> Image {
        let img = self.image.clone();
        if let Some(ref orig) = self.orig_mode {
            if let Ok(current) = img.mode() {
                if current != *orig || matches!(orig.as_str(), "RGBA" | "CMYK") {
                    // Convert RGBA back to original mode
                    if let Ok(img_loaded) = img.materialize() {
                        let converted = match orig.as_str() {
                            "RGB" => DynamicImage::ImageRgb8(img_loaded.to_rgb8()),
                            "L" => {
                                DynamicImage::ImageLuma8(crate::color::pil_grayscale(&img_loaded))
                            }
                            "1" => {
                                // No dither: just threshold at 128 (matching PIL's fill behavior)
                                let gray = crate::color::pil_grayscale_truncate(&img_loaded);
                                let (w, h) = gray.dimensions();
                                let mut out = pillow_rs_image::GrayImage::new(w, h);
                                for (op, gp) in out.pixels_mut().zip(gray.pixels()) {
                                    op[0] = if gp[0] >= 128 { 255 } else { 0 };
                                }
                                DynamicImage::ImageLuma8(out)
                            }
                            "LA" => {
                                // Use alpha from the RGBA image directly (PIL int fill
                                // writes A=0, which comes from fill=(*,*,*,0) in our
                                // draw pipeline, preserving the RGBA alpha channel)
                                let gray = crate::color::pil_grayscale(&img_loaded);
                                let (w, h) = gray.dimensions();
                                let mut ga = pillow_rs_image::GrayAlphaImage::new(w, h);
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
                                    let mut indices = pillow_rs_image::GrayImage::new(w, h);
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
                                    return Image::Paletted(crate::image::PalettedData {
                                        indices,
                                        palette,
                                    });
                                }
                                // Fallback: grayscale approximation
                                DynamicImage::ImageLuma8(crate::color::pil_grayscale(&img_loaded))
                            }
                            "CMYK" => {
                                // Identity: RGBA pixel values ARE CMYK pixel values
                                // (C→R, M→G, Y→B, K→A). Just tag the buffer as CMYK.
                                return Image::Loaded(img_loaded, Some("CMYK".to_string()));
                            }
                            "RGBA" => {
                                // Identity: RGBA pixel values stay RGBA.
                                // Tag with explicit mode so mode() always reports "RGBA".
                                return Image::Loaded(
                                    DynamicImage::ImageRgba8(img_loaded.to_rgba8()),
                                    Some("RGBA".to_string()),
                                );
                            }
                            _ => img_loaded,
                        };
                        let explicit = match orig.as_str() {
                            "P" => Some("P".to_string()),
                            "1" => Some("1".to_string()),
                            _ => None,
                        };
                        return Image::Loaded(converted, explicit);
                    }
                }
            }
        }
        img
    }

    /// Draw an arc (partial ellipse outline).
    /// Uses the same Bresenham quarter-ellipse generator as the ellipse fill,
    /// then performs edge detection to find boundary pixels and filters by angle.
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

    /// Draw a chord (arc + filled to center).
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

    /// Draw a pieslice. Uses the Bresenham ellipse fill with angle clipping.
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

    /// Draw a circle.
    pub fn circle(
        &mut self,
        cx: i32,
        cy: i32,
        radius: f64,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        _width: u32,
    ) -> Result<(), PilError> {
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

    /// Draw a rounded rectangle. Composes corner pieslices/arcs and rectangles
    /// matching PIL's Python algorithm in ImageDraw.py.
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

    /// Draw text at position (x, y) using a font.
    ///
    /// For RGB and RGBA modes, uses the standard RGBA compositing pipeline.
    /// For other modes (1, L, LA, CMYK, P, I, F), renders directly in the
    /// mode's native pixel format, matching PIL's `draw_bitmap` behavior:
    /// - Integer fill values go to the first channel only; other channels get 0.
    /// - Binary modes (1, P, I, F) use PIL's fontmode="1": binary glyphs (coverage >= 128 → 255).
    /// - Anti-aliased modes (L, LA, CMYK) use PIL's BLEND (truncation) per channel.
    pub fn text(
        &mut self,
        x: i32,
        y: i32,
        text: &str,
        font: &crate::font::Font,
        fill: (u8, u8, u8, u8),
    ) -> Result<(), PilError> {
        let mode = self.effective_mode();
        let binary = matches!(mode.as_str(), "1" | "P" | "I" | "F");

        // Font rendering always uses alpha=255 so glyph coverage is preserved.
        // Mode-specific alpha handling (e.g., LA alpha=0 for int fills) is done
        // in text_compose_direct / text_compose_rgba.
        let render_fill = (fill.0, fill.1, fill.2, 255u8);
        let (w, h, pixels) = if binary {
            font.render_text_binary(text, render_fill, 0.0)
        } else {
            font.render_text(text, render_fill, 0.0)
        };
        if w == 0 || h == 0 {
            return Ok(());
        }

        match mode.as_str() {
            "RGB" | "RGBA" => self.text_compose_rgba(x, y, w, h, &pixels),
            _ => self.text_compose_direct(x, y, w, h, &pixels, &mode, fill),
        }
    }

    /// RGBA compositing for text (used for RGB and RGBA modes).
    ///
    /// Pixels from the font renderer have the glyph coverage in the alpha channel
    /// and the fill color in the RGB channels. This function blends them onto the
    /// destination canvas using PIL's BLEND formula for all four channels,
    /// including proper alpha blending when the destination is RGBA.
    fn text_compose_rgba(
        &mut self,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        pixels: &[u8],
    ) -> Result<(), PilError> {
        let img = self.image.materialize()?;
        let mut canvas = img.to_rgba8();
        let (img_w, img_h) = (canvas.width(), canvas.height());
        let mode = self.effective_mode();
        let blend_alpha = mode == "RGBA";

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
                    if sa == 255 {
                        let out_a = if blend_alpha { 255u8 } else { 255u8 };
                        canvas.put_pixel(
                            dx,
                            dy,
                            Rgba([pixels[off], pixels[off + 1], pixels[off + 2], out_a]),
                        );
                    } else {
                        let dp = canvas.get_pixel(dx, dy);
                        let inv = 255u16 - sa as u16;
                        canvas.put_pixel(
                            dx,
                            dy,
                            Rgba([
                                blend_u8(pixels[off], dp[0], sa, inv),
                                blend_u8(pixels[off + 1], dp[1], sa, inv),
                                blend_u8(pixels[off + 2], dp[2], sa, inv),
                                if blend_alpha {
                                    blend_u8(255u8, dp[3], sa, inv)
                                } else {
                                    255u8
                                },
                            ]),
                        );
                    }
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
                            luma.put_pixel(dx, dy, pillow_rs_image::Luma([ink]));
                        }
                    }
                }
                self.image = Image::Loaded(
                    pillow_rs_image::DynamicImage::ImageLuma8(luma),
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
                        luma.put_pixel(dx, dy, pillow_rs_image::Luma([result]));
                    }
                }
                self.image = Image::Loaded(pillow_rs_image::DynamicImage::ImageLuma8(luma), None);
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
                        la.put_pixel(dx, dy, pillow_rs_image::LumaA([new_l, new_a]));
                    }
                }
                self.image = Image::Loaded(
                    pillow_rs_image::DynamicImage::ImageLumaA8(la),
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
                self.image = Image::Loaded(
                    pillow_rs_image::DynamicImage::ImageRgba8(rgba),
                    Some("CMYK".to_string()),
                );
                Ok(())
            }
            "P" => {
                // Binary: write palette index where coverage > 0. fontmode="1".
                if let Some(palette) = self.image.palette() {
                    let img_loaded = img.to_luma8();
                    let (w_i, h_i) = img_loaded.dimensions();
                    let mut indices = pillow_rs_image::GrayImage::new(w_i, h_i);
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
                                indices.put_pixel(dx, dy, pillow_rs_image::Luma([ink]));
                            }
                        }
                    }
                    self.image = Image::Paletted(crate::image::PalettedData { indices, palette });
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
                                luma.put_pixel(dx, dy, pillow_rs_image::Luma([ink]));
                            }
                        }
                    }
                    self.image = Image::Loaded(
                        pillow_rs_image::DynamicImage::ImageLuma8(luma),
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
                self.image = Image::Loaded(
                    pillow_rs_image::DynamicImage::ImageRgba8(rgba),
                    Some(mode.to_string()),
                );
                Ok(())
            }
            _ => {
                // Fallback: RGBA pipeline
                self.text_compose_rgba(x, y, w, h, pixels)
            }
        }
    }

    /// Consume the drawing context and return the modified image.
    pub fn into_image(self) -> Image {
        self.image
    }
}

// ── Drawing primitives ──────────────────────────────────────────────

/// Bresenham's line algorithm with clamping.
pub(crate) fn bresenham_line(
    canvas: &mut RgbaImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: (u8, u8, u8, u8),
    w: u32,
    h: u32,
    raw: bool,
) {
    let mut x = x0;
    let mut y = y0;
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        plot(canvas, x, y, color, w, h, raw);
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

/// Plot a single pixel with bounds checking.
///
/// When `raw` is true (F/I mode), writes the 4 bytes directly as-is without any
/// alpha blending — the 4-byte chunk represents a raw float32 or int32 LE value.
///
/// When `raw` is false, applies the standard alpha blending:
/// - `alpha == 255`: write RGB directly with A=255
/// - `alpha == 0`: write RGB directly with A=0 (PIL int fill behavior)
/// - otherwise: blend with existing pixel
#[inline]
pub(crate) fn plot(
    canvas: &mut RgbaImage,
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
    if raw {
        // F/I mode: write the 4-byte value as-is (float32 or int32 LE)
        canvas.put_pixel(x, y, Rgba([color.0, color.1, color.2, color.3]));
    } else if color.3 == 255 {
        canvas.put_pixel(x, y, Rgba([color.0, color.1, color.2, 255]));
    } else if color.3 == 0 {
        // PIL int fill: value goes to first channel, other channels = 0.
        // Write RGB directly with A=0 (bypass alpha blending) to match
        // LA mode and other multi-channel modes.
        canvas.put_pixel(x, y, Rgba([color.0, color.1, color.2, 0]));
    } else {
        let existing = canvas.get_pixel(x, y);
        let a = color.3 as u16;
        let inv = 255u16 - a;
        canvas.put_pixel(
            x,
            y,
            Rgba([
                ((color.0 as u16 * a + existing[0] as u16 * inv) / 255) as u8,
                ((color.1 as u16 * a + existing[1] as u16 * inv) / 255) as u8,
                ((color.2 as u16 * a + existing[2] as u16 * inv) / 255) as u8,
                color.3.max(existing[3]),
            ]),
        );
    }
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
pub(crate) fn scanline_polygon_fill(
    canvas: &mut RgbaImage,
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
        dx: f64,
    }

    // Build edge list
    let mut edges: Vec<ScanEdge> = Vec::with_capacity(n);
    for i in 0..n {
        let (x0, y0) = points[i];
        let (x1, y1) = points[(i + 1) % n];
        // Skip zero-length edges (coincident vertices)
        if x0 == x1 && y0 == y1 {
            continue;
        }
        let dx = if y0 != y1 {
            (x1 - x0) as f64 / (y1 - y0) as f64
        } else {
            0.0
        };
        edges.push(ScanEdge {
            x0,
            y0,
            xmin: x0.min(x1),
            xmax: x0.max(x1),
            ymin: y0.min(y1),
            ymax: y0.max(y1),
            dx,
        });
    }

    if edges.is_empty() {
        return;
    }

    // Draw horizontal edges immediately (matching PIL's hline in non-alpha mode)
    let iw = img_w as i32;
    let ih = img_h as i32;
    let rgba = Rgba([color.0, color.1, color.2, color.3]);
    for e in &edges {
        if e.ymin == e.ymax && e.ymin >= 0 && e.ymin < ih {
            let x_start = e.xmin.max(0);
            let x_end = e.xmax.min(iw - 1);
            for x in x_start..=x_end {
                canvas.put_pixel(x as u32, e.ymin as u32, rgba);
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
    let mut xx: Vec<f64> = Vec::with_capacity(edge_table.len() * 2);

    // Scanline sweep
    for y in global_ymin..=global_ymax {
        xx.clear();
        let yf = y as f64;

        for edge in &edge_table {
            if y >= edge.ymin && y <= edge.ymax {
                let x = (yf - edge.y0 as f64) * edge.dx + edge.x0 as f64;
                xx.push(x);

                // PIL duplicate at ymax (vertex parity)
                if y == edge.ymax && y < global_ymax {
                    xx.push(x);
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
            let x_start = round_up(xx[i - 1]).max(0).min(iw - 1);
            let x_end = round_down(xx[i]).max(0).min(iw - 1);
            if x_start <= x_end {
                for x in x_start..=x_end {
                    canvas.put_pixel(x as u32, y as u32, rgba);
                }
            }
            i += 2;
        }
    }
}
