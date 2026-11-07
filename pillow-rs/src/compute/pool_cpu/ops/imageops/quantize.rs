//! Exact-mode admission and serial preparation for Pillow palette reduction.
//! TODO(pillow-rs#3ptb): verify the mode × architecture matrix, fallback receipts,
//! and CoreOnly extension behavior before closing the CPU Quantize migration.

use std::sync::Arc;

use crate::compute::ExecutionContext;
use crate::compute::registry::support::{
    ModeSupport, OperationId, OutputModeRule, PaletteKnowledge, RequestFacts, StrategyInputs,
    StrategySelection, SupportAssessment,
};
use crate::error::PilError;
use crate::image::{Image, PalettedData, PipelineOps};
use crate::raster::{DynamicImage, Mode};

use crate::compute::registry::support::PaletteInput;

const ALLOWED_ALPHA_METHODS: &[u32] = &[2, 3];
const RECOGNIZED_QUANTIZE_METHODS: &[u32] = &[0, 1, 2, 3];
const METHOD_RESTRICTED_MODES: &[Mode] = &[
    Mode::ImageLumaA8,
    Mode::ImageLaPremultiplied8,
    Mode::ImagePaletteAlpha8,
    Mode::ImageRgba8,
    Mode::ImageRgbPremultiplied8,
    Mode::ImageLumaA16,
    Mode::ImageRgba16,
    Mode::ImageRgba32F,
];
const PALETTE_SOURCES: &[Mode] = &[Mode::ImageRgb8, Mode::ImageLuma8];
const UNAVAILABLE_METHODS: &[u32] = &[3];
const QUANTIZE_INPUT: crate::compute::registry::support::InputContract =
    crate::compute::registry::support::InputContract::QuantizeParameters {
        min_colors: 1,
        max_colors: 256,
        recognized_methods: RECOGNIZED_QUANTIZE_METHODS,
        method_restricted_modes: METHOD_RESTRICTED_MODES,
        allowed_methods: ALLOWED_ALPHA_METHODS,
        palette_mode: Mode::ImagePalette8,
        palette_sources: PALETTE_SOURCES,
        unavailable_methods: UNAVAILABLE_METHODS,
        kmeans_nonnegative: true,
        colors_rejection: "bad number of colors",
        invalid_method_rejection: "quantization error",
        pillow_rgba_method_rejection: "Fast Octree (method == 2) and libimagequant (method == 3) \
             are the only valid methods for quantizing RGBA images",
        alpha_method_rejection:
            "only Fast Octree (method == 2) is supported for alpha-bearing CoreOnly modes",
        palette_mode_rejection: "bad mode for palette image",
        palette_source_rejection: "only RGB or L mode images can be quantized to a palette",
        unavailable_method_rejection:
            "dependency required by this method was not enabled at compile time",
        kmeans_rejection: "kmeans must not be negative",
    };
