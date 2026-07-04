//! 'glyf' table — glyph outline data (simple + composite).
//!
//! Faithful port of the decode paths in `src/truetype/ttgload.c`:
//! `TT_Load_Simple_Glyph` (flag/delta coordinate decoding) and the composite
//! component loop (`TT_Load_Composite_Glyph`). Output is a flattened list of
//! contours with on/off-curve tags, in font design units.

use crate::casts::{u16_from_i16, u16_from_u32, u32_from_usize};

use crate::error::FontError;
use crate::fixed::{ft_div_fix, ft_mul_fix};
use crate::tt::loca::{get_glyph_location, GlyphLocation};

// Simple glyph flag bits (TrueType spec, ttgload.c:53).
const ON_CURVE: u8 = 0x01;
const X_SHORT_VECTOR: u8 = 0x02;
const Y_SHORT_VECTOR: u8 = 0x04;
const REPEAT_FLAG: u8 = 0x08;
const X_IS_SAME_OR_POSITIVE_SHORT: u8 = 0x10;
const Y_IS_SAME_OR_POSITIVE_SHORT: u8 = 0x20;

// Composite glyph flag bits (ttgload.c:69).
const ARG_1_AND_2_ARE_WORDS: u16 = 0x0001;
const ARGS_ARE_XY_VALUES: u16 = 0x0002;
const ROUND_XY_TO_GRID: u16 = 0x0004;
const WE_HAVE_A_SCALE: u16 = 0x0008;
const MORE_COMPONENTS: u16 = 0x0020;
const WE_HAVE_AN_X_Y_SCALE: u16 = 0x0040;
const WE_HAVE_A_TWO_BY_TWO: u16 = 0x0080;
const WE_HAVE_INSTRUCTIONS: u16 = 0x0100;

/// A single decoded outline point in font design units.
#[derive(Debug, Clone, Copy)]
pub struct OutlinePoint {
    pub x: i32,
    pub y: i32,
    /// `true` for on-curve points, `false` for off-curve (control) points.
    pub on_curve: bool,
}

/// A glyph outline as a flattened list of contours.
#[derive(Debug, Clone, Default)]
pub struct GlyphOutline {
    pub num_contours: u16,
    pub end_pts_of_contours: Vec<u16>,
    pub points: Vec<OutlinePoint>,
    pub xmin: i32,
    pub ymin: i32,
    pub xmax: i32,
    pub ymax: i32,
    /// Whether composite. If true, xmin tracks last sub-glyph's glyf header
    /// (matching C's loader->bbox ttgload.c:324) and lsb tracks last sub's.
    pub is_composite: bool,
    pub sub_lsb: i32,
    /// TrueType bytecode instructions for this glyph (from glyf table).
    /// Only populated for simple glyphs; empty for composites and empty glyphs.
    pub instructions: Vec<u8>,
}

/// A 2×2 fixed-point transform for a composite component (16.16).
#[derive(Debug, Clone, Copy)]
pub struct Affine {
    pub xx: i32,
    pub xy: i32,
    pub yx: i32,
    pub yy: i32,
}

impl Affine {
    const IDENTITY: Self = Self {
        xx: 0x1_0000,
        xy: 0,
        yx: 0,
        yy: 0x1_0000,
    };
}

/// One component of a composite glyph.
#[derive(Debug, Clone)]
struct CompositeComponent {
    glyph_index: u16,
    /// Translation args (font units when ARGS_ARE_XY_VALUES).
    arg1: i32,
    arg2: i32,
    args_are_xy: bool,
    round_xy_to_grid: bool,
    transform: Affine,
}

struct CompositeGlyph {
    components: Vec<CompositeComponent>,
    instructions: Vec<u8>,
}

