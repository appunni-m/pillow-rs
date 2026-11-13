"""Project-neutral relational storage for parity and fault-contract runs."""

from .model import (
    AssetRecord,
    AssetReference,
    CaseRecord,
    CaseStep,
    ComparisonDecision,
    ComparisonRecord,
    DifferenceRecord,
    ExpectedObservation,
    FaultContract,
    ExecutionRecord,
    ImplementationIdentity,
    ObservationSelector,
    ObservationRecord,
    StepBinding,
)
from .postgres import PostgresStore
from .values import case_set_digest, canonical_value_digest
from .campaign import BatchExecutor, CampaignSummary, Comparator, run_target_profile
from .ports import EvidenceStore

__all__ = [
    "AssetRecord",
    "AssetReference",
    "CaseRecord",
    "CaseStep",
    "ComparisonDecision",
    "ComparisonRecord",
    "DifferenceRecord",
    "ExpectedObservation",
    "FaultContract",
    "ExecutionRecord",
    "ImplementationIdentity",
    "ObservationSelector",
    "ObservationRecord",
    "BatchExecutor",
    "CampaignSummary",
    "Comparator",
    "EvidenceStore",
    "PostgresStore",
    "StepBinding",
    "case_set_digest",
    "canonical_value_digest",
    "run_target_profile",
]
