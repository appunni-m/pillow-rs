//! Image mode, size, transparency, and compatibility metadata methods.

use image_slash_star::ImageFormat;

use crate::color::color_type_to_mode;
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, GenericImageView};

use super::{
    Image, ImageInfoValue, PaletteTransparency, PalettedData, clear_palette_info_alpha,
    image_mode_name, known_pipeline_op_dimensions, known_pipeline_op_mode, materialization_cache,
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

    /// Returns child images for multi-frame formats.
    ///
    /// Multi-frame decoding is not implemented in core yet, so this currently
    /// returns an empty vector for all inputs.
    pub fn get_child_images(&self) -> Vec<Image> {
        vec![]
    }

    /// Returns EXIF metadata bytes retained from the encoded source.
    ///
    /// JPEG sources carry the EXIF payload in their APP1 segment; TIFF
    /// sources are themselves a TIFF IFD0 container.  Other formats currently
    /// expose no EXIF bytes, matching Pillow's empty `Exif` object for images
    /// without embedded metadata.
    pub fn getexif(&self) -> Vec<u8> {
        match self {
            Image::Loaded(data) => data.exif.clone().unwrap_or_default(),
            Image::Paletted(data) => data.exif.clone().unwrap_or_default(),
            Image::Pipeline { source, .. } => source.getexif(),
            Image::Bytes { source, .. } => match source.info().format {
                ImageFormat::Jpeg => Self::extract_jpeg_exif(source.bytes()).unwrap_or_default(),
                ImageFormat::Tiff => {
                    // Pillow builds its `Exif` object from the TIFF IFD0
                    // itself; the raw container bytes are the same IFD
                    // payload the orientation parser consumes.
                    source.bytes().to_vec()
                }
                _ => Vec::new(),
            },
        }
    }

    /// Returns retained EXIF bytes in the optional form used by materialized
    /// and derived image storage.
    pub(crate) fn exif_metadata(&self) -> Option<Vec<u8>> {
        let exif = self.getexif();
        (!exif.is_empty()).then_some(exif)
    }

    /// Extracts the Exif APP1 payload from a JPEG byte stream.
    ///
    /// Scans the sequential marker list for `0xFFE1` and returns the segment
    /// payload when it starts with the Exif signature.  This mirrors Pillow's
    /// `JpegImagePlugin` APP1 handling, which retains the payload starting at
    /// the `Exif\0\0` signature.
    fn extract_jpeg_exif(data: &[u8]) -> Option<Vec<u8>> {
        if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 {
            return None;
        }
        let mut offset = 2usize;
        while offset + 4 <= data.len() {
            if data[offset] != 0xFF {
                // Pillow's JpegImagePlugin._open skips non-marker header junk
                // before reading the next marker. Keep scanning so an APP1
                // Exif segment after that junk remains observable through
                // Image.getexif().
                offset += 1;
                continue;
            }
            let marker = data[offset + 1];
            // Standalone markers (RST0-7, TEM) and the SOI marker carry no
            // length field.
            if marker == 0xD8 || (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
                offset += 2;
                continue;
            }
            if marker == 0xD9 {
                return None;
            }
            // Pillow's JpegImagePlugin stops collecting header metadata at
            // SOS and hands the remainder to the entropy decoder. Do not
            // interpret entropy bytes (including later marker-like bytes) as
            // additional APP segments.
            if marker == 0xDA {
                return None;
            }
            let length = usize::from(u16::from_be_bytes([data[offset + 2], data[offset + 3]]));
            if length < 2 {
                return None;
            }
            let segment_end = offset + 2 + length;
            if segment_end > data.len() {
                return None;
            }
            if marker == 0xE1 {
                let payload = &data[offset + 4..segment_end];
                if payload.len() > 6 && payload.starts_with(b"Exif\x00\x00") {
                    return Some(payload.to_vec());
                }
                return None;
            }
            offset = segment_end;
        }
        None
    }

    /// Returns XMP metadata fields.
    ///
    /// Core does not currently parse XMP packets from encoded containers, so
    /// this returns an empty map.
    pub fn getxmp(&self) -> std::collections::HashMap<String, String> {
        std::collections::HashMap::new()
    }

    /// Returns Pillow's internal-image handle equivalent.
    ///
    /// Pure Rust core has no C `Imaging` pointer or capsule to expose, so this
    /// returns `None`. Binding crates may format that as a Pillow-compatible
    /// placeholder.
    pub fn getim(&self) -> Option<u64> {
        None
    }

    /// Returns the known image format name, if the image came from encoded input.
    pub fn format_name(&self) -> Option<String> {
        match self {
            Image::Loaded(data) => data.source_format.map(|format| format.as_str().to_owned()),
            Image::Paletted(data) => data.source_format.map(|format| format.as_str().to_owned()),
            Image::Bytes { format, .. } | Image::Pipeline { format, .. } => {
                format.map(|format| format.as_str().to_owned())
            }
        }
    }

    /// Replaces retained EXIF bytes after a metadata-only image operation.
    ///
    /// The operation is intentionally applied after loading so the lazy image
    /// representation remains the single source of truth for pixel and
    /// metadata state.
    pub(crate) fn with_exif_metadata(mut self, exif: Option<Vec<u8>>) -> Result<Self, PilError> {
        self.load()?;
        match &mut self {
            Image::Loaded(data) => data.exif = exif,
            Image::Paletted(data) => data.exif = exif,
            Image::Bytes { .. } | Image::Pipeline { .. } => {
                unreachable!("load must materialize EXIF metadata target")
            }
        }
        Ok(self)
    }
}

