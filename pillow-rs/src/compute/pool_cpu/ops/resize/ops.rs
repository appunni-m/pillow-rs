//! Resize operation contracts and prepared mode-strategy selection.
//!
//! TODO(pillow-rs#3ptb): route direct/eager and nested geometry callers through
//! prepared plans, then verify boxed-bound validation order against the
//! preserved CPU behavior. One and P keep nearest-only vector strategies and
//! use their serial mode-specific Default routes for filtered requests. Rayon
//! scheduling belongs to the later Parallel CPU backend.

use crate::compute::{
    Architecture, ExecutionContext,
    registry::support::{
        ImplementationSupport, InputContract, ModeSupport, OperationId, OperationInputs,
        OperationSupport, OutputModeRule, StrategyGuard, StrategyInputs, StrategySelection,
        SupportAssessment,
    },
};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::pipeline::ResampleFilter;
use crate::raster::{DynamicImage, Mode};

const RESIZE_INPUTS: &[InputContract] = &[];

const fn native_byte_nearest_architectures(
    mode: Mode,
    channels: u8,
) -> [ImplementationSupport; 3] {
    [
        ImplementationSupport::cpu(
            OperationId::Resize,
            mode,
            Architecture::Sse2,
            cfg!(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2")),
            &[Architecture::Sse2],
            StrategyGuard::ResizeByteNearestVector { channels },
        ),
        ImplementationSupport::cpu(
            OperationId::Resize,
            mode,
            Architecture::Neon,
            cfg!(all(target_arch = "aarch64", target_feature = "neon")),
            &[Architecture::Neon],
            StrategyGuard::ResizeByteNearestVector { channels },
        ),
        ImplementationSupport::cpu(
            OperationId::Resize,
            mode,
            Architecture::WasmSimd128,
            cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
            &[Architecture::WasmSimd128],
            StrategyGuard::ResizeByteNearestVector { channels },
        ),
    ]
}

const fn native_byte_filtered_architectures(
    mode: Mode,
    channels: u8,
) -> [ImplementationSupport; 3] {
    [
        ImplementationSupport::cpu(
            OperationId::Resize,
            mode,
            Architecture::Sse2,
            cfg!(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2")),
            &[Architecture::Sse2],
            StrategyGuard::ResizeByteNativeVector { channels },
        ),
        ImplementationSupport::cpu(
            OperationId::Resize,
            mode,
            Architecture::Neon,
            cfg!(all(target_arch = "aarch64", target_feature = "neon")),
            &[Architecture::Neon],
            StrategyGuard::ResizeByteNativeVector { channels },
        ),
        ImplementationSupport::cpu(
            OperationId::Resize,
            mode,
            Architecture::WasmSimd128,
            cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
            &[Architecture::WasmSimd128],
            StrategyGuard::ResizeByteNativeVector { channels },
        ),
    ]
}

const fn boxed_byte_mode_architectures(
    mode: Mode,
    channels: u8,
    nearest_only: bool,
) -> [ImplementationSupport; 3] {
    let guard = if nearest_only {
        StrategyGuard::ResizeByteBoxedNearestVector { channels }
    } else {
        StrategyGuard::ResizeByteBoxedVector { channels }
    };
    [
        ImplementationSupport::cpu(
            OperationId::ResizeBoxed,
            mode,
            Architecture::Sse2,
            cfg!(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2")),
            &[Architecture::Sse2],
            guard,
        ),
        ImplementationSupport::cpu(
            OperationId::ResizeBoxed,
            mode,
            Architecture::Neon,
            cfg!(all(target_arch = "aarch64", target_feature = "neon")),
            &[Architecture::Neon],
            guard,
        ),
        ImplementationSupport::cpu(
            OperationId::ResizeBoxed,
            mode,
            Architecture::WasmSimd128,
            cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
            &[Architecture::WasmSimd128],
            guard,
        ),
    ]
}

static RESIZE_ONE_ARCHITECTURES: [ImplementationSupport; 3] =
    native_byte_nearest_architectures(Mode::One, 1);
static RESIZE_LUMA8_ARCHITECTURES: [ImplementationSupport; 3] = [
    ImplementationSupport::cpu(
        OperationId::Resize,
        Mode::ImageLuma8,
        Architecture::Sse2,
        cfg!(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2")),
        &[Architecture::Sse2],
        StrategyGuard::ResizeByteNativeVector { channels: 1 },
    ),
    ImplementationSupport::cpu(
        OperationId::Resize,
        Mode::ImageLuma8,
        Architecture::Neon,
        cfg!(all(target_arch = "aarch64", target_feature = "neon")),
        &[Architecture::Neon],
        StrategyGuard::ResizeByteNativeVector { channels: 1 },
    ),
    ImplementationSupport::cpu(
        OperationId::Resize,
        Mode::ImageLuma8,
        Architecture::WasmSimd128,
        cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
        &[Architecture::WasmSimd128],
        StrategyGuard::ResizeByteNativeVector { channels: 1 },
    ),
];
static RESIZE_LUMA_A8_ARCHITECTURES: [ImplementationSupport; 3] =
    native_byte_filtered_architectures(Mode::ImageLumaA8, 2);
static RESIZE_LA_PREMULTIPLIED8_ARCHITECTURES: [ImplementationSupport; 3] =
    native_byte_filtered_architectures(Mode::ImageLaPremultiplied8, 2);
static RESIZE_PALETTE8_ARCHITECTURES: [ImplementationSupport; 3] =
    native_byte_nearest_architectures(Mode::ImagePalette8, 1);
static RESIZE_PALETTE_ALPHA8_ARCHITECTURES: [ImplementationSupport; 3] =
    native_byte_filtered_architectures(Mode::ImagePaletteAlpha8, 2);
static RESIZE_RGB8_ARCHITECTURES: [ImplementationSupport; 3] =
    native_byte_filtered_architectures(Mode::ImageRgb8, 3);
static RESIZE_RGBA8_ARCHITECTURES: [ImplementationSupport; 3] =
    native_byte_filtered_architectures(Mode::ImageRgba8, 4);
static RESIZE_RGB_PREMULTIPLIED8_ARCHITECTURES: [ImplementationSupport; 3] =
    native_byte_filtered_architectures(Mode::ImageRgbPremultiplied8, 4);
static RESIZE_RGBX8_ARCHITECTURES: [ImplementationSupport; 3] =
    native_byte_filtered_architectures(Mode::ImageRgbx8, 4);
static RESIZE_CMYK8_ARCHITECTURES: [ImplementationSupport; 3] =
    native_byte_filtered_architectures(Mode::ImageCmyk8, 4);
static RESIZE_YCBCR8_ARCHITECTURES: [ImplementationSupport; 3] =
    native_byte_filtered_architectures(Mode::ImageYCbCr8, 3);
static RESIZE_LAB8_ARCHITECTURES: [ImplementationSupport; 3] =
    native_byte_filtered_architectures(Mode::ImageLab8, 3);
static RESIZE_HSV8_ARCHITECTURES: [ImplementationSupport; 3] =
    native_byte_filtered_architectures(Mode::ImageHsv8, 3);

static RESIZE_BOXED_ONE_ARCHITECTURES: [ImplementationSupport; 3] =
    boxed_byte_mode_architectures(Mode::One, 1, true);
static RESIZE_BOXED_LUMA8_ARCHITECTURES: [ImplementationSupport; 3] =
    boxed_byte_mode_architectures(Mode::ImageLuma8, 1, false);
static RESIZE_BOXED_LUMA_A8_ARCHITECTURES: [ImplementationSupport; 3] =
    boxed_byte_mode_architectures(Mode::ImageLumaA8, 2, false);
static RESIZE_BOXED_LA_PREMULTIPLIED8_ARCHITECTURES: [ImplementationSupport; 3] =
    boxed_byte_mode_architectures(Mode::ImageLaPremultiplied8, 2, false);
static RESIZE_BOXED_PALETTE8_ARCHITECTURES: [ImplementationSupport; 3] =
    boxed_byte_mode_architectures(Mode::ImagePalette8, 1, true);
static RESIZE_BOXED_PALETTE_ALPHA8_ARCHITECTURES: [ImplementationSupport; 3] =
    boxed_byte_mode_architectures(Mode::ImagePaletteAlpha8, 2, false);
static RESIZE_BOXED_RGB8_ARCHITECTURES: [ImplementationSupport; 3] =
    boxed_byte_mode_architectures(Mode::ImageRgb8, 3, false);
static RESIZE_BOXED_RGBA8_ARCHITECTURES: [ImplementationSupport; 3] =
    boxed_byte_mode_architectures(Mode::ImageRgba8, 4, false);
static RESIZE_BOXED_RGB_PREMULTIPLIED8_ARCHITECTURES: [ImplementationSupport; 3] =
    boxed_byte_mode_architectures(Mode::ImageRgbPremultiplied8, 4, false);
static RESIZE_BOXED_RGBX8_ARCHITECTURES: [ImplementationSupport; 3] =
    boxed_byte_mode_architectures(Mode::ImageRgbx8, 4, false);
static RESIZE_BOXED_CMYK8_ARCHITECTURES: [ImplementationSupport; 3] =
    boxed_byte_mode_architectures(Mode::ImageCmyk8, 4, false);
static RESIZE_BOXED_YCBCR8_ARCHITECTURES: [ImplementationSupport; 3] =
    boxed_byte_mode_architectures(Mode::ImageYCbCr8, 3, false);
static RESIZE_BOXED_LAB8_ARCHITECTURES: [ImplementationSupport; 3] =
    boxed_byte_mode_architectures(Mode::ImageLab8, 3, false);
static RESIZE_BOXED_HSV8_ARCHITECTURES: [ImplementationSupport; 3] =
    boxed_byte_mode_architectures(Mode::ImageHsv8, 3, false);

fn native_byte_architectures(mode: Mode) -> Option<&'static [ImplementationSupport]> {
    match mode {
        Mode::ImageLuma8 => Some(&RESIZE_LUMA8_ARCHITECTURES),
        Mode::One => Some(&RESIZE_ONE_ARCHITECTURES),
        Mode::ImageLumaA8 => Some(&RESIZE_LUMA_A8_ARCHITECTURES),
        Mode::ImageLaPremultiplied8 => Some(&RESIZE_LA_PREMULTIPLIED8_ARCHITECTURES),
        Mode::ImagePalette8 => Some(&RESIZE_PALETTE8_ARCHITECTURES),
        Mode::ImagePaletteAlpha8 => Some(&RESIZE_PALETTE_ALPHA8_ARCHITECTURES),
        Mode::ImageRgb8 => Some(&RESIZE_RGB8_ARCHITECTURES),
        Mode::ImageRgba8 => Some(&RESIZE_RGBA8_ARCHITECTURES),
        Mode::ImageRgbPremultiplied8 => Some(&RESIZE_RGB_PREMULTIPLIED8_ARCHITECTURES),
        Mode::ImageRgbx8 => Some(&RESIZE_RGBX8_ARCHITECTURES),
        Mode::ImageCmyk8 => Some(&RESIZE_CMYK8_ARCHITECTURES),
        Mode::ImageYCbCr8 => Some(&RESIZE_YCBCR8_ARCHITECTURES),
        Mode::ImageLab8 => Some(&RESIZE_LAB8_ARCHITECTURES),
        Mode::ImageHsv8 => Some(&RESIZE_HSV8_ARCHITECTURES),
        _ => None,
    }
}

fn native_byte_boxed_architectures(mode: Mode) -> Option<&'static [ImplementationSupport]> {
    match mode {
        Mode::One => Some(&RESIZE_BOXED_ONE_ARCHITECTURES),
        Mode::ImageLuma8 => Some(&RESIZE_BOXED_LUMA8_ARCHITECTURES),
        Mode::ImageLumaA8 => Some(&RESIZE_BOXED_LUMA_A8_ARCHITECTURES),
        Mode::ImageLaPremultiplied8 => Some(&RESIZE_BOXED_LA_PREMULTIPLIED8_ARCHITECTURES),
        Mode::ImagePalette8 => Some(&RESIZE_BOXED_PALETTE8_ARCHITECTURES),
        Mode::ImagePaletteAlpha8 => Some(&RESIZE_BOXED_PALETTE_ALPHA8_ARCHITECTURES),
        Mode::ImageRgb8 => Some(&RESIZE_BOXED_RGB8_ARCHITECTURES),
        Mode::ImageRgba8 => Some(&RESIZE_BOXED_RGBA8_ARCHITECTURES),
        Mode::ImageRgbPremultiplied8 => Some(&RESIZE_BOXED_RGB_PREMULTIPLIED8_ARCHITECTURES),
        Mode::ImageRgbx8 => Some(&RESIZE_BOXED_RGBX8_ARCHITECTURES),
        Mode::ImageCmyk8 => Some(&RESIZE_BOXED_CMYK8_ARCHITECTURES),
        Mode::ImageYCbCr8 => Some(&RESIZE_BOXED_YCBCR8_ARCHITECTURES),
        Mode::ImageLab8 => Some(&RESIZE_BOXED_LAB8_ARCHITECTURES),
        Mode::ImageHsv8 => Some(&RESIZE_BOXED_HSV8_ARCHITECTURES),
        _ => None,
    }
}

const RESIZE_I32_ARCHITECTURES: &[ImplementationSupport] = &[
    ImplementationSupport::cpu(
        OperationId::Resize,
        Mode::ImageI32,
        Architecture::AvxFma,
        cfg!(target_arch = "x86_64"),
        &[Architecture::AvxFma],
        StrategyGuard::ResizeFilteredFma,
    ),
    ImplementationSupport::cpu(
        OperationId::Resize,
        Mode::ImageI32,
        Architecture::Neon,
        cfg!(all(target_arch = "aarch64", target_feature = "neon")),
        &[Architecture::Neon],
        StrategyGuard::ResizeI32Neon,
    ),
];
const RESIZE_F32_ARCHITECTURES: &[ImplementationSupport] = &[ImplementationSupport::cpu(
    OperationId::Resize,
    Mode::ImageF32,
    Architecture::AvxFma,
    cfg!(target_arch = "x86_64"),
    &[Architecture::AvxFma],
    StrategyGuard::ResizeFilteredFma,
)];
const fn luma16_architectures(mode: Mode) -> [ImplementationSupport; 3] {
    [
        ImplementationSupport::cpu(
            OperationId::Resize,
            mode,
            Architecture::Sse2,
            cfg!(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2")),
            &[Architecture::Sse2],
            StrategyGuard::ResizeLuma16Vector,
        ),
        ImplementationSupport::cpu(
            OperationId::Resize,
            mode,
            Architecture::Neon,
            cfg!(all(target_arch = "aarch64", target_feature = "neon")),
            &[Architecture::Neon],
            StrategyGuard::ResizeLuma16Vector,
        ),
        ImplementationSupport::cpu(
            OperationId::Resize,
            mode,
            Architecture::WasmSimd128,
            cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
            &[Architecture::WasmSimd128],
            StrategyGuard::ResizeLuma16Vector,
        ),
    ]
}
static RESIZE_I16_ARCHITECTURES: [ImplementationSupport; 3] =
    luma16_architectures(Mode::ImageI16);
static RESIZE_I16L_ARCHITECTURES: [ImplementationSupport; 3] =
    luma16_architectures(Mode::ImageI16L);
static RESIZE_I16B_ARCHITECTURES: [ImplementationSupport; 3] =
    luma16_architectures(Mode::ImageI16B);
static RESIZE_I16N_ARCHITECTURES: [ImplementationSupport; 3] =
    luma16_architectures(Mode::ImageI16N);
const RESIZE_BOXED_I32_ARCHITECTURES: &[ImplementationSupport] = &[ImplementationSupport::cpu(
    OperationId::ResizeBoxed,
    Mode::ImageI32,
    Architecture::AvxFma,
    cfg!(target_arch = "x86_64"),
    &[Architecture::AvxFma],
    StrategyGuard::ResizeFilteredFma,
)];
const RESIZE_BOXED_F32_ARCHITECTURES: &[ImplementationSupport] = &[ImplementationSupport::cpu(
    OperationId::ResizeBoxed,
    Mode::ImageF32,
    Architecture::AvxFma,
    cfg!(target_arch = "x86_64"),
    &[Architecture::AvxFma],
    StrategyGuard::ResizeFilteredFma,
)];

fn supported_mode(operation: OperationId, mode: Mode) -> ModeSupport {
    if operation == OperationId::Resize
        && let Some(architectures) = native_byte_architectures(mode)
    {
        return ModeSupport::cpu(
            operation,
            mode,
            OutputModeRule::Preserve,
            RESIZE_INPUTS,
            architectures,
        );
    }
    if operation == OperationId::ResizeBoxed
        && let Some(architectures) = native_byte_boxed_architectures(mode)
    {
        return ModeSupport::cpu(
            operation,
            mode,
            OutputModeRule::Preserve,
            RESIZE_INPUTS,
            architectures,
        );
    }
    let architectures = match (operation, mode) {
        (OperationId::Resize, Mode::ImageI32) => RESIZE_I32_ARCHITECTURES,
        (OperationId::Resize, Mode::ImageF32) => RESIZE_F32_ARCHITECTURES,
        (OperationId::Resize, Mode::ImageI16) => &RESIZE_I16_ARCHITECTURES,
        (OperationId::Resize, Mode::ImageI16L) => &RESIZE_I16L_ARCHITECTURES,
        (OperationId::Resize, Mode::ImageI16B) => &RESIZE_I16B_ARCHITECTURES,
        (OperationId::Resize, Mode::ImageI16N) => &RESIZE_I16N_ARCHITECTURES,
        (OperationId::ResizeBoxed, Mode::ImageI32) => RESIZE_BOXED_I32_ARCHITECTURES,
        (OperationId::ResizeBoxed, Mode::ImageF32) => RESIZE_BOXED_F32_ARCHITECTURES,
        _ => return ModeSupport::serial(operation, mode, RESIZE_INPUTS),
    };
    ModeSupport::cpu(operation, mode, OutputModeRule::Preserve, RESIZE_INPUTS, architectures)
}

fn classify_resize(mode: Mode) -> ModeSupport {
    match mode {
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgb8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8
        | Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N
        | Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => supported_mode(OperationId::Resize, mode),
    }
}

fn classify_boxed_resize(mode: Mode) -> ModeSupport {
    match mode {
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgb8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8
        | Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N
        | Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => supported_mode(OperationId::ResizeBoxed, mode),
    }
}

/// Exhaustive source-mode support for ordinary Resize.
pub(crate) static SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Resize, RESIZE_INPUTS, classify_resize);

/// Exhaustive source-mode support for boxed Resize.
pub(crate) static BOXED_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::ResizeBoxed,
    RESIZE_INPUTS,
    classify_boxed_resize,
);

#[derive(Clone, Copy)]
enum ResizeRequest {
    Standard { filter: ResampleFilter },
    Boxed { filter: ResampleFilter, bounds: (f64, f64, f64, f64) },
}

/// Exact mode and architecture choice retained through execution.
pub(crate) struct PreparedResize<'a> {
    image: &'a DynamicImage,
    context: ExecutionContext,
    width: u32,
    height: u32,
    request: ResizeRequest,
    selection: StrategySelection,
}

