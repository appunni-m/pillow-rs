pub(crate) type Input = crate::raster::GrayImage;
use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_native_mode;
use crate::compute::Architecture;
use crate::compute::registry::support::{
    ImplementationSupport, InputContract, ModeSupport, OperationId, OutputModeRule,
    StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage, Mode};

declare_invert_native_mode!(ImageLuma8, Input, 1, 1);

const AUTOCONTRAST_INPUTS: &[InputContract] = &[
    InputContract::AutocontrastParameters,
    InputContract::CheckedByteDimensions { bytes_per_pixel: 1, allow_empty: true },
    InputContract::ImageOpsLumaMask,
];
static AUTOCONTRAST_ARCHITECTURES: [ImplementationSupport; 3] =
    super::super::helpers::strategy::native_byte_point_architectures(
        OperationId::Autocontrast,
        Mode::ImageLuma8,
        1,
    );
pub(crate) const AUTOCONTRAST_SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Autocontrast,
    Mode::ImageLuma8,
    OutputModeRule::Preserve,
    AUTOCONTRAST_INPUTS,
    &AUTOCONTRAST_ARCHITECTURES,
);

const EQUALIZE_INPUTS: &[InputContract] = &[
    InputContract::CheckedByteDimensions { bytes_per_pixel: 1, allow_empty: true },
    InputContract::ImageOpsLumaMask,
];
static EQUALIZE_ARCHITECTURES: [ImplementationSupport; 3] =
    super::super::helpers::strategy::native_byte_point_architectures(
        OperationId::Equalize,
        Mode::ImageLuma8,
        1,
    );
pub(crate) const EQUALIZE_SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Equalize,
    Mode::ImageLuma8,
    OutputModeRule::Preserve,
    EQUALIZE_INPUTS,
    &EQUALIZE_ARCHITECTURES,
);

/// Prepared native L-mode Equalize kernel strategy.
pub(crate) struct PreparedEqualize<'a> {
    input: &'a GrayImage,
    lut: Vec<u8>,
    selection: StrategySelection,
}

impl<'a> PreparedEqualize<'a> {
    pub(crate) fn new(input: &'a GrayImage, lut: Vec<u8>, selection: StrategySelection) -> Self {
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

/// Prepared L-mode Autocontrast kernel strategy.
pub(crate) struct PreparedAutocontrast<'a> {
    input: &'a GrayImage,
    lut: Vec<u8>,
    selection: StrategySelection,
}

impl<'a> PreparedAutocontrast<'a> {
    pub(crate) fn new(input: &'a GrayImage, lut: Vec<u8>, selection: StrategySelection) -> Self {
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
    InputContract::CheckedByteDimensions { bytes_per_pixel: 1, allow_empty: true },
];
static POSTERIZE_ARCHITECTURES: [ImplementationSupport; 3] =
    super::super::helpers::strategy::native_byte_point_architectures(
        OperationId::Posterize,
        Mode::ImageLuma8,
        1,
    );
pub(crate) const POSTERIZE_SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Posterize,
    Mode::ImageLuma8,
    OutputModeRule::Preserve,
    POSTERIZE_INPUTS,
    &POSTERIZE_ARCHITECTURES,
);

const SOLARIZE_INPUTS: &[InputContract] = &[InputContract::CheckedByteDimensions {
    bytes_per_pixel: 1,
    allow_empty: true,
}];
static SOLARIZE_ARCHITECTURES: [ImplementationSupport; 3] =
    super::super::helpers::strategy::native_byte_point_architectures(
        OperationId::Solarize,
        Mode::ImageLuma8,
        1,
    );
pub(crate) const SOLARIZE_SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Solarize,
    Mode::ImageLuma8,
    OutputModeRule::Preserve,
    SOLARIZE_INPUTS,
    &SOLARIZE_ARCHITECTURES,
);

static COLORIZE_ARCHITECTURES: [ImplementationSupport; 3] =
    super::super::helpers::strategy::native_byte_point_architectures(
        OperationId::Colorize,
        Mode::ImageLuma8,
        1,
    );
pub(crate) const COLORIZE_SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Colorize,
    Mode::ImageLuma8,
    OutputModeRule::Fixed(Mode::ImageRgb8),
    &[],
    &COLORIZE_ARCHITECTURES,
);

/// Prepared L-to-RGB Colorize strategy with its selected architecture.
pub(crate) struct PreparedColorize<'a> {
    input: &'a GrayImage,
    lut: [[u8; 256]; 3],
    selection: StrategySelection,
}

impl<'a> PreparedColorize<'a> {
    pub(crate) fn new(
        input: &'a GrayImage,
        lut: [[u8; 256]; 3],
        selection: StrategySelection,
    ) -> Self {
        Self { input, lut, selection }
    }

    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        let mut output = match self.selection.implementation.architecture {
            Architecture::Default => super::default::colorize_impl::execute(
                self.input.as_raw(),
                self.input.width(),
                self.input.height(),
                &self.lut,
            ),
            Architecture::Sse2 => super::sse2::colorize_impl::execute(
                self.input.as_raw(),
                self.input.width(),
                self.input.height(),
                &self.lut,
            ),
            Architecture::Neon => super::neon::colorize_impl::execute(
                self.input.as_raw(),
                self.input.width(),
                self.input.height(),
                &self.lut,
            ),
            Architecture::WasmSimd128 => super::wasm_simd128::colorize_impl::execute(
                self.input.as_raw(),
                self.input.width(),
                self.input.height(),
                &self.lut,
            ),
            _ => Err(PilError::InternalError(
                "colorize selection has no matching CPU kernel".into(),
            )),
        }?;
        super::super::helpers::hidden_lanes::initialize_point_output_lanes(&mut output)?;
        Ok(output)
    }
}

/// Prepared L-mode Posterize strategy with its selected architecture.
pub(crate) struct PreparedPosterize<'a> {
    input: &'a GrayImage,
    bits: u8,
    selection: StrategySelection,
}

impl<'a> PreparedPosterize<'a> {
    pub(crate) fn new(input: &'a GrayImage, bits: u8, selection: StrategySelection) -> Self {
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

/// Prepared L-mode Solarize strategy with its selected architecture.
pub(crate) struct PreparedSolarize<'a> {
    input: &'a GrayImage,
    threshold: u8,
    selection: StrategySelection,
}

impl<'a> PreparedSolarize<'a> {
    pub(crate) fn new(input: &'a GrayImage, threshold: u8, selection: StrategySelection) -> Self {
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
    image: &GrayImage,
    width: u32,
    height: u32,
    colors: usize,
    kmeans: u32,
    method: u32,
    architecture: Architecture,
) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), PilError> {
    match architecture {
        Architecture::Default => super::default::quantize_impl::execute(
            image, width, height, colors, kmeans, method,
        ),
        _ => Err(PilError::InternalError(
            "L Quantize selected an unimplemented architecture".into(),
        )),
    }
}

pub(crate) fn execute_quantize_with_palette(
    image: &GrayImage,
    palette: &[[u8; 3]],
    dither: bool,
    architecture: Architecture,
) -> Result<Vec<u8>, PilError> {
    match architecture {
        Architecture::Default => {
            super::default::quantize_impl::execute_with_palette(image, palette, dither)
        }
        _ => Err(PilError::InternalError(
            "L Quantize selected an unimplemented architecture".into(),
        )),
    }
}
