use crate::error::PilError;
use crate::font::{
    ImageFontTextBoundsCallbacks, ImageFontTextPositionArithmetic,
    ImageFontTextPositionCallbacks, ImageFontTextPositionComparison,
};
use crate::raster::Mode;

pub(crate) fn bounds_with_callbacks<C: ImageFontTextBoundsCallbacks>(
    callbacks: &mut C,
) -> Result<[C::Value; 4], C::Error> {
    let Some(first) = callbacks.next_line()? else {
        return callbacks.empty_bounds();
    };
    let mut bounds = offset_bounds(callbacks, &first)?;
    while let Some(line) = callbacks.next_line()? {
        let current = offset_bounds(callbacks, &line)?;
        let left = callbacks.minimum(&bounds[0], &current[0])?;
        let top = callbacks.minimum(&bounds[1], &current[1])?;
        let right = callbacks.maximum(&bounds[2], &current[2])?;
        let bottom = callbacks.maximum(&bounds[3], &current[3])?;
        bounds = [left, top, right, bottom];
    }
    Ok(bounds)
}

fn offset_bounds<C: ImageFontTextBoundsCallbacks>(
    callbacks: &mut C,
    line: &C::Value,
) -> Result<[C::Value; 4], C::Error> {
    callbacks.begin_line_bounds(line)?;
    let left = offset_coordinate(callbacks, line, 0)?;
    let top = offset_coordinate(callbacks, line, 1)?;
    let right = offset_coordinate(callbacks, line, 2)?;
    let bottom = offset_coordinate(callbacks, line, 3)?;
    Ok([left, top, right, bottom])
}

fn offset_coordinate<C: ImageFontTextBoundsCallbacks>(
    callbacks: &mut C,
    line: &C::Value,
    index: usize,
) -> Result<C::Value, C::Error> {
    let coordinate = callbacks.bbox_coordinate(index)?;
    let offset = callbacks.line_offset(line, index)?;
    callbacks.add(&coordinate, &offset)
}

pub(crate) fn split_with_callbacks<C: ImageFontTextPositionCallbacks>(
    callbacks: &mut C,
) -> Result<Vec<C::Line>, C::Error> {
    let anchor = callbacks.anchor()?;
    if callbacks.line_count()? == 1 {
        let x = callbacks.origin_x()?;
        let y = callbacks.origin_y()?;
        let line = callbacks.single_line()?;
        return Ok(vec![callbacks.positioned_line(&x, &y, &anchor, &line)?]);
    }
    if callbacks.anchor_component_contains(&anchor, 1, "tb")?
        && callbacks.direction_is_not_top_to_bottom()?
    {
        return Err(PilError::ValueError("anchor not supported for multiline text".into()).into());
    }

    let font_mode = callbacks.font_mode()?;
    let line_spacing = callbacks.line_spacing(&font_mode)?;
    if callbacks.direction_is_top_to_bottom()? {
        return vertical_positions(callbacks, &anchor, &line_spacing);
    }
    horizontal_positions(callbacks, &anchor, &font_mode, &line_spacing)
}

fn vertical_positions<C: ImageFontTextPositionCallbacks>(
    callbacks: &mut C,
    anchor: &C::Value,
    line_spacing: &C::Value,
) -> Result<Vec<C::Line>, C::Error> {
    let mut left = callbacks.origin_x()?;
    let top = callbacks.origin_y()?;
    callbacks.begin_lines()?;
    let mut positioned = Vec::new();
    while let Some(line) = callbacks.next_line()? {
        positioned.push(callbacks.positioned_line(&left, &top, anchor, &line)?);
        left = callbacks.arithmetic(ImageFontTextPositionArithmetic::AddAssign, &left, line_spacing)?;
    }
    Ok(positioned)
}

fn horizontal_positions<C: ImageFontTextPositionCallbacks>(
    callbacks: &mut C,
    anchor: &C::Value,
    font_mode: &C::Value,
    line_spacing: &C::Value,
) -> Result<Vec<C::Line>, C::Error> {
    let widths = measure_line_widths(callbacks, font_mode)?;
    let max_width = maximum_width(callbacks, &widths)?;
    let mut top = vertical_origin(callbacks, anchor, line_spacing)?;
    let mut positioned = Vec::new();
    callbacks.begin_lines()?;
    for (index, width) in widths.iter().enumerate() {
        let Some(line) = callbacks.next_line()? else {
            break;
        };
        let width_difference = callbacks.arithmetic(
            ImageFontTextPositionArithmetic::Subtract,
            &max_width,
            width,
        )?;
        let mut left = callbacks.origin_x()?;
        apply_alignment(callbacks, &mut left, &width_difference)?;
        if should_justify(callbacks, &width_difference, index)? {
            let words = callbacks.split_words(&line)?;
            if words.len() > 1 {
                justify_words(
                    callbacks,
                    &mut positioned,
                    &mut top,
                    &mut left,
                    anchor,
                    &words,
                    &max_width,
                    font_mode,
                    line_spacing,
                )?;
                continue;
            }
        }
        adjust_anchor(callbacks, &mut left, anchor, &width_difference, true)?;
        positioned.push(callbacks.positioned_line(&left, &top, anchor, &line)?);
        top = callbacks.arithmetic(ImageFontTextPositionArithmetic::AddAssign, &top, line_spacing)?;
    }
    Ok(positioned)
}

