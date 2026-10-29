use super::*;
use crate::compute::{Backend, ExecutionContext};
use crate::image::Mode;

mod native;
use native::{pad_native_bytes, pad_native_rows};

#[derive(Clone, Copy)]
struct PadPlan {
    width: u32,
    height: u32,
    source_width: u32,
    source_height: u32,
    content_width: u32,
    content_height: u32,
    fill: (u8, u8, u8, u8),
    resize_filter: ResampleFilter,
    native_identity: bool,
}

pub fn op_pad(
    img: &DynamicImage,
    w: u32,
    h: u32,
    filter: ResampleFilter,
    color: Option<(u8, u8, u8, u8)>,
    centering: (f64, f64),
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    op_pad_in_context(
        &ExecutionContext::for_backend(Backend::Cpu),
        img,
        w,
        h,
        filter,
        color,
        centering,
        explicit_mode,
    )
}

pub(crate) fn op_pad_in_context(
    context: &ExecutionContext,
    img: &DynamicImage,
    w: u32,
    h: u32,
    filter: ResampleFilter,
    color: Option<(u8, u8, u8, u8)>,
    centering: (f64, f64),
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let plan = build_pad_plan(img, w, h, filter, color, explicit_mode)?;
    if let Some(padded) = pad_identity_source(img, &plan, centering, explicit_mode)? {
        return Ok(padded);
    }

    if let Some(padded) = pad_luma_resize_into_canvas(img, &plan, centering, explicit_mode)? {
        return Ok(padded);
    }
    if let Some(padded) = pad_rgbx_resize_into_canvas(img, &plan, centering, explicit_mode)? {
        return Ok(padded);
    }

    let resized_storage = resize_pad_content(context, img, &plan, explicit_mode)?;
    let resized = resized_storage.as_ref().unwrap_or(img);
    if plan.content_width == plan.width && plan.content_height == plan.height {
        return Ok(preserve_mode(img, resized.clone()));
    }
    if let Some(padded) = pad_indexed_content(resized, &plan, centering, explicit_mode) {
        return Ok(padded);
    }
    pad_resized_content(img, resized, &plan, centering, explicit_mode)
}

fn build_pad_plan(
    img: &DynamicImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    color: Option<(u8, u8, u8, u8)>,
    explicit_mode: Option<&str>,
) -> Result<PadPlan, PilError> {
    let has_alpha = matches!(
        img.color(),
        crate::raster::ColorType::Rgba8 | crate::raster::ColorType::La8
    );
    let zero_fourth_default = matches!(explicit_mode, Some("RGBX" | "CMYK"));
    let default_fill = if has_alpha || zero_fourth_default {
        (0, 0, 0, 0)
    } else {
        (0, 0, 0, 255)
    };
    let fill = color.unwrap_or(default_fill);
    let (source_width, source_height) = (img.width(), img.height());
    let image_ratio = source_width as f64 / source_height as f64;
    let target_ratio = width as f64 / height as f64;
    let (content_width, content_height) = contained_size(
        source_width,
        source_height,
        width,
        height,
        image_ratio,
        target_ratio,
    );
    validate_contained_size(source_width, source_height, content_width, content_height)?;
    let resize_filter = if explicit_mode == Some("P") {
        ResampleFilter::Nearest
    } else {
        filter
    };
    let native_identity = native_identity_layout(img, explicit_mode)
        && source_width == content_width
        && source_height == content_height;
    Ok(PadPlan {
        width,
        height,
        source_width,
        source_height,
        content_width,
        content_height,
        fill,
        resize_filter,
        native_identity,
    })
}

fn contained_size(
    source_width: u32,
    source_height: u32,
    width: u32,
    height: u32,
    image_ratio: f64,
    target_ratio: f64,
) -> (u32, u32) {
    if (image_ratio - target_ratio).abs() < 1e-10 {
        (width, height)
    } else if image_ratio > target_ratio {
        (
            width,
            bankers_round(source_height as f64 / source_width as f64 * width as f64) as u32,
        )
    } else {
        (
            bankers_round(source_width as f64 / source_height as f64 * height as f64) as u32,
            height,
        )
    }
}

