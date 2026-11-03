//! Chops contracts, preparation, and exact-mode strategy orchestration.
//!
//! Legacy `Backend::Simd` requests now reach this owner through the CPU pool
//! with the detected architecture in their context. Invert has exact-mode
//! SSE2, NEON, and Wasm SIMD128 strategies for the archived `1/L/P`, `LA/PA`,
//! `RGB`, and `RGBA/CMYK` layouts; its shared byte kernel lives in
//! `pool_cpu/helpers/bytes` and preserves every stored byte.
//! Binary and fused Chops paths now apply their native byte arithmetic to
//! retained omitted lanes, including owned in-place batch execution. Missing
//! state starts from the same canonical per-mode fallback as Paste; Invert
//! retains a changed fallback byte. Offset remaps lanes and Invert transforms
//! every retained byte.

use crate::raster::GenericImageView;
use std::sync::Arc;

use crate::compute::{Architecture, ExecutionContext};
use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OperationInputs, OperationSupport, OutputModeRule,
    PaletteInput, RejectionKind, SecondaryInputFacts, StrategySelection, SupportAssessment,
};
use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

use super::helpers::binary::BinaryOperation;

const BINARY_MODES_1: &[Mode] = &[Mode::One, Mode::ImageLuma8, Mode::ImagePalette8];
const BINARY_MODES_2: &[Mode] = &[
    Mode::ImageLumaA8,
    Mode::ImageLaPremultiplied8,
    Mode::ImagePaletteAlpha8,
];
const BINARY_MODES_3: &[Mode] = &[
    Mode::ImageRgb8,
    Mode::ImageYCbCr8,
    Mode::ImageLab8,
    Mode::ImageHsv8,
];
const LAB_CORE_ONLY_MODES: &[Mode] = &[Mode::ImageLab8];
const LAB_CORE_ONLY_REASON: &str =
    "native three-byte LAB Chops extends Pillow's binary mode contract";
const BINARY_MODES_4: &[Mode] = &[
    Mode::ImageRgba8,
    Mode::ImageCmyk8,
    Mode::ImageRgbPremultiplied8,
    Mode::ImageRgbx8,
];
const BINARY_INPUTS_1: &[InputContract] = &[InputContract::ChopsBinaryModes {
    modes: BINARY_MODES_1,
    core_only_modes: &[],
    core_only_reason: None,
    rejection_reason: "images do not match",
}];
const BINARY_INPUTS_2: &[InputContract] = &[InputContract::ChopsBinaryModes {
    modes: BINARY_MODES_2,
    core_only_modes: &[],
    core_only_reason: None,
    rejection_reason: "images do not match",
}];
const BINARY_INPUTS_3: &[InputContract] = &[InputContract::ChopsBinaryModes {
    modes: BINARY_MODES_3,
    // NO_PILLOW_SUPPORT: LAB uses the native three-byte Chops strategy in Rust.
    core_only_modes: LAB_CORE_ONLY_MODES,
    core_only_reason: Some(LAB_CORE_ONLY_REASON),
    rejection_reason: "images do not match",
}];
const BINARY_INPUTS_4: &[InputContract] = &[InputContract::ChopsBinaryModes {
    modes: BINARY_MODES_4,
    core_only_modes: &[],
    core_only_reason: None,
    rejection_reason: "images do not match",
}];
const LOGICAL_INPUTS: &[InputContract] = &[InputContract::ChopsBinaryModes {
    modes: &[Mode::One],
    core_only_modes: &[],
    core_only_reason: None,
    rejection_reason: "image has wrong mode",
}];
const BINARY_INPUTS: &[InputContract] = &[InputContract::ChopsBinaryModes {
    modes: &[
        Mode::One,
        Mode::ImageLuma8,
        Mode::ImagePalette8,
        Mode::ImageLumaA8,
        Mode::ImageLaPremultiplied8,
        Mode::ImagePaletteAlpha8,
        Mode::ImageRgb8,
        Mode::ImageYCbCr8,
        Mode::ImageLab8,
        Mode::ImageHsv8,
        Mode::ImageRgba8,
        Mode::ImageCmyk8,
        Mode::ImageRgbPremultiplied8,
        Mode::ImageRgbx8,
    ],
    // NO_PILLOW_SUPPORT: LAB participates in native three-byte Chops pairs only in Rust.
    core_only_modes: LAB_CORE_ONLY_MODES,
    core_only_reason: Some(LAB_CORE_ONLY_REASON),
    rejection_reason: "images do not match",
}];
const NO_INPUTS: &[InputContract] = &[];

