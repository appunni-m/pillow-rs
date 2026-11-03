//! Channel split operations.
// TODO(pillow-rs#3ptb): verify child selection/fallback receipts, metadata,
// GPU-to-host identity fallback, lazy output identity, and that branching
// non-palette mode-changing pipelines avoids replay without changing outputs.
// Loaded and Bytes/Pipeline caches successful at call entry report
// `native-identity`; Paletted and cached Bytes P copies report `native-copy`;
// none selects an architecture. Lazy identity routes preserve source graph
// receipts before the terminal identity/copy receipt. Multiband ExtractBand
// selection remains deferred until a returned child is used.
// OperationSupport exposes each mode's palette copy, identity copy, or
// per-band ExtractBand child route.

use crate::compute::registry::support::OperationId;
use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;

fn split_identity_source(
    image: &Image,
    cached_at_entry: bool,
) -> Option<(&DynamicImage, bool)> {
    match image {
        Image::Loaded(data) => Some((data.image.as_ref(), true)),
        Image::Paletted(data) => Some((&data.indices, false)),
        Image::Bytes { materialized, .. } if cached_at_entry => {
            let image = materialized.get()?.as_ref().ok()?;
            Some((
                image.as_ref(),
                image.mode() != crate::raster::Mode::ImagePalette8,
            ))
        }
        Image::Pipeline { materialized, .. } if cached_at_entry => {
            let image = materialized.get()?.as_ref().ok()?;
            Some((image.as_ref(), true))
        }
        Image::Bytes { .. } | Image::Pipeline { .. } => None,
    }
}

fn split_identity_copy(image: &Image) -> Result<Vec<Image>, PilError> {
    let mut band = image.materialized_branch()?;
    match &mut band {
        Image::Loaded(data) => data.source_format = None,
        Image::Paletted(data) => data.source_format = None,
        _ => {}
    }
    Ok(vec![band])
}

fn split_child_source(image: &Image, mode: crate::raster::Mode) -> Result<Image, PilError> {
    let palette_samples =
        matches!(mode, crate::raster::Mode::ImagePalette8 | crate::raster::Mode::ImagePaletteAlpha8);
    if matches!(image, Image::Pipeline { .. }) && !palette_samples {
        image.materialized_branch()
    } else {
        Ok(image.clone())
    }
}

fn split_extract_band_child(image: &Image, source: &Image, index: u8) -> Image {
    let child = Image::push_op(source, PipelineOp::ExtractBand { index });
    match image.backend() {
        Some(backend) => child.use_backend(backend),
        None => child,
    }
}

impl Image {
    /// Splits the image into one image per logical band.
    ///
    /// `P` images return a single paletted clone. Other single-band modes
    /// return one same-mode copy, preserving packed or typed sample storage.
    /// Multiband modes return lazy pipeline images. Internal high-depth modes
    /// preserve their native `I;16` or `F` scalar samples in each band.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization is needed to determine band
    /// count and that materialization fails.
    pub fn split(&self) -> Result<Vec<Image>, PilError> {
        let cached_at_entry = self.is_materialized();
        let mode = self.mode_identity()?;
        let route = OperationId::Split.support().route_for_mode(mode);
        route.prepared_support().validate_mode()?;
        // PIL: P-mode has one band, and split() returns a copy preserving the
        // indexed mode and palette.  Encoded P inputs are still represented as
        // lazy Image::Bytes until their first operation, so checking only the
        // concrete Paletted variant would lose the mode and turn the result
        // into L during the ExtractBand path.
        // Use the exact raster identity for logical band count. Pillow's band
        // names are intentionally absent for CoreOnly modes, which still have
        // native bands and must not be treated as zero-band images.
        if route.has_copy_path() {
            if let Some((input, shares_pixels)) = split_identity_source(self, cached_at_entry) {
                let requested_backend = crate::compute::Backend::pipeline_telemetry_enabled()
                    .then(|| self.cpu_arch_execution_route_advisory().0)
                    .flatten();
                let copy = || split_identity_copy(self);
                return if shares_pixels {
                    crate::compute::execute_eager_native_identity_result(
                        "Split",
                        input,
                        requested_backend,
                        copy,
                    )
                } else {
                    crate::compute::execute_eager_native_copy_result(
                        "Split",
                        input,
                        requested_backend,
                        copy,
                    )
                };
            }
            let mut operation_receipts =
                crate::compute::PipelineOperationTelemetryCapture::begin();
            let input = self.materialized_shared()?;
            operation_receipts.capture();
            let requested_backend = crate::compute::Backend::pipeline_telemetry_enabled()
                .then(|| self.cpu_arch_execution_route_advisory().0)
                .flatten();
            let copy = || split_identity_copy(self);
            let copies_palette_indices =
                matches!(self, Image::Bytes { .. })
                    && mode == crate::raster::Mode::ImagePalette8;
            return if copies_palette_indices {
                crate::compute::execute_eager_native_copy_result_with_receipts(
                    "Split",
                    input.as_ref(),
                    requested_backend,
                    operation_receipts,
                    copy,
                )
            } else {
                crate::compute::execute_eager_native_identity_result_with_receipts(
                    "Split",
                    input.as_ref(),
                    requested_backend,
                    operation_receipts,
                    copy,
                )
            };
        }

        // Determine logical band count from the exact raster mode; shared
        // carriers such as RGB32F do not have the carrier's channel count.
        let mut operation_receipts =
            crate::compute::PipelineOperationTelemetryCapture::begin();
        let img = self.materialized_shared()?;
        operation_receipts.capture();
        let n_bands = usize::from(img.mode().band_count());
        let child = route.pipeline_children_per_band().ok_or_else(|| {
            PilError::InternalError("Split route omitted its per-band pipeline child".into())
        })?;

        // Split materializes a lazy parent to establish its exact band count.
        // Branch non-palette pipelines from those resolved pixels so a
        // mode-changing parent is not replayed once for every child.
        match child {
            OperationId::ExtractBand => {
                let source = split_child_source(self, mode)?;
                Ok((0..n_bands)
                    .map(|index| split_extract_band_child(self, &source, index as u8))
                    .collect())
            }
            _ => Err(PilError::InternalError(
                "Split route declared an unsupported pipeline child".into(),
            )),
        }
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
            let image = Image::frombytes(crate::raster::Mode::from_pillow_name(mode).expect("known test mode"), (2, 1), bytes).expect("source image");
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
