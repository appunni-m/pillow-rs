//! Exact-mode quantization orchestration and native sample routing.
//! TODO(pillow-rs#3ptb): verify mode × architecture fallback receipts and
//! CoreOnly extension behavior before closing the CPU Quantize migration.

use std::borrow::Cow;
use std::sync::Arc;

use crate::compute::ExecutionContext;
use crate::compute::registry::support::PaletteInput;
use crate::compute::registry::support::{
    OperationId, PaletteKnowledge, RequestFacts, StrategyInputs, StrategySelection,
    SupportAssessment,
};
use crate::error::PilError;
use crate::image::{Image, PalettedData, PipelineOps};
use crate::raster::{DynamicImage, Mode};

use super::super as imageops;
use super::super::ops::{QUANTIZE_INPUT, QUANTIZE_SUPPORT, QuantizeSource, quantize_mode_strategy};

/// Returns the mode strategy's default quantization method.
// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn default_method(mode: Mode) -> u32 {
    quantize_mode_strategy(mode).default_method
}

/// Preserves the pre-materialization RGBA method rejection through its declaration.
// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn validate_pillow_rgba_method(mode: Mode, method: u32) -> Result<(), PilError> {
    let crate::compute::registry::support::InputContract::QuantizeParameters {
        allowed_methods,
        pillow_rgba_method_rejection,
        ..
    } = QUANTIZE_INPUT
    else {
        unreachable!("Quantize input declaration must describe its parameters");
    };
    if mode == Mode::ImageRgba8 && !allowed_methods.contains(&method) {
        return Err(PilError::ValueError(pillow_rgba_method_rejection.into()));
    }
    Ok(())
}

/// Preserves the pre-materialization negative k-means rejection through its declaration.
// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn validate_kmeans_count(iterations: i64) -> Result<(), PilError> {
    let crate::compute::registry::support::InputContract::QuantizeParameters {
        kmeans_nonnegative,
        kmeans_rejection,
        ..
    } = QUANTIZE_INPUT
    else {
        unreachable!("Quantize input declaration must describe its parameters");
    };
    if kmeans_nonnegative && iterations < 0 {
        return Err(PilError::ValueError(kmeans_rejection.into()));
    }
    Ok(())
}

/// Quantize prepared from the exact source identity and injected CPU context.
pub(crate) struct PreparedQuantize<'image, 'palette> {
    image: &'image DynamicImage,
    palette: PaletteInput<'palette>,
    mode: Mode,
    source: QuantizeSource,
    colors: usize,
    kmeans: u32,
    method: u32,
    selection: StrategySelection,
}

impl PreparedQuantize<'_, '_> {
    /// Returns the selected exact-mode Default strategy and fallback details.
    pub(crate) const fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    /// Executes the selected source-mode route and constructs the P result.
    pub(crate) fn execute(self) -> Result<Image, PilError> {
        self.validate_selection()?;
        let (width, height) = (self.image.width(), self.image.height());
        if width == 0 || height == 0 {
            // Pillow returns an empty P image before validating colors/methods
            // or entering a quantizer; its new palette has zero entries.
            return build_paletted_image(
                width,
                height,
                QuantizedColors {
                    indices: Vec::new(),
                    palette: Vec::new(),
                    alpha: None,
                },
            );
        }
        let quantized = quantize_by_method(
            self.image,
            self.palette,
            self.source,
            &self.selection,
            width,
            height,
            self.colors,
            self.kmeans,
            self.method,
        )?;
        build_paletted_image(width, height, quantized)
    }

    fn validate_selection(&self) -> Result<(), PilError> {
        if self.image.mode() != self.mode {
            return Err(PilError::InternalError(
                "Quantize prepared mode did not match its DynamicImage tag".into(),
            ));
        }
        if quantize_mode_strategy(self.mode).source != self.source {
            return Err(PilError::InternalError(
                "Quantize source strategy did not match its exact mode".into(),
            ));
        }
        let implementation = self.selection.implementation;
        if implementation.operation != OperationId::Quantize
            || implementation.mode != self.mode
            || implementation.architecture != crate::compute::Architecture::Default
        {
            return Err(PilError::InternalError(
                "Quantize strategy selection did not match its Default mode owner".into(),
            ));
        }
        Ok(())
    }
}

