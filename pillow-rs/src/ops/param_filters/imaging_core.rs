//! Private ImagingCore callbacks used by parameterized filters.

use crate::compute::{Backend, ExecutionContext};
use crate::error::PilError;
use crate::image::Image;
use crate::raster::Mode;

use super::{cpu_only_filter_route_with_execution_route, validate_rank_filter_size};

impl Image {
    /// Replicates edge pixels for the private Pillow `ImagingCore.expand`
    /// callback used by user supplied image filters.
    ///
    /// This entry point preserves the exact raster mode and native samples;
    /// ordinary [`Image::expand`] continues to add a caller-selected fill.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn imaging_core_expand(&self, xmargin: i32, ymargin: i32) -> Result<Image, PilError> {
        self.imaging_core_expand_with_optional_execution_route(xmargin, ymargin, None)
    }

    /// Expands a filter callback image using its injected CPU request and Architecture.
    #[doc(hidden)]
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub fn imaging_core_expand_with_execution_route(
        &self,
        xmargin: i32,
        ymargin: i32,
        requested_backend: Option<Backend>,
        context: ExecutionContext,
    ) -> Result<Image, PilError> {
        self.imaging_core_expand_with_optional_execution_route(
            xmargin,
            ymargin,
            Some((requested_backend, context)),
        )
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn imaging_core_expand_with_optional_execution_route(
        &self,
        xmargin: i32,
        ymargin: i32,
        execution_route: Option<(Option<Backend>, ExecutionContext)>,
    ) -> Result<Image, PilError> {
        // TODO(pillow-rs#3ptb): freeze callback outcomes for negative margins
        // and empty inputs; verify selected/fallback receipts under the injected
        // callback Architecture in the frozen CPU matrix.
        if xmargin < 0 || ymargin < 0 {
            return Err(PilError::ValueError("bad kernel size".into()));
        }
        let (requested_backend, context, fallback_reason) =
            cpu_only_filter_route_with_execution_route(self, "ImagingCoreExpand", execution_route)?;
        let (image, mut operation_receipts) = self.materialized_shared_for_ops_with_receipts()?;
        let output =
            crate::compute::execute_eager_cpu_arch_image_operation_with_context_and_receipts(
                "ImagingCoreExpand",
                image.as_ref(),
                context,
                Some(requested_backend),
                fallback_reason,
                &mut operation_receipts,
                |context| {
                    let prepared =
                        crate::compute::pool_cpu::ops::imageops::helpers::expand_edges::prepare(
                            image.as_ref(),
                            xmargin as u32,
                            ymargin as u32,
                            context,
                        )?;
                    let selection = *prepared.selection();
                    Ok((prepared.execute()?, selection))
                },
            )?;
        self.from_derived_dynamic(output)
    }

    /// Runs the cropped single-band rank selector exposed by the private
    /// Pillow `ImagingCore.rankfilter` callback.
    ///
    /// Scalar modes compose their existing exact-mode rank strategy with a
    /// crop; P uses its separately declared native-index strategy.
    #[doc(hidden)]
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub fn imaging_core_rankfilter(&self, size: i32, rank: i32) -> Result<Image, PilError> {
        self.imaging_core_rankfilter_with_optional_execution_route(size, rank, None)
    }

    /// Runs ImagingCore rankfilter using its injected CPU request and Architecture.
    #[doc(hidden)]
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub fn imaging_core_rankfilter_with_execution_route(
        &self,
        size: i32,
        rank: i32,
        requested_backend: Option<Backend>,
        context: ExecutionContext,
    ) -> Result<Image, PilError> {
        self.imaging_core_rankfilter_with_optional_execution_route(
            size,
            rank,
            Some((requested_backend, context)),
        )
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn imaging_core_rankfilter_with_optional_execution_route(
        &self,
        size: i32,
        rank: i32,
        execution_route: Option<(Option<Backend>, ExecutionContext)>,
    ) -> Result<Image, PilError> {
        // TODO(pillow-rs#3ptb): freeze empty-input and error-order parity for
        // the private callback independently from public ImageFilter checks;
        // verify P's eager path uses the injected callback Architecture.
        let mode = self.mode_identity()?;
        if !matches!(
            mode,
            Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8 | Mode::ImageI32 | Mode::ImageF32
        ) {
            return Err(PilError::ValueError("image has wrong mode".into()));
        }
        if size < 0 {
            return Err(PilError::ValueError("bad filter size".into()));
        }
        let size = size as u32;
        validate_rank_filter_size(size)?;
        if rank < 0 {
            return Err(PilError::ValueError("bad rank value".into()));
        }
        let rank = rank as u32;
        let rank_count = size * size;
        if rank >= rank_count {
            return Err(PilError::ValueError("bad rank value".into()));
        }
        let (width, height) = self.size()?;
        let margin = size / 2;
        let right = width
            .checked_sub(margin)
            .ok_or_else(|| PilError::ValueError("bad image size".into()))?;
        let bottom = height
            .checked_sub(margin)
            .ok_or_else(|| PilError::ValueError("bad image size".into()))?;
        if size == 1 && (width == 0 || height == 0) {
            return Ok(self.copy());
        }

        if mode == Mode::ImagePalette8 {
            return self.imaging_core_palette_rankfilter(size, rank, execution_route);
        }
        self.rank_filter(size, rank)?
            .crop_unsigned((margin, margin, right, bottom))
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn imaging_core_palette_rankfilter(
        &self,
        size: u32,
        rank: u32,
        execution_route: Option<(Option<Backend>, ExecutionContext)>,
    ) -> Result<Image, PilError> {
        let (requested_backend, context, fallback_reason) =
            cpu_only_filter_route_with_execution_route(
                self,
                "ImagingCorePaletteRankFilter",
                execution_route,
            )?;
        let (image, mut operation_receipts) = self.materialized_shared_for_ops_with_receipts()?;
        let output =
            crate::compute::execute_eager_cpu_arch_image_operation_with_context_and_receipts(
                "ImagingCorePaletteRankFilter",
                image.as_ref(),
                context,
                Some(requested_backend),
                fallback_reason,
                &mut operation_receipts,
                |context| {
                    let prepared =
                        crate::compute::pool_cpu::ops::filter::palette8::ops::prepare_imaging_rank_filter(
                            image.as_ref(),
                            size,
                            rank,
                            context,
                        )?;
                    let selection = *prepared.selection();
                    Ok((prepared.execute()?, selection))
                },
            )?;
        self.from_derived_dynamic(output)
    }
}
