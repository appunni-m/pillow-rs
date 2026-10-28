use super::*;

struct NativePadRowInput<'a> {
    source: &'a [u8],
    channels: usize,
    fill_pixel: [u8; 4],
}

struct NativePadRowLayout {
    width: usize,
    height: usize,
    source_stride: usize,
    output_stride: usize,
    copy_height: usize,
    offset_y_end: usize,
    byte_count: usize,
    output_x_bytes: usize,
    full_width_rows: bool,
    append_vertical_rows: bool,
    output_dims: CheckedDims,
}

pub(super) fn pad_native_rows(
    img: &DynamicImage,
    resized: &DynamicImage,
    w: u32,
    h: u32,
    nw: u32,
    nh: u32,
    offset_x: usize,
    offset_y: usize,
    fill: (u8, u8, u8, u8),
    explicit_mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(input) = native_pad_row_input(img, resized, fill, explicit_mode) else {
        return Ok(None);
    };
    let Some(layout) = native_pad_row_layout(
        input.source.len(),
        w,
        h,
        nw,
        nh,
        offset_x,
        offset_y,
        input.channels,
        explicit_mode,
    )?
    else {
        return Ok(None);
    };
    let Some(mut output) = allocate_native_pad_rows(&layout, &input.fill_pixel[..input.channels])
    else {
        return Ok(None);
    };
    if !write_native_pad_rows(
        &mut output,
        input.source,
        &layout,
        &input.fill_pixel[..input.channels],
    ) {
        return Ok(None);
    }
    let image = crate::image_utils::raw_bytes_to_image(w, h, output, input.channels)?;
    Ok(Some(image))
}

fn native_pad_row_input<'a>(
    img: &'a DynamicImage,
    resized: &'a DynamicImage,
    fill: (u8, u8, u8, u8),
    explicit_mode: Option<&str>,
) -> Option<NativePadRowInput<'a>> {
    let (source, channels, fill_pixel) = match (img, resized, explicit_mode) {
        (DynamicImage::ImageLuma8(_), DynamicImage::ImageLuma8(source), None | Some("L")) => {
            (source.as_raw(), 1, [fill.0, 0, 0, 0])
        }
        (DynamicImage::ImageLumaA8(_), DynamicImage::ImageLumaA8(source), None | Some("LA")) => {
            (source.as_raw(), 2, [fill.0, fill.3, 0, 0])
        }
        (DynamicImage::ImageRgb8(_), DynamicImage::ImageRgb8(source), None | Some("RGB")) => {
            (source.as_raw(), 3, [fill.0, fill.1, fill.2, 0])
        }
        // HSV shares RGB storage; copy its raw samples without color conversion.
        (DynamicImage::ImageRgb8(_), DynamicImage::ImageRgb8(source), Some("HSV")) => {
            (source.as_raw(), 3, [fill.0, fill.1, fill.2, 0])
        }
        (DynamicImage::ImageRgba8(_), DynamicImage::ImageRgba8(source), None | Some("RGBA")) => {
            (source.as_raw(), 4, [fill.0, fill.1, fill.2, fill.3])
        }
        // RGBX's fourth byte is padding and CMYK's fourth byte is K.
        (DynamicImage::ImageRgba8(_), DynamicImage::ImageRgba8(source), Some("RGBX" | "CMYK")) => {
            (source.as_raw(), 4, [fill.0, fill.1, fill.2, fill.3])
        }
        _ => return None,
    };
    Some(NativePadRowInput {
        source,
        channels,
        fill_pixel,
    })
}