impl PreparedResize<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn supports_pad_window(&self) -> bool {
        let filtered = matches!(
            self.request,
            ResizeRequest::Standard { filter } if !matches!(filter, ResampleFilter::Nearest)
        );
        filtered
            && match self.selection.implementation.architecture {
                Architecture::Default => matches!(
                    self.image,
                    DynamicImage::ImageLuma8(_) | DynamicImage::ImageRgbx8(_)
                ),
                Architecture::Sse2 | Architecture::Neon | Architecture::WasmSimd128 => {
                    matches!(self.image, DynamicImage::ImageLuma8(_))
                }
                _ => false,
            }
    }

    /// Executes the retained mode and architecture strategy into a Pad window.
    pub(crate) fn execute_into_pad_window(&self, output: &mut [u8]) -> Result<bool, PilError> {
        if !self.supports_pad_window() {
            return Ok(false);
        }
        let ResizeRequest::Standard { filter } = self.request else {
            return Ok(false);
        };
        match (self.selection.implementation.architecture, self.image) {
            (Architecture::Default, DynamicImage::ImageLuma8(image)) => Ok(
                super::luma8::default::resize_impl::execute_into_window(
                    image, self.width, self.height, filter, output,
                ),
            ),
            (Architecture::Sse2, DynamicImage::ImageLuma8(image)) => Ok(
                super::luma8::sse2::resize_impl::execute_into_window(
                    image, self.width, self.height, filter, output,
                ),
            ),
            (Architecture::Neon, DynamicImage::ImageLuma8(image)) => Ok(
                super::luma8::neon::resize_impl::execute_into_window(
                    image, self.width, self.height, filter, output,
                ),
            ),
            (Architecture::WasmSimd128, DynamicImage::ImageLuma8(image)) => Ok(
                super::luma8::wasm_simd128::resize_impl::execute_into_window(
                    image, self.width, self.height, filter, output,
                ),
            ),
            (Architecture::Default, DynamicImage::ImageRgbx8(image)) => {
                super::rgbx8::default::resize_impl::execute_into_window(
                    image,
                    self.width,
                    self.height,
                    filter,
                    output,
                )
            }
            _ => Ok(false),
        }
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        let (filter, bounds) = match self.request {
            ResizeRequest::Standard { filter } => (filter, None),
            ResizeRequest::Boxed { filter, bounds } => (filter, Some(bounds)),
        };
        execute_mode(
            self.image,
            self.context,
            self.width,
            self.height,
            filter,
            bounds,
            self.selection,
        )
    }
}

