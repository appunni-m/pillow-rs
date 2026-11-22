//! Native write operations exposed through Pillow's ImagingCore compatibility surface.

use std::sync::Arc;

use crate::compute::{Backend, ExecutionContext, PipelineOperationTelemetryCapture};
use crate::error::PilError;
use crate::image::{Image, materialization_cache};
use crate::raster::Mode;

/// Transparency key accepted by Pillow's ImagingCore convert_transparent method.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImagingCoreTransparencyKey {
    /// A scalar key; source-mode dispatch determines how Pillow expands it.
    Scalar(i32),
    /// A three-component RGB key.
    Rgb(i32, i32, i32),
}

type ImagingCoreRoute = (Option<Backend>, Option<Backend>, ExecutionContext);

impl Image {
    /// Reports whether this image uses Pillow's explicit single-block allocator.
    ///
    /// Pillow's ImagingCore checks its allocator-owned `block` pointer. Rust
    /// images use owned raster vectors and do not use Pillow's block allocator.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
    pub fn imaging_core_isblock(&self) -> bool {
        false
    }

    /// Converts the native ImagingCore raster while applying a transparent key.
    ///
    /// The key and output mode remain typed at the Rust boundary; the host
    /// adapter owns Pillow's string and scalar-versus-sequence parsing.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    pub fn imaging_core_convert_transparent(
        &self,
        target: Mode,
        transparency: ImagingCoreTransparencyKey,
    ) -> Result<Image, PilError> {
        self.imaging_core_convert_transparent_with_route(target, transparency, None)
    }

    /// Converts ImagingCore pixels under the active host-selected CPU route.
    #[doc(hidden)]
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    pub fn imaging_core_convert_transparent_with_execution_context(
        &self,
        target: Mode,
        transparency: ImagingCoreTransparencyKey,
        requested_backend: Option<Backend>,
        pinned_backend: Option<Backend>,
        context: ExecutionContext,
    ) -> Result<Image, PilError> {
        self.imaging_core_convert_transparent_with_route(
            target,
            transparency,
            Some((requested_backend, pinned_backend, context)),
        )
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn imaging_core_convert_transparent_with_route(
        &self,
        target: Mode,
        transparency: ImagingCoreTransparencyKey,
        route: Option<ImagingCoreRoute>,
    ) -> Result<Image, PilError> {
        let source_mode = self.mode_identity()?;
        crate::compute::pool_cpu::ops::color::ops::validate_convert_transparent_target(
            source_mode,
            target,
        )?;
        let (requested_backend, _, context) = route.unwrap_or_else(|| {
            let (requested_backend, context) = self.cpu_arch_execution_route_advisory();
            (requested_backend, None, context)
        });
        let routed_source = match route {
            Some((_, pinned_backend, context)) => self
                .clone()
                .with_backend_and_execution_context(pinned_backend, context),
            None => self.clone(),
        };
        let (source, mut operation_receipts) =
            routed_source.materialized_shared_for_ops_with_receipts()?;
        // Pillow's C parser leaves scalar-key G/B at zero; its scalar-source
        // conversion branch expands them to R before comparing converted RGB.
        let key = match transparency {
            ImagingCoreTransparencyKey::Scalar(value) => [value as u8, 0, 0],
            ImagingCoreTransparencyKey::Rgb(red, green, blue) => {
                [red as u8, green as u8, blue as u8]
            }
        };
        let output =
            crate::compute::execute_eager_cpu_arch_image_operation_with_context_and_receipts(
                "ImagingCoreConvertTransparent",
                source.as_ref(),
                context,
                requested_backend,
                None,
                &mut operation_receipts,
                |ctx| {
                    let prepared =
                        crate::compute::pool_cpu::ops::color::ops::prepare_convert_transparent(
                            crate::compute::registry::support::OperationInputs {
                                image: source.as_ref(),
                                palette: Default::default(),
                            },
                            target,
                            key,
                            ctx,
                        )?;
                    let selection = *prepared.selection();
                    prepared.execute().map(|result| (result, selection))
                },
            )?;
        Image::from_dynamic_with_mode(output, Some(target))
    }

    /// Changes the callback-visible ImagingCore mode through Pillow's native
    /// same-mode, `1`/`L`, and RGB/RGBA/RGBX paths.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when the target transition is not
    /// supported by the current mode, and propagates materialization errors.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    pub fn imaging_core_setmode(&mut self, target: Mode) -> Result<(), PilError> {
        self.imaging_core_setmode_with_route(target, None)
    }

    /// Changes ImagingCore mode under a context selected by the host wrapper.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn imaging_core_setmode_with_execution_context(
        &mut self,
        target: Mode,
        requested_backend: Option<crate::compute::Backend>,
        pinned_backend: Option<crate::compute::Backend>,
        context: crate::compute::ExecutionContext,
    ) -> Result<(), PilError> {
        self.imaging_core_setmode_with_route(
            target,
            Some((requested_backend, pinned_backend, context)),
        )
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn imaging_core_setmode_with_route(
        &mut self,
        target: Mode,
        route: Option<ImagingCoreRoute>,
    ) -> Result<(), PilError> {
        let mut operation_receipts = PipelineOperationTelemetryCapture::begin();
        let source = self.mode_identity()?;
        let (requested_backend, _, context) = route.unwrap_or_else(|| {
            let (requested_backend, context) = self.cpu_arch_execution_route_advisory();
            (requested_backend, None, context)
        });
        let selection = crate::compute::pool_cpu::ops::put::ops::select_imaging_core_set_mode(
            source, target, &context,
        )?;
        if source != target {
            if let Some((_, pinned_backend, context)) = route {
                *self = self
                    .clone()
                    .with_backend_and_execution_context(pinned_backend, context);
            }
            self.load()?;
            operation_receipts.capture();
            self.mutate_materialized_pixels(|image| {
                crate::compute::pool_cpu::ops::put::ops::execute_imaging_core_set_mode(
                    image, target, selection,
                )
            })?;
        } else {
            operation_receipts.capture();
        }
        crate::compute::record_eager_cpu_operation_receipt_with_selection_and_receipts(
            "ImagingCoreSetMode",
            requested_backend,
            0,
            "ImagingCore setmode uses the serial Default strategy",
            selection,
            operation_receipts,
        );
        Ok(())
    }

    /// Replaces one native band through Pillow's callback-visible ImagingCore API.
    ///
    /// Multiband destinations accept a one-byte single-band source; a
    /// single-band destination requires the same exact mode. The destination
    /// keeps its palette and metadata.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when the source, band, dimensions, or
    /// stored sample layout does not match the destination.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    pub fn imaging_core_putband(&mut self, source: &Image, band: i32) -> Result<(), PilError> {
        self.imaging_core_putband_with_route(source, band, None)
    }

    /// Replaces one ImagingCore band under a context selected by the host wrapper.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn imaging_core_putband_with_execution_context(
        &mut self,
        source: &Image,
        band: i32,
        requested_backend: Option<crate::compute::Backend>,
        pinned_backend: Option<crate::compute::Backend>,
        context: crate::compute::ExecutionContext,
    ) -> Result<(), PilError> {
        self.imaging_core_putband_with_route(
            source,
            band,
            Some((requested_backend, pinned_backend, context)),
        )
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn imaging_core_putband_with_route(
        &mut self,
        source: &Image,
        band: i32,
        route: Option<ImagingCoreRoute>,
    ) -> Result<(), PilError> {
        let mut operation_receipts = PipelineOperationTelemetryCapture::begin();
        let source_mode = source.mode_identity()?;
        crate::compute::pool_cpu::ops::put::helpers::imaging_core_band::validate_put_band_source(
            source_mode,
        )?;
        let destination_mode = self.mode_identity()?;
        let band =
            crate::compute::pool_cpu::ops::put::helpers::imaging_core_band::validate_band_index(
                band,
                destination_mode.pillow_band_count(),
            )?;
        crate::compute::pool_cpu::ops::put::helpers::imaging_core_band::validate_put_band_layout(
            destination_mode,
            source_mode,
            self.size()?,
            source.size()?,
        )?;
        let (requested_backend, _, context) = route.unwrap_or_else(|| {
            let (requested_backend, context) = self.cpu_arch_execution_route_advisory();
            (requested_backend, None, context)
        });
        let selection =
            crate::compute::pool_cpu::ops::put::ops::select_put_band(destination_mode, &context)?;
        let routed_source = match route {
            Some((_, pinned_backend, context)) => source
                .clone()
                .with_backend_and_execution_context(pinned_backend, context),
            None => source.clone(),
        };
        let source_pixels = routed_source.materialized_shared()?;
        if let Some((_, pinned_backend, context)) = route {
            *self = self
                .clone()
                .with_backend_and_execution_context(pinned_backend, context);
        }
        self.load()?;
        operation_receipts.capture();
        self.mutate_materialized_pixels(|destination| {
            crate::compute::pool_cpu::ops::put::ops::execute_put_band(
                destination,
                source_pixels.as_ref(),
                band,
                selection,
            )
        })?;
        crate::compute::record_eager_cpu_operation_receipt_with_selection_and_receipts(
            "PutBand",
            requested_backend,
            0,
            "ImagingCore band writes use the serial Default strategy",
            selection,
            operation_receipts,
        );
        Ok(())
    }

    /// Fills one native byte band through Pillow's callback-visible ImagingCore API.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when the destination mode or band is
    /// unsupported, and propagates materialization errors.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    pub fn imaging_core_fillband(&mut self, band: i32, value: i32) -> Result<(), PilError> {
        self.imaging_core_fillband_with_route(band, value, None)
    }

    /// Fills one ImagingCore band under a context selected by the host wrapper.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn imaging_core_fillband_with_execution_context(
        &mut self,
        band: i32,
        value: i32,
        requested_backend: Option<crate::compute::Backend>,
        pinned_backend: Option<crate::compute::Backend>,
        context: crate::compute::ExecutionContext,
    ) -> Result<(), PilError> {
        self.imaging_core_fillband_with_route(
            band,
            value,
            Some((requested_backend, pinned_backend, context)),
        )
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn imaging_core_fillband_with_route(
        &mut self,
        band: i32,
        value: i32,
        route: Option<ImagingCoreRoute>,
    ) -> Result<(), PilError> {
        let mut operation_receipts = PipelineOperationTelemetryCapture::begin();
        let mode = self.mode_identity()?;
        let (requested_backend, _, context) = route.unwrap_or_else(|| {
            let (requested_backend, context) = self.cpu_arch_execution_route_advisory();
            (requested_backend, None, context)
        });
        let selection = crate::compute::pool_cpu::ops::put::ops::select_fill_band(mode, &context)?;
        let band =
            crate::compute::pool_cpu::ops::put::helpers::imaging_core_band::validate_band_index(
                band,
                mode.pillow_band_count(),
            )?;
        let value = value.clamp(0, i32::from(u8::MAX)) as u8;
        if let Some((_, pinned_backend, context)) = route {
            *self = self
                .clone()
                .with_backend_and_execution_context(pinned_backend, context);
        }
        self.load()?;
        operation_receipts.capture();
        self.mutate_materialized_pixels(|image| {
            crate::compute::pool_cpu::ops::put::ops::execute_fill_band(
                image, band, value, selection,
            )
        })?;
        crate::compute::record_eager_cpu_operation_receipt_with_selection_and_receipts(
            "FillBand",
            requested_backend,
            0,
            "ImagingCore band fills use the serial Default strategy",
            selection,
            operation_receipts,
        );
        Ok(())
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn mutate_materialized_pixels(
        &mut self,
        operation: impl FnOnce(&mut crate::raster::DynamicImage) -> Result<(), PilError>,
    ) -> Result<(), PilError> {
        match self {
            Image::Loaded(data) => {
                operation(Arc::make_mut(&mut data.image))?;
                data.known_zero_fill = false;
            }
            Image::Paletted(data) => {
                operation(&mut data.indices)?;
                data.materialized = materialization_cache();
            }
            _ => {
                return Err(PilError::InternalError(
                    "ImagingCore band mutation did not materialize writable pixels".into(),
                ));
            }
        }
        Ok(())
    }
}