fn native_pad_row_layout(
    source_len: usize,
    w: u32,
    h: u32,
    nw: u32,
    nh: u32,
    offset_x: usize,
    offset_y: usize,
    channels: usize,
    explicit_mode: Option<&str>,
) -> Result<Option<NativePadRowLayout>, PilError> {
    let Some((source_width, source_height, width, height)) =
        native_pad_nonzero_dimensions(w, h, nw, nh)
    else {
        return Ok(None);
    };
    let Some((source_stride, output_stride)) =
        native_pad_row_strides(source_len, source_width, source_height, width, channels)
    else {
        return Ok(None);
    };
    let Some((copy_width, copy_height, offset_y_end, byte_count, output_x_bytes)) =
        native_pad_row_copy_region(w, h, nw, nh, offset_x, offset_y, width, height, channels)
    else {
        return Ok(None);
    };
    let output_dims = CheckedDims::new(w, h, channels as u8)?;
    let full_width_rows =
        output_x_bytes == 0 && copy_width == width && source_stride == output_stride;
    let append_vertical_rows = full_width_rows
        && (matches!(explicit_mode, Some("CMYK" | "RGBX"))
            || (channels == 1 && matches!(explicit_mode, None | Some("L"))));
    Ok(Some(NativePadRowLayout {
        width,
        height,
        source_stride,
        output_stride,
        copy_height,
        offset_y_end,
        byte_count,
        output_x_bytes,
        full_width_rows,
        append_vertical_rows,
        output_dims,
    }))
}

fn native_pad_nonzero_dimensions(
    w: u32,
    h: u32,
    nw: u32,
    nh: u32,
) -> Option<(usize, usize, usize, usize)> {
    if w == 0 || h == 0 || nw == 0 || nh == 0 {
        return None;
    }
    Some((
        usize::try_from(nw).ok()?,
        usize::try_from(nh).ok()?,
        usize::try_from(w).ok()?,
        usize::try_from(h).ok()?,
    ))
}

fn native_pad_row_strides(
    source_len: usize,
    source_width: usize,
    source_height: usize,
    width: usize,
    channels: usize,
) -> Option<(usize, usize)> {
    let source_stride = source_width.checked_mul(channels)?;
    if source_len != source_stride.checked_mul(source_height)? {
        return None;
    }
    Some((source_stride, width.checked_mul(channels)?))
}

fn native_pad_row_copy_region(
    w: u32,
    h: u32,
    nw: u32,
    nh: u32,
    offset_x: usize,
    offset_y: usize,
    width: usize,
    height: usize,
    channels: usize,
) -> Option<(usize, usize, usize, usize, usize)> {
    let copy_width = nw.min(w) as usize;
    let copy_height = nh.min(h) as usize;
    let offset_x_end = offset_x.checked_add(copy_width)?;
    let offset_y_end = offset_y.checked_add(copy_height)?;
    if offset_x_end > width || offset_y_end > height {
        return None;
    }
    Some((
        copy_width,
        copy_height,
        offset_y_end,
        copy_width.checked_mul(channels)?,
        offset_x.checked_mul(channels)?,
    ))
}

fn allocate_native_pad_rows(layout: &NativePadRowLayout, fill_pixel: &[u8]) -> Option<Vec<u8>> {
    let fill_row = fill_pixel.repeat(layout.width);
    if fill_row.len() != layout.output_stride {
        return None;
    }
    let output = if layout.append_vertical_rows {
        Vec::with_capacity(layout.output_dims.total_bytes())
    } else {
        fill_row.repeat(layout.height)
    };
    if !layout.append_vertical_rows && output.len() != layout.output_dims.total_bytes() {
        return None;
    }
    Some(output)
}

fn write_native_pad_rows(
    output: &mut Vec<u8>,
    source: &[u8],
    layout: &NativePadRowLayout,
    fill_pixel: &[u8],
) -> bool {
    if layout.full_width_rows {
        return write_full_width_native_pad_rows(output, source, layout, fill_pixel);
    }
    write_offset_native_pad_rows(output, source, layout)
}

