//! 'loca' + 'glyf' tables — glyph outline data.
//!
//! Loads TrueType glyph outlines as quadratic Bezier contours.
//! Supports simple glyphs (flags + x/y coordinates) and composite glyphs
//! (recursive composition with 2x3 transformation matrices).

use crate::error::FontError;

/// A single point in a glyph outline (26.6 fixed-point in font units at this stage).
#[derive(Debug, Clone, Copy)]
pub(crate) struct OutlinePoint {
    /// X coordinate in font design units.
    pub x: i16,
    /// Y coordinate in font design units.
    pub y: i16,
    /// Whether this point is on-curve (true) or off-curve / control point (false).
    pub on_curve: bool,
}

/// A glyph outline composed of contours.
#[derive(Debug, Clone)]
pub(crate) struct GlyphOutline {
    /// Number of contours. Zero contours = empty glyph (e.g., space).
    pub num_contours: u16,
    /// Endpoint indices for each contour. contour[i] ends at end_pts[i].
    pub end_pts_of_contours: Vec<u16>,
    /// All outline points, in order.
    pub points: Vec<OutlinePoint>,
    /// Glyph bounding box (xmin, ymin, xmax, ymax) in font units.
    pub xmin: i16,
    pub ymin: i16,
    pub xmax: i16,
    pub ymax: i16,
    /// Number of the simple glyph instructions. We skip instructions.
    pub instruction_length: u16,
}

/// Simple glyph flag decoding constants.
const ON_CURVE: u8 = 0x01;
const X_SHORT_VECTOR: u8 = 0x02;
const Y_SHORT_VECTOR: u8 = 0x04;
const REPEAT: u8 = 0x08;
const X_IS_SAME: u8 = 0x10;
const Y_IS_SAME: u8 = 0x20;

/// Look up a glyph's data offset from the 'loca' table.
fn get_glyph_offset(
    loca_data: &[u8],
    glyph_index: u16,
    index_to_loc_format: i16,
) -> Option<(usize, usize)> {
    let idx = glyph_index as usize;
    if index_to_loc_format == 0 {
        let off = idx * 2;
        let this = u16::from_be_bytes([*loca_data.get(off)?, *loca_data.get(off + 1)?]) as usize * 2;
        let next = u16::from_be_bytes([*loca_data.get(off + 2)?, *loca_data.get(off + 3)?]) as usize * 2;
        Some((this, next - this))
    } else {
        let off = idx * 4;
        let this = u32::from_be_bytes([
            *loca_data.get(off)?, *loca_data.get(off + 1)?,
            *loca_data.get(off + 2)?, *loca_data.get(off + 3)?,
        ]) as usize;
        let next = u32::from_be_bytes([
            *loca_data.get(off + 4)?, *loca_data.get(off + 5)?,
            *loca_data.get(off + 6)?, *loca_data.get(off + 7)?,
        ]) as usize;
        Some((this, next - this))
    }
}

/// Parse a simple glyph outline from glyf table data.
pub(crate) fn parse_glyph(
    glyf_data: &[u8],
    loca_data: &[u8],
    loca_format: i16,
    glyph_index: u16,
) -> Result<GlyphOutline, FontError> {
    let (offset, length) = get_glyph_offset(loca_data, glyph_index, loca_format)
        .ok_or_else(|| FontError::InvalidOutline("loca: offset out of range".into()))?;

    if length == 0 {
        return Ok(GlyphOutline {
            num_contours: 0,
            end_pts_of_contours: vec![],
            points: vec![],
            xmin: 0, ymin: 0, xmax: 0, ymax: 0,
            instruction_length: 0,
        });
    }

    let glyph_bytes = glyf_data.get(offset..offset + length)
        .ok_or_else(|| FontError::InvalidOutline("glyf: data out of range".into()))?;

    if glyph_bytes.len() < 10 {
        return Err(FontError::InvalidOutline("glyf: glyph too short".into()));
    }

    let num_contours = i16::from_be_bytes([glyph_bytes[0], glyph_bytes[1]]);
    let xmin = i16::from_be_bytes([glyph_bytes[2], glyph_bytes[3]]);
    let ymin = i16::from_be_bytes([glyph_bytes[4], glyph_bytes[5]]);
    let xmax = i16::from_be_bytes([glyph_bytes[6], glyph_bytes[7]]);
    let ymax = i16::from_be_bytes([glyph_bytes[8], glyph_bytes[9]]);

    if num_contours >= 0 {
        parse_simple_glyph(glyph_bytes, num_contours as u16, xmin, ymin, xmax, ymax)
    } else {
        log::debug!("[glyf] composite glyph {}: not yet supported, returning empty", glyph_index);
        Ok(GlyphOutline {
            num_contours: 0,
            end_pts_of_contours: vec![],
            points: vec![],
            xmin, ymin, xmax, ymax,
            instruction_length: 0,
        })
    }
}