/// Load a glyph outline from 'glyf'/'loca', resolving composite glyphs recursively.
///
/// `depth` guards against malformed recursive composites.
pub fn load_glyph(
    glyf: &[u8],
    loca: &[u8],
    index_to_loc_format: i16,
    glyph_index: u16,
    hmtx: &crate::tt::hmtx::HmtxTable,
) -> Result<GlyphOutline, FontError> {
    load_glyph_inner(glyf, loca, index_to_loc_format, glyph_index, hmtx, 0, None)
}

pub fn load_glyph_with_scaled_component_offsets(
    glyf: &[u8],
    loca: &[u8],
    index_to_loc_format: i16,
    glyph_index: u16,
    hmtx: &crate::tt::hmtx::HmtxTable,
    x_scale: i32,
    y_scale: i32,
) -> Result<GlyphOutline, FontError> {
    load_glyph_inner(
        glyf,
        loca,
        index_to_loc_format,
        glyph_index,
        hmtx,
        0,
        Some((x_scale, y_scale)),
    )
}

fn load_glyph_inner(
    glyf: &[u8],
    loca: &[u8],
    index_to_loc_format: i16,
    glyph_index: u16,
    hmtx: &crate::tt::hmtx::HmtxTable,
    depth: u8,
    component_offset_scale: Option<(i32, i32)>,
) -> Result<GlyphOutline, FontError> {
    if depth > 8 {
        return Err(FontError::InvalidOutline(
            "glyf: composite recursion too deep".into(),
        ));
    }

    let loc = get_glyph_location(loca, glyph_index, index_to_loc_format)
        .ok_or_else(|| FontError::InvalidOutline("loca: glyph index out of range".into()))?;
    if loc.length == 0 {
        return Ok(GlyphOutline::default());
    }

    let bytes = glyf
        .get(loc.offset as usize..loc.offset as usize + loc.length as usize)
        .ok_or_else(|| FontError::InvalidOutline("glyf: data out of range".into()))?;
    if bytes.len() < 10 {
        return Err(FontError::InvalidOutline("glyf: glyph too short".into()));
    }

    let num_contours = i16::from_be_bytes([bytes[0], bytes[1]]);
    let xmin = i16::from_be_bytes([bytes[2], bytes[3]]) as i32;
    let ymin = i16::from_be_bytes([bytes[4], bytes[5]]) as i32;
    let xmax = i16::from_be_bytes([bytes[6], bytes[7]]) as i32;
    let ymax = i16::from_be_bytes([bytes[8], bytes[9]]) as i32;

    if num_contours >= 0 {
        let mut outline = parse_simple_glyph(bytes, u16_from_i16(num_contours))?;
        outline.xmin = xmin;
        outline.ymin = ymin;
        outline.xmax = xmax;
        outline.ymax = ymax;
        outline.is_composite = false;
        outline.sub_lsb = hmtx.get(glyph_index).lsb as i32;
        Ok(outline)
    } else {
        // ── Composite glyph: decode components, recurse, flatten ──
        //
        // C's load_truetype_glyph recursively loads each sub-glyph.
        // Each recursive call hits TT_Load_Glyph_Header (ttgload.c:324)
        // which overwrites `loader->bbox` with the glyf header values.
        // tt_get_metrics does the same for `loader->left_bearing`.
        // The LAST sub-glyph "wins" — shared mutable state written N times.
        //
        // What looks like a bug is actually a deliberate choice. After
        // loading, compute_glyph_metrics (ttgload.c:1962-1968) checks:
        //
        //   if (glyph->format != FT_GLYPH_FORMAT_COMPOSITE)
        //       FT_Outline_Get_CBox(&glyph->outline, &bbox);  // O(n)
        //   else
        //       bbox = loader->bbox;  // O(1) — reuse cached value
        //
        // For composites it SKIPS calling FT_Outline_Get_CBox and reuses
        // whatever is already cached — the last sub-glyph's header. This
        // saves walking every point of every component again.
        //
        // The cost: pp1.x (computed from bbox.xMin - left_bearing)
        // can differ from the actual outline minimum by ±1-2 font units.
        // That's 1/64 of a pixel — invisible at any screen resolution.
        // The tradeoff has been there since FreeType 2.0 (1996).
        //
        // To achieve pixel-identical output we track BOTH last_sub_xmin
        // and last_sub_lsb from the final recursive sub-glyph, then
        // compute pp1.x = xmin - sub_lsb in scaler.rs — exactly
        // matching C's accidental-but-intentional behavior.
        let composite = parse_composite_components(bytes, 10)?;
        let mut points: Vec<OutlinePoint> = Vec::new();
        let mut end_pts: Vec<u16> = Vec::new();
        let mut num_contours_total = 0u16;
        let mut last_sub_xmin = xmin;
        let mut last_sub_lsb = hmtx.get(glyph_index).lsb as i32;

        for comp in composite.components {
            let sub = load_glyph_inner(
                glyf,
                loca,
                index_to_loc_format,
                comp.glyph_index,
                hmtx,
                depth + 1,
                component_offset_scale,
            )?;
            last_sub_xmin = sub.xmin;
            last_sub_lsb = sub.sub_lsb;
            let base = points.len();
            let mut transformed = Vec::with_capacity(sub.points.len());
            for pt in &sub.points {
                transformed.push(transform_point(*pt, &comp, 0, 0));
            }
            let (dx, dy) = if comp.args_are_xy {
                component_xy_offset(&comp, component_offset_scale)
            } else {
                let parent_point = comp.arg1 as usize;
                let component_point = comp.arg2 as usize;
                match (points.get(parent_point), transformed.get(component_point)) {
                    (Some(parent), Some(component)) => {
                        (parent.x - component.x, parent.y - component.y)
                    }
                    _ => (0, 0),
                }
            };
            for pt in transformed {
                points.push(OutlinePoint {
                    x: pt.x + dx,
                    y: pt.y + dy,
                    on_curve: pt.on_curve,
                });
            }
            for &ep in &sub.end_pts_of_contours {
                end_pts.push(u16_from_u32(u32_from_usize(base) + ep as u32));
            }
            num_contours_total = num_contours_total.saturating_add(sub.num_contours);
        }

        Ok(GlyphOutline {
            num_contours: num_contours_total,
            end_pts_of_contours: end_pts,
            points,
            xmin: last_sub_xmin,
            ymin,
            xmax,
            ymax,
            is_composite: true,
            sub_lsb: last_sub_lsb,
            instructions: composite.instructions,
        })
    }
}

