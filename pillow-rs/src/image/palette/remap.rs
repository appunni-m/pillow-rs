//! Palette expansion and index remapping for [`Image`].

use crate::compute::registry::support::OperationId;
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::Mode;

use super::super::Image;
use super::{PaletteModel, operational_palette, split_palette_data};

impl Image {
    /// Validates the exact image mode before a host adapter evaluates
    /// callback-capable palette or destination-map inputs.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn validate_remap_palette_mode(&self) -> Result<(), PilError> {
        let mode = self.mode_identity()?;
        OperationId::RemapPalette
            .support()
            .for_mode(mode)
            .validate_remap_palette_destination_map(0)
    }

    /// Captures the palette bytes and color model used by a RemapPalette call
    /// before a host binding begins iterating a callback-capable destination map.
    // VERIFY: source=reviewed@2026-11-26; parity=pass@migration-parity-31c07518eeb643f49dc41529b17d807f; benchmark=pending
    #[doc(hidden)]
    pub fn remap_palette_source_snapshot(&self) -> Result<(PaletteModel, Vec<u8>), PilError> {
        let mode = self.mode_identity()?;
        self.validate_remap_palette_mode()?;
        let (model, _, bytes) = remap_palette_source(self, mode, None, None)?;
        Ok((model, bytes))
    }

    /// Returns the mode used when indexed samples are expanded for a
    /// palette-dependent operation, without decoding image pixels.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub(crate) fn palette_expansion_mode(&self, source_mode: Mode) -> Option<Mode> {
        match source_mode {
            Mode::ImagePaletteAlpha8 => Some(Mode::ImageRgba8),
            Mode::ImagePalette8 if self.has_attached_palette() => Some(
                if self.palette_alpha().is_some_and(|alpha| !alpha.is_empty()) {
                    Mode::ImageRgba8
                } else {
                    Mode::ImageRgb8
                },
            ),
            _ => None,
        }
    }

    /// Extract palette from Paletted variant (for Pipeline propagation).
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub(crate) fn extract_palette(&self) -> Option<Vec<u8>> {
        match self {
            Image::Paletted(data) if data.palette_attached => Some(operational_palette(data)),
            Image::Paletted(_) => None,
            Image::Loaded(data) => data.palette.clone(),
            Image::Pipeline { palette, ops, .. } => palette
                .clone()
                .or_else(|| ops.palette_output()?.palette.clone()),
            Image::Bytes { info, .. } => info
                .as_ref()
                .and_then(|info| info.palette.as_ref())
                .map(|palette| palette.rgb.clone()),
        }
    }

    /// Remaps palette indices through `dest_map`.
    ///
    /// `P` images operate on palette indices directly. Other modes operate on
    /// RGB color values and return a `P`-tagged pipeline result. Values absent
    /// from `dest_map` map to `0`, matching Pillow's inverse-map behavior.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when the source is not `L` or `P`,
    /// or when `dest_map` contains more than 256 entries. A `P` image without
    /// an attached palette uses an empty RGB palette, matching Pillow. Errors
    /// resolving the source mode are propagated.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub fn remap_palette(&self, dest_map: &[u8]) -> Result<Image, PilError> {
        self.remap_palette_with_source(dest_map, None)
    }

    /// Remaps indices and installs colors selected from an optional source palette.
    ///
    /// Pillow interprets `dest_map[new_index]` as the old index. Pixel samples
    /// therefore use the inverse mapping, while source palette bytes are copied
    /// in the selected palette model's band layout.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub fn remap_palette_with_source(
        &self,
        dest_map: &[u8],
        source_palette: Option<&[u8]>,
    ) -> Result<Image, PilError> {
        self.remap_palette_with_source_model(dest_map, source_palette, None)
    }

    /// Remaps indices using a source palette whose model was resolved before
    /// host callbacks began iterating the destination map.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn remap_palette_with_source_model(
        &self,
        dest_map: &[u8],
        source_palette: Option<&[u8]>,
        source_palette_model: Option<PaletteModel>,
    ) -> Result<Image, PilError> {
        let mode = self.mode_identity()?;
        OperationId::RemapPalette
            .support()
            .for_mode(mode)
            .validate_remap_palette_destination_map(dest_map.len())?;

        let (palette_model, source_bands, source_bytes) =
            remap_palette_source(self, mode, source_palette, source_palette_model)?;
        let remapped_bytes = select_palette_entries(&source_bytes, dest_map, source_bands);
        self.remap_palette_with_selected_entries_for_mode(dest_map, &remapped_bytes, palette_model)
    }

    /// Remaps indices after a host adapter has resolved callback-capable
    /// palette slices in Pillow's per-entry order.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn remap_palette_with_selected_entries(
        &self,
        dest_map: &[u8],
        selected_palette: &[u8],
        palette_model: PaletteModel,
    ) -> Result<Image, PilError> {
        let mode = self.mode_identity()?;
        OperationId::RemapPalette
            .support()
            .for_mode(mode)
            .validate_remap_palette_destination_map(dest_map.len())?;
        self.remap_palette_with_selected_entries_for_mode(dest_map, selected_palette, palette_model)
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn remap_palette_with_selected_entries_for_mode(
        &self,
        dest_map: &[u8],
        selected_palette: &[u8],
        palette_model: PaletteModel,
    ) -> Result<Image, PilError> {
        let (remapped_palette, remapped_alpha, remapped_palette_data) =
            split_palette_data(selected_palette, palette_model, palette_model.pillow_name())?;

        let mut result = Image::push_op(
            self,
            PipelineOp::RemapPalette {
                dest_map: dest_map.to_vec().into(),
            },
        );
        if let Image::Pipeline {
            palette,
            palette_alpha,
            palette_data,
            ..
        } = &mut result
        {
            *palette = Some(remapped_palette);
            *palette_alpha = Some(remapped_alpha);
            *palette_data = Some(remapped_palette_data);
        }
        Ok(result)
    }
}

