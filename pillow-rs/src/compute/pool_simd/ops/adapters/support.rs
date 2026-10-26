//! Contextual admission checks for native SIMD operations.

use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;

use super::layout::SimdImageShape;

#[derive(Clone, Copy)]
enum SupportFamily {
    Pointwise,
    Resize,
    Layout,
    Neighborhood,
    Effects,
    Drawing,
    Mutation,
    Chops,
    Color3dLut,
}

fn support_family(op: &PipelineOp) -> SupportFamily {
    match op {
        PipelineOp::Invert
        | PipelineOp::InvertChops
        | PipelineOp::Grayscale
        | PipelineOp::Colorize { .. }
        | PipelineOp::Solarize { .. }
        | PipelineOp::Posterize { .. }
        | PipelineOp::Brightness { .. }
        | PipelineOp::Contrast { .. }
        | PipelineOp::ColorSaturation { .. }
        | PipelineOp::Sharpness { .. }
        | PipelineOp::Autocontrast { .. }
        | PipelineOp::Equalize
        | PipelineOp::EqualizeMasked { .. }
        | PipelineOp::RemapPalette { .. }
        | PipelineOp::Eval { .. } => SupportFamily::Pointwise,
        PipelineOp::ResizeBoxed { .. }
        | PipelineOp::Resize { .. }
        | PipelineOp::Scale { .. }
        | PipelineOp::Contain { .. }
        | PipelineOp::Cover { .. }
        | PipelineOp::Fit { .. }
        | PipelineOp::Thumbnail { .. }
        | PipelineOp::Transform { .. }
        | PipelineOp::Pad { .. }
        | PipelineOp::Convert { .. }
        | PipelineOp::ConvertLab { .. }
        | PipelineOp::Reduce { .. }
        | PipelineOp::Rotate { .. } => SupportFamily::Resize,
        PipelineOp::PutData { .. }
        | PipelineOp::ExtractBand { .. }
        | PipelineOp::Offset { .. }
        | PipelineOp::Flip
        | PipelineOp::Mirror
        | PipelineOp::Transpose { .. }
        | PipelineOp::Crop { .. }
        | PipelineOp::CropBorder { .. }
        | PipelineOp::Expand { .. }
        | PipelineOp::Constant { .. }
        | PipelineOp::Duplicate => SupportFamily::Layout,
        PipelineOp::Filter3x3 { .. }
        | PipelineOp::Filter5x5 { .. }
        | PipelineOp::BoxBlur { .. }
        | PipelineOp::BoxBlurXY { .. }
        | PipelineOp::GaussianBlur { .. }
        | PipelineOp::MaxFilter { .. }
        | PipelineOp::MinFilter { .. }
        | PipelineOp::MedianFilter { .. }
        | PipelineOp::RankFilter { .. } => SupportFamily::Neighborhood,
        PipelineOp::EffectNoise { .. } | PipelineOp::EffectSpread { .. } => SupportFamily::Effects,
        PipelineOp::DrawLine { .. }
        | PipelineOp::DrawPoint { .. }
        | PipelineOp::DrawEllipse { .. }
        | PipelineOp::DrawCircle { .. }
        | PipelineOp::DrawArc { .. }
        | PipelineOp::DrawChord { .. }
        | PipelineOp::DrawPieslice { .. }
        | PipelineOp::DrawRoundedRect { .. }
        | PipelineOp::DrawPolygon { .. }
        | PipelineOp::DrawRectangle { .. } => SupportFamily::Drawing,
        PipelineOp::PutAlpha { .. }
        | PipelineOp::PutAlphaData { .. }
        | PipelineOp::PutPixel { .. }
        | PipelineOp::AlphaComposite { .. }
        | PipelineOp::CompositeModule { .. }
        | PipelineOp::Paste { .. }
        | PipelineOp::Merge { .. }
        | PipelineOp::BlendModule { .. } => SupportFamily::Mutation,
        PipelineOp::Multiply { .. }
        | PipelineOp::Screen { .. }
        | PipelineOp::Darker { .. }
        | PipelineOp::Lighter { .. }
        | PipelineOp::Difference { .. }
        | PipelineOp::AddModulo { .. }
        | PipelineOp::SubtractModulo { .. }
        | PipelineOp::Overlay { .. }
        | PipelineOp::HardLight { .. }
        | PipelineOp::SoftLight { .. }
        | PipelineOp::LogicalAnd { .. }
        | PipelineOp::LogicalOr { .. }
        | PipelineOp::LogicalXor { .. }
        | PipelineOp::Add { .. }
        | PipelineOp::Subtract { .. } => SupportFamily::Chops,
        PipelineOp::Color3DLut { .. } => SupportFamily::Color3dLut,
    }
}

