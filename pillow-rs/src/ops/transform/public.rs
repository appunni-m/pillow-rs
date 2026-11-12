use super::{PreparedTransformFill, TransformData, TransformFill};
use crate::compute::pool_cpu::ops::transform::ops::{self, TransformRequest};
use crate::compute::{Architecture, ExecutionContext};
use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::{ResampleFilter, TransformMethod};
use crate::raster::{DynamicImage, Mode};

const TRANSFORM_RESAMPLE_GUIDANCE: &str = concat!(
    "Use Image.Resampling.NEAREST (0), Image.Resampling.BILINEAR (2) or ",
    "Image.Resampling.BICUBIC (3)"
);

fn parse_transform_resample(code: i32) -> Result<ResampleFilter, PilError> {
    match code {
        0 => Ok(ResampleFilter::Nearest),
        2 => Ok(ResampleFilter::Bilinear),
        3 => Ok(ResampleFilter::Bicubic),
        1 | 4 | 5 | 6 | 7 => {
            // Pillow distinguishes known resize-only filters from an
            // unknown integer in Image.transform's public error contract.
            Err(transform_resample_public_error(
                Some(code),
                &code.to_string(),
            ))
        }
        // Image.transform has a narrower public filter contract than resize;
        // Pillow reports only its three accepted filters here.
        other => Err(transform_resample_public_error(
            Some(other),
            &other.to_string(),
        )),
    }
}

pub(crate) fn validate_transform_resample_filter(filter: ResampleFilter) -> Result<(), PilError> {
    let code = match filter {
        ResampleFilter::Nearest => 0,
        ResampleFilter::Lanczos => 1,
        ResampleFilter::Bilinear => 2,
        ResampleFilter::Bicubic => 3,
        ResampleFilter::Box => 4,
        ResampleFilter::Hamming => 5,
        ResampleFilter::Mks2013 => 6,
        ResampleFilter::Mks2021 => 7,
    };
    parse_transform_resample(code).map(|_| ())
}

