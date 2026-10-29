//! Channel extraction and alpha mutation methods for [`Image`].

use std::sync::Arc;

use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, GenericImageView};

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

fn putalpha_conversion_target(mode: &str) -> Option<&'static str> {
    match mode {
        "1" | "I" | "I;16" | "I;16L" | "I;16B" | "I;16N" | "F" => Some("LA"),
        "YCbCr" | "HSV" => Some("RGBA"),
        _ => None,
    }
}

#[inline(always)]
fn eager_getchannel_tiny_rgb(
    image: &DynamicImage,
    mode: &str,
    channel: u8,
    backend: crate::compute::Backend,
) -> Result<Option<DynamicImage>, PilError> {
    let operation = PipelineOp::ExtractBand { index: channel };
    match backend {
        crate::compute::Backend::Cpu => {
            let context =
                crate::compute::ExecutionContext::for_backend(crate::compute::Backend::Cpu);
            Ok(Some(crate::compute::execute_eager_cpu_image_operation(
                &context,
                "ExtractBand",
                image,
                // The ExtractBand kernel records its output allocation;
                // manual accounting would count it twice.
                false,
                || {
                    crate::compute::registry::execute_cpu_in_context(
                        &context,
                        &operation,
                        image,
                        Some(mode),
                    )
                },
            )?))
        }
        crate::compute::Backend::Simd => {
            let context =
                crate::compute::ExecutionContext::for_backend(crate::compute::Backend::Simd);
            crate::compute::execute_eager_simd_image_operation(
                &context,
                "ExtractBand",
                image,
                || {
                    let (width, height) = image.dimensions();
                    let mut output =
                        crate::checked_dims::CheckedDims::new_allow_empty(width, height, 1)?
                            .alloc_buffer();
                    let (vector_blocks, scalar_tail, path) =
                        crate::compute::simd_extract_band_into_in_context(
                            &context,
                            image,
                            channel,
                            Some(mode),
                            &mut output,
                        )?;
                    let output = crate::raster::GrayImage::from_raw(width, height, output)
                        .ok_or_else(|| {
                            PilError::InternalError("SIMD ExtractBand buffer shape mismatch".into())
                        })?;
                    Ok((
                        DynamicImage::ImageLuma8(output),
                        vector_blocks,
                        scalar_tail,
                        path,
                    ))
                },
            )
            .map(Some)
        }
        crate::compute::Backend::Gpu => Ok(None),
        crate::compute::Backend::ParallelCpu => Ok(None),
    }
}

