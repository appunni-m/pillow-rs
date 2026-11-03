//! Channel extraction and alpha mutation methods for [`Image`].
// TODO(pillow-rs#3ptb): verify PutAlpha/PutAlphaData wrapper mode-change,
// mutation, metadata, and error order in CPU parity, including integer coercion.
// Their operation declarations now
// agree with the wrapper's Pillow-defined rejections. Native-width modes with
// exact implementations are admitted by the Rust API as CoreOnly extensions;
// the lazy output planner records their typed transitions. It no longer
// predicts transitions for rejected YCbCr/HSV descriptors. `putalpha_data`
// validates the destination mode before inspecting the mask, matching
// Pillow's first validation step. If a later mask check fails, the wrapper
// leaves the destination's opaque-alpha promotion visible. Successful calls
// go directly through PutAlphaData, so they do not stage a second raster pass.
// Include GetChannel's One/P/I;16* identity routes and receipt policy, I/F
// rejection order, CoreOnly high-depth outputs, exact selector counts,
// prepared-child selection receipts, and GPU-to-host identity fallback in CPU
// parity. Loaded and Bytes/Pipeline caches successful at call entry report
// `native-identity`; Paletted copies report `native-copy`. None selects an
// architecture. Lazy identity routes preserve source graph receipts before
// the terminal identity receipt. Multiband ExtractBand selection remains
// deferred until child use.
// The operation route declares copy, shared-buffer, eager, and ExtractBand
// child paths per mode. PutAlpha's Rust/Python ordering and metadata paths have
// been source-reviewed; value and receipt parity remains open.

use std::sync::Arc;

use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OperationModeRoute, OperationRouteGuard,
};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, GenericImageView, Mode};

use super::Image;

/// Host-neutral selector for `Image.getchannel`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelSelector {
    /// Numeric band index.
    Index(i32),
    /// Pillow band name such as `"R"` or `"A"`.
    Name(String),
    /// A host value that is neither an integer nor a string, retaining its
    /// type name for Pillow's public conversion error.
    Invalid(String),
}

/// Host-neutral input for Pillow's `Image.putalpha` wrapper.
#[derive(Debug, Clone)]
pub enum PutAlphaInput {
    /// An integer value. Pillow clamps this to one byte for values outside
    /// 0..255.
    Integer(i64),
    /// An image mask supplied by the caller.
    Image(Box<Image>),
    /// A value that cannot be interpreted as an integer or image mask.
    Invalid(String),
}

// NO_PILLOW_SUPPORT: the operation declaration includes native high-depth
// alpha routes that extend Pillow's operation contract.
fn validate_putalpha_mode(operation: OperationId, mode: Mode) -> Result<(), PilError> {
    operation.support().for_mode(mode).validate_mode()
}

fn promote_putalpha_destination(image: &mut Image, mode: Mode) -> Result<(), PilError> {
    if !matches!(mode, Mode::ImageLumaA8 | Mode::ImagePaletteAlpha8 | Mode::ImageRgba8) {
        image.putalpha(255)?;
    }
    Ok(())
}

fn validated_putalpha_mask(image: &Image, mask: &Image) -> Result<DynamicImage, PilError> {
    let mask_mode = mask.mode_identity()?;
    if !crate::compute::registry::support::PUT_ALPHA_DATA_MASK_MODES.contains(&mask_mode) {
        return Err(PilError::ValueError("illegal image mode".into()));
    }
    let mask_image = mask.materialize()?;
    if !crate::compute::registry::support::PUT_ALPHA_DATA_MASK_MODES
        .contains(&mask_image.mode())
    {
        return Err(PilError::ValueError("illegal image mode".into()));
    }
    if (mask_image.width(), mask_image.height()) != image.size()? {
        return Err(PilError::ValueError("images do not match".into()));
    }
    Ok(mask_image)
}

fn getchannel_band_count(support: ModeSupport) -> usize {
    let ModeSupport::Supported { inputs, .. } = support else {
        unreachable!("GetChannel mode is validated before reading its band contract")
    };
    inputs
        .iter()
        .find_map(|contract| match contract {
            InputContract::CallerValidatedGetChannelIndex { bands } => Some(usize::from(*bands)),
            _ => None,
        })
        .expect("GetChannel support must declare the exact source-mode band count")
}

