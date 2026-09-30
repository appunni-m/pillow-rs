//! Channel split operations.

use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;

impl Image {
    /// Splits the image into one image per logical band.
    ///
    /// `P` images return a single paletted clone. Other single-band modes
    /// return one same-mode copy, preserving packed or typed sample storage.
    /// Multiband modes return lazy pipeline images that extract one channel
    /// when materialized.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization is needed to determine band
    /// count and that materialization fails.
    pub fn split(&self) -> Result<Vec<Image>, PilError> {
        // PIL: P-mode has one band, and split() returns a copy preserving the
        // indexed mode and palette.  Encoded P inputs are still represented as
        // lazy Image::Bytes until their first operation, so checking only the
        // concrete Paletted variant would lose the mode and turn the result
        // into L during the ExtractBand path.
        if self.has_palette_mode() {
            let mut band = self.materialized_branch()?;
            // Pillow-derived images do not retain the source container's
            // ``format`` field, even when the split band stays indexed.
            match &mut band {
                Image::Loaded(data) => data.source_format = None,
                Image::Paletted(data) => data.source_format = None,
                _ => {}
            }
            return Ok(vec![band]);
        }

        // Pillow returns a same-mode image when there is one logical band.
        // The raster carrier can have a different physical layout: I/F use
        // ImageRgba8 for one four-byte scalar, mode 1 is bit-packed at the
        // public boundary, and I;16 uses typed 16-bit samples. ExtractBand is
        // byte-oriented and would either split scalar bytes into extra L
        // images or discard the original mode. A materialized branch keeps
        // the exact mode and storage while sharing immutable pixels until a
        // later mutation needs copy-on-write.
        if self.getbands()?.len() == 1 {
            let mut band = self.materialized_branch()?;
            match &mut band {
                Image::Loaded(data) => data.source_format = None,
                Image::Paletted(data) => data.source_format = None,
                _ => {}
            }
            return Ok(vec![band]);
        }

        // Determine band count from the image
        let img = self.materialize()?;
        let n_bands = match img.color() {
            crate::raster::ColorType::L8 | crate::raster::ColorType::L16 => 1,
            crate::raster::ColorType::La8 | crate::raster::ColorType::La16 => 2,
            crate::raster::ColorType::Rgb8 | crate::raster::ColorType::Rgb16 => 3,
            _ => 4, // Rgba8, Rgba16, or fallback
        };

        // Create N pipeline images, each extracting one band
        let bands: Vec<Image> = (0..n_bands)
            .map(|i| Image::push_op(self, PipelineOp::ExtractBand { index: i as u8 }))
            .collect();

        Ok(bands)
    }
}

#[cfg(test)]
mod tests {
    use crate::image::{Image, PutPixelValue};

    #[test]
    fn single_band_split_preserves_mode_bytes_and_copy_on_write() {
        let cases: &[(&str, &[u8])] = &[
            ("I", &[0x78, 0x56, 0x34, 0x12, 0xfe, 0xff, 0xff, 0xff]),
            ("F", &[0x00, 0x00, 0xa0, 0x3f, 0x00, 0x00, 0x20, 0xc0]),
            ("1", &[0x80]),
            ("I;16", &[0x34, 0x12, 0xcd, 0xab]),
            ("I;16B", &[0x12, 0x34, 0xab, 0xcd]),
        ];

        for &(mode, bytes) in cases {
            let image = Image::frombytes(mode, (2, 1), bytes).expect("source image");
            let bands = image.split().expect("split scalar image");

            assert_eq!(bands.len(), 1, "mode {mode}");
            assert_eq!(bands[0].mode().expect("band mode"), mode);
            assert_eq!(bands[0].tobytes().expect("band bytes"), bytes);

            let mut band = bands[0].clone();
            let replacement = if mode == "1" { 0 } else { 42 };
            band.putpixel_value(0, 0, PutPixelValue::Integer(replacement))
                .expect("mutate split result");
            assert_eq!(image.tobytes().expect("source remains unchanged"), bytes);
            assert_ne!(band.tobytes().expect("mutated band bytes"), bytes);
        }
    }
}
