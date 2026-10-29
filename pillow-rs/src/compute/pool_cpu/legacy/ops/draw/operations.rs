use super::arcs::{draw_arc_on_canvas, draw_chord_on_canvas, draw_pieslice_on_canvas};
use super::canvas::draw_native;
use super::ellipse::draw_ellipse_on_canvas;
use super::polygon::draw_polygon_on_canvas;
use super::primitives::{draw_line_on_canvas, draw_rect_on_canvas};
use super::rounded_rect::{draw_rounded_rect_on_canvas, validate_rounded_rect};
use crate::draw::{DrawCanvas, plot};
use crate::error::PilError;
use crate::raster::DynamicImage;

pub(crate) fn op_draw_line(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    fill: (u8, u8, u8, u8),
    width: u32,
    alpha_blend_rgb: bool,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    Ok(draw_native(img, alpha_blend_rgb, mode, |canvas| {
        draw_line_on_canvas(canvas, x0, y0, x1, y1, fill, width);
    }))
}

pub(crate) fn op_draw_rectangle(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    width: u32,
    alpha_blend_rgb: bool,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    Ok(draw_native(img, alpha_blend_rgb, mode, |canvas| {
        canvas.set_l16_ink_order(mode, false);
        draw_rect_on_canvas(canvas, x0, y0, x1, y1, fill, outline, width);
    }))
}

pub(crate) fn op_draw_rounded_rect(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    radius: f64,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    width: u32,
    alpha_blend_rgb: bool,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    validate_rounded_rect(x0, y0, x1, y1)?;
    Ok(draw_native(img, alpha_blend_rgb, mode, |canvas| {
        canvas.set_l16_ink_order(mode, false);
        draw_rounded_rect_on_canvas(canvas, x0, y0, x1, y1, radius, fill, outline, width);
    }))
}

pub(crate) fn op_draw_ellipse(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    width: u32,
    alpha_blend_rgb: bool,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    Ok(draw_native(img, alpha_blend_rgb, mode, |canvas| {
        canvas.set_l16_ink_order(mode, false);
        draw_ellipse_on_canvas(canvas, x0, y0, x1, y1, fill, outline, width);
    }))
}

pub(crate) fn op_draw_circle(
    img: &DynamicImage,
    cx: i32,
    cy: i32,
    radius: i32,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    width: u32,
    alpha_blend_rgb: bool,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    op_draw_ellipse(
        img,
        cx - radius,
        cy - radius,
        cx + radius,
        cy + radius,
        fill,
        outline,
        width,
        alpha_blend_rgb,
        mode,
    )
}

pub(crate) fn op_draw_polygon(
    img: &DynamicImage,
    points: &[(i32, i32)],
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    width: u32,
    alpha_blend_rgb: bool,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let pts = points.to_vec();
    // Pillow's floating-point ImagingDraw polygon path accepts an outline
    // argument but stores only the fill sample.  Preserve that observable
    // mode-specific behavior before entering the generic native canvas.
    let outline = if mode == Some("F") { None } else { outline };
    Ok(draw_native(img, alpha_blend_rgb, mode, |canvas| {
        canvas.set_l16_ink_order(mode, false);
        draw_polygon_on_canvas(canvas, &pts, fill, outline, width);
    }))
}

pub(crate) fn op_draw_arc(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    start: f64,
    end: f64,
    fill: Option<(u8, u8, u8, u8)>,
    width: u32,
    alpha_blend_rgb: bool,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let Some(fc) = fill else {
        return Ok(img.clone());
    };
    Ok(draw_native(img, alpha_blend_rgb, mode, |canvas| {
        canvas.set_l16_ink_order(mode, false);
        draw_arc_on_canvas(canvas, x0, y0, x1, y1, start, end, fc, width);
    }))
}

pub(crate) fn op_draw_chord(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    start: f64,
    end: f64,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    width: u32,
    alpha_blend_rgb: bool,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    Ok(draw_native(img, alpha_blend_rgb, mode, |canvas| {
        canvas.set_l16_ink_order(mode, false);
        draw_chord_on_canvas(canvas, x0, y0, x1, y1, start, end, fill, outline, width);
    }))
}

pub(crate) fn op_draw_pieslice(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    start: f64,
    end: f64,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    width: u32,
    alpha_blend_rgb: bool,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    Ok(draw_native(img, alpha_blend_rgb, mode, |canvas| {
        canvas.set_l16_ink_order(mode, false);
        draw_pieslice_on_canvas(canvas, x0, y0, x1, y1, start, end, fill, outline, width);
    }))
}

pub(crate) fn op_draw_point(
    img: &DynamicImage,
    points: &[(i32, i32)],
    fill: (u8, u8, u8, u8),
    alpha_blend_rgb: bool,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let pts = points.to_vec();
    Ok(draw_native(img, alpha_blend_rgb, mode, |canvas| {
        let (img_w, img_h) = (canvas.width(), canvas.height());
        for &(x, y) in &pts {
            plot(canvas, x, y, fill, img_w, img_h, false);
        }
    }))
}