pub(super) const INPUTS: &[crate::compute::registry::support::InputContract] = &[
    crate::compute::registry::support::InputContract::PaletteMetadata,
    QUANTIZE_INPUT,
];
const PALETTED_OUTPUT: OutputModeRule = OutputModeRule::Fixed(Mode::ImagePalette8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuantizeSource {
    Luma,
    Palette,
    Rgba,
    RgbPremultiplied,
    Rgb,
    Rgbx,
    Native,
}

#[derive(Clone, Copy)]
enum QuantizeSupportScope {
    Pillow,
    CoreOnly,
}

#[derive(Clone, Copy)]
struct QuantizeModeStrategy {
    support: ModeSupport,
    source: QuantizeSource,
    default_method: u32,
}

/// Binds each exact mode's public contract to the concrete source strategy.
fn quantize_mode_strategy(mode: Mode) -> QuantizeModeStrategy {
    let (source, scope, default_method) = match mode {
        Mode::ImageLuma8 => (QuantizeSource::Luma, QuantizeSupportScope::Pillow, 0),
        Mode::ImagePalette8 => (QuantizeSource::Palette, QuantizeSupportScope::Pillow, 0),
        Mode::ImageRgba8 => (QuantizeSource::Rgba, QuantizeSupportScope::Pillow, 2),
        Mode::ImageRgbPremultiplied8 => {
            (QuantizeSource::RgbPremultiplied, QuantizeSupportScope::CoreOnly, 2)
        }
        Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageLumaA16
        | Mode::ImageRgba16
        | Mode::ImageRgba32F => (QuantizeSource::Native, QuantizeSupportScope::CoreOnly, 2),
        Mode::ImageRgbx8 => (QuantizeSource::Rgbx, QuantizeSupportScope::CoreOnly, 0),
        Mode::ImageRgb8 => (QuantizeSource::Rgb, QuantizeSupportScope::Pillow, 0),
        // NO_PILLOW_SUPPORT: these exact mode strategies extend Pillow's
        // Quantize source contract without changing the Pillow-defined routes.
        Mode::One
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8
        | Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N
        | Mode::ImageRgb16
        | Mode::ImageRgb32F
        => {
            (QuantizeSource::Native, QuantizeSupportScope::CoreOnly, 0)
        }
    };
    let support = match scope {
        QuantizeSupportScope::CoreOnly => ModeSupport::cpu_core_only(
            OperationId::Quantize,
            mode,
            PALETTED_OUTPUT,
            INPUTS,
            &[],
            "CoreOnly mode strategies extend Pillow's Quantize mode contract",
        ),
        QuantizeSupportScope::Pillow => ModeSupport::cpu(
            OperationId::Quantize,
            mode,
            PALETTED_OUTPUT,
            INPUTS,
            &[],
        ),
    };
    QuantizeModeStrategy {
        support,
        source,
        default_method,
    }
}

pub(super) fn classify_mode(mode: Mode) -> ModeSupport {
    quantize_mode_strategy(mode).support
}

/// Returns the mode strategy's default quantization method.
pub(crate) fn default_method(mode: Mode) -> u32 {
    quantize_mode_strategy(mode).default_method
}

/// Preserves the pre-materialization RGBA method rejection through its declaration.
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

/// Maps an already validated L or RGB input to a supplied P palette.
pub(crate) fn execute_with_palette(
    image: &DynamicImage,
    palette_image: &Image,
    colors: u32,
    method: u32,
    dither: bool,
    context: &ExecutionContext,
) -> Result<(Image, StrategySelection), PilError> {
    let mode = image.mode();
    let palette_mode = palette_image.mode_identity()?;
    let selection = select_quantize_request(
        image,
        PaletteKnowledge::Absent,
        colors,
        method,
        None,
        Some(palette_mode),
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
    let target_palette = palette_image.palette_input();
    let palette_bytes = target_palette.rgb.unwrap_or_default().to_vec();
    let palette_alpha = target_palette.alpha.to_vec();
    let palette: Vec<[u8; 3]> = palette_bytes
        .chunks_exact(3)
        .take(256)
        .map(|entry| [entry[0], entry[1], entry[2]])
        .collect();
    let indices = match image {
        DynamicImage::ImageLuma8(luma) => super::luma8::ops::execute_quantize_with_palette(
            luma,
            &palette,
            dither,
            &selection,
        )?,
        DynamicImage::ImageRgb8(rgb) => super::rgb8::ops::execute_quantize_with_palette(
            rgb,
            &palette,
            dither,
            &selection,
        )?,
        _ => {
            return Err(PilError::InternalError(
                "fixed palette strategy received an unsupported exact mode".into(),
            ));
        }
    };
    let indices = crate::raster::GrayImage::from_raw(width, height, indices)
        .ok_or_else(|| PilError::ValueError("palette conversion failed".to_owned()))?;
    Ok((
        Image::Paletted(PalettedData {
            indices: DynamicImage::ImagePalette8(indices),
            palette: palette_bytes,
            palette_alpha,
            source_format: None,
            info: None,
            exif: None,
            materialized: crate::image::materialization_cache(),
        }),
        selection,
    ))
}

fn select_quantize_request(
    image: &DynamicImage,
    source_palette: PaletteKnowledge,
    colors: u32,
    method: u32,
    kmeans: Option<i64>,
    palette_mode: Option<Mode>,
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
            colors,
            method,
            kmeans,
            palette_mode,
        },
    };
    match super::ops::QUANTIZE_SUPPORT.for_mode(mode).assess(context, input)? {
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
                return Err(PilError::InternalError("Quantize L mode tag mismatch".into()));
            };
            let (indices, palette, alpha) = super::luma8::ops::execute_quantize(
                luma,
                width,
                height,
                n_colors,
                kmeans,
                method,
                selection,
            )?;
            return Ok(QuantizedColors { indices, palette, alpha });
        }
        QuantizeSource::Palette => {
            let DynamicImage::ImagePalette8(indices) = image else {
                return Err(PilError::InternalError("Quantize P mode tag mismatch".into()));
            };
            let (indices, palette, alpha) = super::palette8::ops::execute_quantize(
                indices,
                palette,
                width,
                height,
                n_colors,
                kmeans,
                method,
                selection,
            )?;
            return Ok(QuantizedColors { indices, palette, alpha });
        }
        QuantizeSource::Rgba => {
            let DynamicImage::ImageRgba8(rgba) = image else {
                return Err(PilError::InternalError(
                    "RGBA Quantize strategy received a different DynamicImage variant".into(),
                ));
            };
            let (indices, palette, alpha) = super::rgba8::ops::execute_quantize(
                rgba,
                width,
                height,
                n_colors,
                selection,
            )?;
            return Ok(QuantizedColors { indices, palette, alpha });
        }
        QuantizeSource::RgbPremultiplied => {
            return quantize_rgb_premultiplied_source(
                image,
                selection,
                width,
                height,
                n_colors,
            );
        }
        QuantizeSource::Rgbx => {
            return quantize_rgbx_source(
                image,
                selection,
                width,
                height,
                n_colors,
                kmeans,
                method,
            );
        }
        QuantizeSource::Rgb => {
            let DynamicImage::ImageRgb8(rgb) = image else {
                return Err(PilError::InternalError("Quantize RGB mode tag mismatch".into()));
            };
            let (indices, palette, alpha) = super::rgb8::ops::execute_quantize(
                rgb,
                width,
                height,
                n_colors,
                kmeans,
                method,
                selection,
            )?;
            return Ok(QuantizedColors { indices, palette, alpha });
        }
        QuantizeSource::Native => {
            return quantize_native_source(
                image,
                palette,
                selection,
                width,
                height,
                n_colors,
                kmeans,
                method,
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
        return Err(PilError::InternalError("Quantize RGBX mode tag mismatch".into()));
    };
    let (indices, palette, alpha) = super::rgbx8::ops::execute_quantize(
        rgbx.as_raw(), width, height, colors, kmeans, method, selection,
    )?;
    Ok(QuantizedColors { indices, palette, alpha })
}

