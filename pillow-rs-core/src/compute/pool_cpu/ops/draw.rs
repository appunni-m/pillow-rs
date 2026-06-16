//! ImageDraw CPU operations — delegate to existing Draw struct methods.

use crate::draw::Draw;
use crate::error::PilError;
use crate::image::Image;
use image::DynamicImage;

fn draw_on<F>(img: &DynamicImage, f: F) -> Result<DynamicImage, PilError>
where
    F: FnOnce(&mut Draw) -> Result<(), PilError>,
{
    let image = Image::Loaded(img.clone(), None);
    let mut draw = Draw::new(image, None);
    f(&mut draw)?;
    draw.into_image().materialize()
}

pub fn op_draw_line(
    img: &DynamicImage, x0: i32, y0: i32, x1: i32, y1: i32,
    fill: (u8, u8, u8, u8), width: u32,
) -> Result<DynamicImage, PilError> {
    draw_on(img, |draw| draw.line(x0, y0, x1, y1, fill, width))
}

pub fn op_draw_rectangle(
    img: &DynamicImage, x0: i32, y0: i32, x1: i32, y1: i32,
    fill: Option<(u8, u8, u8, u8)>, outline: Option<(u8, u8, u8, u8)>, width: u32,
) -> Result<DynamicImage, PilError> {
    draw_on(img, |draw| draw.rectangle(x0, y0, x1, y1, fill, outline, width))
}

pub fn op_draw_rounded_rect(
    img: &DynamicImage, x0: i32, y0: i32, x1: i32, y1: i32, radius: f64,
    fill: Option<(u8, u8, u8, u8)>, outline: Option<(u8, u8, u8, u8)>, width: u32,
) -> Result<DynamicImage, PilError> {
    draw_on(img, |draw| draw.rounded_rectangle(x0, y0, x1, y1, radius, fill, outline, width))
}

pub fn op_draw_ellipse(
    img: &DynamicImage, x0: i32, y0: i32, x1: i32, y1: i32,
    fill: Option<(u8, u8, u8, u8)>, outline: Option<(u8, u8, u8, u8)>, width: u32,
) -> Result<DynamicImage, PilError> {
    draw_on(img, |draw| draw.ellipse(x0, y0, x1, y1, fill, outline, width))
}

pub fn op_draw_circle(
    img: &DynamicImage, cx: i32, cy: i32, radius: i32,
    fill: Option<(u8, u8, u8, u8)>, outline: Option<(u8, u8, u8, u8)>, width: u32,
) -> Result<DynamicImage, PilError> {
    draw_on(img, |draw| draw.circle(cx, cy, radius as f64, fill, outline, width))
}

pub fn op_draw_polygon(
    img: &DynamicImage, points: &[(i32, i32)],
    fill: Option<(u8, u8, u8, u8)>, outline: Option<(u8, u8, u8, u8)>, width: u32,
) -> Result<DynamicImage, PilError> {
    draw_on(img, |draw| draw.polygon(points, fill, outline, width))
}

pub fn op_draw_arc(
    img: &DynamicImage, x0: i32, y0: i32, x1: i32, y1: i32,
    start: f64, end: f64, fill: Option<(u8, u8, u8, u8)>, width: u32,
) -> Result<DynamicImage, PilError> {
    draw_on(img, |draw| draw.arc(x0, y0, x1, y1, start, end, fill.unwrap_or((0, 0, 0, 255)), width))
}

pub fn op_draw_chord(
    img: &DynamicImage, x0: i32, y0: i32, x1: i32, y1: i32,
    start: f64, end: f64, fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>, width: u32,
) -> Result<DynamicImage, PilError> {
    draw_on(img, |draw| draw.chord(x0, y0, x1, y1, start, end, fill, outline, width))
}

pub fn op_draw_pieslice(
    img: &DynamicImage, x0: i32, y0: i32, x1: i32, y1: i32,
    start: f64, end: f64, fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>, width: u32,
) -> Result<DynamicImage, PilError> {
    draw_on(img, |draw| draw.pieslice(x0, y0, x1, y1, start, end, fill, outline, width))
}

pub fn op_draw_point(
    img: &DynamicImage, points: &[(i32, i32)], fill: (u8, u8, u8, u8),
) -> Result<DynamicImage, PilError> {
    draw_on(img, |draw| draw.point(points, fill))
}
