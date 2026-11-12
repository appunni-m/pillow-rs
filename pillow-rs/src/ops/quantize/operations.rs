//! Public Image quantization inputs and CPU strategy routing.

use crate::compute::PipelineOperationTelemetryCapture;
use crate::error::PilError;
use crate::image::Image;
use crate::raster::Mode;

use super::input::QuantizePalette;

impl Image {
    /// Applies Pillow's public quantize defaults and validation before the
    /// algorithm-specific implementation runs.
    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    pub fn quantize_with_input(
        &self,
        colors: Option<i32>,
        method: Option<i32>,
        kmeans: Option<i32>,
        palette: QuantizePalette,
        dither: Option<bool>,
    ) -> Result<Image, PilError> {
        self.quantize_with_visible_mode(colors, method, kmeans, palette, dither, None)
    }

    /// Host-adapter entry retaining a wrapper-visible Mode for Pillow's
    /// supplied-palette admission while execution still reads DynamicImage.
    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn quantize_with_visible_mode(
        &self,
        colors: Option<i32>,
        method: Option<i32>,
        kmeans: Option<i32>,
        palette: QuantizePalette,
        dither: Option<bool>,
        visible_mode: Option<Mode>,
    ) -> Result<Image, PilError> {
        let colors = colors.unwrap_or(256);
        let source_mode = self.mode_identity()?;
        let has_supplied_palette = !matches!(&palette, QuantizePalette::None);
        let admission_mode = if has_supplied_palette {
            visible_mode.unwrap_or(source_mode)
        } else {
            source_mode
        };
        let method = method.map(|method| method as u32).unwrap_or_else(|| {
            crate::compute::pool_cpu::ops::imageops::ops::default_method(admission_mode)
        });
        crate::compute::pool_cpu::ops::imageops::ops::validate_pillow_rgba_method(
            admission_mode,
            method,
        )?;
        if matches!(&palette, QuantizePalette::Other) {
            return Err(PilError::TypeError(
                "palette must be a supported Pillow-rs Image".to_owned(),
            ));
        }
        if let QuantizePalette::Image(palette_image) = &palette {
            if palette_image.mode_identity()? != Mode::ImagePalette8 {
                return Err(PilError::ValueError(
                    "bad mode for palette image".to_owned(),
                ));
            }
            if !matches!(admission_mode, Mode::ImageRgb8 | Mode::ImageLuma8) {
                return Err(PilError::ValueError(
                    "only RGB or L mode images can be quantized to a palette".to_owned(),
                ));
            }
            return self.quantize_to_palette(
                palette_image,
                colors as u32,
                method,
                dither.unwrap_or(true),
                visible_mode,
            );
        }
        let kmeans = kmeans.unwrap_or(0);
        crate::compute::pool_cpu::ops::imageops::ops::validate_kmeans_count(i64::from(kmeans))?;
        self.quantize(
            colors as u32,
            kmeans as u32,
            None,
            dither.unwrap_or(true),
            method,
        )
    }

    /// Maps an RGB or L image to the explicitly supplied P-mode palette.
    ///
    /// Pillow's `Image.quantize(palette=...)` preserves the reference palette
    /// and uses its entries as the destination colors.
    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    fn quantize_to_palette(
        &self,
        palette_image: &Image,
        colors: u32,
        method: u32,
        dither: bool,
        visible_mode: Option<Mode>,
    ) -> Result<Image, PilError> {
        let mut operation_receipts = PipelineOperationTelemetryCapture::begin();
        let image = self.materialize()?;
        operation_receipts.capture();
        let (requested_backend, context) = self.cpu_arch_execution_route_advisory();
        with_quantize_receipt(operation_receipts, requested_backend, || {
            crate::compute::pool_cpu::ops::imageops::ops::execute_with_palette(
                &image,
                palette_image,
                colors,
                method,
                dither,
                visible_mode,
                &context,
            )
        })
    }

    /// Reduces the image to a `P` image with at most `colors` palette entries.
    ///
    /// L, P, and RGB images use Pillow's selected palette-reduction method;
    /// P samples are resolved through the attached palette. RGBA images use
    /// the default Pillow fast-octree path. RGBa uses an alpha-aware octree
    /// through its CoreOnly strategy. RGBX ignores its unused padding byte and
    /// uses an RGB-only quantizer. LA, La, PA, LA16, RGBA16, and RGBA32F use
    /// native alpha-aware quantization. Other declared CoreOnly modes use their
    /// exact-mode Default strategy and read native samples directly;
    /// these Rust extensions are not Pillow-defined behavior. The result
    /// carries palette indices plus palette data matching the selected mode
    /// strategy.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when `colors` is outside 1..=256, or
    /// another [`PilError`] when materialization fails.
    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    pub fn quantize(
        &self,
        colors: u32,
        kmeans: u32,
        _palette: Option<&Image>,
        _dither: bool,
        method: u32,
    ) -> Result<Image, PilError> {
        let mut operation_receipts = PipelineOperationTelemetryCapture::begin();
        let image = self.materialize()?;
        operation_receipts.capture();
        let palette = self.palette_input();
        let (requested_backend, context) = self.cpu_arch_execution_route_advisory();
        with_quantize_receipt(operation_receipts, requested_backend, || {
            let prepared = crate::compute::pool_cpu::ops::imageops::ops::prepare_quantize(
                &image, palette, colors, kmeans, method, &context,
            )?;
            let selection = *prepared.selection();
            Ok((prepared.execute()?, selection))
        })
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn with_quantize_receipt(
    mut operation_receipts: PipelineOperationTelemetryCapture,
    requested_backend: Option<crate::compute::Backend>,
    execute: impl FnOnce() -> Result<
        (Image, crate::compute::registry::support::StrategySelection),
        PilError,
    >,
) -> Result<Image, PilError> {
    if !crate::compute::Backend::pipeline_telemetry_enabled() {
        return execute().map(|(image, _)| image);
    }
    // This timing/resource sample covers terminal Quantize; ordered source
    // graph operation receipts are restored separately.
    let telemetry =
        crate::compute::begin_eager_cpu_operation_telemetry("Quantize", requested_backend);
    operation_receipts.restore();
    match execute() {
        Ok((image, selection)) => {
            let output_bytes = match &image {
                Image::Paletted(data) => data.indices.as_bytes().len(),
                _ => 0,
            };
            crate::compute::finish_eager_cpu_operation_telemetry_with_selection(
                telemetry,
                output_bytes,
                "Quantize",
                selection,
            );
            Ok(image)
        }
        Err(error) => {
            crate::compute::cancel_eager_cpu_operation_telemetry(telemetry);
            Err(error)
        }
    }
}