fn measure_line_widths<C: ImageFontTextPositionCallbacks>(
    callbacks: &mut C,
    font_mode: &C::Value,
) -> Result<Vec<C::Value>, C::Error> {
    callbacks.begin_lines()?;
    let mut widths = Vec::new();
    while let Some(line) = callbacks.next_line()? {
        widths.push(callbacks.line_width(&line, font_mode)?);
    }
    Ok(widths)
}

fn maximum_width<C: ImageFontTextPositionCallbacks>(
    callbacks: &mut C,
    widths: &[C::Value],
) -> Result<C::Value, C::Error> {
    let Some(first) = widths.first() else {
        return callbacks.integer(0);
    };
    let mut maximum = first.clone();
    for width in widths.iter().skip(1) {
        if callbacks.compare_numbers(
            width,
            &maximum,
            ImageFontTextPositionComparison::GreaterThan,
        )? {
            maximum = width.clone();
        }
    }
    Ok(maximum)
}

fn vertical_origin<C: ImageFontTextPositionCallbacks>(
    callbacks: &mut C,
    anchor: &C::Value,
    line_spacing: &C::Value,
) -> Result<C::Value, C::Error> {
    let mut top = callbacks.origin_y()?;
    let vertical_middle = callbacks.anchor_component_is(anchor, 1, "m")?;
    let vertical_descender = if vertical_middle {
        false
    } else {
        callbacks.anchor_component_is(anchor, 1, "d")?
    };
    if vertical_middle || vertical_descender {
        let line_count = callbacks.line_count()?;
        let count = callbacks.integer(line_count as i64 - 1)?;
        let offset = callbacks.arithmetic(ImageFontTextPositionArithmetic::Multiply, &count, line_spacing)?;
        let offset = if vertical_middle {
            let two = callbacks.float(2.0)?;
            callbacks.arithmetic(ImageFontTextPositionArithmetic::Divide, &offset, &two)?
        } else {
            offset
        };
        top = callbacks.arithmetic(ImageFontTextPositionArithmetic::SubtractAssign, &top, &offset)?;
    }
    Ok(top)
}

fn apply_alignment<C: ImageFontTextPositionCallbacks>(
    callbacks: &mut C,
    left: &mut C::Value,
    width_difference: &C::Value,
) -> Result<(), C::Error> {
    if callbacks.alignment_is("center")? {
        let two = callbacks.float(2.0)?;
        let offset = callbacks.arithmetic(ImageFontTextPositionArithmetic::Divide, width_difference, &two)?;
        *left = callbacks.arithmetic(ImageFontTextPositionArithmetic::AddAssign, left, &offset)?;
    } else if callbacks.alignment_is("right")? {
        *left = callbacks.arithmetic(
            ImageFontTextPositionArithmetic::AddAssign,
            left,
            width_difference,
        )?;
    } else if !callbacks.alignment_is_left_or_justify()? {
        return Err(PilError::ValueError(
            "align must be \"left\", \"center\", \"right\" or \"justify\"".into(),
        )
        .into());
    }
    Ok(())
}

fn should_justify<C: ImageFontTextPositionCallbacks>(
    callbacks: &mut C,
    width_difference: &C::Value,
    index: usize,
) -> Result<bool, C::Error> {
    if !callbacks.alignment_is("justify")? {
        return Ok(false);
    }
    let zero = callbacks.integer(0)?;
    if !callbacks.compare_numbers(
        width_difference,
        &zero,
        ImageFontTextPositionComparison::NotEqual,
    )? {
        return Ok(false);
    }
    Ok(index as i128 != callbacks.line_count()? as i128 - 1)
}