macro_rules! operation_support {
    ($name:ident, $operation:ident, $inputs:ident) => {
        pub(crate) static $name: OperationSupport = OperationSupport::new(
            OperationId::$operation,
            $inputs,
            |mode| support_for_mode(OperationId::$operation, mode),
        );
    };
}

operation_support!(ADD_SUPPORT, ChopsAdd, BINARY_INPUTS);
operation_support!(SUBTRACT_SUPPORT, ChopsSubtract, BINARY_INPUTS);
operation_support!(MULTIPLY_SUPPORT, ChopsMultiply, BINARY_INPUTS);
operation_support!(SCREEN_SUPPORT, ChopsScreen, BINARY_INPUTS);
operation_support!(DARKER_SUPPORT, ChopsDarker, BINARY_INPUTS);
operation_support!(LIGHTER_SUPPORT, ChopsLighter, BINARY_INPUTS);
operation_support!(DIFFERENCE_SUPPORT, ChopsDifference, BINARY_INPUTS);
operation_support!(OVERLAY_SUPPORT, ChopsOverlay, BINARY_INPUTS);
operation_support!(HARD_LIGHT_SUPPORT, ChopsHardLight, BINARY_INPUTS);
operation_support!(SOFT_LIGHT_SUPPORT, ChopsSoftLight, BINARY_INPUTS);
operation_support!(ADD_MODULO_SUPPORT, ChopsAddModulo, BINARY_INPUTS);
operation_support!(SUBTRACT_MODULO_SUPPORT, ChopsSubtractModulo, BINARY_INPUTS);
operation_support!(LOGICAL_AND_SUPPORT, ChopsLogicalAnd, LOGICAL_INPUTS);
operation_support!(LOGICAL_OR_SUPPORT, ChopsLogicalOr, LOGICAL_INPUTS);
operation_support!(LOGICAL_XOR_SUPPORT, ChopsLogicalXor, LOGICAL_INPUTS);
operation_support!(CONSTANT_SUPPORT, ChopsConstant, NO_INPUTS);
operation_support!(OFFSET_SUPPORT, ChopsOffset, NO_INPUTS);
operation_support!(DUPLICATE_SUPPORT, ChopsDuplicate, NO_INPUTS);
operation_support!(INVERT_SUPPORT, ChopsInvert, NO_INPUTS);

fn support_for_mode(operation: OperationId, mode: Mode) -> ModeSupport {
    match mode {
        Mode::One => super::one::ops::support(operation),
        Mode::ImageLuma8 => super::luma8::ops::support(operation),
        Mode::ImageLumaA8 => super::luma_a8::ops::support(operation),
        Mode::ImageLaPremultiplied8 => super::la_premultiplied8::ops::support(operation),
        Mode::ImagePalette8 => super::palette8::ops::support(operation),
        Mode::ImagePaletteAlpha8 => super::palette_alpha8::ops::support(operation),
        Mode::ImageRgb8 => super::rgb8::ops::support(operation),
        Mode::ImageRgba8 => super::rgba8::ops::support(operation),
        Mode::ImageRgbPremultiplied8 => super::rgb_premultiplied8::ops::support(operation),
        Mode::ImageRgbx8 => super::rgbx8::ops::support(operation),
        Mode::ImageCmyk8 => super::cmyk8::ops::support(operation),
        Mode::ImageYCbCr8 => super::ycbcr8::ops::support(operation),
        Mode::ImageLab8 => super::lab8::ops::support(operation),
        Mode::ImageHsv8 => super::hsv8::ops::support(operation),
        Mode::ImageI32 => super::i32::ops::support(operation),
        Mode::ImageF32 => super::f32::ops::support(operation),
        Mode::ImageI16 => super::i16::ops::support(operation),
        Mode::ImageI16L => super::i16l::ops::support(operation),
        Mode::ImageI16B => super::i16b::ops::support(operation),
        Mode::ImageI16N => super::i16n::ops::support(operation),
        Mode::ImageLumaA16 => super::luma_a16::ops::support(operation),
        Mode::ImageRgb16 => super::rgb16::ops::support(operation),
        Mode::ImageRgba16 => super::rgba16::ops::support(operation),
        Mode::ImageRgb32F => super::rgb32f::ops::support(operation),
        Mode::ImageRgba32F => super::rgba32f::ops::support(operation),
    }
}