fn execute_mode(
    image: &DynamicImage,
    context: ExecutionContext,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
    selection: StrategySelection,
) -> Result<DynamicImage, PilError> {
    match image.mode() {
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgb8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8 => execute_byte_mode(
            image,
            &context,
            selection.implementation.architecture,
            width,
            height,
            filter,
            bounds,
        ),
        Mode::ImageI32 => super::i32::ops::resize(
            &context,
            image,
            width,
            height,
            filter,
            bounds,
            selection.implementation.architecture,
        ),
        Mode::ImageF32 => super::f32::ops::resize(
            &context,
            image,
            width,
            height,
            filter,
            bounds,
            selection.implementation.architecture,
        ),
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            execute_luma16_mode(
                image,
                &context,
                selection.implementation.architecture,
                width,
                height,
                filter,
                bounds,
            )
        }
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => {
            execute_typed_mode(
                image,
                &context,
                selection.implementation.architecture,
                width,
                height,
                filter,
                bounds,
            )
        }
    }
}

fn execute_byte_mode(
    image: &DynamicImage,
    context: &ExecutionContext,
    architecture: Architecture,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
) -> Result<DynamicImage, PilError> {
    match image {
        DynamicImage::One(input) => Ok(DynamicImage::One(super::one::ops::resize(
            context, architecture, input, width, height, filter, bounds,
        )?)),
        DynamicImage::ImageLuma8(input) => Ok(DynamicImage::ImageLuma8(
            super::luma8::ops::resize(context, architecture, input, width, height, filter, bounds)?,
        )),
        DynamicImage::ImageLumaA8(input) => Ok(DynamicImage::ImageLumaA8(
            super::luma_a8::ops::resize(context, architecture, input, width, height, filter, bounds)?,
        )),
        DynamicImage::ImageLaPremultiplied8(input) => Ok(DynamicImage::ImageLaPremultiplied8(
            super::la_premultiplied8::ops::resize(context, architecture, input, width, height, filter, bounds)?,
        )),
        DynamicImage::ImagePalette8(input) => Ok(DynamicImage::ImagePalette8(
            super::palette8::ops::resize(context, architecture, input, width, height, filter, bounds)?,
        )),
        DynamicImage::ImagePaletteAlpha8(input) => Ok(DynamicImage::ImagePaletteAlpha8(
            super::palette_alpha8::ops::resize(context, architecture, input, width, height, filter, bounds)?,
        )),
        DynamicImage::ImageRgb8(input) => Ok(DynamicImage::ImageRgb8(
            super::rgb8::ops::resize(context, architecture, input, width, height, filter, bounds)?,
        )),
        DynamicImage::ImageRgba8(input) => Ok(DynamicImage::ImageRgba8(
            super::rgba8::ops::resize(context, architecture, input, width, height, filter, bounds)?,
        )),
        DynamicImage::ImageRgbPremultiplied8(input) => Ok(DynamicImage::ImageRgbPremultiplied8(
            super::rgb_premultiplied8::ops::resize(context, architecture, input, width, height, filter, bounds)?,
        )),
        DynamicImage::ImageRgbx8(input) => Ok(DynamicImage::ImageRgbx8(
            super::rgbx8::ops::resize(context, architecture, input, width, height, filter, bounds)?,
        )),
        DynamicImage::ImageCmyk8(input) => Ok(DynamicImage::ImageCmyk8(
            super::cmyk8::ops::resize(context, architecture, input, width, height, filter, bounds)?,
        )),
        DynamicImage::ImageYCbCr8(input) => Ok(DynamicImage::ImageYCbCr8(
            super::ycbcr8::ops::resize(context, architecture, input, width, height, filter, bounds)?,
        )),
        DynamicImage::ImageLab8(input) => Ok(DynamicImage::ImageLab8(
            super::lab8::ops::resize(context, architecture, input, width, height, filter, bounds)?,
        )),
        DynamicImage::ImageHsv8(input) => Ok(DynamicImage::ImageHsv8(
            super::hsv8::ops::resize(context, architecture, input, width, height, filter, bounds)?,
        )),
        _ => Err(PilError::InternalError(
            "byte Resize mode dispatch received a non-byte image".into(),
        )),
    }
}

