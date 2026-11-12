//! GetChannel contracts and wrapper routes declared by exact-mode owners.

use crate::compute::Backend;
use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OperationModeRoute, OperationRouteGuard,
    OperationRoutePath, OutputModeRule, RejectionKind,
};
use crate::raster::Mode;

const GET_CHANNEL_1: &[InputContract] =
    &[InputContract::CallerValidatedGetChannelIndex { bands: 1 }];
const GET_CHANNEL_2: &[InputContract] =
    &[InputContract::CallerValidatedGetChannelIndex { bands: 2 }];
const GET_CHANNEL_3: &[InputContract] =
    &[InputContract::CallerValidatedGetChannelIndex { bands: 3 }];
const GET_CHANNEL_4: &[InputContract] =
    &[InputContract::CallerValidatedGetChannelIndex { bands: 4 }];
const IDENTITY_COPY: &[OperationRoutePath] = &[OperationRoutePath::IdentityCopy];
const PALETTE_COPY: &[OperationRoutePath] = &[OperationRoutePath::PalettePreservingCopy];
const SHARED_OR_CHILD: &[OperationRoutePath] = &[
    OperationRoutePath::SharedBufferIdentity {
        guards: &[
            OperationRouteGuard::LoadedImage,
            OperationRouteGuard::RequestedBackendIn(&[Backend::Cpu]),
            OperationRouteGuard::ByteStorageLengthMatchesPixelCount,
        ],
    },
    OperationRoutePath::PipelineChild(OperationId::ExtractBand),
];
const EAGER_OR_CHILD: &[OperationRoutePath] = &[
    OperationRoutePath::EagerCpuChild {
        operation: OperationId::ExtractBand,
        guards: &[
            OperationRouteGuard::LoadedImage,
            OperationRouteGuard::RequestedBackendIn(&[Backend::Cpu, Backend::Simd]),
            OperationRouteGuard::PixelCountAtMost(256),
        ],
    },
    OperationRoutePath::PipelineChild(OperationId::ExtractBand),
];
const CHILD: &[OperationRoutePath] = &[OperationRoutePath::PipelineChild(OperationId::ExtractBand)];

#[derive(Clone, Copy)]
pub(crate) enum SupportKind {
    Pillow,
    U16CoreOnly,
    F32CoreOnly,
    I16CoreOnly,
    Rejected,
}

#[derive(Clone, Copy)]
pub(crate) enum RouteKind {
    Identity,
    Palette,
    SharedOrChild,
    EagerOrChild,
    Child,
}

#[derive(Clone, Copy)]
pub(crate) enum OutputKind {
    Preserve,
    Luma8,
    I16,
    F32,
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn inputs(bands: u8) -> &'static [InputContract] {
    match bands {
        1 => GET_CHANNEL_1,
        2 => GET_CHANNEL_2,
        3 => GET_CHANNEL_3,
        4 => GET_CHANNEL_4,
        _ => &[],
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn output(kind: OutputKind) -> OutputModeRule {
    match kind {
        OutputKind::Preserve => OutputModeRule::Preserve,
        OutputKind::Luma8 => OutputModeRule::Fixed(Mode::ImageLuma8),
        OutputKind::I16 => OutputModeRule::Fixed(Mode::ImageI16),
        OutputKind::F32 => OutputModeRule::Fixed(Mode::ImageF32),
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn mode_route(
    mode: Mode,
    bands: u8,
    support_kind: SupportKind,
    route_kind: RouteKind,
    output_kind: OutputKind,
) -> OperationModeRoute {
    let output_mode = output(output_kind);
    let support = match support_kind {
        SupportKind::Pillow => ModeSupport::cpu(
            OperationId::GetChannel,
            mode,
            output_mode,
            inputs(bands),
            &[],
        ),
        SupportKind::U16CoreOnly => ModeSupport::cpu_core_only(
            OperationId::GetChannel,
            mode,
            output_mode,
            inputs(bands),
            &[],
            "native u16 GetChannel extends Pillow while preserving sample width",
        ),
        SupportKind::F32CoreOnly => ModeSupport::cpu_core_only(
            OperationId::GetChannel,
            mode,
            output_mode,
            inputs(bands),
            &[],
            "native f32 GetChannel extends Pillow while preserving sample width",
        ),
        SupportKind::I16CoreOnly => ModeSupport::cpu_core_only(
            OperationId::GetChannel,
            mode,
            output_mode,
            inputs(bands),
            &[],
            "native I;16 GetChannel identity extends Pillow's I16 mode rejection",
        ),
        SupportKind::Rejected => ModeSupport::Rejected {
            mode,
            reason: "image has wrong mode",
            kind: RejectionKind::ValueError,
        },
    };
    if matches!(support, ModeSupport::Rejected { .. }) {
        return OperationModeRoute::Prepared {
            support,
            intermediate_mode: None,
            paths: &[],
        };
    }
    let paths = match route_kind {
        RouteKind::Identity => IDENTITY_COPY,
        RouteKind::Palette => PALETTE_COPY,
        RouteKind::SharedOrChild => SHARED_OR_CHILD,
        RouteKind::EagerOrChild => EAGER_OR_CHILD,
        RouteKind::Child => CHILD,
    };
    OperationModeRoute::WrapperOwned {
        mode,
        support,
        paths,
    }
}

macro_rules! declare_get_channel_route {
    ($mode:ident, $bands:literal, $support:ident, $route:ident, $output:ident $(,)?) => {
        // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
        pub(crate) fn get_channel_mode_route() ->
            $crate::compute::registry::support::OperationModeRoute
        {
            $crate::compute::pool_cpu::ops::analysis::helpers::get_channel_support::mode_route(
                $crate::raster::Mode::$mode,
                $bands,
                $crate::compute::pool_cpu::ops::analysis::helpers::get_channel_support::SupportKind::$support,
                $crate::compute::pool_cpu::ops::analysis::helpers::get_channel_support::RouteKind::$route,
                $crate::compute::pool_cpu::ops::analysis::helpers::get_channel_support::OutputKind::$output,
            )
        }
    };
}

pub(crate) use declare_get_channel_route;
