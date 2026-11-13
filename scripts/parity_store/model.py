"""Typed records exchanged by project adapters and the shared parity store."""

from __future__ import annotations

from dataclasses import dataclass, field
from datetime import datetime
from typing import Mapping, Sequence, TypeAlias, Union


@dataclass(frozen=True, slots=True)
class AssetReference:
    asset_id: str


@dataclass(frozen=True, slots=True)
class StepBinding:
    step_id: str


@dataclass(frozen=True, slots=True)
class AssetRecord:
    """A case artifact by stable ID, with an optional stored source locator."""

    asset_id: str
    asset_kind: str
    locator: str | None = None
    media_type: str | None = None
    expected_sha256: bytes | None = None
    content: bytes | None = None


@dataclass(frozen=True, slots=True)
class CaseStep:
    """One ordered action in a project-owned workflow case."""

    step_id: str
    surface_id: str
    operation_id: str
    arguments: Mapping[str, "TypedValue"] = field(default_factory=dict)
    receiver_step_id: str | None = None


@dataclass(frozen=True, slots=True)
class ObservationSelector:
    """A public value selected for comparison after a case step."""

    step_id: str
    field_path: str | None = None


@dataclass(frozen=True, slots=True)
class ExpectedObservation:
    """A recorded public output for candidate-only cases."""

    step_id: str
    value: "TypedValue"


@dataclass(frozen=True, slots=True)
class FaultContract:
    """A declared fault injection value and public injection point."""

    contract_id: str
    injection_point: str
    fault_value: "TypedValue"


DimensionValue = str | int | bool
TypedValue: TypeAlias = Union[
    None,
    bool,
    int,
    float,
    str,
    bytes,
    AssetReference,
    StepBinding,
    Sequence["TypedValue"],
    Mapping[str, "TypedValue"],
]


@dataclass(frozen=True, slots=True)
class CaseRecord:
    project_id: str
    specification_digest: bytes
    case_id: str
    surface_id: str
    operation_id: str
    verification_kind: str
    source_document: str
    source_document_digest: bytes
    source_ordinal: int
    case_digest: bytes
    estimated_bytes: int
    dimensions: Mapping[str, DimensionValue]
    requirements: tuple[str, ...]
    profiles: tuple[str, ...]
    payload: Mapping[str, TypedValue]
    steps: tuple[CaseStep, ...] = ()
    observation_selectors: tuple[ObservationSelector, ...] = ()
    expected_observations: tuple[ExpectedObservation, ...] = ()
    fault_contract: FaultContract | None = None
    assets: tuple[AssetRecord, ...] = ()


@dataclass(frozen=True, slots=True)
class ImplementationIdentity:
    identity_digest: bytes
    project_id: str
    implementation_kind: str
    implementation_id: str
    profile_id: str | None
    revision: str
    runtime: str
    dirty: bool
    configuration_digest: bytes
    attributes: Mapping[str, DimensionValue] = field(default_factory=dict)


@dataclass(frozen=True, slots=True)
class ObservationRecord:
    step_id: str
    state: str
    value: TypedValue | None = None
    error_class: str | None = None
    error_kind: str | None = None
    error_message: str | None = None
    error_stage: str | None = None
    error_code: str | None = None
    reason: str | None = None


@dataclass(frozen=True, slots=True)
class ExecutionRecord:
    identity_digest: bytes
    project_id: str
    specification_digest: bytes
    case_id: str
    case_digest: bytes
    state: str
    started_at: datetime
    finished_at: datetime
    duration_ns: int | None
    observations: tuple[ObservationRecord, ...]
    reason: str | None = None
    details: Mapping[str, TypedValue] = field(default_factory=dict)


@dataclass(frozen=True, slots=True)
class DifferenceRecord:
    step_id: str
    value_path: str
    kind: str
    message: str
    source_value: TypedValue | None
    target_value: TypedValue | None
    source_value_present: bool = False
    target_value_present: bool = False


@dataclass(frozen=True, slots=True)
class ComparisonRecord:
    run_id: str
    target_identity_digest: bytes
    comparator_identity_digest: bytes
    project_id: str
    specification_digest: bytes
    case_id: str
    source_execution_id: int | None
    target_execution_id: int
    outcome: str
    differences: tuple[DifferenceRecord, ...]
    compared_at: datetime


@dataclass(frozen=True, slots=True)
class ComparisonDecision:
    """Project comparator output before it is bound to database run IDs."""

    outcome: str
    differences: tuple[DifferenceRecord, ...] = ()
