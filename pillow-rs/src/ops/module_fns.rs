//! Pillow `Image` module-level functions.
//!
//! These functions mirror surfaces such as `Image.merge`, `Image.blend`,
//! `Image.composite`, `Image.eval`, and synthetic image effects.

use std::sync::Arc;

use crate::compute::registry::support::{
    InputContract, MergeBandModeRule, ModeSupport, OperationId, PaletteKnowledge, RequestFacts,
    SecondaryInputFacts, StrategyInputs,
};
use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;
use crate::raster::Mode;

mod point;
pub use point::{
    EvalInputKind, eval, eval_callable, eval_float, eval_point_callable, eval_point_transform,
    eval_point_mode, eval_point_mode_replicated_for_image, eval_replicated,
    eval_replicated_for_image, eval_validated, make_float_lut, make_lut, validate_eval_input,
};

/// Creates an image from raw bytes using Pillow's supported raw decoder.
///
/// Decoder selection is part of the public operation contract, not a binding
/// concern. Host adapters pass the decoder name through after extracting it.
pub fn frombytes(
    mode: Mode,
    size: (u32, u32),
    data: &[u8],
    decoder_name: &str,
) -> Result<Image, PilError> {
    if decoder_name != "raw" {
        return Err(PilError::IOError(format!(
            "decoder {decoder_name} not available"
        )));
    }
    Image::frombytes(mode, size, data)
}

/// Creates an image from an owned raw byte buffer, avoiding a second copy for
/// raw raster modes when the decoder can adopt the supplied allocation.
pub fn frombytes_owned(
    mode: Mode,
    size: (u32, u32),
    data: Vec<u8>,
    decoder_name: &str,
) -> Result<Image, PilError> {
    if decoder_name != "raw" {
        return Err(PilError::IOError(format!(
            "decoder {decoder_name} not available"
        )));
    }
    Image::frombytes_owned(mode, size, data)
}

/// Creates an image using a validated raw decoder plan.
pub fn frombytes_with_plan(
    size: (u32, u32),
    data: &[u8],
    decoder_name: &str,
    plan: crate::image::RawDecoderPlan,
) -> Result<Image, PilError> {
    if decoder_name != "raw" {
        return Err(PilError::IOError(format!(
            "decoder {decoder_name} not available"
        )));
    }
    plan.decode(size, data)
}

/// Creates an image from an owned buffer using a validated raw decoder plan.
pub fn frombytes_owned_with_plan(
    size: (u32, u32),
    data: Vec<u8>,
    decoder_name: &str,
    plan: crate::image::RawDecoderPlan,
) -> Result<Image, PilError> {
    if decoder_name != "raw" {
        return Err(PilError::IOError(format!(
            "decoder {decoder_name} not available"
        )));
    }
    plan.decode_owned(size, data)
}

/// Composites `im2` over `im1` and returns a new image.
pub fn alpha_composite(im1: &Image, im2: &Image) -> Result<Image, PilError> {
    let mut result = im1.copy();
    result.alpha_composite(im2, (0, 0), (0, 0))?;
    Ok(result)
}

type MergeLayoutContract = (
    usize,
    MergeBandModeRule,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
);

/// Python-visible callback order derived from Merge's typed support contract.
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct MergeCallbackContract {
    /// Number of input bands required by the requested output mode.
    pub expected_band_count: usize,
    /// Mode required for bands after the first, when the Pillow wrapper checks one.
    pub subsequent_band_mode: Option<Mode>,
    /// Error text for an incorrect band count.
    pub count_rejection: &'static str,
    /// Error text for an invalid later-band mode.
    pub mode_rejection: &'static str,
    /// Error text for a band whose dimensions differ from band zero.
    pub size_rejection: &'static str,
}

fn merge_layout_contract(mode: Mode) -> Result<MergeLayoutContract, PilError> {
    let mode_support = OperationId::Merge.support().for_mode(mode);
    mode_support.validate_mode()?;
    let ModeSupport::Supported { inputs, .. } = mode_support else {
        unreachable!("validated Merge mode support was rejected")
    };
    for contract in inputs {
        if let InputContract::MergeBandLayout {
            band_count,
            mode_rule,
            count_rejection,
            first_mode_rejection,
            mode_rejection,
            size_rejection,
        } = *contract
        {
            return Ok((
                band_count,
                mode_rule,
                count_rejection,
                first_mode_rejection,
                mode_rejection,
                size_rejection,
            ));
        }
    }
    Err(PilError::InternalError(
        "Merge support omitted its band layout contract".into(),
    ))
}

