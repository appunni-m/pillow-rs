//! GetData contracts declared by exact-mode operation owners.

use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OutputModeRule, RejectionKind,
};
use crate::raster::Mode;

const NO_INPUTS: &[InputContract] = &[];
const NO_OUTPUT: OutputModeRule = OutputModeRule::NoRasterResult;
const BAND_1: &[InputContract] = &[InputContract::CallerValidatedGetDataBandIndex { bands: 1 }];
const BAND_2: &[InputContract] = &[InputContract::CallerValidatedGetDataBandIndex { bands: 2 }];
const BAND_3: &[InputContract] = &[InputContract::CallerValidatedGetDataBandIndex { bands: 3 }];
const BAND_4: &[InputContract] = &[InputContract::CallerValidatedGetDataBandIndex { bands: 4 }];

#[derive(Clone, Copy)]
pub(crate) enum SupportKind {
    Pillow,
    CoreOnly,
    Rejected,
}

fn band_inputs(bands: u8) -> &'static [InputContract] {
    match bands {
        1 => BAND_1,
        2 => BAND_2,
        3 => BAND_3,
        4 => BAND_4,
        _ => NO_INPUTS,
    }
}

fn operation_support(
    operation: OperationId,
    mode: Mode,
    bands: u8,
    kind: SupportKind,
    band_indexed: bool,
) -> ModeSupport {
    if matches!(kind, SupportKind::Rejected) {
        return ModeSupport::Rejected {
            mode,
            reason: "image has wrong mode",
            kind: RejectionKind::ValueError,
        };
    }
    let inputs = if band_indexed {
        band_inputs(bands)
    } else {
        NO_INPUTS
    };
    match kind {
        SupportKind::Pillow => ModeSupport::cpu(operation, mode, NO_OUTPUT, inputs, &[]),
        SupportKind::CoreOnly => ModeSupport::cpu_core_only(
            operation,
            mode,
            NO_OUTPUT,
            inputs,
            &[],
            match operation {
                OperationId::GetData => {
                    "native high-depth GetData extends the Pillow mode contract"
                }
                OperationId::GetDataBand => {
                    "native high-depth GetDataBand extends the Pillow mode contract"
                }
                OperationId::GetDataBandInto => {
                    "native high-depth GetDataBandInto extends the Pillow mode contract"
                }
                _ => unreachable!("GetData support helper received another operation"),
            },
        ),
        SupportKind::Rejected => unreachable!("rejected modes return above"),
    }
}

pub(crate) fn mode_supports(
    mode: Mode,
    bands: u8,
    data_kind: SupportKind,
    band_kind: SupportKind,
    band_into_kind: SupportKind,
) -> (ModeSupport, ModeSupport, ModeSupport) {
    (
        operation_support(OperationId::GetData, mode, bands, data_kind, false),
        operation_support(OperationId::GetDataBand, mode, bands, band_kind, true),
        operation_support(
            OperationId::GetDataBandInto,
            mode,
            bands,
            band_into_kind,
            true,
        ),
    )
}

macro_rules! declare_get_data_support {
    ($mode:ident, $bands:literal, $data:ident, $band:ident, $band_into:ident $(,)?) => {
        pub(crate) fn get_data_mode_support() -> (
            $crate::compute::registry::support::ModeSupport,
            $crate::compute::registry::support::ModeSupport,
            $crate::compute::registry::support::ModeSupport,
        ) {
            $crate::compute::pool_cpu::ops::analysis::helpers::get_data_support::mode_supports(
                $crate::raster::Mode::$mode,
                $bands,
                $crate::compute::pool_cpu::ops::analysis::helpers::get_data_support::SupportKind::$data,
                $crate::compute::pool_cpu::ops::analysis::helpers::get_data_support::SupportKind::$band,
                $crate::compute::pool_cpu::ops::analysis::helpers::get_data_support::SupportKind::$band_into,
            )
        }
    };
}

pub(crate) use declare_get_data_support;
