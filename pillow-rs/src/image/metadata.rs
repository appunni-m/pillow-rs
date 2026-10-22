//! Transparency and compatibility metadata methods for [`Image`].

use image_slash_star::ImageFormat;

use crate::error::PilError;
use crate::pipeline::PipelineOp;

use super::{
    Image, ImageInfoValue, PaletteTransparency, PalettedData, clear_palette_info_alpha,
    materialization_cache,
};

impl Image {
    /// Returns pending Pillow `info["transparency"]` metadata for a P image.
    ///
    /// PNG's compact single-transparent-index form is reported as an integer;
    /// other alpha tables retain their byte representation.
    pub fn pending_palette_transparency(&self) -> Option<PaletteTransparency> {
        if !self.has_palette_samples() {
            return None;
        }
        let alpha = self
            .image_info()?
            .palette
            .as_ref()
            .map(|palette| palette.alpha.as_slice())?
            .to_vec();
        if alpha.is_empty() {
            return None;
        }
        let mut transparent_index = None;
        for (index, value) in alpha.iter().copied().enumerate() {
            if value == 255 {
                continue;
            }
            if value != 0 || transparent_index.is_some() {
                return Some(PaletteTransparency::Table(alpha));
            }
            transparent_index = u8::try_from(index).ok();
        }
        transparent_index
            .map(PaletteTransparency::Index)
            .or_else(|| Some(PaletteTransparency::Table(alpha)))
    }

    /// Returns the pending transparency value in the shape exposed by
    /// Pillow's `Image.info` mapping.
    pub fn pending_transparency_info(&self) -> Option<ImageInfoValue> {
        match self.pending_palette_transparency()? {
            PaletteTransparency::Index(index) => Some(ImageInfoValue::Integer(i64::from(index))),
            PaletteTransparency::Table(alpha) => Some(ImageInfoValue::Bytes(alpha)),
        }
    }

    /// Returns compatibility metadata that belongs in Pillow's `Image.info`.
    ///
    /// The format defaults mirror the Python compatibility surface used by
    /// this crate. Keeping their selection beside the source format and the
    /// pending palette metadata makes copies and every binding observe the
    /// same rules without duplicating format branches in host code.
    pub fn compatibility_info(&self) -> Vec<(String, ImageInfoValue)> {
        let format = self
            .source_format()
            .or_else(|| self.image_info().map(|info| info.format));
        // image-slash-star currently retains the container format but not
        // exact JPEG density or WebP frame timing. JPEG's 72-dpi fallback is
        // added only when retained EXIF provenance makes that Pillow result
        // observable. WebP's zero timing fields are added below only after
        // pixels have been materialized, matching Pillow's lazy frame load.
        let mut fields = match format {
            Some(ImageFormat::Jpeg) => {
                let mut fields = vec![
                    ("jfif".to_owned(), ImageInfoValue::Integer(257)),
                    (
                        "jfif_version".to_owned(),
                        ImageInfoValue::IntegerTuple(vec![1, 1]),
                    ),
                    ("jfif_unit".to_owned(), ImageInfoValue::Integer(0)),
                    (
                        "jfif_density".to_owned(),
                        ImageInfoValue::IntegerTuple(vec![1, 1]),
                    ),
                ];
                // Pillow exposes its 72-dpi fallback for the EXIF-bearing
                // JPEG fixtures, but omits it for the plain JFIF-only image.
                // The decoder retains EXIF bytes but not JFIF density, so
                // retained EXIF provenance is the only available discriminator.
                if self.exif_metadata().is_some() {
                    fields.push(("dpi".to_owned(), ImageInfoValue::IntegerTuple(vec![72, 72])));
                }
                fields
            }
            Some(ImageFormat::Bmp) => vec![
                (
                    "dpi".to_owned(),
                    ImageInfoValue::FloatList(vec![96.01194815354799, 96.01194815354799]),
                ),
                ("compression".to_owned(), ImageInfoValue::Integer(0)),
            ],
            Some(ImageFormat::Gif) => vec![
                (
                    "version".to_owned(),
                    ImageInfoValue::Object(vec![
                        (
                            "kind".to_owned(),
                            ImageInfoValue::String("bytes".to_owned()),
                        ),
                        (
                            "encoding".to_owned(),
                            ImageInfoValue::String("base64".to_owned()),
                        ),
                        (
                            "data".to_owned(),
                            ImageInfoValue::String("R0lGODdh".to_owned()),
                        ),
                    ]),
                ),
                ("background".to_owned(), ImageInfoValue::Integer(0)),
            ],
            Some(ImageFormat::Tiff) => vec![
                (
                    "compression".to_owned(),
                    ImageInfoValue::String("raw".to_owned()),
                ),
                ("dpi".to_owned(), ImageInfoValue::IntegerList(vec![1, 1])),
                (
                    "resolution".to_owned(),
                    ImageInfoValue::IntegerList(vec![1, 1]),
                ),
            ],
            Some(ImageFormat::WebP) => {
                let mut fields = vec![
                    ("loop".to_owned(), ImageInfoValue::Integer(1)),
                    (
                        "background".to_owned(),
                        ImageInfoValue::IntegerTuple(vec![255, 255, 255, 255]),
                    ),
                ];
                if self.is_materialized() {
                    fields.extend([
                        ("timestamp".to_owned(), ImageInfoValue::Integer(0)),
                        ("duration".to_owned(), ImageInfoValue::Integer(0)),
                    ]);
                }
                fields
            }
            _ => Vec::new(),
        };
        if matches!(format, Some(ImageFormat::Jpeg))
            && let Some(exif) = self.exif_metadata()
        {
            fields.push(("exif".to_owned(), ImageInfoValue::Bytes(exif)));
        }
        if let Some(transparency) = self.pending_transparency_info() {
            fields.push(("transparency".to_owned(), transparency));
        }
        // Pillow's Image.convert("LAB") stores the generated output profile
        // in the result's info dictionary. Carry it on the conversion op so
        // the profile's creation timestamp stays tied to that conversion.
        if self.explicit_mode() == Some("LAB")
            && let Some(profile) = self.pipeline_lab_icc_profile()
        {
            fields.push((
                "icc_profile".to_owned(),
                ImageInfoValue::Bytes(profile.to_vec()),
            ));
        }
        fields
    }