#[inline(always)]
fn eager_getchannel_tiny_rgb(
    image: &DynamicImage,
    channel: u8,
    backend: crate::compute::Backend,
    context: crate::compute::ExecutionContext,
) -> Result<Option<DynamicImage>, PilError> {
    let operation = PipelineOp::ExtractBand { index: channel };
    match backend {
        crate::compute::Backend::Cpu | crate::compute::Backend::Simd => {
            crate::compute::execute_eager_cpu_arch_image_operation_with_context(
                "ExtractBand",
                image,
                context,
                Some(backend),
                None,
                |ctx| {
                    let prepared = crate::compute::registry::prepare_cpu(
                        &operation,
                        crate::compute::registry::support::OperationInputs {
                            image,
                            palette: crate::compute::registry::support::PaletteInput::default(),
                        },
                        ctx,
                    )?;
                    let selection = *prepared.selection();
                    prepared.execute().map(|result| (result, selection))
                },
            )
            .map(Some)
        }
        crate::compute::Backend::Gpu => Ok(None),
    }
}

fn eager_getchannel_identity_copy(
    image: &Image,
    channel: i32,
    bands: usize,
) -> Result<Image, PilError> {
    let mut operation_receipts = crate::compute::PipelineOperationTelemetryCapture::begin();
    let pixels = image.materialized_shared()?;
    operation_receipts.capture();
    let channel = checked_channel_index(channel, bands)?;
    let _ = u8::try_from(channel).expect("image channel count fits in a byte-sized index");
    let requested_backend = crate::compute::Backend::pipeline_telemetry_enabled()
        .then(|| image.cpu_arch_execution_route_advisory().0)
        .flatten();
    crate::compute::execute_eager_native_identity_result_with_receipts(
        "ExtractBand",
        pixels.as_ref(),
        requested_backend,
        operation_receipts,
        || Ok(image.copy()),
    )
}

fn checked_channel_index(channel: i32, bands: usize) -> Result<usize, PilError> {
    usize::try_from(channel)
        .ok()
        .filter(|&channel| channel < bands)
        .ok_or_else(|| PilError::ValueError("band index out of range".into()))
}

fn eager_getchannel_shared_buffer_identity(
    image: &Image,
    route: OperationModeRoute,
    pixels: &DynamicImage,
) -> Result<Option<Image>, PilError> {
    let Some(guards) = route.shared_buffer_identity_guards() else {
        return Ok(None);
    };
    let pixel_count = crate::checked_dims::CheckedDims::new_allow_empty(
        pixels.width(),
        pixels.height(),
        1,
    )?
    .total_pixels();
    let byte_length = pixels.as_bytes().len();
    let loaded_image = matches!(image, Image::Loaded(_));
    if !eager_route_guards_pass(guards, loaded_image, None, pixel_count, byte_length, false) {
        return Ok(None);
    }
    let requested_backend = image.requested_backend_route()?;
    if !eager_route_guards_pass(
        guards,
        loaded_image,
        requested_backend,
        pixel_count,
        byte_length,
        true,
    ) {
        return Ok(None);
    }
    crate::compute::execute_eager_native_identity_result(
        "ExtractBand",
        pixels,
        requested_backend,
        || Ok(image.copy()),
    )
    .map(Some)
}

fn eager_getchannel_tiny_rgb_child(
    image: &Image,
    route: OperationModeRoute,
    pixels: &DynamicImage,
    channel: u8,
) -> Result<Option<Image>, PilError> {
    let Some(guards) = route.eager_child_guards(OperationId::ExtractBand) else {
        return Ok(None);
    };
    let pixel_count = u64::from(pixels.width()) * u64::from(pixels.height());
    let loaded_image = matches!(image, Image::Loaded(_));
    if !eager_route_guards_pass(
        guards,
        loaded_image,
        None,
        pixel_count,
        pixels.as_bytes().len(),
        false,
    ) {
        return image.queue_getchannel_child(route, channel).map(Some);
    }
    let (requested_backend, context) = image.cpu_arch_execution_route()?;
    if !eager_route_guards_pass(
        guards,
        loaded_image,
        requested_backend,
        pixel_count,
        pixels.as_bytes().len(),
        true,
    ) {
        return Ok(None);
    }
    let backend = requested_backend.expect("satisfied backend guard must be present");
    let extracted = eager_getchannel_tiny_rgb(pixels, channel, backend, context)?;
    let Some(extracted) = extracted else {
        return Ok(None);
    };
    let mut result = Image::from_dynamic(extracted);
    if let Image::Loaded(data) = &mut result {
        data.info = image.image_info();
        data.exif = image.exif_metadata();
    }
    Ok(Some(result))
}