/// Selects the exact mode's serial implementation before running the algorithm.
// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn prepare<'image, 'palette>(
    image: &'image DynamicImage,
    palette: PaletteInput<'palette>,
    colors: u32,
    kmeans: u32,
    method: u32,
    context: &ExecutionContext,
) -> Result<PreparedQuantize<'image, 'palette>, PilError> {
    let mode = image.mode();
    let source = quantize_mode_strategy(mode).source;
    let selection = select_quantize_request(
        image,
        palette.knowledge(),
        colors,
        method,
        Some(i64::from(kmeans)),
        None,
        None,
        None,
        context,
    )?;
    Ok(PreparedQuantize {
        image,
        palette,
        mode,
        source,
        colors: colors as usize,
        kmeans,
        method,
        selection,
    })
}

/// Maps a wrapper-admitted source carrier through supplied or mode-default P metadata.
// VERIFY: source=reviewed@2026-11-12; parity=pass@migration-parity-8c3385ccbd7043bcbc13eebfa0daf52f; benchmark=pending
pub(crate) fn execute_with_palette(
    image: &DynamicImage,
    palette_image: Option<&Image>,
    colors: u32,
    method: u32,
    dither: bool,
    visible_mode: Option<Mode>,
    context: &ExecutionContext,
) -> Result<(Image, StrategySelection), PilError> {
    let mode = image.mode();
    // A known empty table is distinct from missing palette metadata: Pillow
    // rejects the former and synthesizes a mode-selected table for the latter.
    // TODO(pillow-rs#3ptb): freeze absent-table cases for L, RGB, RGBA, RGBa,
    // and RGBX, plus a rejected native source; assert final wrapper-fault order
    // and Default/architecture selection receipts through the Python facade.
    let (palette_mode, palette_rgb, palette_alpha): (Mode, Cow<'_, [u8]>, Cow<'_, [u8]>) =
        match palette_image {
            Some(palette_image) => {
                let palette = palette_image.palette_input();
                match palette.rgb {
                    Some(rgb) => (
                        palette_image.mode_identity()?,
                        Cow::Borrowed(rgb),
                        Cow::Borrowed(palette.alpha),
                    ),
                    None => (
                        Mode::ImagePalette8,
                        default_palette_rgb(mode)?,
                        Cow::Borrowed(&[]),
                    ),
                }
            }
            None => (
                Mode::ImagePalette8,
                default_palette_rgb(mode)?,
                Cow::Borrowed(&[]),
            ),
        };
    let palette_entries = palette_rgb.len() / 3;
    let selection = select_quantize_request(
        image,
        PaletteKnowledge::Absent,
        colors,
        method,
        None,
        Some(palette_mode),
        Some(palette_entries),
        visible_mode,
        context,
    )?;
    if selection.implementation.operation != OperationId::Quantize
        || selection.implementation.mode != mode
    {
        return Err(PilError::InternalError(
            "Quantize palette selection did not match its source mode".into(),
        ));
    }
    let (width, height) = (image.width(), image.height());
    let palette: Vec<[u8; 3]> = palette_rgb
        .chunks_exact(3)
        .take(256)
        .map(|entry| [entry[0], entry[1], entry[2]])
        .collect();
    let indices = match image {
        DynamicImage::ImageLuma8(luma) => {
            imageops::luma8::ops::execute_quantize_with_palette(luma, &palette, dither, &selection)?
        }
        DynamicImage::ImageRgb8(rgb) => {
            imageops::rgb8::ops::execute_quantize_with_palette(rgb, &palette, dither, &selection)?
        }
        DynamicImage::ImageRgba8(rgba) => {
            imageops::rgba8::ops::execute_quantize_with_palette(rgba, &palette, dither, &selection)?
        }
        DynamicImage::ImageRgbPremultiplied8(rgba) => {
            imageops::rgb_premultiplied8::ops::execute_quantize_with_palette(
                rgba, &palette, dither, &selection,
            )?
        }
        DynamicImage::ImageRgbx8(rgba) => {
            imageops::rgbx8::ops::execute_quantize_with_palette(rgba, &palette, dither, &selection)?
        }
        _ => {
            return Err(PilError::ValueError("conversion not supported".into()));
        }
    };
    let indices = crate::raster::GrayImage::from_raw(width, height, indices)
        .ok_or_else(|| PilError::ValueError("palette conversion failed".to_owned()))?;
    Ok((
        Image::Paletted(PalettedData {
            indices: DynamicImage::ImagePalette8(indices),
            palette: palette_rgb.into_owned(),
            palette_alpha: palette_alpha.into_owned(),
            source_format: None,
            info: None,
            exif: None,
            materialized: crate::image::materialization_cache(),
        }),
        selection,
    ))
}

