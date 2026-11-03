pub(crate) type Input = crate::raster::RgbImage;
use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_native_mode;
use crate::compute::Architecture;
use crate::compute::registry::support::{
    ImplementationSupport, InputContract, ModeSupport, OperationId, OutputModeRule,
    StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbImage};

declare_invert_native_mode!(ImageRgb8, Input, 3, 3);

const AUTOCONTRAST_INPUTS: &[InputContract] = &[
    InputContract::AutocontrastParameters,
    InputContract::CheckedByteDimensions { bytes_per_pixel: 3, allow_empty: true },
    InputContract::ImageOpsLumaMask,
];
static AUTOCONTRAST_ARCHITECTURES: [ImplementationSupport; 3] =
    super::super::helpers::strategy::native_byte_point_architectures(
        OperationId::Autocontrast,
        Mode::ImageRgb8,
        3,
    );
pub(crate) const AUTOCONTRAST_SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Autocontrast,
    Mode::ImageRgb8,
    OutputModeRule::Preserve,
    AUTOCONTRAST_INPUTS,
    &AUTOCONTRAST_ARCHITECTURES,
);

const EQUALIZE_INPUTS: &[InputContract] = &[
    InputContract::CheckedByteDimensions { bytes_per_pixel: 3, allow_empty: true },
    InputContract::ImageOpsLumaMask,
];
static EQUALIZE_ARCHITECTURES: [ImplementationSupport; 3] =
    super::super::helpers::strategy::native_byte_point_architectures(
        OperationId::Equalize,
        Mode::ImageRgb8,
        3,
    );
pub(crate) const EQUALIZE_SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Equalize,
    Mode::ImageRgb8,
    OutputModeRule::Preserve,
    EQUALIZE_INPUTS,
    &EQUALIZE_ARCHITECTURES,
);

/// Prepared native RGB-mode Equalize kernel strategy.
pub(crate) struct PreparedEqualize<'a> {
    input: &'a RgbImage,
    lut: Vec<u8>,
    selection: StrategySelection,
}

impl<'a> PreparedEqualize<'a> {
    pub(crate) fn new(input: &'a RgbImage, lut: Vec<u8>, selection: StrategySelection) -> Self {
        Self { input, lut, selection }
    }

    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        match self.selection.implementation.architecture {
            Architecture::Default => super::default::equalize_impl::execute(self.input, &self.lut),
            Architecture::Sse2 => super::sse2::histogram_impl::equalize(self.input, &self.lut),
            Architecture::Neon => super::neon::histogram_impl::equalize(self.input, &self.lut),
            Architecture::WasmSimd128 => {
                super::wasm_simd128::histogram_impl::equalize(self.input, &self.lut)
            }
            _ => Err(PilError::InternalError(
                "native-byte point selection has no matching CPU kernel".into(),
            )),
        }
    }
}

/// Prepared RGB-mode Autocontrast kernel strategy.
pub(crate) struct PreparedAutocontrast<'a> {
    input: &'a RgbImage,
    lut: Vec<u8>,
    selection: StrategySelection,
}

impl<'a> PreparedAutocontrast<'a> {
    pub(crate) fn new(input: &'a RgbImage, lut: Vec<u8>, selection: StrategySelection) -> Self {
        Self { input, lut, selection }
    }

    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        match self.selection.implementation.architecture {
            Architecture::Default => {
                super::default::autocontrast_impl::execute(self.input, &self.lut)
            }
            Architecture::Sse2 => {
                super::sse2::histogram_impl::autocontrast(self.input, &self.lut)
            }
            Architecture::Neon => {
                super::neon::histogram_impl::autocontrast(self.input, &self.lut)
            }
            Architecture::WasmSimd128 => {
                super::wasm_simd128::histogram_impl::autocontrast(self.input, &self.lut)
            }
            _ => Err(PilError::InternalError(
                "native-byte point selection has no matching CPU kernel".into(),
            )),
        }
    }
}

const POSTERIZE_INPUTS: &[InputContract] = &[
    InputContract::PosterizeBits,
    InputContract::CheckedByteDimensions { bytes_per_pixel: 3, allow_empty: true },
];
static POSTERIZE_ARCHITECTURES: [ImplementationSupport; 3] =
    super::super::helpers::strategy::native_byte_point_architectures(
        OperationId::Posterize,
        Mode::ImageRgb8,
        3,
    );