/// Quantizes CoreOnly exact modes from their native typed buffers.
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
    // CMYK's Rgba carrier projects its first three bytes; YCbCr/LAB/HSV's Rgb
    // carriers preserve all three. Their typed sources match DynamicImage's
    // existing RGB projection without allocating a converted raster.
    let (indices, palette, alpha) = match image {
        // NO_PILLOW_SUPPORT: CoreOnly strategies preserve each exact mode and
        // read its native buffer through that mode's Default implementation.
        DynamicImage::One(samples) => super::one::ops::execute_quantize(
            samples.as_raw(), width, height, colors, kmeans, method, selection,
        )?,
        DynamicImage::ImageLumaA8(samples) => super::luma_a8::ops::execute_quantize(
            samples.as_raw(), width, height, colors, kmeans, method, selection,
        )?,
        DynamicImage::ImageLaPremultiplied8(samples) => {
            super::la_premultiplied8::ops::execute_quantize(
                samples.as_raw(), width, height, colors, kmeans, method, selection,
            )?
        }
        DynamicImage::ImagePaletteAlpha8(samples) =>
            super::palette_alpha8::ops::execute_quantize_with_palette(
                samples.as_raw(), palette, width, height, colors, selection,
            )?,
        DynamicImage::ImageCmyk8(samples) => super::cmyk8::ops::execute_quantize(
            samples.as_raw(), width, height, colors, kmeans, method, selection,
        )?,
        DynamicImage::ImageI32(samples) => super::i32::ops::execute_quantize(
            samples.as_raw(), width, height, colors, kmeans, method, selection,
        )?,
        DynamicImage::ImageF32(samples) => super::f32::ops::execute_quantize(
            samples.as_raw(), width, height, colors, kmeans, method, selection,
        )?,
        DynamicImage::ImageYCbCr8(samples) => super::ycbcr8::ops::execute_quantize(
            samples.as_raw(), width, height, colors, kmeans, method, selection,
        )?,
        DynamicImage::ImageLab8(samples) => super::lab8::ops::execute_quantize(
            samples.as_raw(), width, height, colors, kmeans, method, selection,
        )?,
        DynamicImage::ImageHsv8(samples) => super::hsv8::ops::execute_quantize(
            samples.as_raw(), width, height, colors, kmeans, method, selection,
        )?,
        DynamicImage::ImageI16(samples) => super::i16::ops::execute_quantize(
            samples.as_raw(), width, height, colors, kmeans, method, selection,
        )?,
        DynamicImage::ImageI16L(samples) => super::i16l::ops::execute_quantize(
            samples.as_raw(), width, height, colors, kmeans, method, selection,
        )?,
        DynamicImage::ImageI16B(samples) => super::i16b::ops::execute_quantize(
            samples.as_raw(), width, height, colors, kmeans, method, selection,
        )?,
        DynamicImage::ImageI16N(samples) => super::i16n::ops::execute_quantize(
            samples.as_raw(), width, height, colors, kmeans, method, selection,
        )?,
        DynamicImage::ImageLumaA16(samples) => super::luma_a16::ops::execute_quantize(
            samples.as_raw(), width, height, colors, kmeans, method, selection,
        )?,
        DynamicImage::ImageRgb16(samples) => super::rgb16::ops::execute_quantize(
            samples.as_raw(), width, height, colors, kmeans, method, selection,
        )?,
        DynamicImage::ImageRgba16(samples) => super::rgba16::ops::execute_quantize(
            samples.as_raw(), width, height, colors, kmeans, method, selection,
        )?,
        DynamicImage::ImageRgb32F(samples) => super::rgb32f::ops::execute_quantize(
            samples.as_raw(), width, height, colors, kmeans, method, selection,
        )?,
        DynamicImage::ImageRgba32F(samples) => super::rgba32f::ops::execute_quantize(
            samples.as_raw(), width, height, colors, kmeans, method, selection,
        )?,
        _ => {
            return Err(PilError::InternalError(
                "Native CoreOnly Quantize received an unsupported mode".into(),
            ));
        }
    };
    Ok(QuantizedColors { indices, palette, alpha })
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
            super::rgb_premultiplied8::ops::execute_quantize(
                samples.as_raw(), width, height, colors, 0, 2, selection,
            )?
        }
        _ => {
            return Err(PilError::InternalError(
                "RGBa Quantize strategy received a different DynamicImage variant".into(),
            ));
        }
    };
    Ok(QuantizedColors { indices, palette, alpha })
}

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
        source: Arc::new(Image::from_dynamic(
            DynamicImage::ImagePalette8(output))),
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