fn justify_words<C: ImageFontTextPositionCallbacks>(
    callbacks: &mut C,
    positioned: &mut Vec<C::Line>,
    top: &mut C::Value,
    left: &mut C::Value,
    anchor: &C::Value,
    words: &[C::Value],
    max_width: &C::Value,
    font_mode: &C::Value,
    line_spacing: &C::Value,
) -> Result<(), C::Error> {
    adjust_anchor(callbacks, left, anchor, max_width, true)?;
    let word_widths = words
        .iter()
        .map(|word| callbacks.line_width(word, font_mode))
        .collect::<Result<Vec<_>, C::Error>>()?;
    let total = sum_widths(callbacks, &word_widths)?;
    let extra = callbacks.arithmetic(ImageFontTextPositionArithmetic::Subtract, max_width, &total)?;
    let mut word_x = left.clone();
    let word_anchor = callbacks.justified_anchor(anchor)?;
    for (word, word_width) in words.iter().zip(&word_widths) {
        positioned.push(callbacks.positioned_line(&word_x, top, &word_anchor, word)?);
        let gap_count = callbacks.integer(words.len() as i64 - 1)?;
        let gap = callbacks.arithmetic(ImageFontTextPositionArithmetic::Divide, &extra, &gap_count)?;
        let advance = callbacks.arithmetic(ImageFontTextPositionArithmetic::Add, word_width, &gap)?;
        word_x = callbacks.arithmetic(ImageFontTextPositionArithmetic::AddAssign, &word_x, &advance)?;
    }
    *top = callbacks.arithmetic(ImageFontTextPositionArithmetic::AddAssign, top, line_spacing)?;
    Ok(())
}

fn sum_widths<C: ImageFontTextPositionCallbacks>(
    callbacks: &mut C,
    widths: &[C::Value],
) -> Result<C::Value, C::Error> {
    let mut total = callbacks.integer(0)?;
    for width in widths {
        total = callbacks.arithmetic(ImageFontTextPositionArithmetic::Add, &total, width)?;
    }
    Ok(total)
}

fn adjust_anchor<C: ImageFontTextPositionCallbacks>(
    callbacks: &mut C,
    left: &mut C::Value,
    anchor: &C::Value,
    width: &C::Value,
    half_width: bool,
) -> Result<(), C::Error> {
    let middle = callbacks.anchor_component_is(anchor, 0, "m")?;
    let right = if middle {
        false
    } else {
        callbacks.anchor_component_is(anchor, 0, "r")?
    };
    if middle || right {
        let offset = if middle && half_width {
            let two = callbacks.float(2.0)?;
            callbacks.arithmetic(ImageFontTextPositionArithmetic::Divide, width, &two)?
        } else {
            width.clone()
        };
        *left = callbacks.arithmetic(
            ImageFontTextPositionArithmetic::SubtractAssign,
            left,
            &offset,
        )?;
    }
    Ok(())
}

pub(crate) struct PositionedTextLine<'a> {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) text: &'a str,
    pub(crate) anchor: String,
}

struct MultilineAnchor {
    value: String,
    horizontal: char,
    vertical: char,
    vertical_direction: bool,
}

// TODO(pillow-rs#brg0): verify anchored, font-size, and spacing Text outputs in CPU parity.
pub(crate) fn multiline_text_positions<'a>(
    x: f64,
    y: f64,
    lines: &[&'a str],
    font: &crate::font::FreeTypeFont,
    spacing: f64,
    options: &crate::font::ImageFontTextOptions,
    mode: Mode,
) -> Result<Vec<PositionedTextLine<'a>>, PilError> {
    let anchor = resolve_multiline_anchor(lines.len(), options)?;
    if lines.len() == 1 {
        return Ok(vec![PositionedTextLine {
            x,
            y,
            text: lines[0],
            anchor: anchor.value,
        }]);
    }
    let font_mode = text_layout_font_mode(mode, options.embedded_color);
    let line_spacing = multiline_line_spacing(font, options, font_mode, spacing)?;
    if anchor.vertical_direction {
        return Ok(vertical_text_positions(x, y, lines, line_spacing, &anchor));
    }
    horizontal_text_positions(x, y, lines, font, options, font_mode, line_spacing, &anchor)
}

fn resolve_multiline_anchor(
    line_count: usize,
    options: &crate::font::ImageFontTextOptions,
) -> Result<MultilineAnchor, PilError> {
    let vertical_direction = options.direction.as_deref() == Some("ttb");
    let default_anchor = if vertical_direction { "lt" } else { "la" };
    let value = options.anchor.as_deref().unwrap_or(default_anchor);
    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() != 2 {
        return Err(PilError::ValueError(
            if options.anchor_invalid_length_error {
                "anchor must be a 2 character string".into()
            } else {
                format!("bad anchor specified: {value}")
            },
        ));
    }
    if line_count > 1 && matches!(chars[1], 't' | 'b') && !vertical_direction {
        return Err(PilError::ValueError(
            "anchor not supported for multiline text".into(),
        ));
    }
    Ok(MultilineAnchor {
        value: value.to_owned(),
        horizontal: chars[0],
        vertical: chars[1],
        vertical_direction,
    })
}