pub(crate) const POSTERIZE_SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Posterize,
    Mode::ImageRgb8,
    OutputModeRule::Preserve,
    POSTERIZE_INPUTS,
    &POSTERIZE_ARCHITECTURES,
);

const SOLARIZE_INPUTS: &[InputContract] = &[InputContract::CheckedByteDimensions {
    bytes_per_pixel: 3,
    allow_empty: true,
}];
static SOLARIZE_ARCHITECTURES: [ImplementationSupport; 3] =
    super::super::helpers::strategy::native_byte_point_architectures(
        OperationId::Solarize,
        Mode::ImageRgb8,
        3,
    );
pub(crate) const SOLARIZE_SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Solarize,
    Mode::ImageRgb8,
    OutputModeRule::Preserve,
    SOLARIZE_INPUTS,
    &SOLARIZE_ARCHITECTURES,
);

/// Prepared RGB-mode Posterize strategy with its selected architecture.
pub(crate) struct PreparedPosterize<'a> {
    input: &'a RgbImage,
    bits: u8,
    selection: StrategySelection,
}

impl<'a> PreparedPosterize<'a> {
    pub(crate) fn new(input: &'a RgbImage, bits: u8, selection: StrategySelection) -> Self {
        Self { input, bits, selection }
    }

    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        match self.selection.implementation.architecture {
            Architecture::Default => super::default::posterize_impl::execute(self.input, self.bits),
            Architecture::Sse2 => super::sse2::posterize_impl::execute(self.input, self.bits),
            Architecture::Neon => super::neon::posterize_impl::execute(self.input, self.bits),
            Architecture::WasmSimd128 => {
                super::wasm_simd128::posterize_impl::execute(self.input, self.bits)
            }
            _ => Err(PilError::InternalError(
                "native-byte point selection has no matching CPU kernel".into(),
            )),
        }
    }
}

/// Prepared RGB-mode Solarize strategy with its selected architecture.
pub(crate) struct PreparedSolarize<'a> {
    input: &'a RgbImage,
    threshold: u8,
    selection: StrategySelection,
}

impl<'a> PreparedSolarize<'a> {
    pub(crate) fn new(input: &'a RgbImage, threshold: u8, selection: StrategySelection) -> Self {
        Self { input, threshold, selection }
    }

    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        match self.selection.implementation.architecture {
            Architecture::Default => {
                super::default::solarize_impl::execute(self.input, self.threshold)
            }
            Architecture::Sse2 => super::sse2::solarize_impl::execute(self.input, self.threshold),
            Architecture::Neon => super::neon::solarize_impl::execute(self.input, self.threshold),
            Architecture::WasmSimd128 => {
                super::wasm_simd128::solarize_impl::execute(self.input, self.threshold)
            }
            _ => Err(PilError::InternalError(
                "native-byte point selection has no matching CPU kernel".into(),
            )),
        }
    }
}

pub(crate) fn execute_quantize(
    image: &RgbImage,
    width: u32,
    height: u32,
    colors: usize,
    kmeans: u32,
    method: u32,
    selection: &StrategySelection,
) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), PilError> {
    let architecture = super::super::helpers::strategy::selected_quantize_architecture(
        selection,
        Mode::ImageRgb8,
    )?;
    match architecture {
        Architecture::Default => super::default::quantize_impl::execute(
            image, width, height, colors, kmeans, method,
        ),
        _ => Err(PilError::InternalError(
            "RGB Quantize selected an unimplemented architecture".into(),
        )),
    }
}

pub(crate) fn execute_quantize_with_palette(
    image: &RgbImage,
    palette: &[[u8; 3]],
    dither: bool,
    selection: &StrategySelection,
) -> Result<Vec<u8>, PilError> {
    let architecture = super::super::helpers::strategy::selected_quantize_architecture(
        selection,
        Mode::ImageRgb8,
    )?;
    match architecture {
        Architecture::Default => {
            super::default::quantize_impl::execute_with_palette(image, palette, dither)
        }
        _ => Err(PilError::InternalError(
            "RGB Quantize selected an unimplemented architecture".into(),
        )),
    }
}