impl Image {
    /// Returns Pillow band names for the current image mode.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when neither the retained mode metadata nor lazy
    /// image data is sufficient to determine the bands.
    pub fn getbands(&self) -> Result<Vec<String>, PilError> {
        // Pillow's Image.getbands -> ImagingCore.getbands derives names from
        // canonical mode metadata without touching pixel storage. `mode()`
        // already preserves explicit tags for non-standard modes and can read
        // a decoder header before pixel data is available, so this handles
        // truncated lazy images without a duplicate pixel-channel fallback.
        let mode = self.mode()?;
        let bands: Vec<&str> = match mode.as_str() {
            "CMYK" => vec!["C", "M", "Y", "K"],
            "YCbCr" => vec!["Y", "Cb", "Cr"],
            "HSV" => vec!["H", "S", "V"],
            "LAB" => vec!["L", "A", "B"],
            "PA" => vec!["P", "A"],
            "La" => vec!["L", "a"],
            "RGBa" => vec!["R", "G", "B", "a"],
            "RGBX" => vec!["R", "G", "B", "X"],
            "I" | "F" | "P" | "1" => {
                vec![mode.as_str()]
            }
            // Pillow exposes all unsigned 16-bit luma variants as one logical
            // band named "I", regardless of their byte order.
            "I;16" | "I;16L" | "I;16B" | "I;16N" => vec!["I"],
            "L" => vec!["L"],
            "LA" => vec!["L", "A"],
            "RGB" => vec!["R", "G", "B"],
            "RGBA" => vec!["R", "G", "B", "A"],
            _ => Vec::new(),
        };
        Ok(bands.into_iter().map(str::to_string).collect())
    }

    /// Returns image dimensions as `(width, height)` in pixels.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when a lazy image must be decoded and decoding
    /// fails.
    pub fn size(&self) -> Result<(u32, u32), PilError> {
        match self {
            Image::Loaded(data) => return Ok(data.image.dimensions()),
            Image::Paletted(data) => return Ok(data.indices.dimensions()),
            Image::Bytes {
                info: Some(info), ..
            } => return Ok((info.width, info.height)),
            Image::Pipeline {
                source, ops, shape, ..
            } => {
                let planned = shape.get_or_init(|| {
                    let mut dimensions = source.size()?;
                    for op in ops.iter() {
                        let Some(next) = known_pipeline_op_dimensions(op, dimensions) else {
                            return Ok(None);
                        };
                        dimensions = next;
                    }
                    Ok(Some(dimensions))
                });
                if let Some(dimensions) = planned.as_ref().map_err(Clone::clone)? {
                    return Ok(*dimensions);
                }
            }
            _ => {}
        }
        let img = self.materialized_shared()?;
        Ok((img.width(), img.height()))
    }