/// Formats a Pillow `Image.transform` resampling error using the host value's
/// display text while keeping the public contract in the Rust core.
pub fn transform_resample_public_error(code: Option<i32>, display: &str) -> PilError {
    let Some(code) = code else {
        return PilError::ValueError(format!(
            "Unknown resampling filter ({display}). {TRANSFORM_RESAMPLE_GUIDANCE}"
        ));
    };
    match code {
        1 | 4 | 5 | 6 | 7 => {
            let name = match code {
                1 => "Image.Resampling.LANCZOS",
                4 => "Image.Resampling.BOX",
                5 => "Image.Resampling.HAMMING",
                6 => "Image.Resampling.MKS2013",
                7 => "Image.Resampling.MKS2021",
                _ => unreachable!(),
            };
            PilError::ValueError(format!(
                "{name} ({display}) cannot be used. {TRANSFORM_RESAMPLE_GUIDANCE}"
            ))
        }
        _ => PilError::ValueError(format!(
            "Unknown resampling filter ({display}). {TRANSFORM_RESAMPLE_GUIDANCE}"
        )),
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

fn preflight_transform_data(
    method: i32,
    size: (u32, u32),
    data: Option<&TransformData>,
) -> Result<bool, PilError> {
    match method {
        0 => require_affine_data(data).map(|_| false),
        1 => {
            let extent = require_affine_data(data)?;
            validate_extent_data(extent, size).map(|_| false)
        }
        2 | 3 => {
            let data = require_affine_data(data)?;
            if data.len() < 8 {
                return if method == 2 {
                    Err(PilError::ValueError(
                        "wrong number of matrix entries".into(),
                    ))
                } else {
                    Err(PilError::IndexError("tuple index out of range".into()))
                };
            }
            Ok(false)
        }
        4 => preflight_mesh_data(data),
        _ => Err(PilError::ValueError("unknown transformation method".into())),
    }
}

fn require_affine_data(data: Option<&TransformData>) -> Result<&[f64], PilError> {
    match data {
        Some(TransformData::Affine(values)) => Ok(values),
        _ => Err(PilError::ValueError("missing method data".into())),
    }
}

fn validate_extent_data(extent: &[f64], size: (u32, u32)) -> Result<(), PilError> {
    if extent.len() != 4 {
        let message = if extent.len() < 4 {
            format!(
                "not enough values to unpack (expected 4, got {})",
                extent.len()
            )
        } else {
            "too many values to unpack (expected 4)".into()
        };
        return Err(PilError::ValueError(message));
    }
    if size.0 == 0 || size.1 == 0 {
        return Err(PilError::ZeroDivisionError("division by zero".into()));
    }
    Ok(())
}

fn quad_coefficients(size: (u32, u32), corners: &[f64]) -> Result<[f64; 8], PilError> {
    if size.0 == 0 || size.1 == 0 {
        return Err(PilError::ZeroDivisionError("division by zero".into()));
    }
    let x0 = corners[0];
    let y0 = corners[1];
    let inverse_width = 1.0 / f64::from(size.0);
    let inverse_height = 1.0 / f64::from(size.1);
    Ok([
        x0,
        (corners[6] - x0) * inverse_width,
        (corners[2] - x0) * inverse_height,
        (corners[4] - corners[2] - corners[6] + x0) * inverse_width * inverse_height,
        y0,
        (corners[7] - y0) * inverse_width,
        (corners[3] - y0) * inverse_height,
        (corners[5] - corners[3] - corners[7] + y0) * inverse_width * inverse_height,
    ])
}

fn preflight_mesh_data(data: Option<&TransformData>) -> Result<bool, PilError> {
    match data {
        Some(TransformData::Mesh(items)) => {
            // The first record is unpacked before Pillow checks its filter;
            // later records follow the first quad's transform call.
            if let Some((bbox, quad)) = items.first() {
                validate_mesh_pair(bbox, quad)?;
            }
            Ok(items.is_empty())
        }
        Some(TransformData::RawMesh(items)) => {
            if let Some(item) = items.first() {
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
                validate_mesh_pair(&item[0], &item[1])?;
            }
            Ok(items.is_empty())
        }
        Some(TransformData::Affine(values)) if values.is_empty() => Ok(true),
        Some(TransformData::Affine(_)) => Err(PilError::TypeError(
            "cannot unpack non-iterable int object".into(),
        )),
        Some(TransformData::Mapping) => Err(PilError::ValueError(
            "too many values to unpack (expected 2)".into(),
        )),
        Some(TransformData::Text(value)) if value.is_empty() => Ok(true),
        Some(TransformData::Text(_)) => Err(PilError::ValueError(
            "not enough values to unpack (expected 2, got 1)".into(),
        )),
        Some(TransformData::Invalid(type_name)) => Err(PilError::TypeError(format!(
            "'{type_name}' object is not iterable"
        ))),
        None => Err(PilError::ValueError("missing method data".into())),
    }
}

fn validate_mesh_pair(bbox: &[f64], quad: &[f64]) -> Result<(), PilError> {
    if bbox.len() != 4 || quad.len() != 8 {
        return Err(PilError::IndexError("tuple index out of range".into()));
    }
    Ok(())
}

/// Reusable output and routing state for Pillow's per-record MESH execution.
#[doc(hidden)]
pub struct TransformMeshAccumulator {
    output: Image,
    prepared: PreparedTransformFill,
    size: (u32, u32),
    context: ExecutionContext,
    source_scratch: Vec<u8>,
    source_lanes: Vec<u8>,
    output_lanes: Option<Vec<u8>>,
    i16_output_plane: Option<Vec<u8>>,
}

/// Exact-mode destination allocated before the host evaluates method data.
#[doc(hidden)]
pub struct PreparedTransformOutput {
    output: Image,
    prepared: PreparedTransformFill,
    size: (u32, u32),
    context: ExecutionContext,
}

impl TransformMeshAccumulator {
    /// Returns the allocated destination image after the streamed records run.
    pub fn finish(mut self) -> Result<Image, PilError> {
        let mode = self.prepared.mode;
        let data = [0.0; 12];
        let method = TransformMethod::Mesh;
        let request = TransformRequest {
            width: self.size.0,
            height: self.size.1,
            method: &method,
            data: &data,
            filter: ResampleFilter::Nearest,
            fill: self
                .prepared
                .palette_fill
                .map(|index| (index, 0, 0, 255))
                .or(Some(self.prepared.fill)),
            fill_is_none: self.prepared.fill_is_none,
        };
        let output = output_dynamic_mut(&mut self.output)?;
        ops::finish_mesh_records(mode, output, request, self.i16_output_plane.as_deref())?;
        crate::compute::pool_cpu::ops::transform::helpers::hidden_lanes::attach_mesh_output(
            output,
            self.output_lanes,
        )?;
        Ok(self.output)
    }
}

fn output_dynamic_mut(image: &mut Image) -> Result<&mut DynamicImage, PilError> {
    match image {
        Image::Loaded(data) => Ok(std::sync::Arc::make_mut(&mut data.image)),
        Image::Paletted(data) => Ok(&mut data.indices),
        _ => Err(PilError::InternalError(
            "Transform MESH destination was not allocated as a raster".into(),
        )),
    }
}

impl Image {
    /// Applies a Pillow public transform method after host values have been
    /// converted to neutral Rust inputs.
    ///
    /// Methods `0` through `4` cover affine, extent, perspective, quad, and
    /// mesh transforms. Resampling is parsed and retained in the core pipeline
    /// so the selected backend observes the public Pillow filter.
    ///
    /// Pillow's `fill` argument is unused for these built-in transform methods;
    /// `fillcolor` controls pixels outside the transformed area. Keep `_fill`
    /// at this boundary because the host binding forwards Pillow's signature.
    pub fn transform_public(
        &self,
        size: (u32, u32),
        method: i32,
        data: Option<TransformData>,
        resample: i32,
        _fill: i32,
        fillcolor: Option<TransformFill>,
    ) -> Result<Image, PilError> {
        let mode = self.mode_identity()?;
        let prepared = self.prepare_transform_fill_for_mode(mode, fillcolor)?;
        self.transform_prepared_public(prepared, size, method, data, resample, _fill)
    }

    /// Resolves a transform destination fill after the wrapper has captured
    /// the output mode and before it evaluates method data callbacks.
    #[doc(hidden)]
    pub fn prepare_transform_fill_for_mode(
        &self,
        mode: Mode,
        fillcolor: Option<TransformFill>,
    ) -> Result<PreparedTransformFill, PilError> {
        let fill_is_none = fillcolor.is_none();
        let (fill, palette_fill, _) = super::fill::public_transform_fill(fillcolor, mode)?;
        Ok(PreparedTransformFill {
            mode,
            fill,
            palette_fill,
            fill_is_none,
        })
    }

    /// Allocates the MESH destination and captures its backend and architecture route.
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn prepare_transform_mesh_public(
        &self,
        prepared: PreparedTransformFill,
        size: (u32, u32),
    ) -> Result<TransformMeshAccumulator, PilError> {
        let destination = self.prepare_transform_output_public(prepared, size)?;
        self.prepare_transform_mesh_from_output_public(destination)
    }

    /// Allocates an exact-mode destination and captures its backend/architecture context.
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn prepare_transform_output_public(
        &self,
        prepared: PreparedTransformFill,
        size: (u32, u32),
    ) -> Result<PreparedTransformOutput, PilError> {
        if self.mode_identity()? != prepared.mode {
            return Err(PilError::InternalError(
                "Transform destination fill mode differs from the source mode".into(),
            ));
        }
        crate::checked_dims::CheckedDims::new(size.0, size.1, 1)?;
        let output = empty_transform_output(self, size, prepared.fill, prepared.palette_fill)?;
        let (_, context) = self.cpu_arch_execution_route_advisory();
        Ok(PreparedTransformOutput {
            output,
            prepared,
            size,
            context,
        })
    }

    /// Starts a MESH accumulator from the destination allocated before callbacks.
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn prepare_transform_mesh_from_output_public(
        &self,
        destination: PreparedTransformOutput,
    ) -> Result<TransformMeshAccumulator, PilError> {
        if self.mode_identity()? != destination.prepared.mode {
            return Err(PilError::InternalError(
                "Transform MESH destination mode differs from the source mode".into(),
            ));
        }
        let mut output = destination.output;
        let output_lanes = {
            let output = output_dynamic_mut(&mut output)?;
            crate::compute::pool_cpu::ops::transform::helpers::hidden_lanes::initialize_mesh_output(
                output,
            )?
        };
        Ok(TransformMeshAccumulator {
            output,
            prepared: destination.prepared,
            size: destination.size,
            context: destination.context,
            source_scratch: Vec::new(),
            source_lanes: Vec::new(),
            output_lanes,
            i16_output_plane: None,
        })
    }

    /// Applies one QUAD-shaped MESH record to the reusable destination.
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn apply_transform_mesh_record_public(
        &self,
        accumulator: &mut TransformMeshAccumulator,
        box_coordinates: (i64, i64, i64, i64),
        coefficients: [f64; 8],
        filter: ResampleFilter,
    ) -> Result<(), PilError> {
        let mode = accumulator.prepared.mode;
        let source = self.materialized_shared_for_ops()?;
        if source.mode() != mode {
            return Err(PilError::ValueError("image has wrong mode".into()));
        }
        let filter = if matches!(mode, Mode::ImagePalette8 | Mode::One) {
            ResampleFilter::Nearest
        } else {
            filter
        };
        let mut data = [0.0; 12];
        data[..4].copy_from_slice(&[
            box_coordinates.0 as f64,
            box_coordinates.1 as f64,
            box_coordinates.2 as f64,
            box_coordinates.3 as f64,
        ]);
        data[4..].copy_from_slice(&coefficients);
        let method = TransformMethod::Mesh;
        let request = TransformRequest {
            width: accumulator.size.0,
            height: accumulator.size.1,
            method: &method,
            data: &data,
            filter,
            fill: accumulator
                .prepared
                .palette_fill
                .map(|index| (index, 0, 0, 255))
                .or(Some(accumulator.prepared.fill)),
            fill_is_none: accumulator.prepared.fill_is_none,
        };
        let prepared = ops::prepare(source.as_ref(), request, &accumulator.context)?;
        let selection = *prepared.selection();
        crate::compute::begin_pipeline_operation_telemetry("Transform");
        crate::compute::record_pipeline_operation_selection(&selection);
        crate::compute::record_pipeline_operation_path("cpu");
        let result = (|| {
            let output = output_dynamic_mut(&mut accumulator.output)?;
            prepared.apply_mesh_record(
                output,
                box_coordinates,
                coefficients,
                &mut accumulator.source_scratch,
                &mut accumulator.i16_output_plane,
            )?;
            crate::compute::pool_cpu::ops::transform::helpers::hidden_lanes::apply_mesh_record(
                source.as_ref(),
                output,
                request,
                box_coordinates,
                coefficients,
                &selection,
                &mut accumulator.output_lanes,
                &mut accumulator.source_lanes,
            )
        })();
        crate::compute::finish_pipeline_operation_telemetry();
        result
    }

    /// Executes the built-in Transform method with a previously resolved fill.
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn transform_prepared_public(
        &self,
        prepared: PreparedTransformFill,
        size: (u32, u32),
        method: i32,
        data: Option<TransformData>,
        resample: i32,
        _fill: i32,
    ) -> Result<Image, PilError> {
        if !matches!(method, 0..=4) {
            return Err(PilError::ValueError("unknown transformation method".into()));
        }
        // Validate typed method data before the filter, matching the wrapper's
        // method-specific preparation order without executing any image work.
        let empty_mesh = preflight_transform_data(method, size, data.as_ref())?;
        // Pillow only checks a mesh filter from inside per-quad execution.
        let requested_filter = if empty_mesh {
            ResampleFilter::Nearest
        } else {
            parse_transform_resample(resample)?
        };
        if !empty_mesh && self.mode_identity()? != prepared.mode {
            return Err(PilError::ValueError("image has wrong mode".into()));
        }
        let mode = prepared.mode;
        // Pillow's indexed affine transform keeps the raw palette index and
        // therefore uses nearest-neighbor sampling regardless of the public
        // filter code. Keep the operation palette-safe so materialization does
        // not expand P into RGB before the backend runs.
        let filter = if matches!(mode, Mode::ImagePalette8 | Mode::One) {
            ResampleFilter::Nearest
        } else {
            requested_filter
        };
        self.apply_public_transform_method(
            size,
            method,
            data,
            prepared.fill,
            prepared.palette_fill,
            filter,
            prepared.fill_is_none,
        )
    }

    /// Executes a non-MESH host Transform into the destination prepared before
    /// Python method-data callbacks ran.
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn transform_prepared_output_public(
        &self,
        mut destination: PreparedTransformOutput,
        method: i32,
        data: Option<TransformData>,
        resample: i32,
    ) -> Result<Image, PilError> {
        if !matches!(method, 0 | 2 | 3) {
            return Err(PilError::ValueError("unknown transformation method".into()));
        }
        preflight_transform_data(method, destination.size, data.as_ref())?;
        let filter = parse_transform_resample(resample)?;
        let mode = destination.prepared.mode;
        if self.mode_identity()? != mode {
            return Err(PilError::ValueError("image has wrong mode".into()));
        }
        let Some(TransformData::Affine(coefficients)) = data else {
            return Err(PilError::ValueError("missing method data".into()));
        };
        let transform_method = match method {
            0 => TransformMethod::Affine,
            2 => TransformMethod::Perspective,
            3 => TransformMethod::Quad,
            _ => unreachable!("non-MESH method was validated"),
        };
        let filter = if matches!(mode, Mode::ImagePalette8 | Mode::One) {
            ResampleFilter::Nearest
        } else {
            filter
        };
        let request = TransformRequest {
            width: destination.size.0,
            height: destination.size.1,
            method: &transform_method,
            data: &coefficients,
            filter,
            fill: destination
                .prepared
                .palette_fill
                .map(|index| (index, 0, 0, 255))
                .or(Some(destination.prepared.fill)),
            fill_is_none: destination.prepared.fill_is_none,
        };
        let source = self.materialized_shared_for_ops()?;
        if source.mode() != mode {
            return Err(PilError::ValueError("image has wrong mode".into()));
        }
        let prepared = ops::prepare(source.as_ref(), request, &destination.context)?;
        let selection = *prepared.selection();
        crate::compute::begin_pipeline_operation_telemetry("Transform");
        crate::compute::record_pipeline_operation_selection(&selection);
        let path = match selection.implementation.architecture {
            Architecture::Default => "cpu",
            _ => "vector",
        };
        crate::compute::record_pipeline_operation_path(path);
        let result = (|| {
            let output = output_dynamic_mut(&mut destination.output)?;
            prepared.execute_into(output)
        })();
        crate::compute::finish_pipeline_operation_telemetry();
        result?;
        Ok(destination.output)
    }

    fn apply_public_transform_method(
        &self,
        size: (u32, u32),
        method: i32,
        data: Option<TransformData>,
        fill: (u8, u8, u8, u8),
        palette_fill: Option<u8>,
        filter: ResampleFilter,
        fill_is_none: bool,
    ) -> Result<Image, PilError> {
        match method {
            0 => self.transform_public_affine_method(
                size,
                data,
                fill,
                palette_fill,
                filter,
                fill_is_none,
            ),
            1 => self.transform_public_extent_method(
                size,
                data,
                fill,
                palette_fill,
                filter,
                fill_is_none,
            ),
            2 | 3 => self.transform_public_projective_method(
                size,
                method,
                data,
                fill,
                palette_fill,
                filter,
                fill_is_none,
            ),
            4 => transform_public_mesh(self, size, data, fill, palette_fill, filter, fill_is_none),
            _ => Err(PilError::ValueError("unknown transformation method".into())),
        }
    }

    fn transform_public_affine_method(
        &self,
        size: (u32, u32),
        data: Option<TransformData>,
        fill: (u8, u8, u8, u8),
        palette_fill: Option<u8>,
        filter: ResampleFilter,
        fill_is_none: bool,
    ) -> Result<Image, PilError> {
        let Some(TransformData::Affine(matrix)) = data else {
            return Err(PilError::ValueError("missing method data".into()));
        };
        // Keep the public Pillow path on the same core entry points exposed
        // for direct Rust callers. Palette scalar fills retain their index,
        // while tuple/name fills resolve to palette index zero through the
        // same helper.
        if let Some(index) = palette_fill {
            self.transform_affine_palette_index_with_filter(
                size,
                &matrix,
                index,
                filter,
                fill_is_none,
            )
        } else {
            self.transform_affine_with_palette_fill(size, &matrix, fill, None, filter, fill_is_none)
        }
    }

    fn transform_public_extent_method(
        &self,
        size: (u32, u32),
        data: Option<TransformData>,
        fill: (u8, u8, u8, u8),
        palette_fill: Option<u8>,
        filter: ResampleFilter,
        fill_is_none: bool,
    ) -> Result<Image, PilError> {
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
        // Pillow's EXTENT method maps each destination pixel back into the
        // requested source rectangle. Express that mapping through the same
        // affine pipeline so mode, fill, and lazy execution remain shared.
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

    fn transform_public_projective_method(
        &self,
        size: (u32, u32),
        method: i32,
        data: Option<TransformData>,
        fill: (u8, u8, u8, u8),
        palette_fill: Option<u8>,
        filter: ResampleFilter,
        fill_is_none: bool,
    ) -> Result<Image, PilError> {
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
            let coefficients = quad_coefficients(size, &data[..8])?;
            self.transform_quad_with_palette_fill(
                size,
                &coefficients,
                fill,
                palette_fill,
                filter,
                fill_is_none,
            )
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
        Some(TransformData::Mesh(items)) if items.is_empty() => {
            return empty_transform_output(image, size, fill, palette_fill);
        }
        Some(TransformData::Mesh(items)) => flatten_mesh_data(&items)?,
        Some(TransformData::RawMesh(items)) if items.is_empty() => {
            return empty_transform_output(image, size, fill, palette_fill);
        }
        Some(TransformData::RawMesh(items)) => flatten_raw_mesh_data(&items)?,
        Some(TransformData::Affine(items)) if items.is_empty() => {
            return empty_transform_output(image, size, fill, palette_fill);
        }
        Some(TransformData::Affine(_)) => {
            return Err(PilError::TypeError(
                "cannot unpack non-iterable int object".into(),
            ));
        }
        Some(TransformData::Text(value)) if value.is_empty() => {
            return empty_transform_output(image, size, fill, palette_fill);
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

fn empty_transform_output(
    image: &Image,
    size: (u32, u32),
    fill: (u8, u8, u8, u8),
    palette_fill: Option<u8>,
) -> Result<Image, PilError> {
    let mode = image.mode_identity()?;
    if mode == crate::raster::Mode::ImagePalette8 {
        // Pillow copies P's source palette immediately after allocating its
        // output, before method-data callbacks can mutate the source.
        return Ok(Image::new_palette_index_from_source(
            image,
            size.0,
            size.1,
            palette_fill.unwrap_or(fill.0),
        ));
    }
    // Transform normalizes two-band fill to `(luma, alpha, 0, 0)` for kernels,
    // while Image.new stores its alpha in the fourth slot.
    let image_fill = if matches!(
        mode,
        crate::raster::Mode::ImageLumaA8
            | crate::raster::Mode::ImageLaPremultiplied8
            | crate::raster::Mode::ImagePaletteAlpha8
    ) {
        (fill.0, 0, 0, fill.1)
    } else {
        fill
    };
    let mut output = Image::new(size.0, size.1, mode, image_fill)?;
    if mode == crate::raster::Mode::ImagePaletteAlpha8
        && let Image::Loaded(data) = &mut output
    {
        // PA stores its alpha with each pixel but keeps the RGB palette as
        // image metadata. Transform allocates a fresh raster and retains that
        // metadata just as Pillow's Image.transform does.
        data.palette = image.palette();
        data.palette_alpha = image.palette_alpha();
    }
    Ok(output)
}