fn write_full_width_native_pad_rows(
    output: &mut Vec<u8>,
    source: &[u8],
    layout: &NativePadRowLayout,
    fill_pixel: &[u8],
) -> bool {
    let Some(source_byte_count) = layout.source_stride.checked_mul(layout.copy_height) else {
        return false;
    };
    if source_byte_count > source.len() {
        return false;
    }
    if layout.append_vertical_rows {
        let fill_row = fill_pixel.repeat(layout.width);
        for _ in 0..layout.offset_y_end - layout.copy_height {
            output.extend_from_slice(&fill_row);
        }
        output.extend_from_slice(&source[..source_byte_count]);
        for _ in layout.offset_y_end..layout.height {
            output.extend_from_slice(&fill_row);
        }
        return output.len() == layout.output_dims.total_bytes();
    }
    let output_start = (layout.offset_y_end - layout.copy_height) * layout.output_stride;
    let output_end = output_start + source_byte_count;
    output[output_start..output_end].copy_from_slice(&source[..source_byte_count]);
    true
}

fn write_offset_native_pad_rows(
    output: &mut [u8],
    source: &[u8],
    layout: &NativePadRowLayout,
) -> bool {
    for (y, row) in output.chunks_exact_mut(layout.output_stride).enumerate() {
        copy_native_pad_row(row, y, source, layout);
    }
    true
}

fn copy_native_pad_row(
    row: &mut [u8],
    output_y: usize,
    source: &[u8],
    layout: &NativePadRowLayout,
) {
    if output_y < layout.offset_y_end - layout.copy_height || output_y >= layout.offset_y_end {
        return;
    }
    let source_y = output_y - (layout.offset_y_end - layout.copy_height);
    let source_start = source_y * layout.source_stride;
    let output_end = layout.output_x_bytes + layout.byte_count;
    row[layout.output_x_bytes..output_end]
        .copy_from_slice(&source[source_start..source_start + layout.byte_count]);
}

#[inline]
fn fill_pad_span(span: &mut [u8], fill: &[u8]) {
    if fill.iter().all(|&sample| sample == 0) {
        // The output allocation is zero-initialized, so black L/RGB borders
        // are already correct. In particular, do not write a second time to
        // the native zero-fill fast path.
        return;
    }
    for pixel in span.chunks_exact_mut(fill.len()) {
        pixel.copy_from_slice(fill);
    }
}

#[inline]
fn pad_native_row(
    row: &mut [u8],
    output_y: usize,
    source: &[u8],
    source_stride: usize,
    offset_y: usize,
    offset_x_bytes: usize,
    copy_height: usize,
    copy_byte_count: usize,
    fill: &[u8],
) {
    if output_y < offset_y || output_y - offset_y >= copy_height {
        fill_pad_span(row, fill);
        return;
    }

    let content_end = offset_x_bytes + copy_byte_count;
    fill_pad_span(&mut row[..offset_x_bytes], fill);
    let source_start = (output_y - offset_y) * source_stride;
    row[offset_x_bytes..content_end]
        .copy_from_slice(&source[source_start..source_start + copy_byte_count]);
    fill_pad_span(&mut row[content_end..], fill);
}

/// Fill only the border spans and copy the contained pixels in one row pass.
/// Keeping native channels avoids RGBA expansion for L/LA/RGB, and writing
/// each destination byte once reduces the bandwidth of large pads.
pub(super) fn pad_native_bytes(
    original: &DynamicImage,
    resized: &DynamicImage,
    w: u32,
    h: u32,
    nw: u32,
    nh: u32,
    color: (u8, u8, u8, u8),
    centering: (f64, f64),
    explicit_mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_pad_byte_channels(original, resized, explicit_mode) else {
        return Ok(None);
    };
    if w == 0 || h == 0 {
        return Ok(None);
    }
    let Some(layout) = native_pad_byte_layout(resized, w, h, nw, nh, color, centering, channels)?
    else {
        return Ok(None);
    };
    match append_native_vertical_rows(original, &layout, explicit_mode)? {
        NativePadAppend::Ready(image) => return Ok(Some(image)),
        NativePadAppend::Invalid => return Ok(None),
        NativePadAppend::NotApplicable => {}
    }
    let mut output = layout.output_dims.alloc_buffer();
    write_native_pad_byte_rows(&mut output, &layout);
    let padded = crate::image_utils::raw_bytes_to_image(w, h, output, channels)?;
    Ok(Some(preserve_mode(original, padded)))
}

