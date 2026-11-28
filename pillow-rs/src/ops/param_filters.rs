//! Parameterized Pillow image filters.
//!
//! Most methods return lazy pipeline operations. Filters that need multiple
//! passes or mode-specific CPU behavior may materialize immediately.

use crate::compute::{Backend, ExecutionContext};
use crate::error::PilError;
use crate::image::{Image, PalettedData};
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

mod imaging_core;

// VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
fn cpu_only_filter_route_with_execution_route(
    image: &Image,
    operation: &'static str,
    execution_route: Option<(Option<Backend>, ExecutionContext)>,
) -> Result<(Backend, ExecutionContext, Option<String>), PilError> {
    let (requested_backend, context) = match execution_route {
        Some(route) => route,
        None => image.cpu_arch_execution_route()?,
    };
    let routed_backend = requested_backend.unwrap_or(Backend::Cpu);
    let fallback_reason = (requested_backend == Some(Backend::Gpu))
        .then(|| format!("{operation} has no GPU strategy"));
    Ok((routed_backend, context, fallback_reason))
}

/// Formats the Pillow-compatible representation of a Color3DLUT filter.
pub fn color3dlut_repr(
    table_type: &str,
    size: (u32, u32, u32),
    channels: u32,
    target_mode: Option<&str>,
) -> String {
    let target = target_mode
        .map(|mode| format!(" target_mode={mode}"))
        .unwrap_or_default();
    format!(
        "<Color3DLUT from {table_type} size={}x{}x{} channels={channels}{target}>",
        size.0, size.1, size.2
    )
}

/// Host-neutral table input accepted by `ImageFilter.Color3DLUT`.
pub enum Color3DLutTable {
    /// A flat sequence of channel values.
    Flat(Vec<f64>),
    /// One channel-valued sequence per table entry.
    Nested(Vec<Vec<f64>>),
}

/// A table whose dimensions and channel count have passed the public
/// `Color3DLUT` constructor checks.
#[derive(Debug, Clone)]
pub struct PreparedColor3DLut {
    /// The three-dimensional table dimensions.
    pub size: (u32, u32, u32),
    /// Flattened table values in Pillow traversal order.
    pub table: Vec<f64>,
    /// Number of output channels stored at each table entry.
    pub channels: u32,
}

fn validate_color3dlut_size_tuple(size: (u32, u32, u32)) -> Result<(), PilError> {
    if !(2..=65).contains(&size.0) || !(2..=65).contains(&size.1) || !(2..=65).contains(&size.2) {
        return Err(PilError::ValueError(
            "Size should be in [2, 65] range.".into(),
        ));
    }
    Ok(())
}

fn validate_color3dlut_channels(channels: u32) -> Result<(), PilError> {
    if channels != 3 && channels != 4 {
        return Err(PilError::ValueError(
            "Only 3 or 4 output channels are supported".into(),
        ));
    }
    Ok(())
}

fn color3dlut_expected_len(size: (u32, u32, u32), channels: u32) -> usize {
    size.0 as usize * size.1 as usize * size.2 as usize * channels as usize
}

fn color3dlut_table_length_error(
    size: (u32, u32, u32),
    channels: u32,
    actual_len: usize,
) -> PilError {
    PilError::ValueError(format!(
        "The table should have either channels * size**3 float items or size**3 items of channels-sized tuples with floats. Table should be: {}x{}x{}x{}. Actual length: {}",
        channels, size.0, size.1, size.2, actual_len
    ))
}