fn execute_luma16_mode(
    image: &DynamicImage,
    context: &ExecutionContext,
    architecture: Architecture,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
) -> Result<DynamicImage, PilError> {
    match image {
        DynamicImage::ImageI16(input) => {
            super::i16::ops::resize_selected(context, architecture, input, width, height, filter, bounds)
        }
        DynamicImage::ImageI16L(input) => {
            super::i16l::ops::resize_selected(context, architecture, input, width, height, filter, bounds)
        }
        DynamicImage::ImageI16B(input) => {
            super::i16b::ops::resize_selected(context, architecture, input, width, height, filter, bounds)
        }
        DynamicImage::ImageI16N(input) => {
            super::i16n::ops::resize_selected(context, architecture, input, width, height, filter, bounds)
        }
        _ => Err(PilError::InternalError(
            "I;16 Resize mode dispatch received a different image".into(),
        )),
    }
}

fn execute_typed_mode(
    image: &DynamicImage,
    context: &ExecutionContext,
    architecture: Architecture,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
) -> Result<DynamicImage, PilError> {
    match image {
        DynamicImage::ImageLumaA16(input) => {
            super::luma_a16::ops::resize_selected(context, architecture, input, width, height, filter, bounds)
        }
        DynamicImage::ImageRgb16(input) => {
            super::rgb16::ops::resize_selected(context, architecture, input, width, height, filter, bounds)
        }
        DynamicImage::ImageRgba16(input) => {
            super::rgba16::ops::resize_selected(context, architecture, input, width, height, filter, bounds)
        }
        DynamicImage::ImageRgb32F(input) => {
            super::rgb32f::ops::resize_selected(context, architecture, input, width, height, filter, bounds)
        }
        DynamicImage::ImageRgba32F(input) => {
            super::rgba32f::ops::resize_selected(context, architecture, input, width, height, filter, bounds)
        }
        _ => Err(PilError::InternalError(
            "typed Resize mode dispatch received a different image".into(),
        )),
    }
}

