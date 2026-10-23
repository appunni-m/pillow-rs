use super::{TransformData, TransformFill};
use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::ResampleFilter;

const TRANSFORM_RESAMPLE_GUIDANCE: &str = concat!(
    "Use Image.Resampling.NEAREST (0), Image.Resampling.BILINEAR (2) or ",
    "Image.Resampling.BICUBIC (3)"
);

fn parse_transform_resample(code: i32) -> Result<ResampleFilter, PilError> {
    match code {
        0 => Ok(ResampleFilter::Nearest),
        2 => Ok(ResampleFilter::Bilinear),
        3 => Ok(ResampleFilter::Bicubic),
        1 | 4 | 5 => {
            // Pillow distinguishes known resize-only filters from an
            // unknown integer in Image.transform's public error contract.
            let name = match code {
                1 => "Image.Resampling.LANCZOS",
                4 => "Image.Resampling.BOX",
                5 => "Image.Resampling.HAMMING",
                _ => unreachable!(),
            };
            Err(PilError::ValueError(format!(
                "{name} ({code}) cannot be used. {TRANSFORM_RESAMPLE_GUIDANCE}"
            )))
        }
        // Image.transform has a narrower public filter contract than resize;
        // Pillow reports only its three accepted filters here.
        other => Err(PilError::ValueError(format!(
            "Unknown resampling filter ({}). {}",
            other, TRANSFORM_RESAMPLE_GUIDANCE
        ))),
    }
}

fn flatten_mesh_data(items: &[(Vec<f64>, Vec<f64>)]) -> Result<Vec<f64>, PilError> {
    let mut flat = Vec::with_capacity(items.len().saturating_mul(12));
    for (bbox, quad) in items {
        if bbox.len() != 4 {
            return Err(PilError::IndexError("tuple index out of range".into()));
        }
        if quad.len() != 8 {
            return Err(PilError::IndexError("tuple index out of range".into()));
        }
        flat.extend_from_slice(bbox);
        flat.extend_from_slice(quad);
    }
    Ok(flat)
}

fn flatten_raw_mesh_data(items: &[Vec<Vec<f64>>]) -> Result<Vec<f64>, PilError> {
    let mut flat = Vec::with_capacity(items.len().saturating_mul(12));
    for item in items {
        if item.len() != 2 {
            let message = if item.len() < 2 {
                format!(
                    "not enough values to unpack (expected 2, got {})",
                    item.len()
                )
            } else {
                "too many values to unpack (expected 2)".into()
            };
            return Err(PilError::ValueError(message));
        }
        let bbox = &item[0];
        let quad = &item[1];
        if bbox.len() != 4 {
            return Err(PilError::IndexError("tuple index out of range".into()));
        }
        if quad.len() != 8 {
            return Err(PilError::IndexError("tuple index out of range".into()));
        }
        flat.extend_from_slice(bbox);
        flat.extend_from_slice(quad);
    }
    Ok(flat)
}

impl Image {
    /// Applies a Pillow public transform method after host values have been
    /// converted to neutral Rust inputs.
    ///
    /// Methods `0` through `4` cover affine, extent, perspective, quad, and
    /// mesh transforms. Resampling is parsed and retained in the core pipeline
    /// so the selected backend observes the public Pillow filter.
    pub fn transform_public(
        &self,
        size: (u32, u32),
        method: i32,
        data: Option<TransformData>,
        resample: i32,
        _fill: i32,
        fillcolor: Option<TransformFill>,
    ) -> Result<Image, PilError> {
        let requested_filter = parse_transform_resample(resample)?;
        let fill_is_none = fillcolor.is_none();
        // Pillow's indexed affine transform keeps the raw palette index and
        // therefore uses nearest-neighbor sampling regardless of the public
        // filter code. Keep the operation palette-safe so materialization does
        // not expand P into RGB before the backend runs.
        let filter = if self.has_palette_mode() || self.explicit_mode() == Some("1") {
            ResampleFilter::Nearest
        } else {
            requested_filter
        };
        let (fill, palette_fill) = self.public_transform_fill(fillcolor)?;
        match method {
            0 => {
                let Some(TransformData::Affine(matrix)) = data else {
                    return Err(PilError::ValueError("missing method data".into()));
                };
                // Keep the public Pillow path on the same core entry points
                // exposed for direct Rust callers. Palette scalar fills retain
                // their index, while tuple/name fills resolve to palette index
                // zero through the same helper.
                if let Some(index) = palette_fill {
                    self.transform_affine_palette_index_with_filter(
                        size,
                        &matrix,
                        index,
                        filter,
                        fill_is_none,
                    )
                } else {
                    self.transform_affine_with_palette_fill(
                        size,
                        &matrix,
                        fill,
                        None,
                        filter,
                        fill_is_none,
                    )
                }
            }
            1 => {
                let Some(TransformData::Affine(extent)) = data else {
                    return Err(PilError::ValueError("missing method data".into()));
                };
                let [x0, y0, x1, y1] = extent.as_slice() else {
                    let message = if extent.len() < 4 {
                        format!(
                            "not enough values to unpack (expected 4, got {})",
                            extent.len()
                        )
                    } else {
                        "too many values to unpack (expected 4)".into()
                    };
                    return Err(PilError::ValueError(message));
                };
                // Pillow's EXTENT method maps each destination pixel back
                // into the requested source rectangle. Express that mapping
                // through the same affine pipeline used by method 0 so mode,
                // fill, and lazy execution remain shared.
                let matrix = vec![
                    (x1 - x0) / f64::from(size.0),
                    0.0,
                    *x0,
                    0.0,
                    (y1 - y0) / f64::from(size.1),
                    *y0,
                ];
                self.transform_affine_with_palette_fill(
                    size,
                    &matrix,
                    fill,
                    palette_fill,
                    filter,
                    fill_is_none,
                )
            }
            2 | 3 => {
                let Some(TransformData::Affine(data)) = data else {
                    return Err(PilError::ValueError("missing method data".into()));
                };
                if data.len() < 8 {
                    return if method == 2 {
                        Err(PilError::ValueError(
                            "wrong number of matrix entries".into(),
                        ))
                    } else {
                        Err(PilError::IndexError("tuple index out of range".into()))
                    };
                }
                if method == 2 {
                    self.transform_perspective_with_palette_fill(
                        size,
                        &data[..8],
                        fill,
                        palette_fill,
                        filter,
                        fill_is_none,
                    )
                } else {
                    self.transform_quad_with_palette_fill(
                        size,
                        &data[..8],
                        fill,
                        palette_fill,
                        filter,
                        fill_is_none,
                    )
                }
            }
            4 => transform_public_mesh(self, size, data, fill, palette_fill, filter, fill_is_none),
            _ => Err(PilError::ValueError("unknown transformation method".into())),
        }
    }
}