/// Apply a composite component's transform + translation to a point.
fn transform_point(pt: OutlinePoint, comp: &CompositeComponent, dx: i32, dy: i32) -> OutlinePoint {
    // FreeType applies the 2×2 in 16.16 (FT_MulFix) then adds the XY args.
    let x = crate::fixed::ft_mul_fix(pt.x, comp.transform.xx)
        + crate::fixed::ft_mul_fix(pt.y, comp.transform.xy);
    let y = crate::fixed::ft_mul_fix(pt.x, comp.transform.yx)
        + crate::fixed::ft_mul_fix(pt.y, comp.transform.yy);
    OutlinePoint {
        x: x + dx,
        y: y + dy,
        on_curve: pt.on_curve,
    }
}

fn parse_simple_glyph(data: &[u8], num_contours: u16) -> Result<GlyphOutline, FontError> {
    let nc = num_contours as usize;
    if data.len() < 10 + nc * 2 + 2 {
        return Err(FontError::InvalidOutline("glyf: end_pts overflow".into()));
    }
    let end_off = 10usize;
    let mut end_pts = Vec::with_capacity(nc);
    for i in 0..nc {
        end_pts.push(u16::from_be_bytes([
            data[end_off + i * 2],
            data[end_off + i * 2 + 1],
        ]));
    }
    let num_points = end_pts.last().copied().unwrap_or(0) as usize + 1;

    let inst_off = end_off + nc * 2;
    let instruction_length = u16::from_be_bytes([data[inst_off], data[inst_off + 1]]) as usize;
    let instructions = if instruction_length > 0 {
        data[inst_off + 2..inst_off + 2 + instruction_length].to_vec()
    } else {
        Vec::new()
    };
    let mut pos = inst_off + 2 + instruction_length;

    // Decode flags with repeat compaction.
    let mut flags = Vec::with_capacity(num_points);
    while flags.len() < num_points {
        if pos >= data.len() {
            return Err(FontError::InvalidOutline("glyf: flags overflow".into()));
        }
        let flag = data[pos];
        pos += 1;
        flags.push(flag);
        if flag & REPEAT_FLAG != 0 {
            if pos >= data.len() {
                return Err(FontError::InvalidOutline(
                    "glyf: repeat count overflow".into(),
                ));
            }
            let repeat = data[pos] as usize;
            pos += 1;
            for _ in 0..repeat {
                if flags.len() >= num_points {
                    break;
                }
                flags.push(flag);
            }
        }
    }

    // Decode X coordinates.
    let mut points = Vec::with_capacity(num_points);
    let mut x: i32 = 0;
    let mut y: i32 = 0;
    for &flag in &flags {
        if flag & X_SHORT_VECTOR != 0 {
            if pos >= data.len() {
                return Err(FontError::InvalidOutline("glyf: x short overflow".into()));
            }
            let dx = data[pos] as i32;
            pos += 1;
            // X_SHORT only → negative; X_SHORT | X_IS_SAME → positive.
            x += if flag & X_IS_SAME_OR_POSITIVE_SHORT != 0 {
                dx
            } else {
                -dx
            };
        } else if flag & X_IS_SAME_OR_POSITIVE_SHORT == 0 {
            if pos + 2 > data.len() {
                return Err(FontError::InvalidOutline("glyf: x long overflow".into()));
            }
            x += i16::from_be_bytes([data[pos], data[pos + 1]]) as i32;
            pos += 2;
        }
        points.push(OutlinePoint {
            x,
            y: 0,
            on_curve: false,
        });
    }
    // Decode Y coordinates.
    for (i, &flag) in flags.iter().enumerate() {
        if flag & Y_SHORT_VECTOR != 0 {
            if pos >= data.len() {
                return Err(FontError::InvalidOutline("glyf: y short overflow".into()));
            }
            let dy = data[pos] as i32;
            pos += 1;
            y += if flag & Y_IS_SAME_OR_POSITIVE_SHORT != 0 {
                dy
            } else {
                -dy
            };
        } else if flag & Y_IS_SAME_OR_POSITIVE_SHORT == 0 {
            if pos + 2 > data.len() {
                return Err(FontError::InvalidOutline("glyf: y long overflow".into()));
            }
            y += i16::from_be_bytes([data[pos], data[pos + 1]]) as i32;
            pos += 2;
        }
        points[i].y = y;
        points[i].on_curve = flag & ON_CURVE != 0;
    }

    Ok(GlyphOutline {
        num_contours,
        end_pts_of_contours: end_pts,
        points,
        xmin: 0,
        ymin: 0,
        xmax: 0,
        ymax: 0,
        is_composite: false,
        sub_lsb: 0,
        instructions,
    })
}