impl Image {
    /// Extracts one channel as an `L` image.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when `channel` is out of range, or
    /// another [`PilError`] when materialization fails.
    pub fn getchannel(&self, channel: i32) -> Result<Image, PilError> {
        let mode = self.mode()?;
        // Pillow's ImagingCore getchannel entry point rejects scalar I/F
        // storage before band extraction; it does not expose those four-byte
        // samples through the byte-oriented ExtractBand operation.
        if matches!(mode.as_str(), "I" | "F") {
            return Err(PilError::ValueError("image has wrong mode".into()));
        }
        // Validate channel index (requires materialized image for band count)
        let img = self.materialized_shared()?;
        let bands = img.color().channel_count();
        let ch = usize::try_from(channel)
            .ok()
            .filter(|&channel| channel < usize::from(bands))
            .ok_or_else(|| PilError::ValueError("band index out of range".into()))?;
        let backend_channel =
            u8::try_from(ch).expect("image channel count fits in a byte-sized index");
        // Pillow treats a single-band P image's only band as the palette-index
        // image itself, preserving its mode and retained palette. Extracting
        // it through the generic raster path would expand the indices to L and
        // silently lose that observable palette contract. `has_palette_mode`
        // intentionally excludes PA, and a P image exposes exactly one
        // channel, so a valid selector is always 0.
        if self.has_palette_mode() {
            return Ok(self.copy());
        }
        // Extracting the sole byte band from an exact-sized, already-loaded
        // L image is an identity. Share its immutable pixel buffer through
        // Image::copy's copy-on-write semantics rather than scanning and
        // allocating a second full-frame buffer. Keep other backends and
        // non-exact storage on the normal extraction path so their execution
        // and trailing-sample behavior remain unchanged.
        if mode == "L"
            && ch == 0
            && matches!(self, Image::Loaded(_))
            && !cfg!(feature = "parallel")
            && let Some((crate::compute::Backend::Cpu, _)) =
                crate::compute::preferred_active_backend()?
        {
            let pixel_count =
                crate::checked_dims::CheckedDims::new_allow_empty(img.width(), img.height(), 1)?
                    .total_pixels();
            if img.as_bytes().len() == pixel_count {
                return crate::compute::execute_eager_cpu_shared_result(
                    "ExtractBand",
                    img.as_ref(),
                    || Ok(self.copy()),
                );
            }
        }
        // Tiny RGB images can extract their band directly instead of paying
        // for a one-operation pipeline and its later materialization. Keep
        // full-frame work on the pipeline path, and retain lazy execution for
        // pending graphs, SIMD, GPU, and opt-in Parallel CPU profiles.
        if matches!(self, Image::Loaded(_))
            && !cfg!(feature = "parallel")
            && mode == "RGB"
            && u64::from(img.width()) * u64::from(img.height()) <= 256
            && let Some((backend, _)) = crate::compute::preferred_active_backend()?
        {
            let extracted =
                eager_getchannel_tiny_rgb(img.as_ref(), &mode, backend_channel, backend)?;
            if let Some(extracted) = extracted {
                let mut result = Image::from_dynamic(extracted, None);
                if let Image::Loaded(data) = &mut result {
                    data.info = self.image_info();
                    data.exif = self.exif_metadata();
                }
                return Ok(result);
            }
        }
        // The validation above has already materialized a pending pipeline.
        // Branch from those resolved pixels so extracting multiple channels
        // does not replay a mode-changing parent pipeline for every band.
        let extraction_source =
            if matches!(self, Image::Pipeline { .. }) && !self.has_palette_samples() {
                self.materialized_branch()?
            } else {
                self.clone()
            };
        let extracted = Image::push_op(
            &extraction_source,
            PipelineOp::ExtractBand {
                index: backend_channel,
            },
        );
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
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; materialization reports later pipeline
    /// failures.
    pub fn putalpha(&mut self, alpha: u8) -> Result<(), PilError> {
        let mode_name = self.mode()?;
        if let Some(target) = putalpha_conversion_target(&mode_name) {
            // Pillow Image.py derives the alpha mode from getmodebase(), then
            // surfaces the exact failed core conversion when neither in-place
            // setmode nor ImagingConvert supports this source/target pair.
            return Err(PilError::ValueError(format!(
                "conversion from {mode_name} to {target} not supported"
            )));
        }
        let mode = crate::pipeline::PixelMode::from_name(&mode_name).ok_or_else(|| {
            PilError::ValueError(format!("unsupported putalpha mode: {mode_name}"))
        })?;
        let new_self = Image::push_op(self, PipelineOp::PutAlpha { alpha, mode });
        *self = new_self;
        Ok(())
    }

    /// Applies the Python-facing scalar-or-mask `putalpha` contract.
    pub fn putalpha_with_input(&mut self, input: PutAlphaInput) -> Result<(), PilError> {
        match input {
            PutAlphaInput::Integer(value) => self.putalpha(value.clamp(0, 255) as u8),
            PutAlphaInput::Image(mask) => self.putalpha_data(&mask),
            PutAlphaInput::Invalid(type_name) => Err(PilError::TypeError(format!(
                "'{type_name}' object cannot be interpreted as an integer"
            ))),
        }
    }

    /// Replaces the alpha channel from an `L` mask image, matching Pillow's
    /// image-backed ``Image.putalpha``.
    pub fn putalpha_data(&mut self, mask: &Image) -> Result<(), PilError> {
        let mask_img = mask.materialize()?;
        let mask_mode = mask.mode()?;
        if mask_mode != "1" && mask_mode != "L" {
            return Err(PilError::ValueError("illegal image mode".into()));
        }
        if (mask_img.width(), mask_img.height()) != (self.size()?.0, self.size()?.1) {
            return Err(PilError::ValueError("images do not match".into()));
        }
        let mask_luma = mask_img.to_luma8();
        let mode_name = self.mode()?;
        if let Some(target) = putalpha_conversion_target(&mode_name) {
            // Keep image-backed alpha on the same failed conversion path as
            // scalar alpha. Pillow rejects these source/target pairs before
            // queuing the alpha-data operation.
            return Err(PilError::ValueError(format!(
                "conversion from {mode_name} to {target} not supported"
            )));
        }
        let mode = crate::pipeline::PixelMode::from_name(&mode_name).ok_or_else(|| {
            PilError::ValueError(format!("unsupported putalpha mode: {mode_name}"))
        })?;
        let new_self = Image::push_op(
            self,
            PipelineOp::PutAlphaData {
                mask: Arc::new(crate::raster::DynamicImage::ImageLuma8(mask_luma)),
                mode,
            },
        );
        *self = new_self;
        Ok(())
    }
}