pub(super) fn mode_support(mode: Mode, operation: OperationId) -> ModeSupport {
    mode_support_with_architectures(mode, operation, &[])
}

pub(super) fn mode_support_with_architectures(
    mode: Mode,
    operation: OperationId,
    architectures: &'static [crate::compute::registry::support::ImplementationSupport],
) -> ModeSupport {
    if let Some(inputs) = binary_inputs_for(operation, mode) {
        if mode == Mode::ImageLab8 {
            // NO_PILLOW_SUPPORT: LAB binary Chops is a Rust-only native-byte extension.
            return ModeSupport::cpu_core_only(
                operation,
                mode,
                OutputModeRule::Preserve,
                inputs,
                architectures,
                LAB_CORE_ONLY_REASON,
            );
        }
        return ModeSupport::cpu(
            operation,
            mode,
            OutputModeRule::Preserve,
            inputs,
            architectures,
        );
    }
    match operation {
        OperationId::ChopsLogicalAnd | OperationId::ChopsLogicalOr | OperationId::ChopsLogicalXor => {
            if mode == Mode::One {
                ModeSupport::cpu(
                    operation,
                    mode,
                    OutputModeRule::Preserve,
                    LOGICAL_INPUTS,
                    architectures,
                )
            } else {
                rejected(mode, "image has wrong mode")
            }
        }
        // NO_PILLOW_SUPPORT: native high-depth identities extend Constant
        // beyond Pillow's ImageChops mode contract.
        OperationId::ChopsConstant if internal_mode(mode) => ModeSupport::cpu_core_only(
            operation,
            mode,
            OutputModeRule::Fixed(Mode::ImageLuma8),
            NO_INPUTS,
            &[],
            "constant accepts an internal high-depth source mode",
        ),
        OperationId::ChopsConstant => ModeSupport::cpu(
            operation,
            mode,
            OutputModeRule::Fixed(Mode::ImageLuma8),
            NO_INPUTS,
            &[],
        ),
        // NO_PILLOW_SUPPORT: internal high-depth identities extend Offset and
        // Duplicate beyond Pillow's ImageChops mode contract.
        OperationId::ChopsOffset | OperationId::ChopsDuplicate if internal_mode(mode) => {
            ModeSupport::cpu_core_only(
                operation,
                mode,
                OutputModeRule::Preserve,
                NO_INPUTS,
                &[],
                "operation accepts an internal high-depth source mode",
            )
        }
        OperationId::ChopsOffset => ModeSupport::cpu(
            operation,
            mode,
            OutputModeRule::Preserve,
            NO_INPUTS,
            architectures,
        ),
        OperationId::ChopsDuplicate => {
            ModeSupport::cpu(operation, mode, OutputModeRule::Preserve, NO_INPUTS, &[])
        }
        OperationId::ChopsInvert
            if matches!(
                mode,
                Mode::ImageI32
                    | Mode::ImageF32
                    | Mode::ImageI16
                    | Mode::ImageI16L
                    | Mode::ImageI16B
                    | Mode::ImageI16N
                    | Mode::ImageLumaA16
                    | Mode::ImageRgb16
                    | Mode::ImageRgba16
                    | Mode::ImageRgb32F
                    | Mode::ImageRgba32F
            ) =>
        {
            // NO_PILLOW_SUPPORT: direct stored-byte inversion extends
            // ImageChops to numeric and internal high-depth identities.
            ModeSupport::cpu_core_only(
                operation,
                mode,
                OutputModeRule::Preserve,
                NO_INPUTS,
                &[],
                "native stored-byte inversion extends Pillow ImageChops modes",
            )
        }
        OperationId::ChopsInvert => {
            ModeSupport::cpu(operation, mode, OutputModeRule::Preserve, NO_INPUTS, &[])
        }
        _ => rejected(mode, "image has wrong mode"),
    }
}