fn parse_composite_components(data: &[u8], mut pos: usize) -> Result<CompositeGlyph, FontError> {
    let mut components = Vec::new();
    let mut has_instructions = false;
    loop {
        if pos + 4 > data.len() {
            return Err(FontError::InvalidOutline(
                "glyf: composite header overflow".into(),
            ));
        }
        let flags = u16::from_be_bytes([data[pos], data[pos + 1]]);
        let glyph_index = u16::from_be_bytes([data[pos + 2], data[pos + 3]]);
        pos += 4;
        has_instructions |= flags & WE_HAVE_INSTRUCTIONS != 0;

        let mut count = 2usize;
        if flags & ARG_1_AND_2_ARE_WORDS != 0 {
            count += 2;
        }
        if flags & WE_HAVE_A_SCALE != 0 {
            count += 2;
        } else if flags & WE_HAVE_AN_X_Y_SCALE != 0 {
            count += 4;
        } else if flags & WE_HAVE_A_TWO_BY_TWO != 0 {
            count += 8;
        }
        if pos + count > data.len() {
            return Err(FontError::InvalidOutline(
                "glyf: composite component overflow".into(),
            ));
        }

        let args_are_xy = flags & ARGS_ARE_XY_VALUES != 0;
        let (arg1, arg2) = if flags & ARG_1_AND_2_ARE_WORDS != 0 {
            let a = i16::from_be_bytes([data[pos], data[pos + 1]]) as i32;
            let b = i16::from_be_bytes([data[pos + 2], data[pos + 3]]) as i32;
            pos += 4;
            (a, b)
        } else if args_are_xy {
            let a = data[pos] as i8 as i32;
            let b = data[pos + 1] as i8 as i32;
            pos += 2;
            (a, b)
        } else {
            let a = data[pos] as i32;
            let b = data[pos + 1] as i32;
            pos += 2;
            (a, b)
        };

        let mut transform = Affine::IDENTITY;
        if flags & WE_HAVE_A_SCALE != 0 {
            let s = i16::from_be_bytes([data[pos], data[pos + 1]]) as i32 * 4;
            pos += 2;
            transform.xx = s;
            transform.yy = s;
        } else if flags & WE_HAVE_AN_X_Y_SCALE != 0 {
            transform.xx = i16::from_be_bytes([data[pos], data[pos + 1]]) as i32 * 4;
            transform.yy = i16::from_be_bytes([data[pos + 2], data[pos + 3]]) as i32 * 4;
            pos += 4;
        } else if flags & WE_HAVE_A_TWO_BY_TWO != 0 {
            transform.xx = i16::from_be_bytes([data[pos], data[pos + 1]]) as i32 * 4;
            transform.yx = i16::from_be_bytes([data[pos + 2], data[pos + 3]]) as i32 * 4;
            transform.xy = i16::from_be_bytes([data[pos + 4], data[pos + 5]]) as i32 * 4;
            transform.yy = i16::from_be_bytes([data[pos + 6], data[pos + 7]]) as i32 * 4;
            pos += 8;
        }

        components.push(CompositeComponent {
            glyph_index,
            arg1,
            arg2,
            args_are_xy,
            round_xy_to_grid: flags & ROUND_XY_TO_GRID != 0,
            transform,
        });

        if flags & MORE_COMPONENTS == 0 {
            break;
        }
    }

    let instructions = if has_instructions {
        if pos + 2 > data.len() {
            return Err(FontError::InvalidOutline(
                "glyf: composite instruction length overflow".into(),
            ));
        }
        let instruction_length = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
        pos += 2;
        if pos + instruction_length > data.len() {
            return Err(FontError::InvalidOutline(
                "glyf: composite instructions overflow".into(),
            ));
        }
        data[pos..pos + instruction_length].to_vec()
    } else {
        Vec::new()
    };

    Ok(CompositeGlyph {
        components,
        instructions,
    })
}

fn component_xy_offset(comp: &CompositeComponent, scale: Option<(i32, i32)>) -> (i32, i32) {
    if !comp.round_xy_to_grid {
        return (comp.arg1, comp.arg2);
    }

    let Some((x_scale, y_scale)) = scale else {
        return (comp.arg1, comp.arg2);
    };

    (
        rounded_offset_font_units(comp.arg1, x_scale),
        rounded_offset_font_units(comp.arg2, y_scale),
    )
}

fn rounded_offset_font_units(value: i32, scale: i32) -> i32 {
    if scale == 0 {
        return value;
    }
    let scaled = ft_mul_fix(value, scale);
    let rounded = (scaled + 32) & !63;
    ft_div_fix(rounded, scale)
}

// Unused-warning suppressor for the unused `GlyphLocation` import path.
#[allow(dead_code)]
fn _use(_l: GlyphLocation) {}