fn selection(
    support: &'static OperationSupport,
    mode: Mode,
    facts: StrategyInputs,
    ctx: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    match support.for_mode(mode).assess(ctx, facts)? {
        SupportAssessment::Ready(selection) => Ok(selection),
        SupportAssessment::Rejected { reason, kind, .. } => Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => Err(PilError::InternalError(
            "Resize support remained unresolved after materialization".into(),
        )),
    }
}

fn resize_request_facts(
    facts: StrategyInputs,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
) -> StrategyInputs {
    match bounds {
        Some(bounds) => facts.with_boxed_resize(width, height, filter, bounds),
        None => facts.with_resize(width, height, filter, false),
    }
}

/// Prepares a typed Resize dependency for another CPU operation.
pub(crate) fn prepare_direct<'a>(
    inputs: OperationInputs<'a>,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
    ctx: &ExecutionContext,
) -> Result<PreparedResize<'a>, PilError> {
    let boxed = bounds.is_some();
    let support = if boxed { &BOXED_SUPPORT } else { &SUPPORT };
    let facts = resize_request_facts(inputs.facts(), width, height, filter, bounds);
    let selected = selection(support, inputs.mode(), facts, ctx)?;
    let request = match bounds {
        Some(bounds) => ResizeRequest::Boxed { filter, bounds },
        None => ResizeRequest::Standard { filter },
    };
    Ok(PreparedResize {
        image: inputs.image,
        context: *ctx,
        width,
        height,
        request,
        selection: selected,
    })
}

