//! Glyph scaler — font units → 26.6 outline, matching FreeType's
//! `FT_Outline` post-scaling state and `FT_Outline_Get_CBox`.
//!
//! Reference: `src/base/ftoutln.c` (`FT_Outline_Transform` via `FT_MulFix`),
//! `src/base/ftglyph.c` (`FT_Glyph_Get_CBox` with `FT_GLYPH_BBOX_PIXELS`).

use crate::casts::{i16_from_i32, i32_from_f32};

use crate::error::FontError;
use crate::fixed::{ft_div_fix, ft_mul_div, ft_mul_fix};
use crate::outline::{OUTLINE_HIGH_PRECISION, Outline, OutlinePoint};
use crate::tables::FontData;
use crate::tt::glyf::{GlyphOutline, load_glyph_with_scaled_component_offsets};
use crate::tt::hinter::NativeHintMode;

/// Fixed-point scale factors derived from point size and units-per-em.
///
/// These are the 16.16 multipliers FreeType applies to font-unit coordinates
/// (via `FT_MulFix`) to get 26.6 outline coordinates.
#[derive(Debug, Clone, Copy)]
pub struct ScaleMetrics {
    pub x_scale: i32, // 16.16
    pub y_scale: i32, // 16.16
    pub tt_scale: i32,
    pub ppem: i32,
    pub x_ratio: i32,
    pub y_ratio: i32,
    pub point_size: i32,
}

impl ScaleMetrics {
    /// Compute scale metrics from point size (72 DPI) and units_per_em.
    ///
    /// FreeType derives `ppem` from the request and computes
    /// `x_scale = FT_DivFix( ppem << 6, units_per_em )` in `tt_size_reset`.
    pub fn new(size_pt: f32, units_per_em: u16) -> Self {
        // FreeType rounds ppem via the request machinery. For 72 DPI,
        // ppem == round(size_pt). We match FreeType's
        // `FT_MulFix(ppem<<6, 64)/upem`-equivalent by using the rounded ppem.
        let ppem = ppem_from_size(size_pt);
        let ppem_26dot6 = ppem << 6;
        let scale = ft_div_fix_local(ppem_26dot6, units_per_em as i32);
        ScaleMetrics {
            x_scale: scale,
            y_scale: scale,
            tt_scale: scale,
            ppem,
            x_ratio: 0x1_0000,
            y_ratio: 0x1_0000,
            point_size: ppem << 6,
        }
    }

    pub fn from_font_data(data: &FontData) -> Self {
        // C stores x/y scale on the active FT_Size_Metrics; non-square
        // FT_Set_Pixel_Sizes must not rebuild horizontal scale from y ppem
        // (`ftobjs.c` size request path, `ttobjs.c:tt_size_reset`).
        ScaleMetrics {
            x_scale: data.size_x_scale.get(),
            y_scale: data.size_y_scale.get(),
            tt_scale: data.size_tt_scale.get(),
            ppem: data.size_tt_ppem.get(),
            x_ratio: data.size_tt_x_ratio.get(),
            y_ratio: data.size_tt_y_ratio.get(),
            point_size: data.size_tt_point_size.get(),
        }
    }

    /// Scale a font-unit coordinate to 26.6.
    #[inline]
    pub fn scale_x(&self, fu: i32) -> i32 {
        ft_mul_fix(fu, self.x_scale)
    }

    #[inline]
    pub fn scale_y(&self, fu: i32) -> i32 {
        ft_mul_fix(fu, self.y_scale)
    }
}

#[inline]
fn scale_unrounded_fdot6(value: i32, scale: i32) -> i32 {
    ft_mul_fix(value, scale).wrapping_add(32) >> 6
}

pub(crate) fn prepare_native_bytecode_context(
    data: &FontData,
    scale: ScaleMetrics,
    native_hint_mode: NativeHintMode,
    pedantic_hinting: bool,
    cvt: &[i32],
    fpgm: &[u8],
) -> Result<crate::tt::hinter::exec::ExecContext, FontError> {
    let context_scale = crate::tt::hinter::HintScale {
        x_scale: scale.x_scale,
        y_scale: scale.y_scale,
        tt_scale: scale.tt_scale,
        ppem: scale.ppem,
        x_ratio: scale.x_ratio,
        y_ratio: scale.y_ratio,
        point_size: scale.point_size,
        storage_size: data.maxp.max_storage as usize,
        max_function_defs: data.maxp.max_function_defs as usize,
        max_instruction_defs: data.maxp.max_instruction_defs as usize,
        twilight_points: data.maxp.max_twilight_points as usize,
        is_composite: false,
        reset_vectors_at_glyph_entry: false,
        metrics_legacy_phantoms: false,
        pedantic_hinting,
        native_hint_mode,
        phantom_x_override: None,
    };
    let prep = data.prep.as_deref().unwrap_or(&[]);
    crate::tt::hinter::prepare_context(cvt, fpgm, prep, &context_scale)
}

/// FT_DivFix in 16.16 (local alias to avoid importing the whole fixed module).
#[inline]
fn ft_div_fix_local(a: i32, b: i32) -> i32 {
    crate::fixed::ft_div_fix(a, b)
}

/// FreeType ppem computation from a point size at 72 DPI.
///
/// FreeType's default request (`FT_Request_Size`) rounds ppem via
/// `FT_PIX_ROUND( size * 64 ) >> 6`, which for integral/half sizes matches
/// `(size + 0.5).floor()`. We mirror that.
fn ppem_from_size(size_pt: f32) -> i32 {
    // ppem = FT_PIX_ROUND( size << 6 ) >> 6  (size already in pixels at 72dpi).
    let size_26dot6 = i32_from_f32((size_pt * 64.0).round());
    // FT_PIX_ROUND(x) = (x + 32) & ~63  on a 26.6 value.
    ((size_26dot6 + 32) & !63) >> 6
}