pub(crate) fn text_layout_font_mode(mode: Mode, embedded_color: bool) -> &'static str {
    if embedded_color {
        "RGBA"
    } else if matches!(mode, Mode::One | Mode::ImagePalette8 | Mode::ImageI32 | Mode::ImageF32) {
        "1"
    } else {
        "L"
    }
}

fn multiline_line_spacing(
    font: &crate::font::FreeTypeFont,
    options: &crate::font::ImageFontTextOptions,
    font_mode: &str,
    spacing: f64,
) -> Result<f64, PilError> {
    let mut bbox_options = options.clone();
    bbox_options.mode = Some(font_mode.to_owned());
    bbox_options.embedded_color = false;
    bbox_options.direction = None;
    bbox_options.anchor = None;
    bbox_options.align = None;
    bbox_options.stroke_fill = None;
    Ok(f64::from(font.getbbox_with_options("A", &bbox_options)?.3)
        + f64::from(options.stroke_width)
        + spacing)
}

fn vertical_text_positions<'a>(
    x: f64,
    y: f64,
    lines: &[&'a str],
    line_spacing: f64,
    anchor: &MultilineAnchor,
) -> Vec<PositionedTextLine<'a>> {
    lines
        .iter()
        .enumerate()
        .map(|(index, &text)| PositionedTextLine {
            x: x + index as f64 * line_spacing,
            y,
            text,
            anchor: anchor.value.clone(),
        })
        .collect()
}

fn horizontal_text_positions<'a>(
    x: f64,
    y: f64,
    lines: &[&'a str],
    font: &crate::font::FreeTypeFont,
    options: &crate::font::ImageFontTextOptions,
    font_mode: &str,
    line_spacing: f64,
    anchor: &MultilineAnchor,
) -> Result<Vec<PositionedTextLine<'a>>, PilError> {
    let mut width_options = options.clone();
    width_options.mode = Some(font_mode.to_owned());
    width_options.embedded_color = false;
    width_options.stroke_width = 0.0;
    width_options.anchor = None;
    width_options.align = None;
    width_options.stroke_fill = None;
    let widths = lines
        .iter()
        .map(|line| font.getlength_with_options(line, &width_options))
        .collect::<Result<Vec<_>, _>>()?;
    let max_width = widths.iter().copied().fold(0.0_f32, f32::max) as f64;
    let align = options.align.as_deref().unwrap_or("left");
    if !matches!(align, "left" | "center" | "right" | "justify") {
        return Err(PilError::ValueError(
            "align must be \"left\", \"center\", \"right\" or \"justify\"".into(),
        ));
    }

    let mut top = y;
    if anchor.vertical == 'm' {
        top -= (lines.len() - 1) as f64 * line_spacing / 2.0;
    } else if anchor.vertical == 'd' {
        top -= (lines.len() - 1) as f64 * line_spacing;
    }
    let mut positioned = Vec::new();
    for (index, &line) in lines.iter().enumerate() {
        let width_difference = max_width - f64::from(widths[index]);
        let mut left = match align {
            "center" => x + width_difference / 2.0,
            "right" => x + width_difference,
            _ => x,
        };
        if align == "justify" && width_difference != 0.0 && index + 1 != lines.len() {
            let words = line.split(' ').collect::<Vec<_>>();
            if words.len() > 1 {
                if anchor.horizontal == 'm' {
                    left -= max_width / 2.0;
                } else if anchor.horizontal == 'r' {
                    left -= max_width;
                }
                let word_widths = words
                    .iter()
                    .map(|word| font.getlength_with_options(word, &width_options))
                    .collect::<Result<Vec<_>, _>>()?;
                let extra = (max_width
                    - word_widths.iter().map(|width| f64::from(*width)).sum::<f64>())
                    / (words.len() - 1) as f64;
                let word_anchor = format!("l{}", anchor.vertical);
                for (word, width) in words.into_iter().zip(word_widths) {
                    positioned.push(PositionedTextLine {
                        x: left,
                        y: top,
                        text: word,
                        anchor: word_anchor.clone(),
                    });
                    left += f64::from(width) + extra;
                }
                top += line_spacing;
                continue;
            }
        }
        if anchor.horizontal == 'm' {
            left -= width_difference / 2.0;
        } else if anchor.horizontal == 'r' {
            left -= width_difference;
        }
        positioned.push(PositionedTextLine {
            x: left,
            y: top,
            text: line,
            anchor: anchor.value.clone(),
        });
        top += line_spacing;
    }
    Ok(positioned)
}