/// Prepares ordinary or boxed Resize using the operation's declared support.
pub(crate) fn prepare_pipeline<'a>(
    inputs: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let (width, height, filter, request, support) = match op {
        PipelineOp::Resize { w, h, filter } => (
            *w,
            *h,
            *filter,
            ResizeRequest::Standard { filter: *filter },
            &SUPPORT,
        ),
        PipelineOp::ResizeBoxed { w, h, filter, box_coords } => (
            *w,
            *h,
            *filter,
            ResizeRequest::Boxed { filter: *filter, bounds: *box_coords },
            &BOXED_SUPPORT,
        ),
        _ => return Err(PilError::ValueError("expected Resize op".into())),
    };
    let bounds = match request {
        ResizeRequest::Standard { .. } => None,
        ResizeRequest::Boxed { bounds, .. } => Some(bounds),
    };
    let facts = resize_request_facts(inputs.facts(), width, height, filter, bounds);
    let selected = selection(support, inputs.mode(), facts, ctx)?;
    let plan = PreparedResize {
        image: inputs.image,
        context: *ctx,
        width,
        height,
        request,
        selection: selected,
    };
    Ok(crate::compute::registry::PreparedOperation::from_resize(plan))
}

pub(crate) fn native_luma16_resize(
    image: &DynamicImage,
    destination_width: u32,
    destination_height: u32,
    filter: ResampleFilter,
    bounds: super::helpers::typed_nearest::BoxBounds,
) -> Option<DynamicImage> {
    match image {
        DynamicImage::ImageI16(input) => Some(super::i16::ops::resize(
            input,
            destination_width,
            destination_height,
            filter,
            bounds,
        )),
        DynamicImage::ImageI16L(input) => Some(super::i16l::ops::resize(
            input,
            destination_width,
            destination_height,
            filter,
            bounds,
        )),
        DynamicImage::ImageI16B(input) => Some(super::i16b::ops::resize(
            input,
            destination_width,
            destination_height,
            filter,
            bounds,
        )),
        DynamicImage::ImageI16N(input) => Some(super::i16n::ops::resize(
            input,
            destination_width,
            destination_height,
            filter,
            bounds,
        )),
        _ => None,
    }
}

pub(crate) fn native_typed_resize(
    image: &DynamicImage,
    destination_width: u32,
    destination_height: u32,
    filter: ResampleFilter,
    bounds: super::helpers::typed_nearest::BoxBounds,
) -> Option<DynamicImage> {
    match image {
        DynamicImage::ImageLumaA16(input) => Some(super::luma_a16::ops::resize(
            input,
            destination_width,
            destination_height,
            filter,
            bounds,
        )),
        DynamicImage::ImageRgb16(input) => Some(super::rgb16::ops::resize(
            input,
            destination_width,
            destination_height,
            filter,
            bounds,
        )),
        DynamicImage::ImageRgba16(input) => Some(super::rgba16::ops::resize(
            input,
            destination_width,
            destination_height,
            filter,
            bounds,
        )),
        DynamicImage::ImageRgb32F(input) => Some(super::rgb32f::ops::resize(
            input,
            destination_width,
            destination_height,
            filter,
            bounds,
        )),
        DynamicImage::ImageRgba32F(input) => Some(super::rgba32f::ops::resize(
            input,
            destination_width,
            destination_height,
            filter,
            bounds,
        )),
        _ => None,
    }
}