/// Validates the odd square window required by Pillow's rank-family filters.
///
/// The Python `MaxFilter`, `MinFilter`, `MedianFilter`, and `RankFilter`
/// constructors retain the caller's size.  Their native filter call rejects
/// zero and even sizes; silently rounding those values changes the operation
/// instead of reporting the same public error. Pillow's rank kernel also
/// bounds the float32 workspace against C `INT_MAX` before allocation.
fn validate_rank_filter_size(size: u32) -> Result<(), PilError> {
    if size == 0 || size.is_multiple_of(2) {
        return Err(PilError::ValueError("bad filter size".into()));
    }
    let area = u64::from(size) * u64::from(size);
    let max_float_workspace_samples = i32::MAX as u64 / std::mem::size_of::<f32>() as u64;
    if area > max_float_workspace_samples {
        return Err(PilError::ValueError("filter size too large".into()));
    }
    Ok(())
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
fn validate_box_blur_radii(radius_x: f32, radius_y: f32) -> Result<(), PilError> {
    if !(radius_x >= 0.0) || !(radius_y >= 0.0) {
        return Err(PilError::ValueError("radius must be >= 0".into()));
    }
    if radius_x >= 2_147_483_648.0 || radius_y >= 2_147_483_648.0 {
        return Err(PilError::ValueError("radius too large".into()));
    }
    Ok(())
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
fn push_gaussian_blur_operation(
    image: &Image,
    sigma_x: f32,
    sigma_y: f32,
    radius_x: f32,
    radius_y: f32,
    passes: u32,
) -> Image {
    if passes == 3 && sigma_x == sigma_y {
        return Image::push_op(image, PipelineOp::GaussianBlur { sigma: sigma_x });
    }
    Image::push_op(
        image,
        PipelineOp::BoxBlurXY {
            radius_x,
            radius_y,
            passes,
        },
    )
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
fn push_box_blur_operation(image: &Image, radius_x: f32, radius_y: f32, passes: u32) -> Image {
    if passes == 1 && radius_x == radius_y && radius_x.fract() == 0.0 && radius_x <= u32::MAX as f32
    {
        return Image::push_op(
            image,
            PipelineOp::BoxBlur {
                radius: radius_x as u32,
            },
        );
    }
    Image::push_op(
        image,
        PipelineOp::BoxBlurXY {
            radius_x,
            radius_y,
            passes,
        },
    )
}

/// Apply the slice-assignment semantics used by Pillow's Python callbacks.
///
/// Pillow starts with a zero-filled table and assigns each callback result to
/// a fixed-width slice. A result with the wrong length can therefore resize
/// the list, including the append behavior when a shortened list moves the
/// next slice beyond its end. Preserve that behavior before the constructor
/// performs the final exact-length validation.
fn color3dlut_assign_callback_values(
    table: &mut Vec<f64>,
    index: usize,
    width: usize,
    values: Vec<f64>,
) {
    let start = index.min(table.len());
    let end = index.saturating_add(width).min(table.len());
    table.splice(start..end, values);
}

/// Validates and normalizes the user-facing Color3DLUT size argument.
pub fn color3dlut_check_size(values: &[f64]) -> Result<(u32, u32, u32), PilError> {
    let dimensions = match values {
        [value] => [*value as i32; 3],
        [s1, s2, s3] => [*s1 as i32, *s2 as i32, *s3 as i32],
        _ => {
            return Err(PilError::ValueError(
                "Size should be either an integer or a tuple of three integers.".into(),
            ));
        }
    };
    let size = (
        dimensions[0] as u32,
        dimensions[1] as u32,
        dimensions[2] as u32,
    );
    validate_color3dlut_size_tuple(size)?;
    Ok(size)
}

/// Validates and flattens a Color3DLUT table.
pub fn color3dlut_prepare_table(
    table: Color3DLutTable,
    size: (u32, u32, u32),
    channels: u32,
) -> Result<Vec<f64>, PilError> {
    validate_color3dlut_size_tuple(size)?;
    validate_color3dlut_channels(channels)?;
    let expected_len = color3dlut_expected_len(size, channels);
    let table = match table {
        Color3DLutTable::Flat(table) => table,
        Color3DLutTable::Nested(table) => {
            let mut flat = Vec::with_capacity(expected_len);
            for pixel in table {
                if pixel.len() != channels as usize {
                    return Err(PilError::ValueError(format!(
                        "The elements of the table should have a length of {}.",
                        channels
                    )));
                }
                flat.extend(pixel);
            }
            flat
        }
    };
    if table.is_empty() || table.len() != expected_len {
        return Err(color3dlut_table_length_error(size, channels, table.len()));
    }
    Ok(table)
}

/// Validates a flat table once at the Rust public boundary.
pub fn prepare_color3dlut(
    table: Vec<f64>,
    size: (u32, u32, u32),
    channels: u32,
) -> Result<PreparedColor3DLut, PilError> {
    let table = color3dlut_prepare_table(Color3DLutTable::Flat(table), size, channels)?;
    Ok(PreparedColor3DLut {
        size,
        table,
        channels,
    })
}

/// Prepares the table accepted by Pillow's low-level `ImagingCore` adapter.
///
/// Unlike the public `Color3DLUT` constructor, the private callback parser
/// accepts one and two channels, prepares the table, and then rejects those
/// channel counts in the native filter with a distinct error. Keep that
/// observable order at this boundary while returning an executable prepared
/// value only for the supported three/four-channel cases.
pub fn prepare_color3dlut_imaging_core(
    table: Vec<f64>,
    size: (u32, u32, u32),
    channels: u32,
) -> Result<PreparedColor3DLut, PilError> {
    if !(1..=4).contains(&channels) {
        return Err(PilError::ValueError(
            "table_channels should be from 1 to 4".into(),
        ));
    }
    if !(2..=65).contains(&size.0) || !(2..=65).contains(&size.1) || !(2..=65).contains(&size.2) {
        return Err(PilError::ValueError(
            "Table size in any dimension should be from 2 to 65".into(),
        ));
    }
    let expected_len = color3dlut_expected_len(size, channels);
    if table.len() != expected_len {
        return Err(PilError::ValueError(
            "The table should have table_channels * size1D * size2D * size3D float items.".into(),
        ));
    }
    if channels < 3 {
        return Err(PilError::ValueError(
            "table_channels could be 3 or 4".into(),
        ));
    }
    Ok(PreparedColor3DLut {
        size,
        table,
        channels,
    })
}

/// Generates a Color3DLUT table while keeping traversal and callback result
/// policy in the runtime-independent core.
pub fn color3dlut_generate_table<E, F, M>(
    size: (u32, u32, u32),
    channels: u32,
    mut callback: F,
    map_error: M,
) -> Result<Vec<f64>, E>
where
    F: FnMut(&[f64]) -> Result<Vec<f64>, E>,
    M: Fn(PilError) -> E,
{
    validate_color3dlut_size_tuple(size).map_err(&map_error)?;
    validate_color3dlut_channels(channels).map_err(map_error)?;

    let (s1, s2, s3) = size;
    let channel_count = channels as usize;
    let mut table = vec![0.0; color3dlut_expected_len(size, channels)];
    let mut index = 0;
    for b in 0..s3 {
        for g in 0..s2 {
            for r in 0..s1 {
                let args = [
                    r as f64 / (s1 - 1) as f64,
                    g as f64 / (s2 - 1) as f64,
                    b as f64 / (s3 - 1) as f64,
                ];
                let values = callback(&args)?;
                color3dlut_assign_callback_values(&mut table, index, channel_count, values);
                index += channel_count;
            }
        }
    }
    Ok(table)
}

/// Transforms a Color3DLUT table while keeping traversal and callback result
/// validation in the runtime-independent core.
pub fn color3dlut_transform_table<E, F, M>(
    input: &PreparedColor3DLut,
    channels_out: Option<u32>,
    with_normals: bool,
    mut callback: F,
    map_error: M,
) -> Result<(Vec<f64>, u32), E>
where
    F: FnMut(&[f64]) -> Result<Vec<f64>, E>,
    M: Fn(PilError) -> E,
{
    let size = input.size;
    let channels_in = input.channels;
    let table = &input.table;
    let channels_out = channels_out.unwrap_or(channels_in);
    validate_color3dlut_channels(channels_out).map_err(map_error)?;

    let (s1, s2, s3) = size;
    let input_channels = channels_in as usize;
    let output_channels = channels_out as usize;
    let mut output = vec![0.0; color3dlut_expected_len(size, channels_out)];
    let mut index_in = 0usize;
    let mut index_out = 0usize;
    let mut output_layout_unchanged = true;
    for b in 0..s3 {
        for g in 0..s2 {
            for r in 0..s1 {
                let values = &table[index_in..index_in + input_channels];
                let new_values = if with_normals {
                    // Valid LUTs have three or four channels. Keep callback
                    // arguments on the stack instead of allocating a Vec for
                    // every entry; retain a dynamic fallback for manually
                    // constructed PreparedColor3DLut values with more bands.
                    if input_channels <= 4 {
                        let mut args = [0.0; 7];
                        args[0] = r as f64 / (s1 - 1) as f64;
                        args[1] = g as f64 / (s2 - 1) as f64;
                        args[2] = b as f64 / (s3 - 1) as f64;
                        args[3..3 + input_channels].copy_from_slice(values);
                        callback(&args[..3 + input_channels])?
                    } else {
                        let mut args = Vec::with_capacity(input_channels + 3);
                        args.extend([
                            r as f64 / (s1 - 1) as f64,
                            g as f64 / (s2 - 1) as f64,
                            b as f64 / (s3 - 1) as f64,
                        ]);
                        args.extend_from_slice(values);
                        callback(&args)?
                    }
                } else {
                    callback(values)?
                };

                // Most callbacks return exactly the requested channel count.
                // Write that common case in place; use the original Vec splice
                // semantics for an incorrectly sized result so Pillow's later
                // constructor validation still sees the same resized table.
                let output_end = index_out.checked_add(output_channels).filter(|end| {
                    output_layout_unchanged
                        && new_values.len() == output_channels
                        && *end <= output.len()
                });
                if let Some(output_end) = output_end {
                    output[index_out..output_end].copy_from_slice(&new_values);
                } else {
                    output_layout_unchanged = false;
                    color3dlut_assign_callback_values(
                        &mut output,
                        index_out,
                        output_channels,
                        new_values,
                    );
                }
                index_in += input_channels;
                index_out += output_channels;
            }
        }
    }
    Ok((output, channels_out))
}

impl Image {
    /// Applies Gaussian blur with the given radius.
    ///
    /// Larger radius produces more blur.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] for unsupported modes; deferred execution can
    /// report radius and materialization failures.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    pub fn gaussian_blur(&self, radius: f32) -> Result<Image, PilError> {
        self.gaussian_blur_xy_with_passes(radius, radius, 3)
    }

    /// Applies Gaussian blur with independent horizontal and vertical radii.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    pub fn gaussian_blur_xy(&self, radius_x: f32, radius_y: f32) -> Result<Image, PilError> {
        self.gaussian_blur_xy_with_passes(radius_x, radius_y, 3)
    }

    /// Applies Pillow's Gaussian approximation using the requested box-pass count.
    ///
    /// Known-empty inputs copy before pass, radius, and mode validation.
    /// Nonempty inputs reject invalid pass counts and radii before mode checks.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] for invalid pass counts, radii, or modes.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    pub fn gaussian_blur_xy_with_passes(
        &self,
        sigma_x: f32,
        sigma_y: f32,
        passes: i32,
    ) -> Result<Image, PilError> {
        if self
            .known_size()
            .is_some_and(|(width, height)| width == 0 || height == 0)
        {
            return Ok(self.copy());
        }
        if passes < 1 {
            return Err(PilError::ValueError(
                "number of passes must be greater than zero".into(),
            ));
        }
        let passes_u32 = passes as u32;
        let radius_x = crate::compute::helpers::filters::gaussian_blur_radius(sigma_x, passes_u32);
        let radius_y = crate::compute::helpers::filters::gaussian_blur_radius(sigma_y, passes_u32);
        validate_box_blur_radii(radius_x, radius_y)?;
        self.validate_filter("GaussianBlur")?;

        Ok(push_gaussian_blur_operation(
            self, sigma_x, sigma_y, radius_x, radius_y, passes_u32,
        ))
    }

    /// Applies box blur with a uniform kernel radius.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] for unsupported modes or invalid radii; deferred
    /// pipeline execution can report later materialization failures.
    pub fn box_blur(&self, radius: f32) -> Result<Image, PilError> {
        self.box_blur_xy(radius, radius)
    }

    /// Applies box blur with independent horizontal and vertical radii.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    pub fn box_blur_xy(&self, radius_x: f32, radius_y: f32) -> Result<Image, PilError> {
        self.box_blur_xy_with_passes(radius_x, radius_y, 1)
    }

    /// Applies box blur with independent radii and the requested pass count.
    ///
    /// Known-empty inputs copy before pass, radius, and mode validation.
    /// Nonempty inputs reject invalid pass counts and radii before mode checks.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] for invalid pass counts, radii, or modes.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    pub fn box_blur_xy_with_passes(
        &self,
        radius_x: f32,
        radius_y: f32,
        passes: i32,
    ) -> Result<Image, PilError> {
        if self
            .known_size()
            .is_some_and(|(width, height)| width == 0 || height == 0)
        {
            return Ok(self.copy());
        }
        if passes < 1 {
            return Err(PilError::ValueError(
                "number of passes must be greater than zero".into(),
            ));
        }
        validate_box_blur_radii(radius_x, radius_y)?;
        self.validate_filter("BoxBlur")?;

        Ok(push_box_blur_operation(
            self,
            radius_x,
            radius_y,
            passes as u32,
        ))
    }

    /// Applies Pillow-style unsharp masking.
    ///
    /// `radius` controls blur amount, `percent` controls strength (150 = 150%),
    /// `threshold` is minimum difference to apply.
    /// Uses PIL-style GaussianBlur for the blurred version.
    /// Handles any number of channels (L=1, LA=2, RGB=3, RGBA=4).
    /// Uses PIL's exact integer arithmetic: `clip8(original + diff * percent / 100)`.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization, blur execution, allocation
    /// checks, or raw image reconstruction fails.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    pub fn unsharp_mask(
        &self,
        radius: f32,
        percent: i32,
        threshold: i32,
    ) -> Result<Image, PilError> {
        self.unsharp_mask_with_optional_execution_route(radius, percent, threshold, None)
    }

    /// Applies unsharp masking with a route captured by a composed caller.
    // TODO(pillow-rs#3ptb): verify nested ImageFilter route receipts and
    // fallback behavior across the frozen CPU mode/architecture matrix.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn unsharp_mask_with_execution_route(
        &self,
        radius: f32,
        percent: i32,
        threshold: i32,
        requested_backend: Option<Backend>,
        context: ExecutionContext,
    ) -> Result<Image, PilError> {
        self.unsharp_mask_with_optional_execution_route(
            radius,
            percent,
            threshold,
            Some((requested_backend, context)),
        )
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn unsharp_mask_with_optional_execution_route(
        &self,
        radius: f32,
        percent: i32,
        threshold: i32,
        execution_route: Option<(Option<Backend>, ExecutionContext)>,
    ) -> Result<Image, PilError> {
        self.validate_filter("UnsharpMask")?;
        let (routed_backend, context, fallback_reason) =
            cpu_only_filter_route_with_execution_route(self, "UnsharpMask", execution_route)?;
        let (img, mut operation_receipts) = self.materialized_shared_for_ops_with_receipts()?;
        let mode = img.mode();
        let result =
            crate::compute::execute_eager_cpu_arch_image_operation_with_context_and_receipts(
                "UnsharpMask",
                img.as_ref(),
                context,
                Some(routed_backend),
                fallback_reason,
                &mut operation_receipts,
                |context| {
                    let prepared =
                        crate::compute::pool_cpu::ops::filter::ops::prepare_unsharp_mask(
                            img.as_ref(),
                            radius,
                            percent,
                            threshold,
                            context,
                        )?;
                    let selection = *prepared.selection();
                    Ok((prepared.execute()?, selection))
                },
            )?;

        Ok(Image::from_dynamic_with_mode(result, Some(mode))?)
    }

    /// Applies a maximum filter over an odd neighborhood.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] for invalid radii or unsupported modes; deferred
    /// pipeline execution can report later materialization failures.
    pub fn max_filter(&self, size: u32) -> Result<Image, PilError> {
        self.validate_filter("MaxFilter")?;
        validate_rank_filter_size(size)?;
        Ok(Image::push_op(self, PipelineOp::MaxFilter { size }))
    }

    /// Applies a minimum filter over an odd neighborhood.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(Image)`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn min_filter(&self, size: u32) -> Result<Image, PilError> {
        self.validate_filter("MinFilter")?;
        validate_rank_filter_size(size)?;
        Ok(Image::push_op(self, PipelineOp::MinFilter { size }))
    }

    /// Applies a median filter over an odd neighborhood.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(Image)`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn median_filter(&self, size: u32) -> Result<Image, PilError> {
        self.validate_filter("MedianFilter")?;
        validate_rank_filter_size(size)?;
        Ok(Image::push_op(self, PipelineOp::MedianFilter { size }))
    }

    /// Applies a mode filter over an odd neighborhood.
    ///
    /// Each pixel becomes the most common value in its neighborhood when that
    /// value occurs more than twice; otherwise the original pixel is preserved.
    ///
    /// Pillow's public `Image.filter` accepts `1`, `L`, and `P` directly and
    /// splits `LA`, `La`, `PA`, `RGB`, `RGBA`, `RGBa`, `RGBX`, `CMYK`, `YCbCr`,
    /// `LAB`, and `HSV` into L bands before filtering and merging them. The CPU
    /// implementation applies the same independent byte-channel strategy while
    /// preserving the exact mode. P mode filters palette indices and retains
    /// its palette. The kernel uses strict `>` tie-breaking (the lower value
    /// wins), keeps the center sample when the maximum count is at most two,
    /// and skips samples outside the image boundary.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization, allocation checks, or raw image
    /// reconstruction fails.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    pub fn mode_filter(&self, size: u32) -> Result<Image, PilError> {
        self.mode_filter_with_optional_execution_route(size, None)
    }

    /// Applies the mode filter with a route captured by a composed caller.
    // TODO(pillow-rs#3ptb): verify nested ImageFilter route receipts and
    // fallback behavior across the frozen CPU mode/architecture matrix.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn mode_filter_with_execution_route(
        &self,
        size: u32,
        requested_backend: Option<Backend>,
        context: ExecutionContext,
    ) -> Result<Image, PilError> {
        self.mode_filter_with_optional_execution_route(size, Some((requested_backend, context)))
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn mode_filter_with_optional_execution_route(
        &self,
        size: u32,
        execution_route: Option<(Option<Backend>, ExecutionContext)>,
    ) -> Result<Image, PilError> {
        self.validate_filter("Mode")?;
        // Pillow's modefilter uses `size / 2` as its radius.  Positive even
        // sizes therefore have the same neighborhood as the next odd size,
        // while zero and negative Python inputs are identity operations.
        // Keep zero distinct from the normalized rank-family filters.
        let size = if size <= 1 { size } else { size | 1 };

        let source_mode = self.mode_identity()?;
        let palette = self.palette();
        let palette_alpha = self.palette_alpha();
        let (image, mut operation_receipts) = self.materialized_shared_for_ops_with_receipts()?;
        let (routed_backend, context, fallback_reason) =
            cpu_only_filter_route_with_execution_route(self, "ModeFilter", execution_route)?;
        let result =
            crate::compute::execute_eager_cpu_arch_image_operation_with_context_and_receipts(
                "ModeFilter",
                image.as_ref(),
                context,
                Some(routed_backend),
                fallback_reason,
                &mut operation_receipts,
                |context| {
                    let prepared = crate::compute::pool_cpu::ops::filter::ops::prepare_mode_filter(
                        image.as_ref(),
                        size,
                        context,
                    )?;
                    let selection = *prepared.selection();
                    Ok((prepared.execute()?, selection))
                },
            )?;

        if source_mode == Mode::ImagePalette8 {
            let DynamicImage::ImagePalette8(indices) = result else {
                return Err(PilError::InternalError(
                    "ModeFilter changed palette index mode".into(),
                ));
            };
            // A palette image materializes as one-band indices, so the raw
            // output remains an exact P raster rather than a tagged L carrier.
            if let Some(palette) = palette {
                return Ok(Image::Paletted(PalettedData {
                    indices: DynamicImage::ImagePalette8(indices),
                    palette_attached: true,
                    palette,
                    palette_data: self.palette_data().cloned().unwrap_or_else(|| {
                        crate::image::PaletteData::from_alpha(
                            palette_alpha.as_deref().unwrap_or_default(),
                        )
                    }),
                    palette_alpha: palette_alpha.unwrap_or_default(),
                    source_format: None,
                    info: None,
                    exif: None,
                    materialized: crate::image::materialization_cache(),
                }));
            }
            return Ok(Image::from_dynamic(DynamicImage::ImagePalette8(indices)));
        }
        Ok(Image::from_dynamic(result))
    }

    /// Applies a rank filter over an odd neighborhood.
    ///
    /// Each pixel becomes the `rank`-th value after sorting the neighborhood.
    ///
    /// The private `ImagingCore.rankfilter` callback has a cropped result and
    /// is exposed separately by [`Image::imaging_core_rankfilter`].
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(Image)`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn rank_filter(&self, size: u32, rank: u32) -> Result<Image, PilError> {
        self.validate_filter("RankFilter")?;
        validate_rank_filter_size(size)?;
        let window = size * size;
        if rank >= window {
            return Err(PilError::ValueError("bad rank value".into()));
        }
        Ok(Image::push_op(self, PipelineOp::RankFilter { size, rank }))
    }

    /// Applies a 3D color lookup table with trilinear interpolation.
    ///
    /// `size` is the LUT grid size, `table` contains the LUT values, and
    /// `channels` is the number of output channels.
    /// Core-only YCbCr, LAB, and HSV sources require an explicit RGB, RGBA, or
    /// CMYK `target_mode`.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(Image)`; LUT validation is handled during pipeline
    /// execution.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    pub fn color3dlut(
        &self,
        input: PreparedColor3DLut,
        target_mode: Option<crate::raster::Mode>,
    ) -> Result<Image, PilError> {
        let source_mode = self.mode_identity()?;
        let source_support = crate::compute::registry::support::OperationId::Color3DLut
            .support()
            .for_mode(source_mode);
        let PreparedColor3DLut {
            size,
            table,
            channels,
        } = input;
        let target_mode = target_mode.unwrap_or(source_mode);
        let facts = crate::compute::registry::support::StrategyInputs {
            width: 0,
            height: 0,
            byte_len: None,
            palette: crate::compute::registry::support::PaletteKnowledge::Absent,
            request: crate::compute::registry::support::RequestFacts::Unspecified,
        }
        .with_color3dlut(target_mode, channels, size, table.len());
        if source_support.assess_inputs(facts)?.is_some() {
            return Err(PilError::InternalError(
                "Color3DLut input contract remained unresolved before queuing".into(),
            ));
        }

        // Color3DLUT records the exact output identity before lazy execution.
        Ok(Image::push_mode_changing_op(
            self,
            PipelineOp::Color3DLut {
                size,
                table: table.into(),
                channels,
                source_mode,
                target_mode,
            },
            target_mode,
        ))
    }
}
