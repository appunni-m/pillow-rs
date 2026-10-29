//! CPU Thumbnail family orchestration with wrapper-owned Pillow mode routing.

use crate::compute::{Backend, ExecutionContext};
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::ops::pil_resize::{pil_resize, pil_resize_boxed_in_context};
use crate::pipeline::{PipelineOp, ResampleFilter};
use crate::raster::{DynamicImage, GenericImageView};

use super::reduce::{
    execute_reduce, execute_reduce_rgb_with_sparse_rows, reduce_f_thumbnail, reduce_i_thumbnail,
};
use super::resize::{resize_f_in_context, resize_i_boxed_in_context, resize_i_in_context};

mod fusion;
mod kernels;
pub(crate) use fusion::execute_rgb_putpixel_thumbnail_fusion;

enum ThumbnailPreparation {
    Completed(DynamicImage),
    Ready {
        image: DynamicImage,
        resize_box: Option<(f64, f64, f64, f64)>,
    },
}

/// Execute a Thumbnail operation.
/// Computes the scale factor to fit within the given box, preserving aspect ratio.
/// Matches PIL's thumbnail behavior including the reducing_gap optimization
/// (default reducing_gap=2.0) for non-NEAREST filters.
pub(crate) fn execute_thumbnail(
    img: &DynamicImage,
    w: u32,
    h: u32,
    filter: &ResampleFilter,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    execute_thumbnail_in_context(
        &ExecutionContext::for_backend(Backend::Cpu),
        img,
        w,
        h,
        filter,
        explicit_mode,
    )
}

pub(crate) fn execute_thumbnail_in_context(
    context: &ExecutionContext,
    img: &DynamicImage,
    w: u32,
    h: u32,
    filter: &ResampleFilter,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (cur_w, cur_h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return Err(PilError::ValueError("thumbnail size must be > 0".into()));
    }
    // `Image::thumbnail` performs Pillow's aspect-preserving `round_aspect`
    // calculation before queuing this operation so lazy shape metadata and
    // the eventual pixels agree. The operation therefore carries the final
    // dimensions; do not apply the aspect calculation a second time here.
    let new_w = w.max(1).min(cur_w);
    let new_h = h.max(1).min(cur_h);
    // PIL forces NEAREST for mode "1" and "P" to avoid non-binary/interpolated values
    let effective_filter = match explicit_mode {
        Some("1") | Some("P") => ResampleFilter::Nearest,
        _ => *filter,
    };
    let preparation = prepare_thumbnail_work_image(
        img,
        (cur_w, cur_h),
        (new_w, new_h),
        effective_filter,
        explicit_mode,
        context,
    )?;
    let (work_img, resize_box) = match preparation {
        ThumbnailPreparation::Completed(result) => return Ok(preserve_mode(img, result)),
        ThumbnailPreparation::Ready { image, resize_box } => (image, resize_box),
    };
    let result = resize_prepared_thumbnail(
        context,
        &work_img,
        (cur_w, cur_h),
        (new_w, new_h),
        effective_filter,
        explicit_mode,
        resize_box,
    )?;
    Ok(preserve_mode(img, result))
}

#[inline(always)]
fn resize_prepared_thumbnail(
    context: &ExecutionContext,
    work_img: &DynamicImage,
    (cur_w, cur_h): (u32, u32),
    (new_w, new_h): (u32, u32),
    effective_filter: ResampleFilter,
    explicit_mode: Option<&str>,
    resize_box: Option<(f64, f64, f64, f64)>,
) -> Result<DynamicImage, PilError> {
    // Only use F/I thumbnail paths when the image is already stored as Rgba8
    // (4 bytes per pixel), meaning it has been converted to F/I mode already.
    // If the image is still RGB or other format, use normal thumbnail regardless
    // of explicit_mode, because the F/I convert hasn't happened yet in the pipeline.
    let result = match (explicit_mode, work_img, resize_box) {
        (Some("F"), DynamicImage::ImageRgba8(_), Some((left, top, right, bottom)))
            if cur_w % 2 == 0
                && cur_h % 2 == 0
                && cur_w / 2 == work_img.width()
                && cur_h / 2 == work_img.height()
                && left == 0.0
                && top == 0.0
                && right == work_img.width() as f64
                && bottom == work_img.height() as f64
                && new_w < work_img.width()
                && new_h < work_img.height() =>
        {
            resize_f_in_context(context, work_img, new_w, new_h, &effective_filter)?
        }
        (Some("F"), DynamicImage::ImageRgba8(_), Some((left, top, right, bottom))) => {
            // Image.resize adjusts the source box after its reducing-gap pass.
            // Keep that fractional box for F as well; resizing the complete
            // ceil-sized reduction includes partial edge samples that Pillow
            // deliberately excludes from the final convolution.
            crate::ops::pil_resize::pil_resize_boxed_in_context(
                context,
                work_img,
                new_w,
                new_h,
                left,
                top,
                right,
                bottom,
                effective_filter,
                explicit_mode,
            )
        }
        (Some("F"), DynamicImage::ImageRgba8(_), None) => {
            resize_f_in_context(context, work_img, new_w, new_h, &effective_filter)?
        }
        (Some("I"), DynamicImage::ImageRgba8(_), Some((left, top, right, bottom))) => {
            resize_i_boxed_in_context(
                context,
                work_img,
                new_w,
                new_h,
                left,
                top,
                right,
                bottom,
                effective_filter,
            )?
        }
        (Some("I"), DynamicImage::ImageRgba8(_), None) => {
            resize_i_in_context(context, work_img, new_w, new_h, &effective_filter)?
        }
        (_, _, Some((left, top, right, bottom))) => pil_resize_boxed_in_context(
            context,
            work_img,
            new_w,
            new_h,
            left,
            top,
            right,
            bottom,
            effective_filter,
            explicit_mode,
        ),
        _ => pil_resize(work_img, new_w, new_h, effective_filter, explicit_mode),
    };
    Ok(result)
}