fn validate_contained_size(
    source_width: u32,
    source_height: u32,
    content_width: u32,
    content_height: u32,
) -> Result<(), PilError> {
    if source_width != 0 && source_height != 0 && (content_width == 0 || content_height == 0) {
        return Err(PilError::ValueError("height and width must be > 0".into()));
    }
    if source_width == 0 && source_height != 0 && content_height != source_height {
        return Err(PilError::ValueError("height and width must be > 0".into()));
    }
    Ok(())
}

fn native_identity_layout(img: &DynamicImage, explicit_mode: Option<&str>) -> bool {
    matches!(
        (explicit_mode, img),
        (None | Some("L"), DynamicImage::ImageLuma8(_))
            | (None | Some("LA"), DynamicImage::ImageLumaA8(_))
            | (None | Some("RGB"), DynamicImage::ImageRgb8(_))
            | (Some("HSV"), DynamicImage::ImageRgb8(_))
            | (None | Some("RGBA"), DynamicImage::ImageRgba8(_))
            | (Some("RGBX"), DynamicImage::ImageRgba8(_))
            | (Some("CMYK"), DynamicImage::ImageRgba8(_))
    )
}

fn pad_identity_source(
    img: &DynamicImage,
    plan: &PadPlan,
    centering: (f64, f64),
    explicit_mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let source_nonempty = plan.source_width != 0 && plan.source_height != 0;
    let content_is_source =
        plan.content_width == plan.source_width && plan.content_height == plan.source_height;
    let has_border = plan.content_width != plan.width || plan.content_height != plan.height;
    if source_nonempty
        && content_is_source
        && has_border
        && !matches!(explicit_mode, Some("P" | "PA"))
    {
        return pad_native_bytes(
            img,
            img,
            plan.width,
            plan.height,
            plan.content_width,
            plan.content_height,
            plan.fill,
            centering,
            explicit_mode,
        );
    }
    Ok(None)
}

fn pad_luma_resize_into_canvas(
    img: &DynamicImage,
    plan: &PadPlan,
    centering: (f64, f64),
    explicit_mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    if !matches!(explicit_mode, None | Some("L"))
        || !matches!(img, DynamicImage::ImageLuma8(_))
        || matches!(plan.resize_filter, ResampleFilter::Nearest)
        || (plan.source_width, plan.source_height) == (plan.content_width, plan.content_height)
        || plan.content_width != plan.width
        || plan.content_height >= plan.height
    {
        return Ok(None);
    }
    let canvas = CheckedDims::new(plan.width, plan.height, 1)?;
    let content = CheckedDims::new(plan.content_width, plan.content_height, 1)?;
    let offset_y = bankers_round(
        (f64::from(plan.height) - f64::from(plan.content_height)) * centering.1.clamp(0.0, 1.0),
    ) as usize;
    let window = resize_window(canvas, content, offset_y, "L")?;
    let mut output = vec![plan.fill.0; canvas.total_bytes()];
    if pil_resize_luma_i32_into_window(
        img,
        plan.content_width,
        plan.content_height,
        plan.resize_filter,
        &mut output[window],
    ) {
        let result = crate::image_utils::raw_bytes_to_image(plan.width, plan.height, output, 1)?;
        return Ok(Some(preserve_mode(img, result)));
    }
    Ok(None)
}

fn pad_rgbx_resize_into_canvas(
    img: &DynamicImage,
    plan: &PadPlan,
    centering: (f64, f64),
    explicit_mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    if explicit_mode != Some("RGBX")
        || !matches!(img, DynamicImage::ImageRgba8(_))
        || matches!(plan.resize_filter, ResampleFilter::Nearest)
        || (plan.source_width, plan.source_height) == (plan.content_width, plan.content_height)
        || plan.content_width != plan.width
        || plan.content_height >= plan.height
    {
        return Ok(None);
    }
    let canvas = CheckedDims::new(plan.width, plan.height, 4)?;
    let content = CheckedDims::new(plan.content_width, plan.content_height, 4)?;
    let offset_y = bankers_round(
        (f64::from(plan.height) - f64::from(plan.content_height)) * centering.1.clamp(0.0, 1.0),
    ) as usize;
    let window = resize_window(canvas, content, offset_y, "RGBX")?;
    let fill_row =
        [plan.fill.0, plan.fill.1, plan.fill.2, plan.fill.3].repeat(canvas.width as usize);
    let mut output = fill_row.repeat(canvas.height as usize);
    if pil_resize_rgbx_into_window(
        img,
        plan.content_width,
        plan.content_height,
        plan.resize_filter,
        &mut output[window],
    ) {
        let result = crate::image_utils::raw_bytes_to_image(plan.width, plan.height, output, 4)?;
        return Ok(Some(preserve_mode(img, result)));
    }
    Ok(None)
}