fn native_pad_byte_channels(
    original: &DynamicImage,
    resized: &DynamicImage,
    explicit_mode: Option<&str>,
) -> Option<usize> {
    // Storage alone does not identify semantics: RGBA also carries F/I, while
    // RGBX and CMYK use distinct meanings for their fourth byte.
    match (original, resized, explicit_mode) {
        (DynamicImage::ImageLuma8(_), DynamicImage::ImageLuma8(_), None | Some("L")) => Some(1),
        (DynamicImage::ImageLumaA8(_), DynamicImage::ImageLumaA8(_), None | Some("LA")) => Some(2),
        (DynamicImage::ImageRgb8(_), DynamicImage::ImageRgb8(_), None | Some("RGB" | "HSV")) => {
            Some(3)
        }
        (
            DynamicImage::ImageRgba8(_),
            DynamicImage::ImageRgba8(_),
            None | Some("RGBA" | "RGBX" | "CMYK"),
        ) => Some(4),
        _ => None,
    }
}

struct NativePadBytesLayout<'a> {
    source: &'a [u8],
    width: usize,
    height: usize,
    source_stride: usize,
    output_stride: usize,
    copy_width: usize,
    copy_height: usize,
    offset_x_bytes: usize,
    offset_y: usize,
    copy_byte_count: usize,
    copy_end_y: usize,
    channels: usize,
    fill: [u8; 4],
    output_dims: CheckedDims,
}

fn native_pad_byte_source_stride(
    source_len: usize,
    source_width: usize,
    source_height: usize,
    channels: usize,
) -> Result<Option<usize>, PilError> {
    let Some(source_stride) = source_width.checked_mul(channels) else {
        return Err(PilError::ValueError("pad source stride overflow".into()));
    };
    let Some(expected_source_len) = source_stride.checked_mul(source_height) else {
        return Err(PilError::ValueError("pad source length overflow".into()));
    };
    Ok((source_len == expected_source_len).then_some(source_stride))
}

fn native_pad_byte_layout<'a>(
    resized: &'a DynamicImage,
    w: u32,
    h: u32,
    nw: u32,
    nh: u32,
    color: (u8, u8, u8, u8),
    centering: (f64, f64),
    channels: usize,
) -> Result<Option<NativePadBytesLayout<'a>>, PilError> {
    let width =
        usize::try_from(w).map_err(|_| PilError::ValueError("pad width overflow".into()))?;
    let height =
        usize::try_from(h).map_err(|_| PilError::ValueError("pad height overflow".into()))?;
    let source_width = usize::try_from(nw)
        .map_err(|_| PilError::ValueError("pad source width overflow".into()))?;
    let source_height = usize::try_from(nh)
        .map_err(|_| PilError::ValueError("pad source height overflow".into()))?;
    let source = resized.as_bytes();
    let Some(source_stride) =
        native_pad_byte_source_stride(source.len(), source_width, source_height, channels)?
    else {
        return Ok(None);
    };
    let copy_width = source_width.min(width);
    let copy_height = source_height.min(height);
    let (offset_x, offset_y) = native_pad_offsets(w, h, nw, nh, centering);
    let Some(offset_x_bytes) = offset_x.checked_mul(channels) else {
        return Err(PilError::ValueError(
            "pad destination offset overflow".into(),
        ));
    };
    let Some(copy_byte_count) = copy_width.checked_mul(channels) else {
        return Err(PilError::ValueError("pad copy length overflow".into()));
    };
    let Some(output_stride) = width.checked_mul(channels) else {
        return Err(PilError::ValueError("pad output stride overflow".into()));
    };
    let Some(content_end) = offset_x_bytes.checked_add(copy_byte_count) else {
        return Ok(None);
    };
    let Some(copy_end_y) = offset_y.checked_add(copy_height) else {
        return Ok(None);
    };
    if content_end > output_stride || copy_end_y > height {
        return Ok(None);
    }
    let Some(fill) = native_pad_fill(color, channels) else {
        return Ok(None);
    };
    let output_dims = CheckedDims::new(w, h, channels as u8)?;
    Ok(Some(NativePadBytesLayout {
        source,
        width,
        height,
        source_stride,
        output_stride,
        copy_width,
        copy_height,
        offset_x_bytes,
        offset_y,
        copy_byte_count,
        copy_end_y,
        channels,
        fill,
        output_dims,
    }))
}