/// Borrows or builds Pillow's default palette bytes from the source mode.
// VERIFY: source=reviewed@2026-11-12; parity=pass@migration-parity-8c3385ccbd7043bcbc13eebfa0daf52f; benchmark=pending
fn default_palette_rgb(source_mode: Mode) -> Result<Cow<'static, [u8]>, PilError> {
    match quantize_mode_strategy(source_mode).source {
        QuantizeSource::Luma => Ok(Cow::Owned(
            crate::compute::pool_cpu::helpers::quantization::identity_grayscale_palette(),
        )),
        QuantizeSource::Rgb
        | QuantizeSource::Rgba
        | QuantizeSource::RgbPremultiplied
        | QuantizeSource::Rgbx => Ok(Cow::Borrowed(
            crate::compute::pool_cpu::helpers::quantization::web_palette_colors(),
        )),
        QuantizeSource::Palette | QuantizeSource::Native => {
            Err(PilError::ValueError("conversion not supported".into()))
        }
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pass@quantize-supplied-palette-20261112-cpu-default; benchmark=pending
fn select_quantize_request(
    image: &DynamicImage,
    source_palette: PaletteKnowledge,
    colors: u32,
    method: u32,
    kmeans: Option<i64>,
    palette_mode: Option<Mode>,
    palette_entries: Option<usize>,
    visible_mode: Option<Mode>,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    let mode = image.mode();
    let input = StrategyInputs {
        width: image.width(),
        height: image.height(),
        byte_len: Some(image.as_bytes().len()),
        palette: source_palette,
        request: RequestFacts::Quantize {
            source_mode: mode,
            visible_mode,
            colors,
            method,
            kmeans,
            palette_mode,
            palette_entries,
        },
    };
    match QUANTIZE_SUPPORT.for_mode(mode).assess(context, input)? {
        SupportAssessment::Ready(selection) => Ok(selection),
        SupportAssessment::Rejected { reason, kind, .. } => Err(kind.into_error(reason)),
        SupportAssessment::Deferred { guard, .. } => Err(PilError::InternalError(format!(
            "Quantize preparation left input facts unresolved: {guard:?}"
        ))),
    }
}

struct QuantizedColors {
    indices: Vec<u8>,
    palette: Vec<u8>,
    alpha: Option<Vec<u8>>,
}

fn quantize_by_method(
    image: &DynamicImage,
    palette: PaletteInput<'_>,
    source: QuantizeSource,
    selection: &StrategySelection,
    width: u32,
    height: u32,
    n_colors: usize,
    kmeans: u32,
    method: u32,
) -> Result<QuantizedColors, PilError> {
    match source {
        QuantizeSource::Luma => {
            let DynamicImage::ImageLuma8(luma) = image else {
                return Err(PilError::InternalError(
                    "Quantize L mode tag mismatch".into(),
                ));
            };
            let (indices, palette, alpha) = imageops::luma8::ops::execute_quantize(
                luma, width, height, n_colors, kmeans, method, selection,
            )?;
            return Ok(QuantizedColors {
                indices,
                palette,
                alpha,
            });
        }
        QuantizeSource::Palette => {
            let DynamicImage::ImagePalette8(indices) = image else {
                return Err(PilError::InternalError(
                    "Quantize P mode tag mismatch".into(),
                ));
            };
            let (indices, palette, alpha) = imageops::palette8::ops::execute_quantize(
                indices, palette, width, height, n_colors, kmeans, method, selection,
            )?;
            return Ok(QuantizedColors {
                indices,
                palette,
                alpha,
            });
        }
        QuantizeSource::Rgba => {
            let DynamicImage::ImageRgba8(rgba) = image else {
                return Err(PilError::InternalError(
                    "RGBA Quantize strategy received a different DynamicImage variant".into(),
                ));
            };
            let (indices, palette, alpha) =
                imageops::rgba8::ops::execute_quantize(rgba, width, height, n_colors, selection)?;
            return Ok(QuantizedColors {
                indices,
                palette,
                alpha,
            });
        }
        QuantizeSource::RgbPremultiplied => {
            return quantize_rgb_premultiplied_source(image, selection, width, height, n_colors);
        }
        QuantizeSource::Rgbx => {
            return quantize_rgbx_source(image, selection, width, height, n_colors, kmeans, method);
        }
        QuantizeSource::Rgb => {
            let DynamicImage::ImageRgb8(rgb) = image else {
                return Err(PilError::InternalError(
                    "Quantize RGB mode tag mismatch".into(),
                ));
            };
            let (indices, palette, alpha) = imageops::rgb8::ops::execute_quantize(
                rgb, width, height, n_colors, kmeans, method, selection,
            )?;
            return Ok(QuantizedColors {
                indices,
                palette,
                alpha,
            });
        }
        QuantizeSource::Native => {
            return quantize_native_source(
                image, palette, selection, width, height, n_colors, kmeans, method,
            );
        }
    }
}

/// Quantizes RGBX without interpreting its padding byte as alpha.
fn quantize_rgbx_source(
    image: &DynamicImage,
    selection: &StrategySelection,
    width: u32,
    height: u32,
    colors: usize,
    kmeans: u32,
    method: u32,
) -> Result<QuantizedColors, PilError> {
    let DynamicImage::ImageRgbx8(rgbx) = image else {
        return Err(PilError::InternalError(
            "Quantize RGBX mode tag mismatch".into(),
        ));
    };
    let (indices, palette, alpha) = imageops::rgbx8::ops::execute_quantize(
        rgbx.as_raw(),
        width,
        height,
        colors,
        kmeans,
        method,
        selection,
    )?;
    Ok(QuantizedColors {
        indices,
        palette,
        alpha,
    })
}

/// Quantizes CoreOnly exact modes through their native mode strategies.
fn quantize_native_source(
    image: &DynamicImage,
    palette: PaletteInput<'_>,
    selection: &StrategySelection,
    width: u32,
    height: u32,
    colors: usize,
    kmeans: u32,
    method: u32,
) -> Result<QuantizedColors, PilError> {
    macro_rules! quantize_native_mode {
        ($implementation:path, $samples:expr) => {
            $implementation(
                $samples.as_raw(),
                width,
                height,
                colors,
                kmeans,
                method,
                selection,
            )?
        };
    }

    // NO_PILLOW_SUPPORT: these exact-mode Quantize routes extend Pillow's set.
    let (indices, palette, alpha) = match image {
        DynamicImage::One(samples) => {
            quantize_native_mode!(imageops::one::ops::execute_quantize, samples)
        }
        DynamicImage::ImageLumaA8(samples) => {
            quantize_native_mode!(imageops::luma_a8::ops::execute_quantize, samples)
        }
        DynamicImage::ImageLaPremultiplied8(samples) => {
            quantize_native_mode!(imageops::la_premultiplied8::ops::execute_quantize, samples)
        }
        DynamicImage::ImagePaletteAlpha8(samples) => {
            imageops::palette_alpha8::ops::execute_quantize_with_palette(
                samples.as_raw(),
                palette,
                width,
                height,
                colors,
                selection,
            )?
        }
        DynamicImage::ImageCmyk8(samples) => {
            quantize_native_mode!(imageops::cmyk8::ops::execute_quantize, samples)
        }
        DynamicImage::ImageI32(samples) => {
            quantize_native_mode!(imageops::i32::ops::execute_quantize, samples)
        }
        DynamicImage::ImageF32(samples) => {
            quantize_native_mode!(imageops::f32::ops::execute_quantize, samples)
        }
        DynamicImage::ImageYCbCr8(samples) => {
            quantize_native_mode!(imageops::ycbcr8::ops::execute_quantize, samples)
        }
        DynamicImage::ImageLab8(samples) => {
            quantize_native_mode!(imageops::lab8::ops::execute_quantize, samples)
        }
        DynamicImage::ImageHsv8(samples) => {
            quantize_native_mode!(imageops::hsv8::ops::execute_quantize, samples)
        }
        DynamicImage::ImageI16(samples) => {
            quantize_native_mode!(imageops::i16::ops::execute_quantize, samples)
        }
        DynamicImage::ImageI16L(samples) => {
            quantize_native_mode!(imageops::i16l::ops::execute_quantize, samples)
        }
        DynamicImage::ImageI16B(samples) => {
            quantize_native_mode!(imageops::i16b::ops::execute_quantize, samples)
        }
        DynamicImage::ImageI16N(samples) => {
            quantize_native_mode!(imageops::i16n::ops::execute_quantize, samples)
        }
        DynamicImage::ImageLumaA16(samples) => {
            quantize_native_mode!(imageops::luma_a16::ops::execute_quantize, samples)
        }
        DynamicImage::ImageRgb16(samples) => {
            quantize_native_mode!(imageops::rgb16::ops::execute_quantize, samples)
        }
        DynamicImage::ImageRgba16(samples) => {
            quantize_native_mode!(imageops::rgba16::ops::execute_quantize, samples)
        }
        DynamicImage::ImageRgb32F(samples) => {
            quantize_native_mode!(imageops::rgb32f::ops::execute_quantize, samples)
        }
        DynamicImage::ImageRgba32F(samples) => {
            quantize_native_mode!(imageops::rgba32f::ops::execute_quantize, samples)
        }
        _ => {
            return Err(PilError::InternalError(
                "Native CoreOnly Quantize received an unsupported mode".into(),
            ));
        }
    };
    Ok(QuantizedColors {
        indices,
        palette,
        alpha,
    })
}

/// Quantizes Rust-only four-byte modes with the established alpha-aware octree.
fn quantize_rgb_premultiplied_source(
    image: &DynamicImage,
    selection: &StrategySelection,
    width: u32,
    height: u32,
    colors: usize,
) -> Result<QuantizedColors, PilError> {
    let (indices, palette, alpha) = match image {
        DynamicImage::ImageRgbPremultiplied8(samples) => {
            imageops::rgb_premultiplied8::ops::execute_quantize(
                samples.as_raw(),
                width,
                height,
                colors,
                0,
                2,
                selection,
            )?
        }
        _ => {
            return Err(PilError::InternalError(
                "RGBa Quantize strategy received a different DynamicImage variant".into(),
            ));
        }
    };
    Ok(QuantizedColors {
        indices,
        palette,
        alpha,
    })
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn build_paletted_image(
    width: u32,
    height: u32,
    quantized: QuantizedColors,
) -> Result<Image, PilError> {
    let mut output = crate::raster::GrayImage::new(width, height);
    for (index, pixel) in output.pixels_mut().enumerate() {
        pixel[0] = quantized.indices.get(index).copied().unwrap_or(0);
    }
    Ok(Image::Pipeline {
        source: Arc::new(Image::from_dynamic(DynamicImage::ImagePalette8(output))),
        ops: PipelineOps::empty(),
        format: None,
        backend: None,
        palette: Some(quantized.palette),
        palette_alpha: quantized.alpha,
        compatibility_icc_profile: None,
        materialized: crate::image::materialization_cache(),
        shape: crate::image::pipeline_shape_cache(),
        mode: crate::image::pipeline_mode_cache_for(Some(Mode::ImagePalette8)),
    })
}