fn concrete_identity_source(
    image: &Image,
    cached_at_entry: bool,
) -> Option<(&DynamicImage, bool)> {
    match image {
        Image::Loaded(data) => Some((data.image.as_ref(), true)),
        Image::Paletted(data) => Some((&data.indices, false)),
        Image::Bytes { materialized, .. } | Image::Pipeline { materialized, .. }
            if cached_at_entry =>
        {
            let image = materialized.get()?.as_ref().ok()?;
            Some((image.as_ref(), true))
        }
        Image::Bytes { .. } | Image::Pipeline { .. } => None,
    }
}

fn eager_route_guards_pass(
    guards: &[OperationRouteGuard],
    loaded_image: bool,
    requested_backend: Option<crate::compute::Backend>,
    pixel_count: u64,
    byte_length: usize,
    backend_resolved: bool,
) -> bool {
    guards.iter().all(|guard| match guard {
        OperationRouteGuard::LoadedImage => loaded_image,
        OperationRouteGuard::RequestedBackendIn(backends) => {
            !backend_resolved
                || requested_backend.is_some_and(|backend| backends.contains(&backend))
        }
        OperationRouteGuard::NonEmptyInput => pixel_count != 0,
        // Channel extraction never has Draw context/mask facts, so it cannot
        // take a Draw-only wrapper route.
        OperationRouteGuard::DrawBitmapRouteIn(_) => false,
        OperationRouteGuard::PixelCountAtMost(limit) => pixel_count <= *limit,
        OperationRouteGuard::ByteStorageLengthMatchesPixelCount => {
            usize::try_from(pixel_count).ok() == Some(byte_length)
        }
    })
}

impl Image {
    /// Extracts one channel while preserving single-band identity and native scalar width.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when `channel` is out of range, or
    /// another [`PilError`] when materialization fails.
    pub fn getchannel(&self, channel: i32) -> Result<Image, PilError> {
        let cached_at_entry = self.is_materialized();
        let mode = self.mode_identity()?;
        let route = OperationId::GetChannel.support().route_for_mode(mode);
        let support = route.prepared_support();
        // The wrapper declaration preserves Pillow's I/F rejection before
        // materialization and before validating the requested channel index.
        support.validate_mode()?;
        let bands = getchannel_band_count(support);
        // Pillow's core copies any one-band UINT8 source directly, preserving
        // mode `1`; its P copy also retains the palette. Rust extends the same
        // native identity contract to I;16* modes, which Pillow rejects. The
        // mode support declaration identifies these exact wrapper-owned routes.
        if route.has_copy_path() {
            return self.getchannel_identity_copy(channel, bands, cached_at_entry);
        }
        self.getchannel_materialized_child(route, channel, bands)
    }

    fn getchannel_identity_copy(
        &self,
        channel: i32,
        bands: usize,
        cached_at_entry: bool,
    ) -> Result<Image, PilError> {
        if let Some((input, shares_pixels)) = concrete_identity_source(self, cached_at_entry) {
            let _ = checked_channel_index(channel, bands)?;
            let requested_backend = crate::compute::Backend::pipeline_telemetry_enabled()
                .then(|| self.cpu_arch_execution_route_advisory().0)
                .flatten();
            let copy = || Ok(self.copy());
            return if shares_pixels {
                crate::compute::execute_eager_native_identity_result(
                    "ExtractBand",
                    input,
                    requested_backend,
                    copy,
                )
            } else {
                crate::compute::execute_eager_native_copy_result(
                    "ExtractBand",
                    input,
                    requested_backend,
                    copy,
                )
            };
        }
        eager_getchannel_identity_copy(self, channel, bands)
    }

    fn getchannel_materialized_child(
        &self,
        route: OperationModeRoute,
        channel: i32,
        bands: usize,
    ) -> Result<Image, PilError> {
        // Keep lazy decode and pipeline errors ahead of index validation for
        // sources without a successful cached materialization.
        let mut operation_receipts = crate::compute::PipelineOperationTelemetryCapture::begin();
        let img = self.materialized_shared()?;
        operation_receipts.capture();
        let ch = checked_channel_index(channel, bands)?;
        let backend_channel =
            u8::try_from(ch).expect("image channel count fits in a byte-sized index");
        // The operation declaration owns the exact-size loaded-L identity
        // route. Resolve only the backend request for this no-kernel path;
        // architecture selection belongs to the child implementation path.
        if let Some(result) = eager_getchannel_shared_buffer_identity(self, route, img.as_ref())? {
            return Ok(result);
        }
        // Tiny RGB images can extract their band directly instead of paying
        // for a one-operation pipeline and its later materialization. Keep
        // full-frame work on the pipeline path, and retain lazy execution for
        // pending graphs and routes without an eager implementation.
        if let Some(result) = eager_getchannel_tiny_rgb_child(
            self,
            route,
            img.as_ref(),
            backend_channel,
        )? {
            return Ok(result);
        }
        // The validation above has already materialized a pending pipeline.
        // Branch from those resolved pixels so extracting multiple channels
        // does not replay a mode-changing parent pipeline for every band.
        self.queue_getchannel_child(route, backend_channel)
    }