    /// Returns the Pillow mode string for this image.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when lazy data must be materialized to determine
    /// the mode and that materialization fails.
    pub fn mode(&self) -> Result<String, PilError> {
        match self {
            Image::Paletted(_) => return Ok("P".to_owned()),
            Image::Loaded(data) => {
                return Ok(data
                    .explicit_mode
                    .clone()
                    .unwrap_or_else(|| image_mode_name(data.decoded_mode).to_owned()));
            }
            Image::Bytes {
                info: Some(info), ..
            } => return Ok(image_mode_name(info.mode).to_owned()),
            Image::Pipeline {
                explicit_mode: Some(mode),
                ..
            } => return Ok(mode.clone()),
            Image::Pipeline {
                source,
                ops,
                explicit_mode: None,
                mode,
                ..
            } => {
                let planned = mode.get_or_init(|| {
                    let mut current = source.mode()?;
                    if ops.mode_preserving() {
                        return Ok(Some(current));
                    }
                    for op in ops.iter() {
                        let Some(next) = known_pipeline_op_mode(op, &current) else {
                            return Ok(None);
                        };
                        current = next;
                    }
                    Ok(Some(current))
                });
                if let Some(mode) = planned.as_ref().map_err(Clone::clone)? {
                    return Ok(mode.clone());
                }
            }
            _ => {}
        }
        let img = self.materialized_shared()?;
        Ok(self.mode_from_materialized(&img))
    }

    /// Returns the mode after the caller has already materialized this image.
    ///
    /// The fallible [`Image::mode`] method must decode lazy inputs when no
    /// header mode is available. Once a concrete buffer is in hand, that
    /// operation cannot fail; keeping this path infallible also makes it clear
    /// that mode selection does not trigger a second decode.
    pub(crate) fn mode_from_materialized(&self, image: &DynamicImage) -> String {
        match self {
            Image::Paletted(_) => "P".to_owned(),
            Image::Loaded(data) => data
                .explicit_mode
                .clone()
                .unwrap_or_else(|| image_mode_name(data.decoded_mode).to_owned()),
            Image::Bytes {
                info: Some(info), ..
            } => image_mode_name(info.mode).to_owned(),
            Image::Pipeline {
                explicit_mode: Some(mode),
                ..
            } => mode.clone(),
            _ => color_type_to_mode(image.color()).to_owned(),
        }
    }
}

#[cfg(test)]
mod compatibility_info_tests {
    use super::{Image, ImageInfoValue, PilError};

    #[test]
    fn crop_after_pipeline_preserves_jpeg_info() -> Result<(), PilError> {
        let image = Image::open_bytes(
            include_bytes!("../../tests/fixtures/assets/image/rgb-small.jpg").to_vec(),
        )?;
        let expected = image.compatibility_info();
        assert!(expected.iter().any(|(key, _)| key == "jfif"));
        let inverted = crate::ops::imageops::invert(&image)?;
        let cropped = inverted.crop(Some((0, 0, 2, 2)))?;
        assert_eq!(cropped.compatibility_info(), expected);
        assert!(cropped.source_format().is_none());
        Ok(())
    }

    #[test]
    fn webp_timing_defaults_appear_only_after_load() -> Result<(), PilError> {
        let mut image = Image::open_bytes(
            include_bytes!("../../tests/fixtures/assets/image/rgb-small.webp").to_vec(),
        )?;
        let expected = [
            ("timestamp".to_owned(), ImageInfoValue::Integer(0)),
            ("duration".to_owned(), ImageInfoValue::Integer(0)),
        ];

        assert!(!image.is_materialized());
        let lazy_info = image.compatibility_info();
        assert!(
            lazy_info
                .iter()
                .all(|(key, _)| key != "timestamp" && key != "duration")
        );

        image.load()?;
        let loaded_info = image.compatibility_info();
        assert!(expected.iter().all(|field| loaded_info.contains(field)));
        Ok(())
    }
}