/// Returns host callback constraints from the same declaration used by Merge.
///
/// Python adapters use this to preserve `Image.merge`'s observable order while
/// keeping all mode and arity rules in the core support contract.
#[doc(hidden)]
pub fn merge_callback_contract(mode: Mode) -> Result<MergeCallbackContract, PilError> {
    let (
        expected_band_count,
        mode_rule,
        count_rejection,
        _first_mode_rejection,
        mode_rejection,
        size_rejection,
    ) = merge_layout_contract(mode)?;
    let subsequent_band_mode = match mode_rule {
        MergeBandModeRule::ExactTarget if expected_band_count > 1 => Some(mode),
        MergeBandModeRule::ExactTarget => None,
        MergeBandModeRule::LumaOrFirstPalette => Some(Mode::ImageLuma8),
    };
    Ok(MergeCallbackContract {
        expected_band_count,
        subsequent_band_mode,
        count_rejection,
        mode_rejection,
        size_rejection,
    })
}

fn validate_merge_shape(mode: Mode, band_count: usize) -> Result<(), PilError> {
    let (expected, _, count_rejection, _, _, _) = merge_layout_contract(mode)?;
    if band_count != expected {
        return Err(PilError::ValueError(count_rejection.into()));
    }
    Ok(())
}

fn validate_merge_bands(mode: Mode, bands: &[Image]) -> Result<(), PilError> {
    let (_, mode_rule, _, first_mode_rejection, mode_rejection, size_rejection) =
        merge_layout_contract(mode)?;
    match mode_rule {
        MergeBandModeRule::ExactTarget => {
            if bands[0].mode_identity()? != mode {
                return Err(PilError::ValueError(first_mode_rejection.into()));
            }
        }
        MergeBandModeRule::LumaOrFirstPalette => {
            for band in bands.iter().skip(1) {
                // Pillow checks each later band's mode before its size, then
                // leaves band zero's native storage check to ImagingMerge.
                if band.mode_identity()? != Mode::ImageLuma8 {
                    return Err(PilError::ValueError(mode_rejection.into()));
                }
                let band_size = band.size()?;
                let first_size = bands[0].size()?;
                if band_size != first_size {
                    return Err(PilError::ValueError(size_rejection.into()));
                }
            }
            if !matches!(
                bands[0].mode_identity()?,
                Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8
            ) {
                return Err(PilError::ValueError(first_mode_rejection.into()));
            }
        }
    }
    Ok(())
}

/// Host-neutral input classification for `Image.merge` bands.
#[derive(Debug, Clone)]
pub enum MergeInput {
    /// A Rust image extracted by a binding.
    Image(Box<Image>),
    /// A non-image host value, retaining only its type name for the
    /// Pillow-compatible attribute error raised by the merge implementation.
    Invalid(String),
}

/// Merges single-band images into a multi-band image.
///
/// `mode` determines the required band count. Pillow's byte layouts include
/// `L`, `La`, `PA`, `RGB`, `RGBX`, `RGBA`, `RGBa`, `YCbCr`, `HSV`, `LAB`, and
/// `CMYK`; the typed scalar layouts `1`, `I`, `F`, `P`, and `I;16*` are also
/// accepted.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `mode` is unsupported or `bands` has
/// the wrong length.
pub fn merge(mode: Mode, bands: &[Image]) -> Result<Image, PilError> {
    validate_merge_shape(mode, bands.len())?;
    validate_merge_bands(mode, bands)?;

    Ok(Image::push_op(
        &bands[0],
        PipelineOp::Merge {
            mode,
            bands: bands.to_vec().into(),
        },
    ))
}