    fn queue_getchannel_child(
        &self,
        route: OperationModeRoute,
        channel: u8,
    ) -> Result<Image, PilError> {
        let child = route.pipeline_child().ok_or_else(|| {
            PilError::InternalError("GetChannel route omitted its pipeline child".into())
        })?;
        let operation = match child {
            OperationId::ExtractBand => PipelineOp::ExtractBand { index: channel },
            _ => {
                return Err(PilError::InternalError(
                    "GetChannel route declared an unsupported pipeline child".into(),
                ));
            }
        };
        let extraction_source =
            if matches!(self, Image::Pipeline { .. }) && !self.has_palette_samples() {
                self.materialized_branch()?
            } else {
                self.clone()
            };
        let extracted = Image::push_op(&extraction_source, operation);
        Ok(match self.backend() {
            Some(backend) => extracted.use_backend(backend),
            None => extracted,
        })
    }

    /// Resolves a numeric or named public channel selector before extraction.
    pub fn getchannel_selector(&self, selector: ChannelSelector) -> Result<Image, PilError> {
        let channel = match selector {
            ChannelSelector::Index(channel) => channel,
            ChannelSelector::Name(name) => {
                let bands = self.getbands()?;
                bands.iter().position(|band| band == &name).map_or_else(
                    || {
                        Err(PilError::ValueError(format!(
                            "The image has no channel \"{name}\""
                        )))
                    },
                    |index| Ok(index as i32),
                )?
            }
            ChannelSelector::Invalid(type_name) => {
                return Err(PilError::TypeError(format!(
                    "'{type_name}' object cannot be interpreted as an integer"
                )));
            }
        };
        self.getchannel(channel)
    }

    /// Queues replacement of the alpha channel.
    ///
    /// Materialization preserves Pillow-style mode intent: `L` becomes `LA`,
    /// `RGB` becomes `RGBA`, and existing `LA`/`RGBA` alpha bytes are replaced.
    /// Native-width Rust modes with declared CoreOnly strategies remain
    /// available as a Rust extension without a Pillow parity claim.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; materialization reports later pipeline
    /// failures.
    pub fn putalpha(&mut self, alpha: u8) -> Result<(), PilError> {
        let image_mode = self.mode_identity()?;
        validate_putalpha_mode(OperationId::PutAlpha, image_mode)?;
        let new_self = Image::push_op(
            self,
            PipelineOp::PutAlpha {
                alpha,
                mode: image_mode,
            },
        );
        *self = new_self;
        Ok(())
    }

    /// Applies the Python-facing scalar-or-mask `putalpha` contract.
    pub fn putalpha_with_input(&mut self, input: PutAlphaInput) -> Result<(), PilError> {
        match input {
            PutAlphaInput::Integer(value) => self.putalpha(value.clamp(0, 255) as u8),
            PutAlphaInput::Image(mask) => self.putalpha_data(&mask),
            PutAlphaInput::Invalid(type_name) => {
                let mode = self.mode_identity()?;
                validate_putalpha_mode(OperationId::PutAlpha, mode)?;
                promote_putalpha_destination(self, mode)?;
                Err(PilError::TypeError(format!(
                    "'{type_name}' object cannot be interpreted as an integer"
                )))
            }
        }
    }

    /// Replaces the alpha channel from a `1` or `L` mask image, matching Pillow's
    /// image-backed ``Image.putalpha``. Native-width Rust modes use their
    /// declared CoreOnly strategies without a Pillow parity claim.
    pub fn putalpha_data(&mut self, mask: &Image) -> Result<(), PilError> {
        let image_mode = self.mode_identity()?;
        validate_putalpha_mode(OperationId::PutAlphaData, image_mode)?;

        // Pillow promotes before mask validation. Because this wrapper holds
        // exclusive access, apply the same visible mutation only on failure;
        // valid calls stay on the direct native PutAlphaData route.
        let mask_img = match validated_putalpha_mask(self, mask) {
            Ok(mask) => mask,
            Err(error) => {
                promote_putalpha_destination(self, image_mode)?;
                return Err(error);
            }
        };
        let operation = PipelineOp::PutAlphaData {
            mask: Arc::new(mask_img),
            mode: image_mode,
        };
        let new_self = Image::push_op(self, operation);
        *self = new_self;
        Ok(())
    }
}