fn native_pad_offsets(w: u32, h: u32, nw: u32, nh: u32, centering: (f64, f64)) -> (usize, usize) {
    let (cx, cy) = (centering.0.clamp(0.0, 1.0), centering.1.clamp(0.0, 1.0));
    if nw != w {
        (bankers_round((w as f64 - nw as f64) * cx) as usize, 0)
    } else {
        (0, bankers_round((h as f64 - nh as f64) * cy) as usize)
    }
}

fn native_pad_fill(color: (u8, u8, u8, u8), channels: usize) -> Option<[u8; 4]> {
    match channels {
        1 => Some([color.0, 0, 0, 0]),
        2 => Some([color.0, color.3, 0, 0]),
        3 => Some([color.0, color.1, color.2, 0]),
        4 => Some([color.0, color.1, color.2, color.3]),
        _ => None,
    }
}

enum NativePadAppend {
    NotApplicable,
    Invalid,
    Ready(DynamicImage),
}

fn append_native_vertical_rows(
    original: &DynamicImage,
    layout: &NativePadBytesLayout<'_>,
    explicit_mode: Option<&str>,
) -> Result<NativePadAppend, PilError> {
    if !append_native_mode(explicit_mode, layout.channels)
        || layout.offset_x_bytes != 0
        || layout.copy_width != layout.width
        || layout.source_stride != layout.output_stride
    {
        return Ok(NativePadAppend::NotApplicable);
    }
    let Some(source_bytes) = layout.source_stride.checked_mul(layout.copy_height) else {
        return Ok(NativePadAppend::Invalid);
    };
    if source_bytes > layout.source.len() {
        return Ok(NativePadAppend::Invalid);
    }
    let fill_row = layout.fill[..layout.channels].repeat(layout.width);
    if fill_row.len() != layout.output_stride {
        return Ok(NativePadAppend::Invalid);
    }
    let mut output = Vec::with_capacity(layout.output_dims.total_bytes());
    for _ in 0..layout.offset_y {
        output.extend_from_slice(&fill_row);
    }
    output.extend_from_slice(&layout.source[..source_bytes]);
    for _ in layout.copy_end_y..layout.height {
        output.extend_from_slice(&fill_row);
    }
    if output.len() != layout.output_dims.total_bytes() {
        return Ok(NativePadAppend::Invalid);
    }
    let padded = crate::image_utils::raw_bytes_to_image(
        layout.output_dims.width,
        layout.output_dims.height,
        output,
        layout.channels,
    )?;
    Ok(NativePadAppend::Ready(preserve_mode(original, padded)))
}

fn append_native_mode(explicit_mode: Option<&str>, channels: usize) -> bool {
    matches!(
        (explicit_mode, channels),
        (None | Some("L"), 1) | (Some("RGB"), 3) | (Some("HSV"), 3) | (Some("RGBA"), 4)
    )
}

fn write_native_pad_byte_rows(output: &mut [u8], layout: &NativePadBytesLayout<'_>) {
    write_native_pad_byte_rows_serial(output, layout);
}

fn write_native_pad_byte_rows_serial(output: &mut [u8], layout: &NativePadBytesLayout<'_>) {
    for (y, row) in output.chunks_exact_mut(layout.output_stride).enumerate() {
        pad_native_row(
            row,
            y,
            layout.source,
            layout.source_stride,
            layout.offset_y,
            layout.offset_x_bytes,
            layout.copy_height,
            layout.copy_byte_count,
            &layout.fill[..layout.channels],
        );
    }
}