fn transform_public_mesh(
    image: &Image,
    size: (u32, u32),
    data: Option<TransformData>,
    fill: (u8, u8, u8, u8),
    palette_fill: Option<u8>,
    filter: ResampleFilter,
    fill_is_none: bool,
) -> Result<Image, PilError> {
    let data = match data {
        Some(TransformData::Mesh(items)) => flatten_mesh_data(&items)?,
        Some(TransformData::RawMesh(items)) => flatten_raw_mesh_data(&items)?,
        Some(TransformData::Affine(items)) if items.is_empty() => {
            let mode = image.mode()?;
            if let Some(index) = palette_fill {
                return Ok(Image::new_palette_index(size.0, size.1, index));
            }
            // The transform fill tuple is normalized for raster
            // kernels as `(luma, alpha, 0, 0)`, while Image.new
            // stores LA/PA alpha in the fourth slot. Preserve the
            // public two-component fill when an empty mesh uses
            // Image.new directly.
            let image_fill = if matches!(mode.as_str(), "LA" | "PA") {
                (fill.0, 0, 0, fill.1)
            } else {
                fill
            };
            return Image::new(size.0, size.1, &mode, image_fill);
        }
        Some(TransformData::Affine(_)) => {
            return Err(PilError::TypeError(
                "cannot unpack non-iterable int object".into(),
            ));
        }
        Some(TransformData::Mapping) => {
            return Err(PilError::ValueError(
                "too many values to unpack (expected 2)".into(),
            ));
        }
        Some(TransformData::Text(_)) => {
            return Err(PilError::ValueError(
                "not enough values to unpack (expected 2, got 1)".into(),
            ));
        }
        Some(TransformData::Invalid(type_name)) => {
            return Err(PilError::TypeError(format!(
                "'{type_name}' object is not iterable"
            )));
        }
        None => return Err(PilError::ValueError("missing method data".into())),
    };
    image.transform_mesh_with_filter(size, data, fill, filter, fill_is_none)
}

#[cfg(test)]
mod tests {
    use super::parse_transform_resample;
    use crate::error::PilError;

    #[test]
    fn transform_resize_only_filter_errors_match_pillow() {
        for (code, name) in [
            (1, "Image.Resampling.LANCZOS"),
            (4, "Image.Resampling.BOX"),
            (5, "Image.Resampling.HAMMING"),
        ] {
            let error = parse_transform_resample(code).expect_err("resize-only filter");
            assert!(
                matches!(error, PilError::ValueError(message) if message == format!(
                    "{name} ({code}) cannot be used. Use Image.Resampling.NEAREST (0), Image.Resampling.BILINEAR (2) or Image.Resampling.BICUBIC (3)"
                ))
            );
        }
    }

    #[test]
    fn transform_unknown_filter_error_remains_distinct() {
        let error = parse_transform_resample(6).expect_err("unknown filter");
        assert!(matches!(error, PilError::ValueError(message) if message ==
            "Unknown resampling filter (6). Use Image.Resampling.NEAREST (0), Image.Resampling.BILINEAR (2) or Image.Resampling.BICUBIC (3)"));
    }
}