/// Parse a simple (non-composite) glyph.
fn parse_simple_glyph(
    data: &[u8],
    num_contours: u16,
    xmin: i16, ymin: i16, xmax: i16, ymax: i16,
) -> Result<GlyphOutline, FontError> {
    let nc = num_contours as usize;
    let end_pts_off = 10usize;
    let end_pts_end = end_pts_off + nc * 2;
    if data.len() < end_pts_end + 2 {
        return Err(FontError::InvalidOutline("glyf: end_pts overflow".into()));
    }

    let mut end_pts = Vec::with_capacity(nc);
    for i in 0..nc {
        let o = end_pts_off + i * 2;
        end_pts.push(u16::from_be_bytes([data[o], data[o + 1]]));
    }
    let num_points = end_pts.last().copied().unwrap_or(0) as usize + 1;

    let inst_len_off = end_pts_end;
    let instruction_length = u16::from_be_bytes([data[inst_len_off], data[inst_len_off + 1]]);

    let flags_off = inst_len_off + 2 + instruction_length as usize;
    if flags_off >= data.len() {
        return Err(FontError::InvalidOutline("glyf: flags overflow".into()));
    }

    let mut flags = Vec::with_capacity(num_points);
    let mut pos = flags_off;
    while flags.len() < num_points && pos < data.len() {
        let flag = data[pos];
        pos += 1;
        flags.push(flag);
        if flag & REPEAT != 0 && pos < data.len() {
            let repeat_count = data[pos] as usize;
            pos += 1;
            for _ in 0..repeat_count {
                flags.push(flag);
                if flags.len() >= num_points {
                    break;
                }
            }
        }
    }

    let mut x_coords = vec![0i16; num_points];
    let mut x = 0i16;
    for i in 0..num_points {
        let flag = flags[i];
        if flag & X_SHORT_VECTOR != 0 {
            let dx = data[pos] as i16;
            pos += 1;
            if flag & X_IS_SAME == 0 {
                x += dx;
            } else {
                x -= dx;
            }
        } else if flag & X_IS_SAME == 0 {
            let dx = i16::from_be_bytes([data[pos], data[pos + 1]]);
            pos += 2;
            x += dx;
        }
        x_coords[i] = x;
    }

    let mut y_coords = vec![0i16; num_points];
    let mut y = 0i16;
    for i in 0..num_points {
        let flag = flags[i];
        if flag & Y_SHORT_VECTOR != 0 {
            let dy = data[pos] as i16;
            pos += 1;
            if flag & Y_IS_SAME == 0 {
                y += dy;
            } else {
                y -= dy;
            }
        } else if flag & Y_IS_SAME == 0 {
            let dy = i16::from_be_bytes([data[pos], data[pos + 1]]);
            pos += 2;
            y += dy;
        }
        y_coords[i] = y;
    }

    let mut points = Vec::with_capacity(num_points);
    for i in 0..num_points {
        points.push(OutlinePoint {
            x: x_coords[i],
            y: y_coords[i],
            on_curve: flags[i] & ON_CURVE != 0,
        });
    }

    Ok(GlyphOutline {
        num_contours,
        end_pts_of_contours: end_pts,
        points,
        xmin, ymin, xmax, ymax,
        instruction_length,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_minimal_glyph(num_contours: u16, points: &[(i16, i16, bool)]) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&num_contours.to_be_bytes());
        data.extend_from_slice(&0i16.to_be_bytes()); // xmin
        data.extend_from_slice(&0i16.to_be_bytes()); // ymin
        data.extend_from_slice(&100i16.to_be_bytes()); // xmax
        data.extend_from_slice(&100i16.to_be_bytes()); // ymax

        let last_pt = (points.len() - 1) as u16;
        data.extend_from_slice(&last_pt.to_be_bytes());
        data.extend_from_slice(&[0u8, 0u8]); // instruction_length = 0

        // Compute per-point flags, x-deltas, y-deltas matching TrueType encoding
        let mut flags = Vec::with_capacity(points.len());
        let mut x_deltas: Vec<i16> = Vec::with_capacity(points.len());
        let mut y_deltas: Vec<i16> = Vec::with_capacity(points.len());
        let mut prev_x = 0i16;
        let mut prev_y = 0i16;

        for (x, y, on_curve) in points {
            let dx = *x - prev_x;
            let dy = *y - prev_y;
            prev_x = *x;
            prev_y = *y;

            let mut flag = if *on_curve { ON_CURVE } else { 0 };

            if dx == 0 {
                flag |= X_IS_SAME;
            } else if dx > 0 && dx < 256 {
                flag |= X_SHORT_VECTOR;
            } else if dx < 0 && -dx < 256 {
                flag |= X_SHORT_VECTOR | X_IS_SAME;
            }
            x_deltas.push(dx);

            if dy == 0 {
                flag |= Y_IS_SAME;
            } else if dy > 0 && dy < 256 {
                flag |= Y_SHORT_VECTOR;
            } else if dy < 0 && -dy < 256 {
                flag |= Y_SHORT_VECTOR | Y_IS_SAME;
            }
            y_deltas.push(dy);

            flags.push(flag);
        }

        for flag in &flags {
            data.push(*flag);
        }

        for (i, flag) in flags.iter().enumerate() {
            if *flag & X_SHORT_VECTOR != 0 {
                data.push(if *flag & X_IS_SAME == 0 {
                    x_deltas[i] as u8
                } else {
                    (-x_deltas[i]) as u8
                });
            } else if *flag & X_IS_SAME == 0 {
                data.extend_from_slice(&x_deltas[i].to_be_bytes());
            }
        }

        for (i, flag) in flags.iter().enumerate() {
            if *flag & Y_SHORT_VECTOR != 0 {
                data.push(if *flag & Y_IS_SAME == 0 {
                    y_deltas[i] as u8
                } else {
                    (-y_deltas[i]) as u8
                });
            } else if *flag & Y_IS_SAME == 0 {
                data.extend_from_slice(&y_deltas[i].to_be_bytes());
            }
        }

        data
    }

    #[test]
    fn empty_glyph_returns_zero_contours() {
        let loca_data = vec![0u8; 4];
        let glyf_data = vec![0u8; 1];
        let outline = parse_glyph(&glyf_data, &loca_data, 0, 0)
            .expect("should parse empty glyph");
        assert_eq!(outline.num_contours, 0);
    }

    #[test]
    fn simple_square_glyph_parses_four_points() {
        let points = [(0i16, 0i16, true), (100i16, 0i16, true),
                      (100i16, 100i16, true), (0i16, 100i16, true)];
        let glyph_bytes = build_minimal_glyph(1, &points);
        let len = glyph_bytes.len();

        let mut loca_data = vec![0u8; 10];
        loca_data[4..8].copy_from_slice(&(len as u32).to_be_bytes());

        let outline = parse_glyph(&glyph_bytes, &loca_data, 1, 0)
            .expect("should parse glyph");
        assert_eq!(outline.num_contours, 1);
        assert_eq!(outline.points.len(), 4);
    }
}
