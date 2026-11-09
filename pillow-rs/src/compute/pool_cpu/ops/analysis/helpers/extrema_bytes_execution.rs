//! Prepared execution for the Rust/JS byte-valued GetExtrema API.

use super::super::ops::GET_EXTREMA_BYTES_SUPPORT;
use crate::compute::ExecutionContext;
use crate::compute::registry::support::{OperationId, StrategySelection};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(crate) struct PreparedGetExtremaBytes<'a> {
    image: &'a DynamicImage,
    mode: Mode,
    selection: StrategySelection,
}

impl PreparedGetExtremaBytes<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<Vec<(u8, u8)>, PilError> {
        if self.image.mode() != self.mode {
            return Err(PilError::InternalError(
                "byte-valued GetExtrema mode did not match its DynamicImage tag".into(),
            ));
        }
        let implementation = self.selection.implementation;
        if implementation.operation != OperationId::GetExtremaBytes
            || implementation.mode != self.mode
        {
            return Err(PilError::InternalError(
                "GetExtremaBytes strategy selection did not match its operation and mode".into(),
            ));
        }
        if implementation.architecture != crate::compute::Architecture::Default {
            return Err(PilError::InternalError(format!(
                "GetExtremaBytes has no {:?} {:?} strategy",
                self.mode, implementation.architecture,
            )));
        }
        execute_mode(self.image, self.selection)
    }
}

fn execute_mode(
    image: &DynamicImage,
    selection: StrategySelection,
) -> Result<Vec<(u8, u8)>, PilError> {
    match image {
        DynamicImage::One(image) => {
            super::super::one::ops::execute_getextrema_bytes(image, selection)
        }
        DynamicImage::ImageLuma8(image) => {
            super::super::luma8::ops::execute_getextrema_bytes(image, selection)
        }
        DynamicImage::ImageLumaA8(image) => {
            super::super::luma_a8::ops::execute_getextrema_bytes(image, selection)
        }
        DynamicImage::ImageLaPremultiplied8(image) => {
            super::super::la_premultiplied8::ops::execute_getextrema_bytes(image, selection)
        }
        DynamicImage::ImagePalette8(image) => {
            super::super::palette8::ops::execute_getextrema_bytes(image, selection)
        }
        DynamicImage::ImagePaletteAlpha8(image) => {
            super::super::palette_alpha8::ops::execute_getextrema_bytes(image, selection)
        }
        DynamicImage::ImageRgb8(image) => {
            super::super::rgb8::ops::execute_getextrema_bytes(image, selection)
        }
        DynamicImage::ImageRgba8(image) => {
            super::super::rgba8::ops::execute_getextrema_bytes(image, selection)
        }
        DynamicImage::ImageRgbPremultiplied8(image) => {
            super::super::rgb_premultiplied8::ops::execute_getextrema_bytes(image, selection)
        }
        DynamicImage::ImageRgbx8(image) => {
            super::super::rgbx8::ops::execute_getextrema_bytes(image, selection)
        }
        DynamicImage::ImageCmyk8(image) => {
            super::super::cmyk8::ops::execute_getextrema_bytes(image, selection)
        }
        DynamicImage::ImageYCbCr8(image) => {
            super::super::ycbcr8::ops::execute_getextrema_bytes(image, selection)
        }
        DynamicImage::ImageLab8(image) => {
            super::super::lab8::ops::execute_getextrema_bytes(image, selection)
        }
        DynamicImage::ImageHsv8(image) => {
            super::super::hsv8::ops::execute_getextrema_bytes(image, selection)
        }
        DynamicImage::ImageI32(image) => {
            super::super::i32::ops::execute_getextrema_bytes(image, selection)
        }
        DynamicImage::ImageF32(image) => {
            super::super::f32::ops::execute_getextrema_bytes(image, selection)
        }
        DynamicImage::ImageI16(image) => {
            super::super::i16::ops::execute_getextrema_bytes(image, selection)
        }
        DynamicImage::ImageI16L(image) => {
            super::super::i16l::ops::execute_getextrema_bytes(image, selection)
        }
        DynamicImage::ImageI16B(image) => {
            super::super::i16b::ops::execute_getextrema_bytes(image, selection)
        }
        DynamicImage::ImageI16N(image) => {
            super::super::i16n::ops::execute_getextrema_bytes(image, selection)
        }
        DynamicImage::ImageLumaA16(image) => {
            super::super::luma_a16::ops::execute_getextrema_bytes(image, selection)
        }
        DynamicImage::ImageRgb16(image) => {
            super::super::rgb16::ops::execute_getextrema_bytes(image, selection)
        }
        DynamicImage::ImageRgba16(image) => {
            super::super::rgba16::ops::execute_getextrema_bytes(image, selection)
        }
        DynamicImage::ImageRgb32F(image) => {
            super::super::rgb32f::ops::execute_getextrema_bytes(image, selection)
        }
        DynamicImage::ImageRgba32F(image) => {
            super::super::rgba32f::ops::execute_getextrema_bytes(image, selection)
        }
    }
}