/// A glyph scaled and positioned for rasterization, plus its metrics.
pub struct ScaledGlyph {
    /// Outline in 26.6, with origin at the glyph's pixel bbox bottom-left.
    pub outline: Outline,
    pub advance_width: i32,      // 26.6
    pub slot_advance_width: i32, // 26.6, after hinted phantom adjustment
    /// Hinted horizontal phantom points before bbox-origin translation.
    pub phantom_pp1_x: i32,
    pub phantom_pp2_x: i32,
    pub vertical_bearing_x_advance_width: i32,
    pub lsb: i32, // 26.6
    /// Raw scaled CBox before pixel floor/ceil conversion.
    pub cbox_x_min: i32,
    pub cbox_y_min: i32,
    pub cbox_x_max: i32,
    pub cbox_y_max: i32,
    /// Raw `FT_Outline_Get_CBox` result before bitmap-origin translation.
    pub outline_cbox_x_min: i32,
    pub outline_cbox_y_min: i32,
    pub outline_cbox_x_max: i32,
    pub outline_cbox_y_max: i32,
    /// Exact `FT_Outline_Get_BBox` result before bitmap-origin translation.
    pub outline_bbox_x_min: i32,
    pub outline_bbox_y_min: i32,
    pub outline_bbox_x_max: i32,
    pub outline_bbox_y_max: i32,
    /// Pixel CBox (FT_GLYPH_BBOX_PIXELS): x/yMin floored, x/yMax ceiled.
    pub bbox_x_min: i32,
    pub bbox_y_min: i32,
    pub bbox_x_max: i32,
    pub bbox_y_max: i32,
    /// Metrics synthesized by FreeType's auto-hinter from the pre-hint slot
    /// vector and the final hinted bbox.
    pub autohint_vertical: Option<AutohintVerticalMetrics>,
    /// Metrics derived by the native TrueType path from pp3/pp4 and the final
    /// hinted bbox.
    pub native_vertical: Option<NativeVerticalMetrics>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutohintVerticalMetrics {
    pub bearing_x: i32,
    pub bearing_y: i32,
    pub advance: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeVerticalMetrics {
    pub bearing_y: i32,
    pub advance: i32,
}

#[derive(Debug, Clone, Copy)]
struct HintStyle {
    is_italic: bool,
    no_horizontal_hinting: bool,
    stem_adjust: bool,
    horz_snap: bool,
    vert_snap: bool,
}

/// Scale a glyph's outline to 26.6 and translate it so its pixel bbox's
/// bottom-left corner sits at (0,0) — the convention `ftsmooth`/`ft_bitmap`
/// use when rendering into a sized bitmap.
///
/// # Returns
///
/// `ScaledGlyph` with hinted outline points (translated to pixel-bbox origin),
/// pixel bbox coordinates, and advance width.
pub fn scale_glyph(
    data: &FontData,
    glyph_index: u16,
    latin_metrics: Option<&crate::autohint::AfLatinMetrics>,
    is_italic: bool,
) -> Result<ScaledGlyph, FontError> {
    scale_glyph_impl(
        data,
        glyph_index,
        latin_metrics,
        HintStyle {
            is_italic,
            no_horizontal_hinting: false,
            stem_adjust: true,
            horz_snap: false,
            vert_snap: false,
        },
        true,
        false,
        NativeHintMode::Normal,
        false,
        false,
        false,
        false,
        None,
        true,
    )
}

pub fn scale_glyph_for_metrics(
    data: &FontData,
    glyph_index: u16,
    is_italic: bool,
) -> Result<ScaledGlyph, FontError> {
    scale_glyph_impl(
        data,
        glyph_index,
        None,
        HintStyle {
            is_italic,
            no_horizontal_hinting: false,
            stem_adjust: true,
            horz_snap: false,
            vert_snap: false,
        },
        true,
        false,
        NativeHintMode::Normal,
        true,
        false,
        false,
        true,
        None,
        true,
    )
}

pub fn scale_glyph_for_metrics_with_bytecode_context(
    data: &FontData,
    glyph_index: u16,
    is_italic: bool,
    bytecode_context: Option<&crate::tt::hinter::exec::ExecContext>,
) -> Result<ScaledGlyph, FontError> {
    scale_glyph_for_metrics_with_bytecode_context_and_mode(
        data,
        glyph_index,
        is_italic,
        NativeHintMode::Normal,
        bytecode_context,
    )
}

pub fn scale_glyph_for_metrics_with_bytecode_context_and_mode(
    data: &FontData,
    glyph_index: u16,
    is_italic: bool,
    native_hint_mode: NativeHintMode,
    bytecode_context: Option<&crate::tt::hinter::exec::ExecContext>,
) -> Result<ScaledGlyph, FontError> {
    scale_glyph_for_metrics_with_bytecode_context_and_mode_and_hdmx(
        data,
        glyph_index,
        is_italic,
        native_hint_mode,
        bytecode_context,
        true,
    )
}

pub fn scale_glyph_for_metrics_with_bytecode_context_and_mode_and_hdmx(
    data: &FontData,
    glyph_index: u16,
    is_italic: bool,
    native_hint_mode: NativeHintMode,
    bytecode_context: Option<&crate::tt::hinter::exec::ExecContext>,
    use_hdmx: bool,
) -> Result<ScaledGlyph, FontError> {
    // C `tt_loader_init` disables v40 backward compatibility for
    // `FT_RENDER_MODE_MONO`; `TT_Hint_Glyph` then saves current hinted phantom
    // points for `compute_glyph_metrics` (ttgload.c:790-865, 2270-2318).
    let legacy_hinter_phantoms = native_hint_mode != NativeHintMode::Mono;
    scale_glyph_impl(
        data,
        glyph_index,
        None,
        HintStyle {
            is_italic,
            no_horizontal_hinting: false,
            stem_adjust: true,
            horz_snap: false,
            vert_snap: false,
        },
        true,
        false,
        native_hint_mode,
        true,
        false,
        false,
        legacy_hinter_phantoms,
        bytecode_context,
        use_hdmx,
    )
}

pub fn scale_glyph_for_metrics_with_autohint(
    data: &FontData,
    glyph_index: u16,
    latin_metrics: Option<&crate::autohint::AfLatinMetrics>,
    is_italic: bool,
) -> Result<ScaledGlyph, FontError> {
    scale_glyph_for_metrics_with_autohint_and_mode(
        data,
        glyph_index,
        latin_metrics,
        is_italic,
        NativeHintMode::Normal,
    )
}

pub fn scale_glyph_for_metrics_with_autohint_and_mode(
    data: &FontData,
    glyph_index: u16,
    latin_metrics: Option<&crate::autohint::AfLatinMetrics>,
    is_italic: bool,
    native_hint_mode: NativeHintMode,
) -> Result<ScaledGlyph, FontError> {
    let (style, target_mono) = match native_hint_mode {
        NativeHintMode::Normal => (
            HintStyle {
                is_italic,
                no_horizontal_hinting: false,
                stem_adjust: true,
                horz_snap: false,
                vert_snap: false,
            },
            false,
        ),
        NativeHintMode::Mono => (
            HintStyle {
                is_italic,
                no_horizontal_hinting: false,
                stem_adjust: true,
                horz_snap: true,
                vert_snap: true,
            },
            true,
        ),
        NativeHintMode::Lcd => (
            HintStyle {
                is_italic,
                no_horizontal_hinting: true,
                stem_adjust: false,
                horz_snap: true,
                vert_snap: false,
            },
            false,
        ),
        NativeHintMode::LcdV => (
            HintStyle {
                is_italic,
                no_horizontal_hinting: false,
                stem_adjust: true,
                horz_snap: false,
                vert_snap: true,
            },
            false,
        ),
    };
    scale_glyph_impl(
        data,
        glyph_index,
        latin_metrics,
        style,
        true,
        target_mono,
        native_hint_mode,
        false,
        true,
        false,
        true,
        None,
        true,
    )
}

pub fn scale_glyph_for_metrics_with_autohint_preserve_advance(
    data: &FontData,
    glyph_index: u16,
    latin_metrics: Option<&crate::autohint::AfLatinMetrics>,
    is_italic: bool,
) -> Result<ScaledGlyph, FontError> {
    scale_glyph_impl(
        data,
        glyph_index,
        latin_metrics,
        HintStyle {
            is_italic,
            no_horizontal_hinting: false,
            stem_adjust: true,
            horz_snap: false,
            vert_snap: false,
        },
        true,
        false,
        NativeHintMode::Normal,
        true,
        false,
        false,
        true,
        None,
        true,
    )
}

pub fn scale_glyph_for_metrics_light(
    data: &FontData,
    glyph_index: u16,
    latin_metrics: Option<&crate::autohint::AfLatinMetrics>,
    is_italic: bool,
) -> Result<ScaledGlyph, FontError> {
    scale_glyph_impl(
        data,
        glyph_index,
        latin_metrics,
        HintStyle {
            is_italic,
            no_horizontal_hinting: true,
            stem_adjust: false,
            horz_snap: false,
            vert_snap: false,
        },
        true,
        false,
        NativeHintMode::Normal,
        false,
        false,
        false,
        true,
        None,
        true,
    )
}

/// Scale a glyph through the native TrueType default load path.
pub fn scale_glyph_native_default(
    data: &FontData,
    glyph_index: u16,
    latin_metrics: Option<&crate::autohint::AfLatinMetrics>,
    is_italic: bool,
) -> Result<ScaledGlyph, FontError> {
    scale_glyph_impl(
        data,
        glyph_index,
        latin_metrics,
        HintStyle {
            is_italic,
            no_horizontal_hinting: false,
            stem_adjust: true,
            horz_snap: false,
            vert_snap: false,
        },
        true,
        false,
        NativeHintMode::Normal,
        false,
        false,
        true,
        false,
        None,
        true,
    )
}

pub fn scale_glyph_native_default_with_bytecode_context(
    data: &FontData,
    glyph_index: u16,
    latin_metrics: Option<&crate::autohint::AfLatinMetrics>,
    is_italic: bool,
    bytecode_context: Option<&crate::tt::hinter::exec::ExecContext>,
) -> Result<ScaledGlyph, FontError> {
    scale_glyph_native_default_with_bytecode_context_and_mode(
        data,
        glyph_index,
        latin_metrics,
        is_italic,
        NativeHintMode::Normal,
        bytecode_context,
    )
}

pub fn scale_glyph_native_default_with_bytecode_context_and_mode(
    data: &FontData,
    glyph_index: u16,
    latin_metrics: Option<&crate::autohint::AfLatinMetrics>,
    is_italic: bool,
    native_hint_mode: NativeHintMode,
    bytecode_context: Option<&crate::tt::hinter::exec::ExecContext>,
) -> Result<ScaledGlyph, FontError> {
    scale_glyph_impl(
        data,
        glyph_index,
        latin_metrics,
        HintStyle {
            is_italic,
            no_horizontal_hinting: false,
            stem_adjust: true,
            horz_snap: false,
            vert_snap: false,
        },
        true,
        false,
        native_hint_mode,
        false,
        false,
        true,
        false,
        bytecode_context,
        true,
    )
}

pub fn scale_glyph_light(
    data: &FontData,
    glyph_index: u16,
    latin_metrics: Option<&crate::autohint::AfLatinMetrics>,
    is_italic: bool,
) -> Result<ScaledGlyph, FontError> {
    scale_glyph_impl(
        data,
        glyph_index,
        latin_metrics,
        HintStyle {
            is_italic,
            no_horizontal_hinting: true,
            stem_adjust: false,
            horz_snap: false,
            vert_snap: false,
        },
        true,
        false,
        NativeHintMode::Normal,
        false,
        false,
        false,
        false,
        None,
        true,
    )
}

/// Scale a glyph for `FT_LOAD_TARGET_LCD`.
///
/// LCD target hinting keeps vertical alignment but disables horizontal
/// grid-fitting to preserve subpixel coverage.
pub fn scale_glyph_lcd(
    data: &FontData,
    glyph_index: u16,
    latin_metrics: Option<&crate::autohint::AfLatinMetrics>,
    is_italic: bool,
) -> Result<ScaledGlyph, FontError> {
    scale_glyph_impl(
        data,
        glyph_index,
        latin_metrics,
        HintStyle {
            is_italic,
            no_horizontal_hinting: true,
            stem_adjust: false,
            horz_snap: true,
            vert_snap: false,
        },
        true,
        false,
        NativeHintMode::Lcd,
        false,
        false,
        false,
        false,
        None,
        true,
    )
}

/// Scale a glyph for `FT_LOAD_TARGET_LCD_V`.
///
/// Vertical LCD target keeps FreeType's normal horizontal fitting behavior and
/// stem adjustment; the vertical subpixel expansion happens during rendering.
pub fn scale_glyph_lcd_v(
    data: &FontData,
    glyph_index: u16,
    latin_metrics: Option<&crate::autohint::AfLatinMetrics>,
    is_italic: bool,
) -> Result<ScaledGlyph, FontError> {
    scale_glyph_impl(
        data,
        glyph_index,
        latin_metrics,
        HintStyle {
            is_italic,
            no_horizontal_hinting: false,
            stem_adjust: true,
            horz_snap: false,
            vert_snap: true,
        },
        true,
        false,
        NativeHintMode::LcdV,
        false,
        false,
        false,
        false,
        None,
        true,
    )
}

/// Scale a glyph for `FT_LOAD_TARGET_MONO` autohint behavior.
pub fn scale_glyph_mono(
    data: &FontData,
    glyph_index: u16,
    latin_metrics: Option<&crate::autohint::AfLatinMetrics>,
    is_italic: bool,
) -> Result<ScaledGlyph, FontError> {
    scale_glyph_impl(
        data,
        glyph_index,
        latin_metrics,
        HintStyle {
            is_italic,
            no_horizontal_hinting: false,
            stem_adjust: true,
            horz_snap: true,
            vert_snap: true,
        },
        true,
        true,
        NativeHintMode::Mono,
        false,
        false,
        false,
        false,
        None,
        true,
    )
}

/// Scale a glyph without autohinting or native TrueType bytecode.
///
/// This models the Rust side of `FT_LOAD_NO_HINTING` fixture execution.
pub fn scale_glyph_no_hinting(
    data: &FontData,
    glyph_index: u16,
    is_italic: bool,
) -> Result<ScaledGlyph, FontError> {
    scale_glyph_impl(
        data,
        glyph_index,
        None,
        HintStyle {
            is_italic,
            no_horizontal_hinting: false,
            stem_adjust: true,
            horz_snap: false,
            vert_snap: false,
        },
        false,
        false,
        NativeHintMode::Normal,
        false,
        false,
        false,
        false,
        None,
        true,
    )
}

#[allow(clippy::too_many_arguments)]
fn scale_glyph_impl(
    data: &FontData,
    glyph_index: u16,
    latin_metrics: Option<&crate::autohint::AfLatinMetrics>,
    style: HintStyle,
    allow_bytecode: bool,
    target_mono: bool,
    native_hint_mode: NativeHintMode,
    round_component_offsets: bool,
    use_autohint_advance: bool,
    reset_vectors_at_glyph_entry: bool,
    legacy_hinter_phantoms: bool,
    bytecode_context: Option<&crate::tt::hinter::exec::ExecContext>,
    use_hdmx: bool,
) -> Result<ScaledGlyph, FontError> {
    scale_glyph_impl_with_context(
        data,
        glyph_index,
        latin_metrics,
        style,
        allow_bytecode,
        target_mono,
        native_hint_mode,
        round_component_offsets,
        use_autohint_advance,
        reset_vectors_at_glyph_entry,
        legacy_hinter_phantoms,
        bytecode_context,
        use_hdmx,
    )
    .map(|(glyph, _)| glyph)
}

#[allow(clippy::too_many_arguments)]
fn scale_glyph_impl_with_context(
    data: &FontData,
    glyph_index: u16,
    latin_metrics: Option<&crate::autohint::AfLatinMetrics>,
    style: HintStyle,
    allow_bytecode: bool,
    target_mono: bool,
    native_hint_mode: NativeHintMode,
    round_component_offsets: bool,
    use_autohint_advance: bool,
    reset_vectors_at_glyph_entry: bool,
    legacy_hinter_phantoms: bool,
    bytecode_context: Option<&crate::tt::hinter::exec::ExecContext>,
    use_hdmx: bool,
) -> Result<(ScaledGlyph, Option<crate::tt::hinter::exec::ExecContext>), FontError> {
    let scale = ScaleMetrics::from_font_data(data);

    let h_metric = data.hmtx.get(glyph_index);
    let lsb = scale.scale_x(h_metric.lsb as i32);

    let outline_raw: std::rc::Rc<crate::tt::glyf::GlyphOutline> = if round_component_offsets {
        if data.cff.is_some() {
            data.load_glyph_outline(glyph_index)?
        } else {
            std::rc::Rc::new(load_glyph_with_scaled_component_offsets(
                &data.glyf_data,
                &data.loca_data,
                data.head.index_to_loc_format,
                glyph_index,
                &data.hmtx,
                scale.x_scale,
                scale.y_scale,
            )?)
        }
    } else {
        data.load_glyph_outline(glyph_index)?
    };
    let hori_advance_fu =
        data.hmtx_hori_advance_with_gvar_delta(glyph_index, outline_raw.points.len())?;
    let advance_width = scale.scale_x(hori_advance_fu);
    let mut slot_advance_width = advance_width;

    if data.cff.is_none() && !allow_bytecode && latin_metrics.is_none() {
        // C: unhinted TrueType loads scale phantom points independently, then
        // compute `horiAdvance = pp2.x - pp1.x` in `compute_glyph_metrics`
        // (`src/truetype/ttgload.c`).  Scaling the raw advance as one value
        // loses one 26.6 unit whenever `pp1.x` and `pp2.x` round differently.
        let pp1x_fu = outline_raw.bbox_xmin - h_metric.lsb as i32;
        let pp2x_fu = pp1x_fu + hori_advance_fu;
        slot_advance_width = scale.scale_x(pp2x_fu) - scale.scale_x(pp1x_fu);
    }

    let fallback_metrics = if latin_metrics.is_none()
        && glyph_index != 0
        && allow_bytecode
        && !round_component_offsets
        && should_use_default_autohint(data)
    {
        // Use the self-referencing Arc from FontData to avoid cloning
        // the entire font data (including ~750KB raw_data buffer) per glyph.
        let arc = data.self_arc.get().cloned().unwrap_or_else(|| {
            // Fallback: only reached if self_arc was never set (shouldn't happen).
            #[allow(clippy::arc_with_non_send_sync)]
            std::sync::Arc::new(data.clone())
        });
        let globals = crate::autohint::globals::FaceGlobals::new(arc, style.is_italic);
        globals.get_metrics(glyph_index)
    } else {
        None
    };
    let hint_metrics = latin_metrics.or(fallback_metrics.as_deref());

    // Scale all points to 26.6.  X uses the base scale; Y uses the adjusted
    // vertical scale (x-height optimization) from latin_metrics if available.
    // Autohint metrics are scaled from the active public size before glyph
    // loading; valid FreeType size setup never supplies a zero vertical scale.
    let y_adj = hint_metrics.map_or(scale.y_scale, |m| m.axis[1].scale);
    let use_autohint = hint_metrics.is_some();

    if outline_raw.num_contours == 0 || outline_raw.points.is_empty() {
        let autohint_vertical = if use_autohint {
            // C: the auto-hinter path still updates slot vertical metrics for
            // empty outlines.  The outline bbox is zero, but the synthetic
            // vertical vector and adjusted vertical scale are preserved.
            Some(empty_autohint_vertical_metrics(
                data,
                glyph_index,
                hori_advance_fu,
                y_adj,
            ))
        } else {
            None
        };
        let mut outline = Outline {
            flags: outline_raw.outline_flags,
            ..Outline::default()
        };
        // C `TT_Load_Glyph` applies `FT_OUTLINE_HIGH_PRECISION` to every
        // scaled TrueType slot below 24 ppem, even when the glyph has no
        // points (`src/truetype/ttgload.c:2569-2577`).
        if scale.ppem < 24 {
            outline.flags = OUTLINE_HIGH_PRECISION;
        }
        return Ok((
            ScaledGlyph {
                outline,
                advance_width,
                slot_advance_width,
                phantom_pp1_x: 0,
                phantom_pp2_x: slot_advance_width,
                vertical_bearing_x_advance_width: slot_advance_width,
                lsb,
                cbox_x_min: 0,
                cbox_y_min: 0,
                cbox_x_max: 0,
                cbox_y_max: 0,
                outline_cbox_x_min: 0,
                outline_cbox_y_min: 0,
                outline_cbox_x_max: 0,
                outline_cbox_y_max: 0,
                outline_bbox_x_min: 0,
                outline_bbox_y_min: 0,
                outline_bbox_x_max: 0,
                outline_bbox_y_max: 0,
                bbox_x_min: 0,
                bbox_y_min: 0,
                bbox_x_max: 0,
                bbox_y_max: 0,
                autohint_vertical,
                native_vertical: None,
            },
            None,
        ));
    }

    // pp1.x origin shift (ttgload.c:2582). Without this, italic fonts
    // produce 26.6 coords that differ from C by 1 unit (e.g. 344→345),
    // changing the DDA prod init → pixel mismatch.
    let top_level_pp1x_fu = outline_raw.bbox_xmin - h_metric.lsb as i32;
    let hinted_pp1x_fu = if outline_raw.is_composite {
        outline_raw.xmin - outline_raw.sub_lsb
    } else {
        top_level_pp1x_fu
    };
    let pp1x_fu = if !use_autohint && !allow_bytecode {
        // C sets pp1 from the top-level glyph header before recursing into
        // components.  `FT_LOAD_NO_HINTING` scales that phantom point and
        // translates the final outline by the scaled pp1.
        top_level_pp1x_fu
    } else {
        hinted_pp1x_fu
    };
    let target_light_autohint = use_autohint
        && style.no_horizontal_hinting
        && !style.stem_adjust
        && !style.horz_snap
        && !style.vert_snap;
    let autohint_pp1x_fu = if target_light_autohint {
        // C's auto-hinter loads glyphs with `FT_LOAD_NO_SCALE` and computes
        // its own pp1 from `hints->x_delta`; light mode keeps that x delta at
        // zero (afloader.c:273-285, 407-410, 489-500).  For composites this
        // means the autohinter sees the top-level glyph origin, not the
        // selected component phantom used by native TrueType metrics.
        top_level_pp1x_fu
    } else {
        pp1x_fu
    };
    let mut phantom_pp1_x = scale.scale_x(pp1x_fu);
    let mut phantom_pp2_x = scale.scale_x(pp1x_fu + hori_advance_fu);
    let (raw_pp3_y, raw_pp4_y) = vertical_phantom_font_units(data, glyph_index, outline_raw.ymax);
    let mut phantom_pp3_y = ft_mul_fix(raw_pp3_y, y_adj);
    let mut phantom_pp4_y = ft_mul_fix(raw_pp4_y, y_adj);

    #[cfg(debug_assertions)]
    if log::log_enabled!(target: "autohint::pipeline", log::Level::Trace) {
        log::trace!(target: "autohint::pipeline", "[PP1X] gi={glyph_index} cmp={} hdr_xmin={} lsb={} pp1x_fu={pp1x_fu}",
            outline_raw.is_composite, outline_raw.xmin, h_metric.lsb);
    }

    // Shift raw outline for autohinter fx/fy edge detection — now computed
    // lazily inside the autohint block below to avoid wasted clones on
    // no-hinting and bytecode-only paths.
    let no_hinting_scaled = if !use_autohint && !allow_bytecode && outline_raw.is_composite {
        Some(crate::tt::glyf::load_glyph_scaled_no_hinting(
            &data.glyf_data,
            &data.loca_data,
            data.head.index_to_loc_format,
            glyph_index,
            &data.hmtx,
            scale.x_scale,
            y_adj,
        )?)
    } else {
        None
    };
    // FreeType translates TrueType outlines back by the scaled left phantom
    // point after loading. Apply it after scaling so FT_MulFix rounding stays
    // separate from point-coordinate rounding.
    let can_execute_native_bytecode = data.fpgm.is_some() && data.cvt.is_some();
    let native_bytecode_context = if !use_autohint && allow_bytecode {
        if let (Some(fpgm), Some(cvt)) = (&data.fpgm, &data.cvt) {
            Some(if let Some(context) = bytecode_context {
                std::borrow::Cow::Borrowed(context)
            } else {
                // C prepares the size execution state before entering
                // `TT_Hint_Glyph` (`ttobjs.c`, `ttgload.c`). Keep the direct
                // Rust scaler helpers working, but establish their context at
                // the same caller boundary and reuse it for component loads.
                std::borrow::Cow::Owned(prepare_native_bytecode_context(
                    data,
                    ScaleMetrics {
                        y_scale: y_adj,
                        ..scale
                    },
                    native_hint_mode,
                    false,
                    cvt,
                    fpgm,
                )?)
            })
        } else {
            None
        }
    } else {
        None
    };
    let bytecode_context = native_bytecode_context.as_deref();
    let no_hinting_origin_shift_x =
        if data.cff.is_none() && !use_autohint && (!allow_bytecode || !can_execute_native_bytecode)
        {
            // C `TT_Load_Glyph` translates every loaded TrueType outline by
            // `-loader.pp1.x` after scaling, even when the font has no fpgm/cvt
            // tables and no glyph bytecode runs (`src/truetype/ttgload.c:2578-2583`).
            scale.scale_x(pp1x_fu)
        } else {
            0
        };
    let mut composite_use_my_metrics_advance = None;
    let mut composite_use_my_metrics_vertical_advance = None;
    let mut composite_use_my_metrics_phantoms = None;
    let mut composite_point_tags = None;
    let mut scaled: Vec<OutlinePoint> =
        if outline_raw.is_composite && !use_autohint && allow_bytecode {
            let composite = scale_composite_components(
                data,
                &outline_raw,
                style.is_italic,
                &scale,
                legacy_hinter_phantoms,
                native_hint_mode,
                bytecode_context,
                use_hdmx,
            )?;
            composite_use_my_metrics_advance = composite.use_my_metrics_advance;
            composite_use_my_metrics_vertical_advance = composite.use_my_metrics_vertical_advance;
            composite_use_my_metrics_phantoms = composite.use_my_metrics_phantoms;
            composite_point_tags = Some(composite.tags);
            composite.points
        } else if let Some(ref no_hint_outline) = no_hinting_scaled {
            // Composite no-hinting path: pre-scaled coordinates from
            // load_glyph_scaled_no_hinting.  Use them directly.
            let shift_x = no_hinting_origin_shift_x;
            let mut scaled = Vec::with_capacity(outline_raw.points.len());
            for (index, p) in outline_raw.points.iter().enumerate() {
                let sp = &no_hint_outline.points[index];
                scaled.push(OutlinePoint {
                    x: sp.x - shift_x,
                    y: sp.y,
                    on_curve: p.on_curve,
                });
            }
            scaled
        } else if use_autohint {
            // FreeType's autofit loader reloads the glyph with
            // `FT_LOAD_NO_SCALE` before `af_glyph_hints_reload`; at that point
            // the outline has integer `gvar` deltas, not the scaled
            // `unrounded` sidecar used by the native TrueType loader.
            let shift_x = autohint_pp1x_fu;
            let origin_shift = no_hinting_origin_shift_x;
            let mut scaled = Vec::with_capacity(outline_raw.points.len());
            for p in &outline_raw.points {
                let x = scale.scale_x(p.x - shift_x);
                let y = ft_mul_fix(p.y, y_adj);
                scaled.push(OutlinePoint {
                    x: x - origin_shift,
                    y,
                    on_curve: p.on_curve,
                });
            }
            scaled
        } else {
            // No-hinting or bytecode-only: simplest scaling loop.
            // Unconditional per-point path avoids branches inside the loop.
            let shift_x = no_hinting_origin_shift_x;
            let mut scaled = Vec::with_capacity(outline_raw.points.len());
            let mut tags = Vec::with_capacity(outline_raw.points.len());
            for (index, p) in outline_raw.points.iter().enumerate() {
                let (x, y) = outline_raw.unrounded_points.as_ref().map_or_else(
                    || (scale.scale_x(p.x), ft_mul_fix(p.y, y_adj)),
                    |unrounded| {
                        let point = unrounded.get(index).copied().unwrap_or({
                            crate::tt::glyf::UnroundedPoint {
                                x: p.x << 6,
                                y: p.y << 6,
                            }
                        });
                        (
                            scale_unrounded_fdot6(point.x, scale.x_scale),
                            scale_unrounded_fdot6(point.y, y_adj),
                        )
                    },
                );
                scaled.push(OutlinePoint {
                    x: x - shift_x,
                    y,
                    on_curve: p.on_curve,
                });
                tags.push(raw_public_curve_tag(&outline_raw, p));
            }
            composite_point_tags = Some(tags);
            scaled
        };

    // ── Hinting dispatch ────────────────────────────────────────────────
    let mut tt_outline_flags = 0;
    let mut contour_dropouts = Vec::new();
    let mut point_tags = composite_point_tags.unwrap_or_else(|| {
        outline_raw
            .points
            .iter()
            .map(|point| raw_public_curve_tag(&outline_raw, point))
            .collect::<Vec<_>>()
    });
    let mut final_hint_context = None;
    if use_autohint {
        // Pass pp1x shift directly to reload via apply_hints instead of
        // building a shifted_raw GlyphOutline clone (~150ns saved).
        let hinted_advance = autohint_glyph(
            &mut scaled,
            &outline_raw, // original outline — reload applies pp1x_shift internally
            &scale,
            glyph_index,
            hint_metrics,
            style,
            data,
            HintTarget {
                is_italic: style.is_italic,
                mono: target_mono,
            },
            autohint_pp1x_fu,
        );
        if use_autohint_advance {
            if let Some(advance_width) = hinted_advance {
                slot_advance_width = advance_width;
            }
        }
    } else if allow_bytecode {
        if let Some(bytecode_context) = bytecode_context {
            // Bytecode VM: run on glyphs with per-glyph instructions.
            // Falls through to unhinted on error (graceful degradation).
            let raw_pts: Vec<OutlinePoint> = outline_raw
                .points
                .iter()
                .map(|p| OutlinePoint {
                    x: p.x,
                    y: p.y,
                    on_curve: p.on_curve,
                })
                .collect();
            let hs = crate::tt::hinter::HintScale {
                x_scale: scale.x_scale,
                y_scale: y_adj,
                tt_scale: scale.tt_scale,
                ppem: scale.ppem,
                x_ratio: scale.x_ratio,
                y_ratio: scale.y_ratio,
                point_size: scale.point_size,
                storage_size: data.maxp.max_storage as usize,
                max_function_defs: data.maxp.max_function_defs as usize,
                max_instruction_defs: data.maxp.max_instruction_defs as usize,
                twilight_points: data.maxp.max_twilight_points as usize,
                is_composite: outline_raw.is_composite,
                reset_vectors_at_glyph_entry,
                metrics_legacy_phantoms: legacy_hinter_phantoms,
                pedantic_hinting: bytecode_context.pedantic_hinting,
                native_hint_mode,
                phantom_x_override: composite_use_my_metrics_phantoms,
            };
            let hint_result = crate::tt::hinter::hint_glyph(
                &mut scaled,
                &raw_pts,
                &point_tags,
                &outline_raw.end_pts_of_contours,
                advance_width,
                hori_advance_fu,
                scale.scale_x(pp1x_fu),
                pp1x_fu,
                raw_pp3_y,
                raw_pp4_y,
                &hs,
                &outline_raw.instructions,
                bytecode_context,
            );
            match hint_result {
                Ok(outcome) => {
                    phantom_pp1_x = outcome.pp1_x;
                    phantom_pp2_x = outcome.pp2_x;
                    phantom_pp3_y = outcome.pp3_y;
                    phantom_pp4_y = outcome.pp4_y;
                    tt_outline_flags = outcome.outline_flags;
                    contour_dropouts = outcome.contour_dropouts;
                    point_tags = outcome.point_tags;
                    if let Some(advance_width) = composite_use_my_metrics_advance {
                        // C `load_truetype_glyph` keeps the subglyph phantoms
                        // when a composite component has `USE_MY_METRICS`
                        // (ttgload.c:1838-1869). In the covered DejaVu
                        // composites, parent bytecode does not alter that
                        // selected horizontal advance before metrics are read.
                        slot_advance_width = advance_width;
                    } else {
                        slot_advance_width = outcome.advance_width;
                    }
                    if outline_raw.is_composite && outline_raw.instructions.is_empty() {
                        // C keeps the selected component phantoms when
                        // `USE_MY_METRICS` is set; otherwise it restores the
                        // parent phantoms before final `-loader.pp1.x`
                        // translation (`ttgload.c:1838-1869, 2578-2583`).
                        let final_pp1_x = composite_use_my_metrics_phantoms
                            .map_or_else(|| scale.scale_x(top_level_pp1x_fu), |(pp1, _)| pp1);
                        let outline_delta = outcome.pp1_x - final_pp1_x;
                        if outline_delta != 0 {
                            for point in &mut scaled {
                                point.x += outline_delta;
                            }
                        }
                        phantom_pp1_x = final_pp1_x;
                        phantom_pp2_x = final_pp1_x.wrapping_add(outcome.advance_width);
                    }
                    final_hint_context = Some(outcome.context);
                }
                // `hint_glyph` owns the pinned `TT_Hint_Glyph` error policy:
                // it suppresses non-pedantic `TT_Run_Context` failures and
                // returns `Err` only for FT_LOAD_PEDANTIC (`ttgload.c:828-837`).
                Err(e) => return Err(e),
            }
        }
    }

    // C `tt_loader_init` enables `size->widthp` only when v40 backward
    // compatibility is inactive, notably for mono loads (ttgload.c:2280-2313).
    let hdmx_slot_advance_width = if use_hdmx
        && !legacy_hinter_phantoms
        && allow_bytecode
        && data.normalized_variation_coords.is_empty()
    {
        data.hdmx
            .as_ref()
            .and_then(|hdmx| hdmx.width_for_ppem(scale.ppem, glyph_index))
            .map(|width| i32::from(width) * 64)
    } else {
        None
    };
    if let Some(width) = hdmx_slot_advance_width {
        // C `compute_glyph_metrics` prefers `loader->widthp[glyph] * 64`
        // for hinted native TrueType loads when an hdmx ppem record is active
        // (ttgload.c:1974-1977). It affects slot metrics, not the outline
        // cbox/bbox.
        slot_advance_width = width;
    }
    let vertical_bearing_x_advance_width = if native_hint_mode == NativeHintMode::Mono {
        if hdmx_slot_advance_width.is_some() {
            slot_advance_width
        } else if let Some(advance_width) = composite_use_my_metrics_vertical_advance {
            // C `load_truetype_glyph` copies the selected subglyph phantom
            // points for `USE_MY_METRICS`; `compute_glyph_metrics` derives
            // synthetic vertical bearing from that effective phantom delta
            // (ttgload.c:1838-1869, 1980-2104).  Simple selected
            // components can already have rounded phantoms, while nested
            // composites can still expose pre-grid phantoms.
            advance_width
        } else if outline_raw.is_composite && outline_raw.instructions.is_empty() {
            // C scales composite phantom points before loading components
            // and restores them after each component that does not set
            // `USE_MY_METRICS`; without composite bytecode those raw
            // phantoms reach `compute_glyph_metrics` unchanged.
            advance_width
        } else {
            slot_advance_width
        }
    } else {
        slot_advance_width
    };

    // FT_Outline_Get_CBox: raw 26.6 min/max of the (hinted) points.
    let mut x_min = scaled[0].x;
    let mut y_min = scaled[0].y;
    let mut x_max = scaled[0].x;
    let mut y_max = scaled[0].y;
    for p in &scaled {
        x_min = x_min.min(p.x);
        y_min = y_min.min(p.y);
        x_max = x_max.max(p.x);
        y_max = y_max.max(p.y);
    }
    let outline_cbox = BBox {
        x_min,
        y_min,
        x_max,
        y_max,
    };
    let outline_bbox =
        outline_exact_bbox(&scaled, &outline_raw.end_pts_of_contours).unwrap_or(outline_cbox);

    // FT_GLYPH_BBOX_PIXELS: floor the min, ceil the max (FT_PIX_FLOOR/CEIL on 26.6),
    // then convert to integer pixels.
    let px_x_min = pixel_floor(x_min);
    let px_y_min = pixel_floor(y_min);
    let px_x_max = pixel_ceil(x_max);
    let px_y_max = pixel_ceil(y_max);
    let autohint_vertical = if use_autohint {
        Some(autohint_vertical_metrics(
            data,
            glyph_index,
            &outline_raw,
            hori_advance_fu,
            y_adj,
            x_min,
            y_max,
        ))
    } else {
        None
    };
    let native_vertical = if use_autohint {
        None
    } else {
        native_vertical_metrics(data, scale.y_scale, phantom_pp3_y, phantom_pp4_y, y_max)
    };

    // Translate outline so its pixel bbox sits at (0,0).
    // The translation preserves subpixel fractional parts (only clears the
    // integer-pixel portion via ft_pix_floor), so anti-aliasing is preserved.
    let off_x = ft_pix_floor(x_min);
    let off_y = ft_pix_floor(y_min);
    for p in &mut scaled {
        p.x -= off_x;
        p.y -= off_y;
    }

    // C `TT_Load_Glyph` sets `FT_OUTLINE_HIGH_PRECISION` for scaled TrueType
    // outlines below 24 ppem before the black rasterizer sees them
    // (`src/truetype/ttgload.c:2569-2577`).
    let mut outline_flags = outline_raw.outline_flags | tt_outline_flags;
    if scale.ppem < 24 {
        outline_flags |= OUTLINE_HIGH_PRECISION;
    }

    let outline = Outline {
        n_contours: outline_raw.num_contours as i32,
        contours: outline_raw
            .end_pts_of_contours
            .iter()
            .map(|&e| e as i16)
            .collect(),
        points: scaled,
        tags: point_tags,
        contour_dropouts,
        flags: outline_flags,
        cbox_x_min: 0,
        cbox_y_min: 0,
        cbox_x_max: px_x_max - px_x_min,
        cbox_y_max: px_y_max - px_y_min,
    };

    Ok((
        ScaledGlyph {
            outline,
            advance_width,
            slot_advance_width,
            phantom_pp1_x,
            phantom_pp2_x,
            vertical_bearing_x_advance_width,
            lsb,
            cbox_x_min: x_min,
            cbox_y_min: y_min,
            cbox_x_max: x_max,
            cbox_y_max: y_max,
            outline_cbox_x_min: outline_cbox.x_min,
            outline_cbox_y_min: outline_cbox.y_min,
            outline_cbox_x_max: outline_cbox.x_max,
            outline_cbox_y_max: outline_cbox.y_max,
            outline_bbox_x_min: outline_bbox.x_min,
            outline_bbox_y_min: outline_bbox.y_min,
            outline_bbox_x_max: outline_bbox.x_max,
            outline_bbox_y_max: outline_bbox.y_max,
            bbox_x_min: px_x_min,
            bbox_y_min: px_y_min,
            bbox_x_max: px_x_max,
            bbox_y_max: px_y_max,
            autohint_vertical,
            native_vertical,
        },
        final_hint_context,
    ))
}

pub(crate) fn should_use_default_autohint(data: &FontData) -> bool {
    let has_font_program = data.fpgm.as_ref().is_some_and(|fpgm| !fpgm.is_empty());
    let prep_len = data.prep.as_ref().map_or(0, Vec::len);

    !has_font_program && prep_len <= 7 && !data.loca_data.is_empty()
}

fn raw_public_curve_tag(outline: &GlyphOutline, point: &crate::tt::glyf::OutlinePoint) -> u8 {
    if outline.has_cubic_tags {
        point.tag & 3
    } else if point.on_curve {
        0x01
    } else {
        0x00
    }
}

fn autohint_vertical_metrics(
    data: &FontData,
    glyph_index: u16,
    raw_outline: &GlyphOutline,
    hori_advance_fu: i32,
    y_scale: i32,
    hinted_x_min: i32,
    hinted_y_max: i32,
) -> AutohintVerticalMetrics {
    let mut raw_x_min = raw_outline.points[0].x;
    let mut raw_y_min = raw_outline.points[0].y;
    let mut raw_y_max = raw_outline.points[0].y;
    for point in &raw_outline.points {
        raw_x_min = raw_x_min.min(point.x);
        raw_y_min = raw_y_min.min(point.y);
        raw_y_max = raw_y_max.max(point.y);
    }

    let (top_fu, advance_fu) =
        vertical_top_and_advance_font_units(data, glyph_index, raw_y_max - raw_y_min);

    let vvector_x = ft_mul_fix(
        -(hori_advance_fu / 2),
        ScaleMetrics::from_font_data(data).x_scale,
    );
    let vvector_y = ft_mul_fix(top_fu - raw_y_max, y_scale);

    let (bearing_x, bearing_y) = if data.cff.is_some() {
        if data.vmtx.is_some() {
            (
                ft_pix_floor(ft_pix_floor(hinted_x_min) + vvector_x),
                ft_pix_floor(ft_pix_ceil(hinted_y_max) + vvector_y),
            )
        } else {
            // CFF without `vmtx` leaves vertical bearings at zero in
            // `cff_slot_load`; `af_loader_load_glyph` then builds `vvector`
            // as zero minus the pre-hint horizontal cbox
            // (`src/cff/cffgload.c:646-742`, `src/autofit/afloader.c:506-537`).
            let x_scale = ScaleMetrics::from_font_data(data).x_scale;
            let cff_vvector_x = -ft_mul_fix(raw_x_min, x_scale);
            let cff_vvector_y = -ft_mul_fix(raw_y_max, y_scale);
            (
                ft_pix_floor(ft_pix_floor(hinted_x_min) + cff_vvector_x),
                ft_pix_floor(ft_pix_ceil(hinted_y_max) + cff_vvector_y),
            )
        }
    } else {
        (
            ft_pix_floor(ft_pix_floor(hinted_x_min) + vvector_x),
            ft_pix_floor(ft_pix_ceil(hinted_y_max) + vvector_y),
        )
    };

    AutohintVerticalMetrics {
        bearing_x,
        bearing_y,
        advance: ft_pix_round(ft_mul_fix(advance_fu, y_scale)),
    }
}

fn empty_autohint_vertical_metrics(
    data: &FontData,
    glyph_index: u16,
    hori_advance_fu: i32,
    y_scale: i32,
) -> AutohintVerticalMetrics {
    let (top_fu, advance_fu) = vertical_top_and_advance_font_units(data, glyph_index, 0);
    let x_scale = ScaleMetrics::from_font_data(data).x_scale;

    AutohintVerticalMetrics {
        bearing_x: ft_pix_floor(ft_mul_fix(-(hori_advance_fu / 2), x_scale)),
        bearing_y: ft_pix_floor(ft_mul_fix(top_fu, y_scale)),
        advance: ft_pix_round(ft_mul_fix(advance_fu, y_scale)),
    }
}

fn vertical_top_and_advance_font_units(
    data: &FontData,
    glyph_index: u16,
    height_fu: i32,
) -> (i32, i32) {
    if let Some(vmtx) = &data.vmtx {
        let vertical = vmtx.get(glyph_index);
        (vertical.tsb as i32, vertical.advance_height as i32)
    } else {
        // TrueType `compute_glyph_metrics` narrows the unscaled bbox height
        // through `FT_Short` before synthesizing vertical metrics when vmtx
        // is absent (`ttgload.c:2017-2035`). A valid full-range glyf bbox has
        // height 65535, which the pinned two's-complement target makes -1.
        let height_fu = if data.cff.is_none() {
            i32::from(i16_from_i32(height_fu))
        } else {
            height_fu
        };
        let advance_fu = vertical_advance_font_units(data);
        ((advance_fu - height_fu) / 2, advance_fu)
    }
}

fn vertical_advance_font_units(data: &FontData) -> i32 {
    if let Some(os2) = &data.os2 {
        return os2.s_typo_ascender as i32 - os2.s_typo_descender as i32;
    }
    data.hhea.ascent as i32 - data.hhea.descent as i32
}

fn vertical_phantom_font_units(data: &FontData, glyph_index: u16, y_max: i32) -> (i32, i32) {
    if let Some(vmtx) = &data.vmtx {
        let vertical = vmtx.get(glyph_index);
        let pp3_y = y_max + vertical.tsb as i32;
        return (pp3_y, pp3_y - vertical.advance_height as i32);
    }

    let (ascender, descender) = data
        .os2
        .as_ref()
        .map_or((data.hhea.ascent as i32, data.hhea.descent as i32), |os2| {
            (os2.s_typo_ascender as i32, os2.s_typo_descender as i32)
        });
    let advance = ascender.saturating_sub(descender).abs();
    (ascender, ascender - advance)
}

fn native_vertical_metrics(
    data: &FontData,
    y_scale: i32,
    pp3_y: i32,
    pp4_y: i32,
    bbox_y_max: i32,
) -> Option<NativeVerticalMetrics> {
    data.vmtx.as_ref()?;

    // C `compute_glyph_metrics` derives vmtx metrics from vertical phantoms
    // after hinting, not by scaling raw `vmtx.tsb` directly
    // (`src/truetype/ttgload.c:1991-2079`).
    let top_fu = ft_div_fix(pp3_y.wrapping_sub(bbox_y_max), y_scale) as i16 as i32;
    let advance_fu = if pp3_y <= pp4_y {
        0
    } else {
        ft_div_fix(pp3_y.wrapping_sub(pp4_y), y_scale) as u16 as i32
    };

    Some(NativeVerticalMetrics {
        bearing_y: ft_mul_fix(top_fu, y_scale),
        advance: ft_mul_fix(advance_fu, y_scale),
    })
}

#[allow(clippy::too_many_arguments)]
fn scale_composite_components(
    data: &FontData,
    outline_raw: &GlyphOutline,
    is_italic: bool,
    scale: &ScaleMetrics,
    legacy_hinter_phantoms: bool,
    native_hint_mode: NativeHintMode,
    bytecode_context: Option<&crate::tt::hinter::exec::ExecContext>,
    use_hdmx: bool,
) -> Result<CompositeScaleResult, FontError> {
    let mut points: Vec<OutlinePoint> = Vec::with_capacity(outline_raw.points.len());
    let mut tags: Vec<u8> = Vec::with_capacity(outline_raw.points.len());
    let mut use_my_metrics_advance = None;
    let mut use_my_metrics_vertical_advance = None;
    let mut use_my_metrics_phantoms = None;
    for comp in &outline_raw.components {
        let (sub, _) = scale_glyph_impl_with_context(
            data,
            comp.glyph_index,
            None,
            HintStyle {
                is_italic,
                no_horizontal_hinting: false,
                stem_adjust: true,
                horz_snap: false,
                vert_snap: false,
            },
            true,
            false,
            native_hint_mode,
            false,
            false,
            false,
            legacy_hinter_phantoms,
            bytecode_context,
            use_hdmx,
        )?;
        if comp.use_my_metrics {
            use_my_metrics_advance = Some(sub.slot_advance_width);
            use_my_metrics_vertical_advance = Some(sub.vertical_bearing_x_advance_width);
            use_my_metrics_phantoms = Some((sub.phantom_pp1_x, sub.phantom_pp2_x));
        }
        let off_x = ft_pix_floor(sub.outline_cbox_x_min);
        let off_y = ft_pix_floor(sub.outline_cbox_y_min);
        let sub_tags = sub.outline.tags.clone();
        let mut transformed = Vec::with_capacity(sub.outline.points.len());
        for (index, point) in sub.outline.points.iter().enumerate() {
            // C aborts the parent on a recursive component error, including
            // pedantic VM failures (`ttgload.c:1838-1859`). Components otherwise
            // load before the final top-level `-loader.pp1.x` translation
            // (`ttgload.c:1858-1888, 2578-2583`), so add the standalone glyph's
            // subglyph pp1 back when reconstructing component coordinates.
            let x = point.x + off_x + sub.phantom_pp1_x;
            let y = point.y + off_y;
            transformed.push(OutlinePoint {
                x: ft_mul_fix(x, comp.transform.xx) + ft_mul_fix(y, comp.transform.xy),
                y: ft_mul_fix(x, comp.transform.yx) + ft_mul_fix(y, comp.transform.yy),
                on_curve: point.on_curve,
            });
            tags.push(sub_tags.get(index).copied().unwrap_or(if point.on_curve {
                0x01
            } else {
                0x00
            }));
        }
        let (dx, dy) = if comp.args_are_xy {
            let scaled_x = scale.scale_x(comp.arg1);
            let scaled_y = scale.scale_y(comp.arg2);
            let (dx, dy) = if comp.round_xy_to_grid {
                // Mono target loads disable the legacy compatibility branch,
                // so C's rounded component X offset affects slot metrics.
                // The normal/light compatibility path keeps the historical X
                // placement used by existing pixel fixtures; Y was already
                // rounded in both branches.
                let dx = if native_hint_mode == NativeHintMode::Mono {
                    ft_pix_round(scaled_x)
                } else {
                    scaled_x
                };
                (dx, ft_pix_round(scaled_y))
            } else {
                (scaled_x, scaled_y)
            };
            (dx, dy)
        } else {
            // C: TT_Process_Composite_Component in ttgload.c:1051-1079.
            // Match the current component point to a point from previously
            // loaded components after the component transform has been applied.
            let parent = points
                .get(comp.arg1 as usize)
                .ok_or(FontError::InvalidComposite)?;
            let component = transformed
                .get(comp.arg2 as usize)
                .ok_or(FontError::InvalidComposite)?;
            (parent.x - component.x, parent.y - component.y)
        };
        for point in transformed {
            points.push(OutlinePoint {
                x: point.x + dx,
                y: point.y + dy,
                on_curve: point.on_curve,
            });
        }
    }
    Ok(CompositeScaleResult {
        points,
        tags,
        use_my_metrics_advance,
        use_my_metrics_vertical_advance,
        use_my_metrics_phantoms,
    })
}

struct CompositeScaleResult {
    points: Vec<OutlinePoint>,
    tags: Vec<u8>,
    use_my_metrics_advance: Option<i32>,
    use_my_metrics_vertical_advance: Option<i32>,
    use_my_metrics_phantoms: Option<(i32, i32)>,
}

#[derive(Debug, Clone, Copy)]
struct BBox {
    x_min: i32,
    y_min: i32,
    x_max: i32,
    y_max: i32,
}

fn update_bbox_point(bbox: &mut BBox, point: OutlinePoint) {
    bbox.x_min = bbox.x_min.min(point.x);
    bbox.y_min = bbox.y_min.min(point.y);
    bbox.x_max = bbox.x_max.max(point.x);
    bbox.y_max = bbox.y_max.max(point.y);
}

fn outline_exact_bbox(points: &[OutlinePoint], contours: &[u16]) -> Option<BBox> {
    if points.is_empty() || contours.is_empty() {
        return Some(BBox {
            x_min: 0,
            y_min: 0,
            x_max: 0,
            y_max: 0,
        });
    }

    let mut cbox = BBox {
        x_min: i32::MAX,
        y_min: i32::MAX,
        x_max: i32::MIN + 1,
        y_max: i32::MIN + 1,
    };
    let mut bbox = cbox;
    for &point in points {
        update_bbox_point(&mut cbox, point);
        if point.on_curve {
            update_bbox_point(&mut bbox, point);
        }
    }

    if cbox.x_min >= bbox.x_min
        && cbox.x_max <= bbox.x_max
        && cbox.y_min >= bbox.y_min
        && cbox.y_max <= bbox.y_max
    {
        return Some(bbox);
    }

    decompose_bbox(points, contours, bbox)
}

fn decompose_bbox(points: &[OutlinePoint], contours: &[u16], mut bbox: BBox) -> Option<BBox> {
    let mut first = 0usize;
    for &last_u16 in contours {
        let last = last_u16 as usize;
        if last < first || last >= points.len() {
            return None;
        }

        let mut start = points[first];
        let last_point = points[last];
        let mut point_index = first as isize;
        let mut limit = last;

        if !start.on_curve {
            if last_point.on_curve {
                start = last_point;
                limit = limit.saturating_sub(1);
            } else {
                start = OutlinePoint {
                    x: (start.x + last_point.x) / 2,
                    y: (start.y + last_point.y) / 2,
                    on_curve: true,
                };
            }
            point_index = first as isize - 1;
        }

        update_bbox_point(&mut bbox, start);
        let mut last_emitted = start;

        while point_index < limit as isize {
            point_index += 1;
            let point = points[point_index as usize];
            if point.on_curve {
                last_emitted = point;
                continue;
            }

            let mut control = point;
            loop {
                if point_index < limit as isize {
                    point_index += 1;
                    let next = points[point_index as usize];
                    if next.on_curve {
                        bbox_conic_to(last_emitted, control, next, &mut bbox);
                        last_emitted = next;
                        break;
                    }

                    let middle = OutlinePoint {
                        x: (control.x + next.x) / 2,
                        y: (control.y + next.y) / 2,
                        on_curve: true,
                    };
                    bbox_conic_to(last_emitted, control, middle, &mut bbox);
                    last_emitted = middle;
                    control = next;
                } else {
                    bbox_conic_to(last_emitted, control, start, &mut bbox);
                    last_emitted = start;
                    break;
                }
            }
        }

        first = last + 1;
    }

    Some(bbox)
}

fn bbox_conic_to(from: OutlinePoint, control: OutlinePoint, to: OutlinePoint, bbox: &mut BBox) {
    update_bbox_point(bbox, to);
    if control.x < bbox.x_min || control.x > bbox.x_max {
        bbox_conic_check(from.x, control.x, to.x, &mut bbox.x_min, &mut bbox.x_max);
    }
    if control.y < bbox.y_min || control.y > bbox.y_max {
        bbox_conic_check(from.y, control.y, to.y, &mut bbox.y_min, &mut bbox.y_max);
    }
}

fn bbox_conic_check(y1: i32, y2: i32, y3: i32, min: &mut i32, max: &mut i32) {
    let y1 = y1 - y2;
    let y3 = y3 - y2;
    let y = y2 + ft_mul_div(y1, y3, y1 + y3);
    *min = (*min).min(y);
    *max = (*max).max(y);
}

/// `FT_PIX_ROUND(x)` on a 26.6 value → rounded pixel (in 26.6, subpixel cleared).
#[inline]
pub fn ft_pix_round(x: i32) -> i32 {
    (x + 32) & !63
}

/// `FT_PIX_FLOOR(x)` on a 26.6 value.
#[inline]
pub fn ft_pix_floor(x: i32) -> i32 {
    x & !63
}

/// `FT_PIX_CEIL(x)` on a 26.6 value.
#[inline]
pub fn ft_pix_ceil(x: i32) -> i32 {
    (x + 63) & !63
}

/// Convert a 26.6 value to an integer pixel (truncate subpixel). Used after a
/// FT_PIX_* snap, or for raw floor.
#[inline]
pub fn to_pixel(x: i32) -> i32 {
    x >> 6
}

/// Round 26.6 to nearest pixel (FT_PIX_ROUND → int).
#[inline]
pub fn pixel_round(x: i32) -> i32 {
    to_pixel(ft_pix_round(x))
}

/// Floor 26.6 to integer pixel.
#[inline]
pub fn pixel_floor(x: i32) -> i32 {
    to_pixel(ft_pix_floor(x))
}

/// Ceil 26.6 to integer pixel.
#[inline]
pub fn pixel_ceil(x: i32) -> i32 {
    to_pixel(ft_pix_ceil(x))
}

// ── Auto-hinting bridge ───────────────────────────────────────────────────

/// Apply auto-hinting to scaled glyph coordinates.
///
/// Builds a temporary Outline structure, invokes the Latin auto-hinter
/// (`autohint::apply_hints`) which grid-fits edge positions and interpolates
/// the remaining points, then reads the results back from the outline.
///
/// Uses the adjusted vertical scale from `latin_metrics` when the x-height
/// optimization is active.
#[allow(clippy::too_many_arguments)]
fn autohint_glyph(
    scaled: &mut Vec<OutlinePoint>,
    raw_outline: &GlyphOutline,
    scale: &ScaleMetrics,
    glyph_index: u16,
    metrics: Option<&crate::autohint::AfLatinMetrics>,
    style: HintStyle,
    font_data: &FontData,
    target: HintTarget,
    pp1x_shift: i32,
) -> Option<i32> {
    use crate::outline::Outline;

    // Build a temporary Outline with scaled 26.6 coords.  Move the point
    // buffer into the outline and back out after hinting so glyph loads mirror
    // C's in-place glyph-zone mutation without cloning every point.
    let points = std::mem::take(scaled);
    let mut outline = Outline {
        n_contours: raw_outline.num_contours as i32,
        contours: raw_outline
            .end_pts_of_contours
            .iter()
            .map(|&e| e as i16)
            .collect(),
        points,
        tags: Vec::new(),
        contour_dropouts: Vec::new(),
        flags: 0,
        cbox_x_min: 0,
        cbox_y_min: 0,
        cbox_x_max: 1,
        cbox_y_max: 1,
    };

    // Run the auto-hinter.  `apply_hints` modifies `outline.points` in-place.
    // Use the adjusted vertical scale if the autohinter computed one.
    // Matches the active autohint metrics scale chosen before glyph loading.
    let y_adj = metrics.map_or(scale.y_scale, |m| m.axis[1].scale);
    let output = crate::autohint::apply_hints(
        &mut outline,
        raw_outline,
        scale.x_scale,
        y_adj,
        0,
        0,
        glyph_index,
        metrics,
        style.is_italic,
        style.no_horizontal_hinting,
        style.stem_adjust,
        style.horz_snap,
        style.vert_snap,
        Some(font_data),
        target.mono,
        pp1x_shift,
    );

    *scaled = outline.points;
    output.advance_width
}

#[derive(Debug, Clone, Copy)]
struct HintTarget {
    is_italic: bool,
    mono: bool,
}