/// Merges host-extracted band inputs while keeping invalid-item handling in
/// the core contract.
///
/// Binding layers may only classify a host value as an image or retain its
/// type name. They must not decide whether a missing item means a mode error,
/// an arity error, or an invalid band. Preserve that ordering here and let the
/// existing mode/arity validation remain the single source of truth.
pub fn merge_inputs(mode: Mode, bands: &[MergeInput]) -> Result<Image, PilError> {
    validate_merge_shape(mode, bands.len())?;
    let mut images = Vec::with_capacity(bands.len());
    for band in bands {
        match band {
            MergeInput::Image(image) => images.push((**image).clone()),
            MergeInput::Invalid(type_name) => {
                return Err(PilError::AttributeError(format!(
                    "'{type_name}' object has no attribute 'load'"
                )));
            }
        }
    }
    merge(mode, &images)
}

/// Blends two same-sized images by linear interpolation.
///
/// `alpha` may extrapolate outside `0.0..=1.0`. Each sample is blended
/// with fused float32 arithmetic, then clipped to the byte range and truncated.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] for incompatible modes or unequal dimensions, or
/// another [`PilError`] when metadata lookup fails.
pub fn blend(image1: &Image, image2: &Image, alpha: f64) -> Result<Image, PilError> {
    let mode1 = image1.mode_identity()?;
    let mode2 = image2.mode_identity()?;
    let backend = image1.backend().or(image2.backend());
    let support = OperationId::BlendModule.support();
    let mode_support = support.for_mode(mode1);
    mode_support.validate_blend_input_mode(mode2)?;
    let (w1, h1) = image1.size()?;
    let (w2, h2) = image2.size()?;
    let facts = StrategyInputs {
        width: w1,
        height: h1,
        byte_len: None,
        palette: PaletteKnowledge::Unknown,
        request: RequestFacts::Unspecified,
    }
    .with_blend_inputs(
        SecondaryInputFacts {
            mode: mode2,
            width: w2,
            height: h2,
            byte_len: None,
        },
        alpha,
    );
    if mode_support.assess_inputs(facts)?.is_some() {
        return Err(PilError::InternalError(
            "Blend wrapper did not resolve all declared input facts".into(),
        ));
    }
    let output = Image::push_op(
        image1,
        PipelineOp::BlendModule {
            other: Arc::new(image2.clone()),
            alpha,
        },
    );
    // Stateful enhancers can materialize their base before blending. In that
    // case the original source is the only operand that still carries the
    // caller's explicit backend selection.
    Ok(match backend {
        Some(backend) => output.use_backend(backend),
        None => output,
    })
}

/// Composites `image1` over `image2` using `mask`.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `image1` and `mask` dimensions differ,
/// or another [`PilError`] when size lookup fails.
pub fn composite(image1: &Image, image2: &Image, mask: &Image) -> Result<Image, PilError> {
    // PIL.composite = image2.copy() followed by image2.paste(image1, None, mask)
    // The output matches image2's size; smaller images are pasted at (0,0).
    // PIL requires image1 and mask to have the same size (via paste).
    // image2 can be a different size (it's the background canvas).
    let (w1, h1) = image1.size()?;
    let (wm, hm) = mask.size()?;
    crate::compute::pool_cpu::ops::composite::ops::validate_composite_wrapper_dimensions(
        (w1, h1),
        (wm, hm),
    )?;
    let mask_alpha = crate::compute::pool_cpu::ops::composite::ops::composite_wrapper_mask_alpha(
        mask.mode_identity()?,
    )?;
    let output_mode = image2.mode_identity()?;
    let source = if image1.mode_identity()? == output_mode {
        image1.clone()
    } else {
        // Pillow 12.2 PIL.Image.composite starts from image2.copy(), then
        // Image.paste converts image1 to the destination mode before blending.
        image1.convert(output_mode, None, None, None, None)?
    };
    let mut result = Image::push_op(
        &source,
        PipelineOp::CompositeModule {
            other: Arc::new(image2.clone()),
            mask: Arc::new(mask.clone()),
            mask_alpha,
        },
    );
    if let Image::Pipeline {
        ref mut palette,
        ref mut palette_alpha,
        ..
    } = result
    {
        if output_mode == Mode::ImagePalette8 {
            // The output begins as image2.copy(), so palette ownership follows
            // image2 even when image1 was converted to P for the paste.
            *palette = Some(image2.extract_palette().unwrap_or_default());
            *palette_alpha = image2.palette_alpha();
        } else if output_mode == Mode::ImagePaletteAlpha8 {
            // Image.composite starts with image2.copy(); Pillow preserves the
            // destination PA image's palette on that newly allocated result.
            *palette = image2.extract_palette();
            *palette_alpha = image2.palette_alpha();
        } else {
            *palette = None;
            *palette_alpha = None;
        }
    }
    Ok(result)
}