fn binary_inputs_for(operation: OperationId, mode: Mode) -> Option<&'static [InputContract]> {
    if !is_byte_binary(operation) {
        return None;
    }
    match mode {
        Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8 => Some(BINARY_INPUTS_1),
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => {
            Some(BINARY_INPUTS_2)
        }
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => {
            Some(BINARY_INPUTS_3)
        }
        Mode::ImageRgba8
        | Mode::ImageCmyk8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8 => Some(BINARY_INPUTS_4),
        _ => None,
    }
}

fn is_byte_binary(operation: OperationId) -> bool {
    matches!(
        operation,
        OperationId::ChopsAdd
            | OperationId::ChopsSubtract
            | OperationId::ChopsMultiply
            | OperationId::ChopsScreen
            | OperationId::ChopsDarker
            | OperationId::ChopsLighter
            | OperationId::ChopsDifference
            | OperationId::ChopsOverlay
            | OperationId::ChopsHardLight
            | OperationId::ChopsSoftLight
            | OperationId::ChopsAddModulo
            | OperationId::ChopsSubtractModulo
    )
}

fn internal_mode(mode: Mode) -> bool {
    matches!(
        mode,
        Mode::ImageLumaA16
            | Mode::ImageRgb16
            | Mode::ImageRgba16
            | Mode::ImageRgb32F
            | Mode::ImageRgba32F
    )
}

fn rejected(mode: Mode, reason: &'static str) -> ModeSupport {
    ModeSupport::Rejected {
        mode,
        reason,
        kind: RejectionKind::ValueError,
    }
}

#[derive(Clone, Copy)]
pub(crate) enum ChopsRequest {
    Binary(BinaryOperation),
    Constant(u8),
    Offset { x: i32, y: i32 },
    Duplicate,
    Invert,
}

impl ChopsRequest {
    fn operation(self) -> OperationId {
        match self {
            Self::Binary(operation) => match operation {
                BinaryOperation::Add { .. } => OperationId::ChopsAdd,
                BinaryOperation::Subtract { .. } => OperationId::ChopsSubtract,
                BinaryOperation::Multiply => OperationId::ChopsMultiply,
                BinaryOperation::Screen => OperationId::ChopsScreen,
                BinaryOperation::Darker => OperationId::ChopsDarker,
                BinaryOperation::Lighter => OperationId::ChopsLighter,
                BinaryOperation::Difference => OperationId::ChopsDifference,
                BinaryOperation::Overlay => OperationId::ChopsOverlay,
                BinaryOperation::HardLight => OperationId::ChopsHardLight,
                BinaryOperation::SoftLight => OperationId::ChopsSoftLight,
                BinaryOperation::AddModulo => OperationId::ChopsAddModulo,
                BinaryOperation::SubtractModulo => OperationId::ChopsSubtractModulo,
                BinaryOperation::LogicalAnd => OperationId::ChopsLogicalAnd,
                BinaryOperation::LogicalOr => OperationId::ChopsLogicalOr,
                BinaryOperation::LogicalXor => OperationId::ChopsLogicalXor,
            },
            Self::Constant(_) => OperationId::ChopsConstant,
            Self::Offset { .. } => OperationId::ChopsOffset,
            Self::Duplicate => OperationId::ChopsDuplicate,
            Self::Invert => OperationId::ChopsInvert,
        }
    }
}

/// A fully admitted ImageChops operation with its selected mode strategy.
pub(crate) struct ChopsExecutionPlan {
    mode: Mode,
    request: ChopsRequest,
    other: Option<Arc<DynamicImage>>,
    selection: StrategySelection,
}

/// Registry plan paired with the borrowed source used for allocating execution.
pub(crate) struct PreparedChops<'a> {
    image: &'a DynamicImage,
    plan: ChopsExecutionPlan,
}

/// Two adjacent native Multiply and Screen operations sharing one operand.
///
/// Both declarations are assessed independently and must select the same
/// architecture before the fused kernel can execute.
pub(crate) struct PreparedChopsFusion<'a> {
    image: &'a DynamicImage,
    mode: Mode,
    other: Arc<DynamicImage>,
    selection: StrategySelection,
}