fn prepare_thumbnail_work_image(
    context: &ExecutionContext,
    img: &DynamicImage,
    (cur_w, cur_h): (u32, u32),
    (new_w, new_h): (u32, u32),
    effective_filter: ResampleFilter,
    explicit_mode: Option<&str>,
) -> Result<ThumbnailPreparation, PilError> {
    // PIL's thumbnail uses reducing_gap=2.0 by default: first integer-reduce
    // by up to scale/reducing_gap, then resize the rest.
    // This matches PIL's ImagingReduce then ImagingResample two-step.
    // Skip reducing_gap for modes with alpha (LA, RGBA) to avoid premultiply
    // issues. F/I use RGBA storage internally but are scalar modes, and CMYK
    // uses the same four-byte storage without an alpha channel.
    // TODO(pillow-rs#3ptb): Select alpha and scalar behavior from the wrapper's
    // cached Mode, not the carrier color plus compatibility mode string.
    let has_alpha = matches!(
        img.color(),
        crate::raster::ColorType::La8 | crate::raster::ColorType::Rgba8
    ) && !matches!(explicit_mode, Some("F" | "I" | "CMYK" | "RGBa" | "RGBX"));
    let needs_reduce = !matches!(effective_filter, ResampleFilter::Nearest) && !has_alpha;
    let mut resize_box = None;
    let work_img = if needs_reduce {
        let scale_x = cur_w as f64 / new_w as f64;
        let scale_y = cur_h as f64 / new_h as f64;
        // Image.resize computes these independently for the two axes before
        // calling Image.reduce(factor=(factor_x, factor_y)).
        let factor_x = ((scale_x / 2.0) as u32).max(1);
        let factor_y = ((scale_y / 2.0) as u32).max(1);
        if factor_x > 1 || factor_y > 1 {
            // Image.resize keeps the original full-image box after reduce and
            // scales its right/bottom edges by the integer factors. When the
            // source dimensions are not divisible by a factor, that box ends
            // inside the ceil-sized reduced image; using the whole reduced
            // image changes boundary pixels (Pillow's _get_safe_box path).
            resize_box = Some((
                0.0,
                0.0,
                cur_w as f64 / factor_x as f64,
                cur_h as f64 / factor_y as f64,
            ));
            match (explicit_mode, img) {
                // I has a serial even 2x2 path that streams directly into its
                // resize kernel. Native F/I samples call the active scalar
                // reducers directly; other modes use Reduce's family router.
                (Some("I"), DynamicImage::ImageRgba8(_))
                    if factor_x == 2 && factor_y == 2 && cur_w % 2 == 0 && cur_h % 2 == 0 =>
                {
                    return Ok(ThumbnailPreparation::Completed(
                        kernels::i::resize_i_thumbnail_reduce2x2(
                            context,
                            img,
                            new_w,
                            new_h,
                            effective_filter,
                        )?,
                    ));
                }
                (Some("F"), DynamicImage::ImageRgba8(_)) => reduce_f_thumbnail(
                    img,
                    cur_w.div_ceil(factor_x),
                    cur_h.div_ceil(factor_y),
                    factor_x,
                    factor_y,
                )?,
                (Some("I"), DynamicImage::ImageRgba8(_)) => reduce_i_thumbnail(
                    img,
                    cur_w.div_ceil(factor_x),
                    cur_h.div_ceil(factor_y),
                    factor_x,
                    factor_y,
                )?,
                _ => execute_reduce(img, factor_x, factor_y, explicit_mode)?,
            }
        } else {
            img.clone()
        }
    } else {
        img.clone()
    };
    Ok(ThumbnailPreparation::Ready {
        image: work_img,
        resize_box,
    })
}

/// Return whether a validated RGB pixel-write prefix can be folded into a
/// 2×2 non-nearest thumbnail reduction without materializing a full source clone.
pub(crate) fn rgb_putpixel_thumbnail_fusion_supported(
    img: &DynamicImage,
    putpixel_ops: &[PipelineOp],
    thumbnail: &PipelineOp,
    mode: Option<&str>,
) -> bool {
    let DynamicImage::ImageRgb8(_) = img else {
        return false;
    };
    if !matches!(mode, None | Some("RGB"))
        || putpixel_ops.is_empty()
        || !putpixel_ops.iter().all(|op| {
            matches!(
                op,
                PipelineOp::PutPixel {
                    palette_index: false,
                    ..
                }
            )
        })
    {
        return false;
    }
    let PipelineOp::Thumbnail { w, h, filter } = thumbnail else {
        return false;
    };
    if matches!(filter, ResampleFilter::Nearest) {
        return false;
    }

    let (source_width, source_height) = img.dimensions();
    if source_width < 2
        || source_height < 2
        || source_width % 2 != 0
        || source_height % 2 != 0
        || (source_width as usize).saturating_mul(source_height as usize) < 512 * 512
    {
        return false;
    }
    let (new_width, new_height) = (
        (*w).max(1).min(source_width),
        (*h).max(1).min(source_height),
    );
    if new_width == 0 || new_height == 0 {
        return false;
    }
    let factor_x = ((source_width as f64 / new_width as f64 / 2.0) as u32).max(1);
    let factor_y = ((source_height as f64 / new_height as f64 / 2.0) as u32).max(1);
    factor_x == 2 && factor_y == 2
}

#[cfg(test)]
#[path = "thumbnail/tests.rs"]
mod tests;