pub(crate) fn prepare<'a>(
    image: &'a DynamicImage,
    context: &ExecutionContext,
) -> Result<PreparedGetExtremaBytes<'a>, PilError> {
    let mode = image.mode();
    let selection = GET_EXTREMA_BYTES_SUPPORT
        .for_mode(mode)
        .select_serial(context)?;
    Ok(PreparedGetExtremaBytes {
        image,
        mode,
        selection,
    })
}

/// Declares a mode-owned Default strategy for the byte-valued compatibility API.
macro_rules! declare_getextrema_bytes_owner {
    ($mode:ident, $image_ty:ty, $implementation:path $(,)?) => {
        pub(crate) fn execute_getextrema_bytes(
            image: &$image_ty,
            selection: crate::compute::registry::support::StrategySelection,
        ) -> Result<Vec<(u8, u8)>, crate::error::PilError> {
            let implementation = selection.implementation;
            if implementation.operation
                != crate::compute::registry::support::OperationId::GetExtremaBytes
                || implementation.mode != crate::raster::Mode::$mode
            {
                return Err(crate::error::PilError::InternalError(
                    "GetExtremaBytes strategy selection did not match its operation and mode"
                        .into(),
                ));
            }
            match implementation.architecture {
                crate::compute::Architecture::Default => Ok(($implementation)(image)),
                architecture => Err(crate::error::PilError::InternalError(format!(
                    "GetExtremaBytes has no {:?} {architecture:?} strategy",
                    crate::raster::Mode::$mode,
                ))),
            }
        }
    };
}

pub(crate) use declare_getextrema_bytes_owner;
pub(crate) const fn support(
    mode: Mode,
    core_only: bool,
) -> crate::compute::registry::support::ModeSupport {
    use crate::compute::registry::support::{
        InputContract, ModeSupport, OperationId, OutputModeRule,
    };
    const NO_INPUTS: &[InputContract] = &[];
    if core_only {
        ModeSupport::cpu_core_only(
            OperationId::GetExtremaBytes,
            mode,
            OutputModeRule::NoRasterResult,
            NO_INPUTS,
            &[],
            "Rust/JS byte-valued extrema preserves the exact-mode result contract",
        )
    } else {
        ModeSupport::cpu(
            OperationId::GetExtremaBytes,
            mode,
            OutputModeRule::NoRasterResult,
            NO_INPUTS,
            &[],
        )
    }
}

macro_rules! declare_getextrema_bytes_support {
    ($mode:ident, pillow $(,)?) => {
        pub(crate) const GET_EXTREMA_BYTES_SUPPORT:
            $crate::compute::registry::support::ModeSupport =
            $crate::compute::pool_cpu::ops::analysis::helpers::extrema_bytes_execution::support(
                $crate::raster::Mode::$mode,
                false,
            );
    };
    ($mode:ident, core_only $(,)?) => {
        pub(crate) const GET_EXTREMA_BYTES_SUPPORT:
            $crate::compute::registry::support::ModeSupport =
            $crate::compute::pool_cpu::ops::analysis::helpers::extrema_bytes_execution::support(
                $crate::raster::Mode::$mode,
                true,
            );
    };
}

pub(crate) use declare_getextrema_bytes_support;