// VERIFY: source=reviewed@2026-11-26; parity=pass@migration-parity-31c07518eeb643f49dc41529b17d807f; benchmark=pending
fn remap_palette_source(
    image: &Image,
    mode: Mode,
    explicit_palette: Option<&[u8]>,
    explicit_palette_model: Option<PaletteModel>,
) -> Result<(PaletteModel, usize, Vec<u8>), PilError> {
    if let Some(bytes) = explicit_palette {
        // Pillow chooses the explicit palette model from total byte length.
        let model = explicit_palette_model.unwrap_or_else(|| {
            if bytes.len() > 768 {
                PaletteModel::Rgba
            } else {
                PaletteModel::Rgb
            }
        });
        return Ok((model, model.remap_palette_bands(), bytes.to_vec()));
    }

    if mode == Mode::ImageLuma8 {
        let grayscale = (0u8..=u8::MAX)
            .flat_map(|value| [value, value, value])
            .collect();
        return Ok((
            PaletteModel::Rgb,
            PaletteModel::Rgb.remap_palette_bands(),
            grayscale,
        ));
    }

    if !image.has_attached_palette() {
        // Pillow's ImagingCore exposes a missing P palette as empty RGB bytes;
        // remapping still completes and leaves the result without palette data.
        return Ok((
            PaletteModel::Rgb,
            PaletteModel::Rgb.remap_palette_bands(),
            Vec::new(),
        ));
    }
    let model = image.palette_model().unwrap_or(PaletteModel::Rgb);
    let bytes = image
        .getpalette_rawmode(model.pillow_name())?
        .unwrap_or_default();
    Ok((model, model.remap_palette_bands(), bytes))
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn select_palette_entries(source: &[u8], dest_map: &[u8], bands: usize) -> Vec<u8> {
    let mut selected = Vec::with_capacity(dest_map.len() * bands);
    for &old_index in dest_map {
        let start = usize::from(old_index) * bands;
        if let Some(entry) = source.get(start..start + bands) {
            selected.extend_from_slice(entry);
        }
    }
    selected
}