fn resize_window(
    canvas: CheckedDims,
    content: CheckedDims,
    offset_y: usize,
    mode: &str,
) -> Result<std::ops::Range<usize>, PilError> {
    let start = offset_y
        .checked_mul(canvas.row_stride())
        .ok_or_else(|| PilError::DimensionError(format!("{mode} pad offset overflow")))?;
    start
        .checked_add(content.total_bytes())
        .filter(|end| *end <= canvas.total_bytes())
        .map(|end| start..end)
        .ok_or_else(|| PilError::DimensionError(format!("{mode} pad window overflow")))
}

fn resize_pad_content(
    context: &ExecutionContext,
    img: &DynamicImage,
    plan: &PadPlan,
    explicit_mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let (new_width, new_height) = (plan.content_width, plan.content_height);
    if new_width == 0 || new_height == 0 {
        return Ok(Some(preserve_mode(
            img,
            pil_resize(
                img,
                new_width,
                new_height,
                plan.resize_filter,
                explicit_mode,
            ),
        )));
    }
    if plan.native_identity {
        return Ok(None);
    }
    let source = if explicit_mode == Some("F") && matches!(img, DynamicImage::ImageRgba8(_)) {
        execute_resize_in_context(
            context,
            img,
            new_width.max(1),
            new_height.max(1),
            &plan.resize_filter,
            explicit_mode
                .and_then(Mode::from_pillow_name)
                .unwrap_or_else(|| Mode::for_image(img, None)),
        )?
    } else {
        pil_resize(
            img,
            new_width.max(1),
            new_height.max(1),
            plan.resize_filter,
            explicit_mode,
        )
    };
    Ok(Some(source))
}

fn pad_indexed_content(
    resized: &DynamicImage,
    plan: &PadPlan,
    centering: (f64, f64),
    explicit_mode: Option<&str>,
) -> Option<DynamicImage> {
    match explicit_mode {
        Some("P") => {
            let source = resized.to_luma8();
            let fill = plan.fill.0;
            let mut padded = crate::raster::GrayImage::from_pixel(
                plan.width,
                plan.height,
                crate::raster::Luma([fill]),
            );
            let (offset_x, offset_y) = pad_offsets(plan, centering);
            for y in 0..plan.content_height.min(plan.height) {
                for x in 0..plan.content_width.min(plan.width) {
                    let (target_x, target_y) = (offset_x + x, offset_y + y);
                    if target_x < plan.width && target_y < plan.height {
                        padded.put_pixel(target_x, target_y, *source.get_pixel(x, y));
                    }
                }
            }
            Some(DynamicImage::ImageLuma8(padded))
        }
        Some("PA") => {
            let source = resized.to_luma_alpha8();
            let fill = [plan.fill.0, plan.fill.3];
            let mut padded = crate::raster::GrayAlphaImage::from_pixel(
                plan.width,
                plan.height,
                crate::raster::LumaA(fill),
            );
            let (offset_x, offset_y) = pad_offsets(plan, centering);
            for y in 0..plan.content_height.min(plan.height) {
                for x in 0..plan.content_width.min(plan.width) {
                    let (target_x, target_y) = (offset_x + x, offset_y + y);
                    if target_x < plan.width && target_y < plan.height {
                        padded.put_pixel(target_x, target_y, *source.get_pixel(x, y));
                    }
                }
            }
            Some(DynamicImage::ImageLumaA8(padded))
        }
        _ => None,
    }
}