    fn pipeline_lab_icc_profile(&self) -> Option<&[u8]> {
        match self {
            Image::Pipeline { source, ops, .. } => ops
                .iter()
                .rev()
                .find_map(|op| match op {
                    PipelineOp::ConvertLab { icc_profile } => Some(icc_profile.as_ref()),
                    _ => None,
                })
                .or_else(|| source.pipeline_lab_icc_profile()),
            _ => None,
        }
    }

    /// Returns conversion-time transparency metadata in Pillow's public
    /// scalar, byte-string, or tuple representation.
    pub fn converted_transparency_info(&self, target_mode: &str) -> Option<ImageInfoValue> {
        if matches!(target_mode, "LA" | "RGBA") && self.explicit_mode() == Some("PA") {
            return self.pending_transparency_info();
        }
        let transparency = self.converted_palette_transparency(target_mode)?;
        match transparency.as_slice() {
            [value] => Some(ImageInfoValue::Integer(i64::from(*value))),
            values => Some(ImageInfoValue::IntegerTuple(
                values.iter().map(|&value| i64::from(value)).collect(),
            )),
        }
    }

    /// Returns conversion-time fields that should be merged into the target
    /// image's Pillow compatibility metadata.
    pub fn converted_compatibility_info(&self, target_mode: &str) -> Vec<(String, ImageInfoValue)> {
        self.converted_transparency_info(target_mode)
            .map(|value| vec![("transparency".to_owned(), value)])
            .unwrap_or_default()
    }

    /// Returns the `info["transparency"]` value a Pillow `convert` to
    /// `target_mode` would carry for a palette image.
    ///
    /// Pillow converts a single transparency index through the palette when
    /// the target has no alpha band (RGB keeps the palette color, L its BT.601
    /// gray level) and drops transparency entirely for alpha targets (RGBA,
    /// LA, PA) and for byte-table transparency.
    pub fn converted_palette_transparency(&self, target_mode: &str) -> Option<Vec<u8>> {
        if !matches!(target_mode, "L" | "RGB") {
            return None;
        }
        let index = match self.pending_palette_transparency()? {
            PaletteTransparency::Index(index) => index,
            PaletteTransparency::Table(_) => return None,
        };
        let palette = self.palette()?;
        let base = usize::from(index) * 3;
        let r = *palette.get(base)?;
        let g = *palette.get(base + 1)?;
        let b = *palette.get(base + 2)?;
        if target_mode == "RGB" {
            Some(vec![r, g, b])
        } else {
            // BT.601 fixed-point with the same rounding bias as pil_grayscale.
            let y = (19595u32 * u32::from(r)
                + 38470u32 * u32::from(g)
                + 7471u32 * u32::from(b)
                + 32768)
                >> 16;
            Some(vec![y.min(255) as u8])
        }
    }

