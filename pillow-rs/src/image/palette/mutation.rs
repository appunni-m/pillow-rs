//! Palette attachment and alpha mutation for [`Image`].

use std::sync::Arc;

use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

use super::super::materialization_cache;
use super::super::{Image, LoadedData, PalettedData, PipelineOps};
use super::{PaletteData, PaletteModel, clear_pending_palette_alpha, split_palette_data};

type PaletteExecutionRoute = (
    Option<crate::compute::Backend>,
    crate::compute::ExecutionContext,
);

impl Image {
    /// Stages Pillow's L/LA-to-P/PA identity change before palette decoding.
    ///
    /// The pixel carrier stays unchanged. Encoded sources remain lazy until
    /// the wrapper reaches Pillow's required load boundary.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub fn stage_palette_attachment(&mut self) -> Result<(), PilError> {
        self.stage_palette_attachment_with_route(None).map(|_| ())
    }

    /// Stages the mode change and reports whether pixels were materialized at entry.
    ///
    /// Pillow's wrapper calls `load()` after palette assignment when its existing
    /// core image mode differs from the new wrapper mode. A lazy encoded image has
    /// no core pixels yet and can keep that load deferred for a native palette.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn stage_palette_attachment_with_load_requirement(&mut self) -> Result<bool, PilError> {
        self.stage_palette_attachment_with_route(None)
    }

    /// Stages palette mode under the route captured by the Python caller.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn stage_palette_attachment_with_execution_context(
        &mut self,
        pinned_backend: Option<crate::compute::Backend>,
        context: crate::compute::ExecutionContext,
    ) -> Result<(), PilError> {
        self.stage_palette_attachment_with_route(Some((pinned_backend, context)))
            .map(|_| ())
    }

    /// Stages the mode change and reports whether pixels were materialized at entry.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn stage_palette_attachment_with_load_requirement_and_execution_context(
        &mut self,
        pinned_backend: Option<crate::compute::Backend>,
        context: crate::compute::ExecutionContext,
    ) -> Result<bool, PilError> {
        self.stage_palette_attachment_with_route(Some((pinned_backend, context)))
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn stage_palette_attachment_with_route(
        &mut self,
        route: Option<PaletteExecutionRoute>,
    ) -> Result<bool, PilError> {
        let source_mode = self.mode_identity()?;
        let load_after_transition = self.is_materialized();
        let target_mode = match source_mode {
            Mode::ImageLuma8 => Mode::ImagePalette8,
            Mode::ImageLumaA8 => Mode::ImagePaletteAlpha8,
            Mode::ImagePalette8 | Mode::ImagePaletteAlpha8 => return Ok(false),
            _ => return Err(PilError::ValueError("illegal image mode".to_owned())),
        };

        if let Image::Loaded(data) = self
            && Arc::strong_count(&data.image) == 1
        {
            let placeholder = match source_mode {
                Mode::ImageLuma8 => DynamicImage::ImageLuma8(crate::raster::GrayImage::new(0, 0)),
                Mode::ImageLumaA8 => {
                    DynamicImage::ImageLumaA8(crate::raster::GrayAlphaImage::new(0, 0))
                }
                _ => unreachable!("source mode was validated"),
            };
            let image = Arc::get_mut(&mut data.image)
                .expect("loaded image was uniquely owned before palette mode staging");
            let source = std::mem::replace(image, placeholder);
            *image = match (source_mode, source) {
                (Mode::ImageLuma8, DynamicImage::ImageLuma8(image)) => {
                    DynamicImage::ImagePalette8(image)
                }
                (Mode::ImageLumaA8, DynamicImage::ImageLumaA8(image)) => {
                    DynamicImage::ImagePaletteAlpha8(image)
                }
                _ => unreachable!("loaded pixels did not match their mode identity"),
            };
            data.palette = Some(Vec::new());
            data.palette_alpha = Some(Vec::new());
            data.palette_data = Some(PaletteData::Rgb);
            return Ok(load_after_transition);
        }

        let source_context = self.injected_execution_context();
        let staged = Image::Pipeline {
            source: Arc::new(self.clone()),
            ops: PipelineOps::empty(),
            format: self.source_format(),
            backend: self.backend(),
            palette: Some(Vec::new()),
            palette_alpha: Some(Vec::new()),
            palette_data: Some(PaletteData::Rgb),
            compatibility_icc_profile: self.compatibility_icc_profile().map(Arc::from),
            materialized: materialization_cache(),
            shape: super::super::pipeline_shape_cache(),
            mode: super::super::pipeline_mode_cache_for(Some(target_mode)),
        };
        let staged = match route {
            Some((pinned_backend, context)) => {
                staged.with_backend_and_execution_context(pinned_backend, context)
            }
            None => match source_context {
                Some(context) => staged.with_execution_context(context),
                None => staged,
            },
        };
        *self = staged;
        Ok(load_after_transition)
    }

    /// Attaches a palette without changing the image's sample bytes.
    ///
    /// Pillow reinterprets `L` samples as palette indices (`P`) and `LA`
    /// samples as index/alpha pairs (`PA`). Existing `P`/`PA` samples are also
    /// retained byte-for-byte while their palette is replaced. The mutation is
    /// stored in this core value rather than binding-only shadow state. `P`
    /// images therefore enter later CPU, SIMD, or GPU work through the existing
    /// shared indexed representation.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] for an illegal image mode, unsupported
    /// raw palette mode, or more than 256 complete entries. Incomplete trailing
    /// bytes do not contribute to Pillow's raw-layout entry count.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub fn putpalette(&mut self, data: &[u8], rawmode: &str) -> Result<(), PilError> {
        self.putpalette_with_route(
            data,
            PaletteModel::inferred_from_rawmode(rawmode),
            rawmode,
            true,
            None,
        )
    }

    /// Attaches a palette using an explicit color model and raw byte layout.
    ///
    /// This keeps the palette's RGB/RGBA/CMYK identity separate from the
    /// raw-mode strategy that decodes its bytes. The raw decoder enforces
    /// Pillow's 256-entry limit using that layout's bit width.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub fn putpalette_with_model(
        &mut self,
        data: &[u8],
        model: PaletteModel,
        rawmode: &str,
    ) -> Result<(), PilError> {
        self.putpalette_with_route(data, model, rawmode, false, None)
    }

    /// Attaches palette metadata without materializing indexed pixel data.
    ///
    /// Pillow's wrapper commits a dirty palette before the later image load.
    /// This entry point mirrors that boundary so a failed source load retains
    /// the newly attached palette and the already-selected P/PA mode.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub fn attach_palette(&mut self, data: &[u8], rawmode: &str) -> Result<(), PilError> {
        self.attach_palette_with_model(data, PaletteModel::inferred_from_rawmode(rawmode), rawmode)
    }

    /// Attaches palette metadata using a captured backend and architecture.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn attach_palette_with_execution_context(
        &mut self,
        data: &[u8],
        rawmode: &str,
        pinned_backend: Option<crate::compute::Backend>,
        context: crate::compute::ExecutionContext,
    ) -> Result<(), PilError> {
        self.attach_palette_with_model_and_execution_context(
            data,
            PaletteModel::inferred_from_rawmode(rawmode),
            rawmode,
            pinned_backend,
            context,
        )
    }

    /// Attaches a typed palette without materializing indexed pixel data.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub fn attach_palette_with_model(
        &mut self,
        data: &[u8],
        model: PaletteModel,
        rawmode: &str,
    ) -> Result<(), PilError> {
        self.attach_palette_with_route(data, model, rawmode, None)
    }

    /// Attaches palette metadata using the execution route captured by a caller.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn attach_palette_with_model_and_execution_context(
        &mut self,
        data: &[u8],
        model: PaletteModel,
        rawmode: &str,
        pinned_backend: Option<crate::compute::Backend>,
        context: crate::compute::ExecutionContext,
    ) -> Result<(), PilError> {
        self.attach_palette_with_route(data, model, rawmode, Some((pinned_backend, context)))
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn attach_palette_with_route(
        &mut self,
        data: &[u8],
        model: PaletteModel,
        rawmode: &str,
        route: Option<PaletteExecutionRoute>,
    ) -> Result<(), PilError> {
        let source_mode = self.mode_identity()?;
        let context = route.or_else(|| {
            self.injected_execution_context()
                .map(|context| (self.backend(), context))
        });
        match source_mode {
            Mode::ImageLuma8 | Mode::ImageLumaA8 => {
                self.stage_palette_attachment_with_route(context)?;
            }
            Mode::ImagePalette8 | Mode::ImagePaletteAlpha8 => {
                self.apply_palette_execution_route(context);
            }
            _ => return Err(PilError::ValueError("illegal image mode".to_owned())),
        }
        self.replace_indexed_palette_with_model(data, model, rawmode)?;
        self.apply_palette_execution_route(context);
        Ok(())
    }

    /// Attaches a palette while materializing through the caller's pipeline route.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn putpalette_with_execution_context(
        &mut self,
        data: &[u8],
        rawmode: &str,
        pinned_backend: Option<crate::compute::Backend>,
        context: crate::compute::ExecutionContext,
    ) -> Result<(), PilError> {
        self.putpalette_with_route(
            data,
            PaletteModel::inferred_from_rawmode(rawmode),
            rawmode,
            true,
            Some((pinned_backend, context)),
        )
    }

    /// Attaches a typed palette through the caller's pipeline route.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn putpalette_with_model_and_execution_context(
        &mut self,
        data: &[u8],
        model: PaletteModel,
        rawmode: &str,
        pinned_backend: Option<crate::compute::Backend>,
        context: crate::compute::ExecutionContext,
    ) -> Result<(), PilError> {
        self.putpalette_with_route(data, model, rawmode, false, Some((pinned_backend, context)))
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn putpalette_with_route(
        &mut self,
        data: &[u8],
        palette_model: PaletteModel,
        rawmode: &str,
        keep_indexed_pipeline_lazy: bool,
        route: Option<PaletteExecutionRoute>,
    ) -> Result<(), PilError> {
        let source_mode = self.mode_identity()?;
        if !matches!(
            source_mode,
            Mode::ImageLuma8 | Mode::ImageLumaA8 | Mode::ImagePalette8 | Mode::ImagePaletteAlpha8
        ) {
            return Err(PilError::ValueError("illegal image mode".to_owned()));
        }
        let mode = match source_mode {
            Mode::ImageLuma8 => Mode::ImagePalette8,
            Mode::ImageLumaA8 => Mode::ImagePaletteAlpha8,
            mode => mode,
        };
        if source_mode != mode {
            self.stage_palette_attachment_with_route(route)?;
        }

        if matches!(source_mode, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8)
            && matches!(mode, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8)
            && keep_indexed_pipeline_lazy
            && rawmode == palette_model.pillow_name()
        {
            self.apply_palette_execution_route(route);
            return self.replace_indexed_palette_with_model(data, palette_model, rawmode);
        }

        let source_format = self.source_format();
        let info = self.image_info();
        let exif = self.exif_metadata();
        let materialized = match route {
            Some((pinned_backend, context)) => self
                .clone()
                .with_backend_and_execution_context(pinned_backend, context)
                .materialize()?,
            None => self.materialize()?,
        };
        let (palette, palette_alpha, palette_data) =
            split_palette_data(data, palette_model, rawmode)?;

        *self = match mode {
            Mode::ImageLuma8 | Mode::ImagePalette8 => Image::Paletted(PalettedData {
                indices: DynamicImage::ImagePalette8(materialized.to_luma8()),
                palette_attached: true,
                palette,
                palette_data,
                palette_alpha,
                source_format,
                info,
                exif,
                materialized: materialization_cache(),
            }),
            Mode::ImageLumaA8 | Mode::ImagePaletteAlpha8 => {
                let image = DynamicImage::ImagePaletteAlpha8(materialized.to_luma_alpha8());
                Image::Loaded(LoadedData {
                    image: Arc::new(image),
                    known_zero_fill: false,
                    palette: Some(palette),
                    palette_data: Some(palette_data),
                    palette_alpha: Some(palette_alpha),
                    source_format,
                    info,
                    exif,
                })
            }
            _ => unreachable!("putpalette mode was validated"),
        };
        Ok(())
    }

    /// Applies Python ImagePalette transparency to an attached palette.
    ///
    /// This mirrors Pillow's `_putpalettealpha`: alpha storage has 256 slots,
    /// defaults to opaque, and an index may be set beyond the currently
    /// retained color entries. The Python wrapper keeps its public
    /// `info["transparency"]` value; this method clears only the codec-side
    /// pending marker after committing the alpha table.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub fn putpalettealpha(&mut self, index: i32, alpha: i32) -> Result<(), PilError> {
        self.putpalettealpha_with_route(index, alpha, None)
    }

    /// Applies one palette alpha write after validation using the caller's route.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn putpalettealpha_with_execution_context(
        &mut self,
        index: i32,
        alpha: i32,
        pinned_backend: Option<crate::compute::Backend>,
        context: crate::compute::ExecutionContext,
    ) -> Result<(), PilError> {
        self.putpalettealpha_with_route(index, alpha, Some((pinned_backend, context)))
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn putpalettealpha_with_route(
        &mut self,
        index: i32,
        alpha: i32,
        route: Option<PaletteExecutionRoute>,
    ) -> Result<(), PilError> {
        if !self.has_attached_palette() {
            return Err(PilError::ValueError("image has no palette".to_owned()));
        }
        if !(0..256).contains(&index) {
            return Err(PilError::ValueError(
                "palette index out of range".to_owned(),
            ));
        }

        self.apply_palette_execution_route(route);
        self.load()?;
        self.transition_attached_palette_to_rgba()?;
        let mut palette_alpha = self.palette_alpha().unwrap_or_default();
        palette_alpha.resize(256, 255);
        palette_alpha[index as usize] = alpha as u8;
        self.store_committed_palette_alpha(palette_alpha)
    }

    /// Applies a prefix of Python ImagePalette transparency bytes.
    ///
    /// Existing raw LA palette alpha is preserved after the supplied prefix;
    /// missing entries remain opaque. The Python wrapper keeps its public
    /// `info["transparency"]` value and clears the codec-side pending marker.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub fn putpalettealphas(&mut self, values: &[u8]) -> Result<(), PilError> {
        self.putpalettealphas_with_route(values, None)
    }

    /// Applies a palette alpha prefix after validation using the caller's route.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn putpalettealphas_with_execution_context(
        &mut self,
        values: &[u8],
        pinned_backend: Option<crate::compute::Backend>,
        context: crate::compute::ExecutionContext,
    ) -> Result<(), PilError> {
        self.putpalettealphas_with_route(values, Some((pinned_backend, context)))
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn putpalettealphas_with_route(
        &mut self,
        values: &[u8],
        route: Option<PaletteExecutionRoute>,
    ) -> Result<(), PilError> {
        if !self.has_attached_palette() {
            return Err(PilError::ValueError("image has no palette".to_owned()));
        }
        if values.len() > 256 {
            return Err(PilError::ValueError(
                "palette index out of range".to_owned(),
            ));
        }

        self.apply_palette_execution_route(route);
        self.load()?;
        self.transition_attached_palette_to_rgba()?;
        let mut palette_alpha = self.palette_alpha().unwrap_or_default();
        palette_alpha.resize(256, 255);
        palette_alpha[..values.len()].copy_from_slice(values);
        self.store_committed_palette_alpha(palette_alpha)
    }

    /// Commits alpha already supplied by an LA raw palette.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub fn commit_palette_alpha(&mut self) -> Result<(), PilError> {
        self.commit_palette_alpha_with_route(None)
    }

    /// Commits raw palette alpha after validation using the caller's route.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn commit_palette_alpha_with_execution_context(
        &mut self,
        pinned_backend: Option<crate::compute::Backend>,
        context: crate::compute::ExecutionContext,
    ) -> Result<(), PilError> {
        self.commit_palette_alpha_with_route(Some((pinned_backend, context)))
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn commit_palette_alpha_with_route(
        &mut self,
        route: Option<PaletteExecutionRoute>,
    ) -> Result<(), PilError> {
        if !self.has_attached_palette() {
            return Err(PilError::ValueError("image has no palette".to_owned()));
        }
        self.apply_palette_execution_route(route);
        self.load()?;
        self.transition_attached_palette_to_rgba()?;
        let mut palette_alpha = self.palette_alpha().unwrap_or_default();
        palette_alpha.resize(256, 255);
        self.store_committed_palette_alpha(palette_alpha)
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn apply_palette_execution_route(&mut self, route: Option<PaletteExecutionRoute>) {
        if let Some((pinned_backend, context)) = route {
            *self = self
                .clone()
                .with_backend_and_execution_context(pinned_backend, context);
        }
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn transition_attached_palette_to_rgba(&mut self) -> Result<(), PilError> {
        match self {
            Image::Paletted(data) => {
                data.palette_data
                    .transition_to_rgba(&mut data.palette, &mut data.palette_alpha);
            }
            Image::Loaded(data) => {
                let Some(palette) = data.palette.as_mut() else {
                    return Err(PilError::ValueError("image has no palette".to_owned()));
                };
                data.palette_data
                    .get_or_insert(PaletteData::Rgb)
                    .transition_to_rgba(palette, data.palette_alpha.get_or_insert_with(Vec::new));
            }
            _ => return Err(PilError::ValueError("image has no palette".to_owned())),
        }
        Ok(())
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn store_committed_palette_alpha(&mut self, palette_alpha: Vec<u8>) -> Result<(), PilError> {
        match self {
            Image::Paletted(data) => {
                data.palette_data
                    .transition_to_rgba(&mut data.palette, &mut data.palette_alpha);
                data.palette_alpha = palette_alpha;
                clear_pending_palette_alpha(&mut data.info);
            }
            Image::Loaded(data) if data.palette.is_some() => {
                if let Some(palette) = data.palette.as_mut() {
                    data.palette_data
                        .get_or_insert(PaletteData::Rgb)
                        .transition_to_rgba(
                            palette,
                            data.palette_alpha.get_or_insert_with(Vec::new),
                        );
                }
                data.palette_alpha = Some(palette_alpha);
                clear_pending_palette_alpha(&mut data.info);
            }
            _ => return Err(PilError::ValueError("image has no palette".to_owned())),
        }
        Ok(())
    }

    /// Replaces palette metadata on an indexed image without decoding or
    /// materializing its pixel pipeline.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub(crate) fn replace_indexed_palette(
        &mut self,
        data: &[u8],
        rawmode: &str,
    ) -> Result<(), PilError> {
        self.replace_indexed_palette_with_model(
            data,
            PaletteModel::inferred_from_rawmode(rawmode),
            rawmode,
        )
    }

    /// Copies palette metadata from a converted indexed image to this image.
    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
    pub(crate) fn replace_palette_state_from(&mut self, source: &Image) {
        let attached = source.has_attached_palette();
        let palette = source.extract_palette();
        let alpha = source.palette_alpha().unwrap_or_default();
        let palette_data = source
            .palette_data()
            .cloned()
            .unwrap_or_else(|| PaletteData::from_alpha(&alpha));

        match self {
            Image::Paletted(destination) => {
                destination.palette = palette.unwrap_or_default();
                destination.palette_data = palette_data;
                destination.palette_alpha = alpha;
                destination.palette_attached = attached;
            }
            Image::Loaded(destination) => {
                destination.palette = attached.then_some(palette.unwrap_or_default());
                destination.palette_data = attached.then_some(palette_data);
                destination.palette_alpha = attached.then_some(alpha);
            }
            Image::Pipeline {
                palette: destination_palette,
                palette_alpha: destination_alpha,
                palette_data: destination_data,
                ..
            } => {
                *destination_palette = if attached { palette } else { None };
                *destination_alpha = attached.then_some(alpha);
                *destination_data = attached.then_some(palette_data);
            }
            Image::Bytes { .. } => {}
        }
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn replace_indexed_palette_with_model(
        &mut self,
        data: &[u8],
        palette_model: PaletteModel,
        rawmode: &str,
    ) -> Result<(), PilError> {
        let mode = self.mode_identity()?;
        if !matches!(mode, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8) {
            return Err(PilError::ValueError("illegal image mode".to_owned()));
        }
        let (palette, palette_alpha, palette_data) =
            split_palette_data(data, palette_model, rawmode)?;

        match self {
            Image::Paletted(image) => {
                image.palette = palette;
                image.palette_attached = true;
                image.palette_alpha = palette_alpha;
                image.palette_data = palette_data;
            }
            Image::Loaded(image) => {
                image.palette = Some(palette);
                image.palette_data = Some(palette_data);
                image.palette_alpha = Some(palette_alpha);
            }
            Image::Pipeline {
                ops,
                palette: current_palette,
                palette_alpha: current_alpha,
                palette_data: current_data,
                ..
            } if ops.is_empty() => {
                *current_palette = Some(palette);
                *current_alpha = Some(palette_alpha);
                *current_data = Some(palette_data);
            }
            _ => {
                // Keep earlier operations attached to their original palette;
                // this empty metadata layer applies the updated palette only
                // to operations queued after the color was resolved.
                let source = Arc::new(self.clone());
                *self = Image::Pipeline {
                    format: self.source_format(),
                    backend: self.backend(),
                    source,
                    ops: super::super::PipelineOps::empty(),
                    palette: Some(palette),
                    palette_alpha: Some(palette_alpha),
                    palette_data: Some(palette_data),
                    compatibility_icc_profile: self.compatibility_icc_profile().map(Arc::from),
                    materialized: materialization_cache(),
                    shape: super::super::pipeline_shape_cache(),
                    mode: super::super::pipeline_mode_cache_for(Some(mode)),
                };
            }
        }
        Ok(())
    }

    /// Installs the representable portion of a Python-owned Draw palette.
    ///
    /// `ImagePalette.getcolor` retains arbitrary byte lengths until
    /// `Image.load`; drawing only needs the first 256 complete entries to
    /// resolve indexed pixels. The wrapper keeps the original bytes and
    /// performs strict realization at load time.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub(crate) fn replace_indexed_palette_from_draw(
        &mut self,
        data: &[u8],
        rawmode: &str,
    ) -> Result<(), PilError> {
        let entry_size = match rawmode {
            "RGB" => 3,
            "RGBA" => 4,
            "LA" => 2,
            "CMYK" => 4,
            _ => return Err(PilError::ValueError("unrecognized raw mode".to_owned())),
        };
        let complete_size = (data.len() / entry_size).min(256) * entry_size;
        self.replace_indexed_palette(&data[..complete_size], rawmode)
    }
}