fn pad_offsets(plan: &PadPlan, centering: (f64, f64)) -> (u32, u32) {
    if plan.content_width != plan.width {
        (
            bankers_round(
                (f64::from(plan.width) - f64::from(plan.content_width))
                    * centering.0.clamp(0.0, 1.0),
            ) as u32,
            0,
        )
    } else {
        (
            0,
            bankers_round(
                (f64::from(plan.height) - f64::from(plan.content_height))
                    * centering.1.clamp(0.0, 1.0),
            ) as u32,
        )
    }
}

fn pad_resized_content(
    original: &DynamicImage,
    resized: &DynamicImage,
    plan: &PadPlan,
    centering: (f64, f64),
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (offset_x, offset_y) = pad_offsets(plan, centering);
    if let Some(native_output) = pad_native_rows(
        original,
        resized,
        plan.width,
        plan.height,
        plan.content_width,
        plan.content_height,
        offset_x as usize,
        offset_y as usize,
        plan.fill,
        explicit_mode,
    )? {
        return Ok(native_output);
    }
    pad_rgba_compat(
        original,
        resized,
        plan,
        offset_x as usize,
        offset_y as usize,
    )
}

fn pad_rgba_compat(
    original: &DynamicImage,
    resized: &DynamicImage,
    plan: &PadPlan,
    offset_x: usize,
    offset_y: usize,
) -> Result<DynamicImage, PilError> {
    // TODO(pillow-rs#3ptb): Replace this widening path with native per-mode
    // fill and row-copy strategies; Pad must not convert solely to reuse an
    // RGBA canvas.
    let source_rgba = resized.to_rgba8();
    let width = plan.width as usize;
    let height = plan.height as usize;
    let copy_width = plan.content_width.min(plan.width) as usize;
    let copy_height = plan.content_height.min(plan.height) as usize;
    let output_stride = width.saturating_mul(4);
    let source_stride = plan.content_width as usize * 4;
    let fill = [plan.fill.0, plan.fill.1, plan.fill.2, plan.fill.3];
    let mut output = CheckedDims::new(plan.width, plan.height, 4)?.alloc_buffer();
    fill_rgba_pad_canvas(&mut output, output_stride, width, height, fill);
    copy_rgba_pad_content(
        &mut output,
        output_stride,
        width,
        height,
        source_rgba.as_raw(),
        source_stride,
        offset_x,
        offset_y,
        copy_width,
        copy_height,
    );
    let padded = crate::image_utils::raw_bytes_to_image(plan.width, plan.height, output, 4)?;
    Ok(preserve_mode(original, padded))
}

fn fill_rgba_pad_canvas(
    output: &mut [u8],
    output_stride: usize,
    _width: usize,
    _height: usize,
    fill: [u8; 4],
) {
    for row in output.chunks_exact_mut(output_stride) {
        fill_rgba_pad_row(row, fill);
    }
}

#[inline]
fn fill_rgba_pad_row(row: &mut [u8], fill: [u8; 4]) {
    for pixel in row.chunks_exact_mut(4) {
        pixel.copy_from_slice(&fill);
    }
}

fn copy_rgba_pad_content(
    output: &mut [u8],
    output_stride: usize,
    _width: usize,
    _height: usize,
    source: &[u8],
    source_stride: usize,
    offset_x: usize,
    offset_y: usize,
    copy_width: usize,
    copy_height: usize,
) {
    copy_rgba_pad_rows(
        output,
        output_stride,
        source,
        source_stride,
        offset_x,
        offset_y,
        copy_width,
        copy_height,
    );
}

fn copy_rgba_pad_rows(
    output: &mut [u8],
    output_stride: usize,
    source: &[u8],
    source_stride: usize,
    offset_x: usize,
    offset_y: usize,
    copy_width: usize,
    copy_height: usize,
) {
    for output_y in offset_y..offset_y.saturating_add(copy_height) {
        let source_start = (output_y - offset_y) * source_stride;
        let output_start = output_y * output_stride + offset_x * 4;
        let byte_count = copy_width * 4;
        output[output_start..output_start + byte_count]
            .copy_from_slice(&source[source_start..source_start + byte_count]);
    }
}

#[cfg(test)]
mod tests;