/// Check contextual image/layout conditions after the operation allow-list.
/// Scalar-only legacy adapters are rejected before execution so automatic
/// routing can select CPU and strict SIMD requests fail before pixel work.
pub(crate) fn simd_supports_for_image(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> bool {
    if !crate::compute::registry::simd_supports(op).unwrap_or(false) {
        return false;
    }
    let family = support_family(op);
    simd_image_family_support(family, img, op, mode)
}

fn simd_image_family_support(
    family: SupportFamily,
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> bool {
    match family {
        SupportFamily::Pointwise => image_pointwise_support(img, op, mode),
        SupportFamily::Resize => image_resize_support(img, op, mode),
        SupportFamily::Layout => image_layout_support(img, op, mode),
        SupportFamily::Neighborhood => image_neighborhood_support(img, op, mode),
        SupportFamily::Effects => image_effects_support(img, op, mode),
        SupportFamily::Drawing => image_drawing_support(img, op, mode),
        SupportFamily::Mutation => image_mutation_support(img, op, mode),
        SupportFamily::Chops => super::chops::supports_for_image(img, op, mode),
        SupportFamily::Color3dLut => image_color3dlut_support(img, op, mode),
    }
}

fn image_pointwise_support(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> bool {
    match op {
        PipelineOp::Invert | PipelineOp::InvertChops => {
            super::effects::supports_for_image(img, op, mode)
        }
        PipelineOp::Grayscale => super::grayscale::supports_image(img, mode),
        PipelineOp::Colorize { .. } => super::colorize::supports_for_image(img, mode),
        PipelineOp::Solarize { .. }
        | PipelineOp::Posterize { .. }
        | PipelineOp::Brightness { .. }
        | PipelineOp::Contrast { .. }
        | PipelineOp::ColorSaturation { .. }
        | PipelineOp::Sharpness { .. } => super::enhance::supports_for_image(img, op, mode),
        PipelineOp::Autocontrast { .. }
        | PipelineOp::Equalize
        | PipelineOp::EqualizeMasked { .. } => super::histogram::supports_for_image(img, op, mode),
        PipelineOp::RemapPalette { .. } | PipelineOp::Eval { .. } => {
            super::lut::supports_for_image(img, op, mode)
        }
        _ => false,
    }
}

fn image_resize_support(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> bool {
    match op {
        PipelineOp::ResizeBoxed { .. }
        | PipelineOp::Resize { .. }
        | PipelineOp::Scale { .. }
        | PipelineOp::Contain { .. }
        | PipelineOp::Cover { .. }
        | PipelineOp::Fit { .. } => super::resize::supports_for_image(img, op, mode),
        PipelineOp::Thumbnail { w, h, filter } => {
            super::thumbnail::native_thumbnail_supported_for_image(img, *w, *h, *filter, mode)
        }
        PipelineOp::Transform {
            w,
            h,
            method,
            data,
            filter,
            ..
        } => super::transform::supports_for_image(img, *w, *h, method, data, *filter, mode),
        PipelineOp::Pad { .. } => super::pad::supports_for_image(img, op, mode),
        PipelineOp::Convert {
            mode: target,
            matrix,
            dither: _,
        } => super::native_convert_supported_for_image(img, target, matrix.as_deref(), mode),
        PipelineOp::ConvertLab { .. } => super::convert::supports_lab_for_image(img, mode),
        PipelineOp::Reduce { x_factor, y_factor } => {
            super::native_reduce_supported_for_image(img, *x_factor, *y_factor, mode)
        }
        PipelineOp::Rotate { .. } => super::rotate::supports_for_image(img, op, mode),
        _ => false,
    }
}

fn image_layout_support(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> bool {
    match op {
        PipelineOp::PutData {
            mode: data_mode, ..
        } => super::put_data::supports_for_image(img, *data_mode, mode),
        PipelineOp::ExtractBand { index } => {
            super::extract_band::supports_for_image(img, *index, mode)
        }
        PipelineOp::Offset { x, .. } => super::offset::supports_for_image(img, *x, mode),
        PipelineOp::Flip | PipelineOp::Mirror => super::flip::supports_for_image(img, op, mode),
        PipelineOp::Transpose { .. } => super::transpose::supports_for_image(img, mode),
        PipelineOp::Crop { .. } | PipelineOp::CropBorder { .. } => {
            super::crop::supports_for_image(img, op, mode)
        }
        PipelineOp::Expand { .. } => super::expand::supports_for_image(img, op, mode),
        PipelineOp::Constant { .. } => super::chops::supports_constant_for_image(img),
        PipelineOp::Duplicate => super::duplicate::supports_for_image(img, mode),
        _ => false,
    }
}

fn image_neighborhood_support(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> bool {
    match op {
        PipelineOp::Filter3x3 { .. } | PipelineOp::Filter5x5 { .. } => {
            super::filter::supports_for_image(img, op, mode)
        }
        PipelineOp::BoxBlur { .. } | PipelineOp::BoxBlurXY { .. } => {
            super::box_blur::supports_for_image(img, op, mode)
        }
        PipelineOp::GaussianBlur { sigma } => {
            super::gaussian_blur::supports_for_image(img, *sigma, mode)
        }
        PipelineOp::MaxFilter { .. }
        | PipelineOp::MinFilter { .. }
        | PipelineOp::MedianFilter { .. }
        | PipelineOp::RankFilter { .. } => super::median_filter::supports_for_image(img, op, mode),
        _ => false,
    }
}

fn image_effects_support(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> bool {
    super::effects::supports_for_image(img, op, mode)
}

fn image_drawing_support(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> bool {
    super::draw::supports_for_image(img, op, mode)
}

fn image_mutation_support(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> bool {
    match op {
        PipelineOp::PutAlpha { .. } | PipelineOp::PutAlphaData { .. } => {
            super::put_alpha::supports_for_image(img, op, mode)
        }
        PipelineOp::PutPixel {
            x,
            y,
            palette_index: _,
            ..
        } => super::put_pixel::native_putpixel_supported_for_image(img, *x, *y, mode),
        PipelineOp::AlphaComposite { .. } => {
            super::alpha_composite::supports_for_image(img, op, mode)
        }
        PipelineOp::CompositeModule {
            other,
            mask,
            mask_alpha,
        } => super::paste::native_composite_plan_for_image(img, other, mask, *mask_alpha, mode)
            .is_some(),
        PipelineOp::Paste { .. } => image_paste_support(img, op, mode),
        PipelineOp::Merge {
            mode: target_mode,
            bands,
            logical_mode,
        } => super::merge::supports_image_contract(img, target_mode, bands, mode, logical_mode),
        PipelineOp::BlendModule { .. } => super::module_blend::supports_for_image(img, op, mode),
        _ => false,
    }
}

fn image_paste_support(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> bool {
    let PipelineOp::Paste {
        source,
        x,
        y,
        w,
        h,
        mask,
        mask_alpha,
    } = op
    else {
        return false;
    };
    super::native_paste_plan_for_image(
        img,
        source,
        *x,
        *y,
        *w,
        *h,
        mask.as_ref(),
        *mask_alpha,
        mode,
    )
    .is_some()
        || super::native_paste_rgba_to_rgb_supported(img, source, *w, *h, mask.as_ref(), mode)
        || super::native_paste_rgb_to_rgba_supported(img, source, *w, *h, mask.as_ref(), mode)
}

fn image_color3dlut_support(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> bool {
    let PipelineOp::Color3DLut {
        size,
        table,
        channels,
        source_mode,
        target_mode,
    } = op
    else {
        return false;
    };
    super::color3dlut_supported_for_image(
        img,
        *size,
        table.len(),
        *channels,
        *source_mode,
        *target_mode,
        mode,
    )
}

pub(super) fn simd_supports_for_shape(
    shape: SimdImageShape,
    op: &PipelineOp,
    mode: Option<&str>,
) -> bool {
    let family = support_family(op);
    simd_shape_family_support(family, shape, op, mode)
}

fn simd_shape_family_support(
    family: SupportFamily,
    shape: SimdImageShape,
    op: &PipelineOp,
    mode: Option<&str>,
) -> bool {
    match family {
        SupportFamily::Pointwise => shape_pointwise_support(shape, op, mode),
        SupportFamily::Resize => shape_resize_support(shape, op, mode),
        SupportFamily::Layout => shape_layout_support(shape, op, mode),
        SupportFamily::Neighborhood => shape_neighborhood_support(shape, op, mode),
        SupportFamily::Effects => super::effects::supports_for_shape(shape, op, mode),
        SupportFamily::Drawing => super::draw::supports_for_shape(shape, op, mode),
        SupportFamily::Mutation => shape_mutation_support(shape, op, mode),
        SupportFamily::Chops => super::chops::supports_for_shape(shape, op, mode),
        SupportFamily::Color3dLut => shape_color3dlut_support(shape, op, mode),
    }
}

fn shape_pointwise_support(shape: SimdImageShape, op: &PipelineOp, mode: Option<&str>) -> bool {
    match op {
        PipelineOp::Invert | PipelineOp::InvertChops => {
            super::effects::supports_for_shape(shape, op, mode)
        }
        PipelineOp::Grayscale => super::grayscale::supports_shape(shape, mode),
        PipelineOp::Colorize { .. } => super::colorize::supports_for_shape(shape, mode),
        PipelineOp::Solarize { .. }
        | PipelineOp::Posterize { .. }
        | PipelineOp::Brightness { .. }
        | PipelineOp::Contrast { .. }
        | PipelineOp::ColorSaturation { .. }
        | PipelineOp::Sharpness { .. } => super::enhance::supports_for_shape(shape, op, mode),
        PipelineOp::Autocontrast { .. }
        | PipelineOp::Equalize
        | PipelineOp::EqualizeMasked { .. } => {
            super::histogram::supports_for_shape(shape, op, mode)
        }
        PipelineOp::RemapPalette { .. } | PipelineOp::Eval { .. } => {
            super::lut::supports_for_shape(shape, op, mode)
        }
        _ => false,
    }
}

fn shape_resize_support(shape: SimdImageShape, op: &PipelineOp, mode: Option<&str>) -> bool {
    match op {
        PipelineOp::ResizeBoxed { .. }
        | PipelineOp::Resize { .. }
        | PipelineOp::Scale { .. }
        | PipelineOp::Contain { .. }
        | PipelineOp::Cover { .. }
        | PipelineOp::Fit { .. } => super::resize::supports_for_shape(shape, op, mode),
        PipelineOp::Thumbnail { w, h, filter } => {
            super::thumbnail::native_thumbnail_supported_for_shape(shape, *w, *h, *filter, mode)
        }
        PipelineOp::Transform {
            w,
            h,
            method,
            data,
            filter,
            ..
        } => super::transform::supports_for_shape(shape, *w, *h, method, data, *filter, mode),
        PipelineOp::Pad { .. } => super::pad::supports_for_shape(shape, op, mode),
        PipelineOp::Convert {
            mode: target,
            matrix,
            dither: _,
        } => super::native_convert_supported_for_shape(shape, target, matrix.as_deref(), mode),
        PipelineOp::ConvertLab { .. } => super::convert::supports_lab_for_shape(shape, mode),
        PipelineOp::Reduce { x_factor, y_factor } => {
            super::native_reduce_supported_for_shape(shape, *x_factor, *y_factor, mode)
        }
        PipelineOp::Rotate { .. } => super::rotate::supports_for_shape(shape, op, mode),
        _ => false,
    }
}

fn shape_layout_support(shape: SimdImageShape, op: &PipelineOp, mode: Option<&str>) -> bool {
    match op {
        PipelineOp::PutData {
            mode: data_mode, ..
        } => super::put_data::supports_for_shape(shape, *data_mode, mode),
        PipelineOp::ExtractBand { index } => {
            super::extract_band::supports_for_shape(shape, *index, mode)
        }
        PipelineOp::Offset { x, .. } => super::offset::supports_for_shape(shape, *x, mode),
        PipelineOp::Flip | PipelineOp::Mirror => super::flip::supports_for_shape(shape, op, mode),
        PipelineOp::Transpose { .. } => super::transpose::supports_for_shape(shape, mode),
        PipelineOp::Crop { .. } | PipelineOp::CropBorder { .. } => {
            super::crop::supports_for_shape(shape, op, mode)
        }
        PipelineOp::Expand { .. } => super::expand::supports_for_shape(shape, op, mode),
        PipelineOp::Constant { .. } => super::chops::supports_constant_for_shape(shape),
        PipelineOp::Duplicate => super::duplicate::supports_for_shape(shape, mode),
        _ => false,
    }
}

fn shape_neighborhood_support(shape: SimdImageShape, op: &PipelineOp, mode: Option<&str>) -> bool {
    match op {
        PipelineOp::Filter3x3 { .. } | PipelineOp::Filter5x5 { .. } => {
            super::filter::supports_for_shape(shape, op, mode)
        }
        PipelineOp::BoxBlur { .. } | PipelineOp::BoxBlurXY { .. } => {
            super::box_blur::supports_for_shape(shape, op, mode)
        }
        PipelineOp::GaussianBlur { sigma } => {
            super::gaussian_blur::supports_for_shape(shape, *sigma, mode)
        }
        PipelineOp::MaxFilter { .. }
        | PipelineOp::MinFilter { .. }
        | PipelineOp::MedianFilter { .. }
        | PipelineOp::RankFilter { .. } => {
            super::median_filter::supports_for_shape(shape, op, mode)
        }
        _ => false,
    }
}

fn shape_mutation_support(shape: SimdImageShape, op: &PipelineOp, mode: Option<&str>) -> bool {
    match op {
        PipelineOp::PutAlpha { .. } | PipelineOp::PutAlphaData { .. } => {
            super::put_alpha::supports_for_shape(shape, op, mode)
        }
        PipelineOp::PutPixel {
            x,
            y,
            palette_index: _,
            ..
        } => super::put_pixel::native_putpixel_supported_for_shape(shape, *x, *y, mode),
        PipelineOp::AlphaComposite { .. } => {
            super::alpha_composite::supports_for_shape(shape, op, mode)
        }
        PipelineOp::CompositeModule {
            other,
            mask,
            mask_alpha,
        } => super::paste::native_composite_plan_for_shape(shape, other, mask, *mask_alpha, mode)
            .is_some(),
        PipelineOp::Paste { .. } => shape_paste_support(shape, op, mode),
        PipelineOp::Merge {
            mode: target_mode,
            bands,
            logical_mode,
        } => super::merge::supports_shape_contract(shape, target_mode, bands, mode, logical_mode),
        PipelineOp::BlendModule { .. } => super::module_blend::supports_for_shape(shape, op, mode),
        _ => false,
    }
}

fn shape_paste_support(shape: SimdImageShape, op: &PipelineOp, mode: Option<&str>) -> bool {
    let PipelineOp::Paste {
        source,
        x,
        y,
        w,
        h,
        mask,
        mask_alpha,
    } = op
    else {
        return false;
    };
    super::native_paste_plan_for_shape(
        shape,
        source,
        *x,
        *y,
        *w,
        *h,
        mask.as_ref(),
        *mask_alpha,
        mode,
    )
    .is_some()
        || super::native_paste_rgba_to_rgb_supported_for_shape(
            shape,
            source,
            *w,
            *h,
            mask.as_ref(),
            mode,
        )
        || super::native_paste_rgb_to_rgba_supported_for_shape(
            shape,
            source,
            *w,
            *h,
            mask.as_ref(),
            mode,
        )
}

fn shape_color3dlut_support(shape: SimdImageShape, op: &PipelineOp, mode: Option<&str>) -> bool {
    let PipelineOp::Color3DLut {
        size,
        table,
        channels,
        source_mode,
        target_mode,
    } = op
    else {
        return false;
    };
    super::color3dlut_supported_for_shape(
        shape,
        *size,
        table.len(),
        *channels,
        *source_mode,
        *target_mode,
        mode,
    )
}