impl PreparedChopsFusion<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<super::helpers::fusion::FusedChopsOutput, PilError> {
        let mut output = execute_fused_multiply_screen(
            self.image,
            &self.other,
            self.mode,
            &self.selection,
        )?;
        let lanes = super::helpers::binary::output_hidden_lanes_with(
            self.image,
            &self.other,
            |left, right| {
                let multiplied = BinaryOperation::Multiply.apply_to_byte(left, right);
                BinaryOperation::Screen.apply_to_byte(multiplied, right)
            },
        )?;
        if !output.image.replace_hidden_pixel_lanes(lanes) {
            return Err(PilError::InternalError(
                "Chops fusion output physical lanes did not match its mode".into(),
            ));
        }
        Ok(output)
    }
}

fn execute_fused_multiply_screen(
    image: &DynamicImage,
    other: &DynamicImage,
    mode: Mode,
    selection: &StrategySelection,
) -> Result<super::helpers::fusion::FusedChopsOutput, PilError> {
    match mode {
        Mode::ImageLuma8 => super::luma8::ops::execute_fused(image, other, selection),
        Mode::ImageLumaA8 => super::luma_a8::ops::execute_fused(image, other, selection),
        Mode::ImageRgb8 => super::rgb8::ops::execute_fused(image, other, selection),
        Mode::ImageRgba8 => super::rgba8::ops::execute_fused(image, other, selection),
        _ => Err(PilError::InternalError(
            "Chops fusion received a mode without a fused strategy".into(),
        )),
    }
}

impl ChopsExecutionPlan {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn can_execute_in_place(&self, image: &DynamicImage) -> bool {
        if self.selection.implementation.architecture == Architecture::Default
            || image.mode() != self.mode
            || !matches!(
                self.request,
                ChopsRequest::Binary(BinaryOperation::Add { .. } | BinaryOperation::Subtract { .. })
            )
        {
            return false;
        }
        if image.width() == 0 || image.height() == 0 {
            return image.as_bytes().is_empty();
        }
        let height = image.height() as usize;
        let bytes = image.as_bytes();
        bytes.len() % height == 0 && bytes.len() / height >= 8
    }

    pub(crate) fn execute(self, image: &DynamicImage) -> Result<DynamicImage, PilError> {
        let selection = &self.selection;
        let other = self.other.as_deref();
        let mut output = super::helpers::dispatch::execute(
            self.mode, image, self.request, other, selection,
        )?;
        if let ChopsRequest::Binary(operation) = self.request {
            let other = other.ok_or_else(|| {
                PilError::InternalError("Chops binary plan lost its secondary image".into())
            })?;
            super::helpers::binary::carry_hidden_lanes(image, other, &mut output, operation)?;
        } else if matches!(self.request, ChopsRequest::Invert) {
            super::helpers::unary::carry_inverted_lane_fallback(image, &mut output)?;
        }
        Ok(output)
    }

    pub(crate) fn execute_in_place(self, image: &mut DynamicImage) -> Result<(), PilError> {
        let selection = &self.selection;
        if !self.can_execute_in_place(image) {
            return Err(PilError::InternalError(
                "Chops plan is not eligible for owned vector execution".into(),
            ));
        }
        let other = self.other.as_deref().ok_or_else(|| {
            PilError::InternalError("Chops in-place plan lost its secondary image".into())
        })?;
        let hidden_lanes = match self.request {
            ChopsRequest::Binary(operation) => Some(
                super::helpers::binary::output_hidden_lanes(image, other, operation)?,
            ),
            _ => None,
        };
        super::helpers::dispatch::execute_in_place(
            self.mode, image, self.request, other, selection,
        )?;
        if let Some(hidden_lanes) = hidden_lanes {
            if !image.replace_hidden_pixel_lanes(hidden_lanes) {
                return Err(PilError::InternalError(
                    "Chops in-place output physical lanes did not match its mode".into(),
                ));
            }
        }
        Ok(())
    }
}

impl PreparedChops<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        self.plan.selection()
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        self.plan.execute(self.image)
    }
}

pub(crate) fn prepare_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    context: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let plan = prepare_plan(input, op, context)?;
    Ok(crate::compute::registry::PreparedOperation::from_chops(
        PreparedChops {
            image: input.image,
            plan,
        },
    ))
}