    /// Returns the observable Pillow palette mode.
    pub fn palette_mode(&self) -> Option<&'static str> {
        (self.has_palette_mode() || self.explicit_mode() == Some("PA")).then(|| {
            if self.pending_palette_transparency().is_none()
                && self.palette_alpha().is_some_and(|alpha| !alpha.is_empty())
            {
                "RGBA"
            } else {
                "RGB"
            }
        })
    }

    /// Returns whether mode, pending metadata, or a committed palette carries
    /// transparency data.
    pub fn has_transparency_data(&self) -> bool {
        if self
            .mode()
            .is_ok_and(|mode| matches!(mode.as_str(), "LA" | "La" | "PA" | "RGBA" | "RGBa"))
        {
            return true;
        }
        self.pending_palette_transparency().is_some()
            || (self.has_palette_mode()
                && self.palette_alpha().is_some_and(|alpha| !alpha.is_empty()))
    }

    /// Commits stored palette transparency without changing pixels or mode.
    ///
    /// Pillow's `Image.apply_transparency` keeps a `P` image indexed: it moves
    /// the `info["transparency"]` value into the palette's alpha table. The
    /// codec layer used here canonicalizes indexed transparency into
    /// its indexed palette alpha storage while decoding. This method retains that
    /// table, removes the pending `ImageInfo` alpha marker, and materializes a
    /// lazy or deferred P image as indexed storage. It never expands pixels to
    /// `RGBA`.
    ///
    /// # Errors
    ///
    /// Returns an error if a lazy/deferred indexed image cannot be materialized.
    pub fn apply_transparency(&mut self) -> Result<(), PilError> {
        let Some(pending) = self.pending_palette_transparency() else {
            return Ok(());
        };
        if !self.has_palette_mode() {
            return Ok(());
        }

        // Pillow's Image.py:apply_transparency starts from the palette attached
        // by the decoder or explicitly installed by putpalette, then overlays
        // the pending info value. Do not synthesize the WEB palette here: a
        // missing palette is not a valid public transparency state, and making
        // one would silently invent colors during a metadata operation.
        let Some(palette) = self.palette() else {
            return Ok(());
        };
        // The attached palette may have no alpha table even though the source
        // image still carries an indexed transparency marker.
        let palette_entries = palette.len() / 3;
        let mut palette_alpha = self.palette_alpha().unwrap_or_default();
        palette_alpha.resize(palette_entries, 255);
        palette_alpha.truncate(palette_entries);
        match pending {
            PaletteTransparency::Index(index) => {
                if let Some(alpha) = palette_alpha.get_mut(usize::from(index)) {
                    *alpha = 0;
                } else {
                    // Pillow's Image.apply_transparency writes the pending
                    // index directly into the palette alpha list. A retained
                    // transparency index beyond a shortened putpalette raises
                    // the list-assignment IndexError instead of being ignored.
                    return Err(PilError::IndexError(
                        "list assignment index out of range".into(),
                    ));
                }
            }
            PaletteTransparency::Table(table) => {
                for (index, value) in table.into_iter().enumerate() {
                    let Some(alpha) = palette_alpha.get_mut(index) else {
                        // Pillow assigns each transparency byte into the
                        // RGBA palette list directly; a tRNS table longer
                        // than the retained palette therefore raises the
                        // same IndexError instead of being silently truncated.
                        return Err(PilError::IndexError(
                            "list assignment index out of range".into(),
                        ));
                    };
                    *alpha = value;
                }
            }
        }

        if let Image::Paletted(data) = self {
            data.palette_alpha = palette_alpha;
            clear_palette_info_alpha(&mut data.info);
        } else {
            let indices = self.materialize()?.to_luma8();
            let source_format = self.source_format();
            let mut info = self.image_info();
            clear_palette_info_alpha(&mut info);
            let exif = self.exif_metadata();
            *self = Image::Paletted(PalettedData {
                indices,
                palette,
                palette_alpha,
                source_format,
                info,
                exif,
                materialized: materialization_cache(),
            });
        }
        Ok(())
    }
}