/// Generates Gaussian noise using the source image dimensions.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; deferred pipeline execution reports later
/// materialization failures.
pub fn effect_noise(image: &Image, sigma: f64) -> Result<Image, PilError> {
    Ok(Image::push_op(image, PipelineOp::EffectNoise { sigma }))
}

/// Generates a Gaussian-noise `L` image for Pillow's module-level effect.
pub fn effect_noise_from_size(size: (u32, u32), sigma: f64) -> Result<Image, PilError> {
    let source = Image::new(size.0, size.1, Mode::ImageLuma8, (0, 0, 0, 255))?;
    effect_noise(&source, sigma)
}

/// Spreads pixels outward by up to `distance` pixels.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; deferred pipeline execution reports later
/// materialization failures.
pub fn effect_spread(image: &Image, distance: u32) -> Result<Image, PilError> {
    Ok(Image::push_op(image, PipelineOp::EffectSpread { distance }))
}

/// Generates a 256 by 256 linear gradient.
///
/// Single-channel modes increase from black at the top to white at the bottom.
/// Supported modes are `"1"`, `"L"`, `"P"`, `"I"`, and `"F"`.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `mode` is unsupported, or another
/// [`PilError`] when raw image construction fails.
pub fn linear_gradient(mode: Mode) -> Result<Image, PilError> {
    let generated = crate::compute::execute_eager_cpu_linear_gradient(mode)?;
    Image::from_generated_dynamic(generated, mode)
}

/// Generates a 256 by 256 radial gradient.
///
/// Supported modes are `"1"`, `"L"`, `"P"`, `"I"`, and `"F"`. Pixel values
/// follow Pillow's radial distance formula.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `mode` is unsupported, or another
/// [`PilError`] when raw image construction fails.
pub fn radial_gradient(mode: Mode) -> Result<Image, PilError> {
    let generated = crate::compute::execute_eager_cpu_radial_gradient(mode)?;
    Image::from_generated_dynamic(generated, mode)
}

/// Generates a Mandelbrot effect image.
///
/// `size` is output dimensions, `extent` is `(x0, y0, x1, y1)` in the complex
/// plane, and `quality` controls iteration count.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] for zero size, negative extent dimensions,
/// or `quality < 2`. Returns [`PilError::DimensionError`] when allocation
/// checks fail.
pub fn effect_mandelbrot(
    size: (u32, u32),
    extent: (f64, f64, f64, f64),
    quality: i32,
) -> Result<Image, PilError> {
    let (x0, y0, x1, y1) = extent;
    if quality < 2 || x0 >= x1 || y0 >= y1 {
        return Err(PilError::ValueError("unrecognized argument value".into()));
    }
    let (width, height) = size;
    if width == 0 || height == 0 {
        return Image::new(width, height, Mode::ImageLuma8, (0, 0, 0, 255));
    }

    let generated = crate::compute::execute_eager_cpu_mandelbrot(size, extent, quality)?;
    Image::from_generated_dynamic(generated, Mode::ImageLuma8)
}

/// Validates a host-provided Mandelbrot extent and delegates to the typed
/// Rust implementation. The host type name is metadata used only to preserve
/// Pillow's diagnostic for non-four-item sequences.
pub fn effect_mandelbrot_with_extent(
    size: (u32, u32),
    extent: Option<&[f64]>,
    extent_type: &str,
    quality: i32,
) -> Result<Image, PilError> {
    let Some(extent) = extent else {
        return Err(PilError::TypeError(format!(
            "argument 2 must be 4-item sequence, not {extent_type}"
        )));
    };
    if extent.len() != 4 {
        // Pillow's argument parser reports the observed sequence arity before
        // it falls back to the host type name used for non-sequences.
        return Err(PilError::TypeError(format!(
            "argument 2 must be sequence of length 4, not {}",
            extent.len()
        )));
    }
    effect_mandelbrot(size, (extent[0], extent[1], extent[2], extent[3]), quality)
}