pub(crate) fn prepare_plan(
    input: OperationInputs<'_>,
    op: &PipelineOp,
    context: &ExecutionContext,
) -> Result<ChopsExecutionPlan, PilError> {
    let request = request(op)?;
    let operation = request.operation();
    let mode = input.mode();
    let support = operation.support().for_mode(mode);
    if let ModeSupport::Rejected { reason, kind, .. } = support {
        return Err(kind.into_error(reason));
    }
    let other = secondary(op).map(|image| image.materialized_shared_for_ops()).transpose()?;
    let facts = match other.as_deref() {
        Some(image) => {
            let other = SecondaryInputFacts {
                mode: image.mode(),
                width: image.width(),
                height: image.height(),
                byte_len: Some(image.as_bytes().len()),
            };
            match op {
                PipelineOp::Add { scale, offset, .. }
                | PipelineOp::Subtract { scale, offset, .. } => {
                    input.facts().with_chops_affine_secondary(other, *scale, *offset)
                }
                _ => input.facts().with_chops_secondary(other),
            }
        }
        None => input.facts(),
    };
    let selection = match support.assess(context, facts)? {
        SupportAssessment::Ready(selection) => selection,
        SupportAssessment::Rejected { reason, kind, .. } => return Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => {
            return Err(PilError::InternalError("materialized Chops inputs have unresolved support facts".into()));
        }
    };
    Ok(ChopsExecutionPlan {
        mode,
        request,
        other,
        selection,
    })
}

/// Prepares the archived fused Multiply-then-Screen formula when both steps
/// share an operand and admit the same exact mode and architecture strategy.
pub(crate) fn prepare_fused_multiply_screen<'a>(
    image: &'a DynamicImage,
    multiply: &PipelineOp,
    screen: &PipelineOp,
    context: &ExecutionContext,
) -> Result<Option<PreparedChopsFusion<'a>>, PilError> {
    let (PipelineOp::Multiply { other: multiply_other }, PipelineOp::Screen { other: screen_other }) =
        (multiply, screen)
    else {
        return Ok(None);
    };
    if !multiply_other.shares_execution_source(screen_other) {
        return Ok(None);
    }

    let mode = image.mode();
    let channels = match mode {
        Mode::ImageLuma8 => 1,
        Mode::ImageLumaA8 => 2,
        Mode::ImageRgb8 => 3,
        Mode::ImageRgba8 => 4,
        _ => return Ok(None),
    };
    let Ok(dimensions) = crate::checked_dims::CheckedDims::new_allow_empty(
        image.width(),
        image.height(),
        channels,
    ) else {
        return Ok(None);
    };
    if dimensions.row_stride() == 0 || image.as_bytes().len() != dimensions.total_bytes() {
        return Ok(None);
    }

    let other = multiply_other.materialized_shared_for_ops()?;
    let screen_other = screen_other.materialized_shared_for_ops()?;
    if other.mode() != mode
        || screen_other.mode() != mode
        || other.dimensions() != image.dimensions()
        || screen_other.dimensions() != image.dimensions()
        || other.as_bytes().len() != dimensions.total_bytes()
        || screen_other.as_bytes().len() != dimensions.total_bytes()
    {
        return Ok(None);
    }
    let secondary = SecondaryInputFacts {
        mode: other.mode(),
        width: other.width(),
        height: other.height(),
        byte_len: Some(other.as_bytes().len()),
    };
    let facts = OperationInputs {
        image,
        palette: PaletteInput::default(),
    }
    .facts()
    .with_chops_secondary(secondary);
    let multiply_selection = match MULTIPLY_SUPPORT.for_mode(mode).assess(context, facts)? {
        SupportAssessment::Ready(selection) => selection,
        SupportAssessment::Rejected { .. } | SupportAssessment::Deferred { .. } => {
            return Ok(None);
        }
    };
    let screen_selection = match SCREEN_SUPPORT.for_mode(mode).assess(context, facts)? {
        SupportAssessment::Ready(selection) => selection,
        SupportAssessment::Rejected { .. } | SupportAssessment::Deferred { .. } => {
            return Ok(None);
        }
    };
    if multiply_selection.implementation.architecture
        != screen_selection.implementation.architecture
    {
        return Ok(None);
    }

    Ok(Some(PreparedChopsFusion {
        image,
        mode,
        other,
        selection: multiply_selection,
    }))
}

