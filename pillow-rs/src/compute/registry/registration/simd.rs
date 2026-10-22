//! Simd operation registration.
use super::prelude::{PilError, Registry, adapters};
use super::simd_set;
fn register_section_a(m: &mut Registry) -> Result<(), PilError> {
    simd_set(m, "Invert", adapters::simd_invert)?;

    simd_set(m, "InvertChops", adapters::simd_invert_chops)?;
    Ok(())
}

fn register_section_b(m: &mut Registry) -> Result<(), PilError> {
    simd_set(m, "Solarize", adapters::simd_solarize)?;

    simd_set(m, "Posterize", adapters::simd_posterize)?;

    simd_set(m, "Grayscale", adapters::simd_grayscale)?;

    simd_set(m, "Colorize", adapters::simd_colorize)?;

    simd_set(m, "Brightness", adapters::simd_brightness)?;

    simd_set(m, "Contrast", adapters::simd_contrast)?;

    simd_set(m, "ColorSaturation", adapters::simd_color_saturation)?;

    simd_set(m, "Sharpness", adapters::simd_sharpness)?;

    simd_set(m, "Autocontrast", adapters::simd_autocontrast)?;

    simd_set(m, "Equalize", adapters::simd_equalize)?;

    simd_set(m, "RemapPalette", adapters::simd_remap_palette)?;

    simd_set(m, "Resize", adapters::simd_resize)?;

    simd_set(m, "Scale", adapters::simd_scale)?;

    simd_set(m, "Thumbnail", adapters::simd_thumbnail)?;

    simd_set(m, "Contain", adapters::simd_contain)?;

    simd_set(m, "Cover", adapters::simd_cover)?;

    simd_set(m, "Fit", adapters::simd_fit)?;

    simd_set(m, "Transform", adapters::simd_transform)?;

    simd_set(m, "Pad", adapters::simd_pad)?;

    simd_set(m, "Convert", adapters::simd_convert)?;

    simd_set(m, "ConvertLab", adapters::simd_convert_lab)?;

    simd_set(m, "Reduce", adapters::simd_reduce)?;

    simd_set(m, "EffectNoise", adapters::simd_effect_noise)?;

    simd_set(m, "EffectSpread", adapters::simd_effect_spread)?;

    simd_set(m, "Color3DLut", adapters::simd_color3dlut)?;

    simd_set(m, "Offset", adapters::simd_offset)?;
    Ok(())
}

fn register_section_c(m: &mut Registry) -> Result<(), PilError> {
    simd_set(m, "Flip", adapters::simd_flip)?;

    simd_set(m, "Mirror", adapters::simd_mirror)?;

    simd_set(m, "Filter3x3", adapters::simd_filter_3x3)?;

    simd_set(m, "Filter5x5", adapters::simd_filter_5x5)?;

    simd_set(m, "MaxFilter", adapters::simd_max_filter)?;

    simd_set(m, "MinFilter", adapters::simd_min_filter)?;

    simd_set(m, "MedianFilter", adapters::simd_median_filter)?;

    simd_set(m, "RankFilter", adapters::simd_rank_filter)?;

    simd_set(m, "BoxBlur", adapters::simd_box_blur)?;

    simd_set(m, "GaussianBlur", adapters::simd_gaussian_blur)?;
    Ok(())
}

fn register_section_d(m: &mut Registry) -> Result<(), PilError> {
    simd_set(m, "Add", adapters::simd_add)?;

    simd_set(m, "Subtract", adapters::simd_subtract)?;

    simd_set(m, "Multiply", adapters::simd_multiply)?;

    simd_set(m, "Screen", adapters::simd_screen)?;

    simd_set(m, "Darker", adapters::simd_darker)?;

    simd_set(m, "Lighter", adapters::simd_lighter)?;

    simd_set(m, "Difference", adapters::simd_difference)?;

    simd_set(m, "Overlay", adapters::simd_overlay)?;

    simd_set(m, "HardLight", adapters::simd_hard_light)?;

    simd_set(m, "SoftLight", adapters::simd_soft_light)?;

    simd_set(m, "AddModulo", adapters::simd_add_modulo)?;

    simd_set(m, "SubtractModulo", adapters::simd_subtract_modulo)?;

    simd_set(m, "LogicalAnd", adapters::simd_logical_and)?;

    simd_set(m, "LogicalOr", adapters::simd_logical_or)?;

    simd_set(m, "LogicalXor", adapters::simd_logical_xor)?;
    Ok(())
}

fn register_section_e(m: &mut Registry) -> Result<(), PilError> {
    simd_set(m, "Transpose", adapters::simd_transpose)?;

    simd_set(m, "CropBorder", adapters::simd_crop_border)?;

    simd_set(m, "Crop", adapters::simd_crop)?;

    simd_set(m, "Expand", adapters::simd_expand)?;

    simd_set(m, "Constant", adapters::simd_constant)?;

    simd_set(m, "Duplicate", adapters::simd_duplicate)?;

    simd_set(m, "Rotate", adapters::simd_rotate)?;

    simd_set(m, "PutAlpha", adapters::simd_put_alpha)?;

    simd_set(m, "PutAlphaData", adapters::simd_put_alpha_data)?;

    simd_set(m, "PutPixel", adapters::simd_put_pixel)?;

    simd_set(m, "DrawLine", adapters::simd_draw_line)?;

    simd_set(m, "DrawPoint", adapters::simd_draw_point)?;

    simd_set(m, "DrawRectangle", adapters::simd_draw_rectangle)?;

    simd_set(m, "DrawRoundedRect", adapters::simd_draw_rounded_rect)?;

    simd_set(m, "DrawPolygon", adapters::simd_draw_polygon)?;

    simd_set(m, "DrawEllipse", adapters::simd_draw_ellipse)?;

    simd_set(m, "DrawCircle", adapters::simd_draw_circle)?;

    simd_set(m, "DrawArc", adapters::simd_draw_arc)?;

    simd_set(m, "DrawChord", adapters::simd_draw_chord)?;

    simd_set(m, "DrawPieslice", adapters::simd_draw_pieslice)?;

    simd_set(m, "Eval", adapters::simd_eval)?;

    simd_set(m, "PutData", adapters::simd_put_data)?;

    simd_set(m, "Paste", adapters::simd_paste)?;

    simd_set(m, "Merge", adapters::simd_merge)?;

    simd_set(m, "BlendModule", adapters::simd_blend_module)?;

    simd_set(m, "CompositeModule", adapters::simd_composite_module)?;

    simd_set(m, "AlphaComposite", adapters::simd_alpha_composite)?;

    simd_set(m, "ExtractBand", adapters::simd_extract_band)?;
    Ok(())
}

pub(super) fn register(m: &mut Registry) -> Result<(), PilError> {
    register_section_a(m)?;
    register_section_b(m)?;
    register_section_c(m)?;
    register_section_d(m)?;
    register_section_e(m)?;
    Ok(())
}