fn secondary(op: &PipelineOp) -> Option<&Arc<Image>> {
    match op {
        PipelineOp::Add { other, .. }
        | PipelineOp::Subtract { other, .. }
        | PipelineOp::Multiply { other }
        | PipelineOp::Screen { other }
        | PipelineOp::Darker { other }
        | PipelineOp::Lighter { other }
        | PipelineOp::Difference { other }
        | PipelineOp::Overlay { other }
        | PipelineOp::HardLight { other }
        | PipelineOp::SoftLight { other }
        | PipelineOp::AddModulo { other }
        | PipelineOp::SubtractModulo { other }
        | PipelineOp::LogicalAnd { other }
        | PipelineOp::LogicalOr { other }
        | PipelineOp::LogicalXor { other } => Some(other),
        _ => None,
    }
}

fn request(op: &PipelineOp) -> Result<ChopsRequest, PilError> {
    let request = match op {
        PipelineOp::Add { scale, offset, .. } => ChopsRequest::Binary(BinaryOperation::Add {
            scale: *scale as f32,
            offset: *offset as f32,
            exact_default_parameters: *scale == 1.0 && *offset == 0.0,
        }),
        PipelineOp::Subtract { scale, offset, .. } => {
            ChopsRequest::Binary(BinaryOperation::Subtract {
                scale: *scale as f32,
                offset: *offset as f32,
                exact_default_parameters: *scale == 1.0 && *offset == 0.0,
            })
        }
        PipelineOp::Multiply { .. } => ChopsRequest::Binary(BinaryOperation::Multiply),
        PipelineOp::Screen { .. } => ChopsRequest::Binary(BinaryOperation::Screen),
        PipelineOp::Darker { .. } => ChopsRequest::Binary(BinaryOperation::Darker),
        PipelineOp::Lighter { .. } => ChopsRequest::Binary(BinaryOperation::Lighter),
        PipelineOp::Difference { .. } => ChopsRequest::Binary(BinaryOperation::Difference),
        PipelineOp::Overlay { .. } => ChopsRequest::Binary(BinaryOperation::Overlay),
        PipelineOp::HardLight { .. } => ChopsRequest::Binary(BinaryOperation::HardLight),
        PipelineOp::SoftLight { .. } => ChopsRequest::Binary(BinaryOperation::SoftLight),
        PipelineOp::AddModulo { .. } => ChopsRequest::Binary(BinaryOperation::AddModulo),
        PipelineOp::SubtractModulo { .. } => ChopsRequest::Binary(BinaryOperation::SubtractModulo),
        PipelineOp::LogicalAnd { .. } => ChopsRequest::Binary(BinaryOperation::LogicalAnd),
        PipelineOp::LogicalOr { .. } => ChopsRequest::Binary(BinaryOperation::LogicalOr),
        PipelineOp::LogicalXor { .. } => ChopsRequest::Binary(BinaryOperation::LogicalXor),
        PipelineOp::Constant { value } => ChopsRequest::Constant(*value),
        PipelineOp::Offset { x, y } => ChopsRequest::Offset { x: *x, y: *y },
        PipelineOp::Duplicate => ChopsRequest::Duplicate,
        PipelineOp::InvertChops => ChopsRequest::Invert,
        _ => return Err(PilError::ValueError("expected ImageChops operation".into())),
    };
    Ok(request)
}

pub(super) fn execute_shared(
    image: &DynamicImage,
    request: ChopsRequest,
) -> Result<DynamicImage, PilError> {
    match request {
        ChopsRequest::Binary(_) => Err(PilError::InternalError(
            "Chops binary request bypassed its exact-mode strategy".into(),
        )),
        ChopsRequest::Constant(value) => Ok(super::helpers::unary::constant(
            image.width(),
            image.height(),
            value,
        )),
        ChopsRequest::Offset { .. } => Err(PilError::InternalError(
            "Chops Offset bypassed its exact-mode Default strategy".into(),
        )),
        ChopsRequest::Duplicate => Ok(super::helpers::unary::duplicate(image)),
        ChopsRequest::Invert => Err(PilError::InternalError(
            "Chops Invert bypassed its exact-mode Default strategy".into(),
        )),
    }
}
