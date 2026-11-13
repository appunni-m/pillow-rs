"""Project-neutral catalog and evidence-store ports."""

from __future__ import annotations

from collections.abc import Iterable, Mapping, Sequence
from contextlib import AbstractContextManager
from typing import Protocol

from .model import (
    CaseRecord,
    ComparisonRecord,
    DimensionValue,
    ExecutionRecord,
    ImplementationIdentity,
)


class EvidenceStore(Protocol):
    """Typed catalog and evidence boundary for project parity adapters.

    Concrete stores keep their database connection, SQL dialect, transaction
    implementation, migrations, and physical result-table layout behind this
    port. Project importers and the shared campaign coordinator can therefore
    use the same interface without depending on a database driver.
    """

    def migrate(self) -> None:
        """Apply the store's ordered, checksummed schema migrations."""

    def transaction(self) -> AbstractContextManager[object]:
        """Return a context manager that commits on success and rolls back on error."""

    def register_project(self, project_id: str, display_name: str) -> None:
        """Register a project namespace."""

    def register_specification(
        self,
        project_id: str,
        specification_digest: bytes,
        schema_id: str,
        source_ref: str,
    ) -> None:
        """Bind a project specification digest to its declared schema and source."""

    def register_specification_documents(
        self,
        project_id: str,
        specification_digest: bytes,
        documents: Sequence[tuple[str, str | None, bytes]],
    ) -> None:
        """Register source document names, schema IDs, and content digests."""

    def specification_documents_match(
        self,
        project_id: str,
        specification_digest: bytes,
        documents: Sequence[tuple[str, str | None, bytes]],
    ) -> bool:
        """Check whether a specification digest has this exact source snapshot."""

    def existing_case_ids(
        self,
        project_id: str,
        specification_digest: bytes,
        case_ids: Sequence[str],
    ) -> set[str]:
        """Return requested cases already imported for a snapshot."""

    def register_operations(
        self,
        project_id: str,
        specification_digest: bytes,
        operations: Iterable[tuple[str, str]],
    ) -> None:
        """Register stable public surface and operation identifiers."""

    def register_requirements(
        self,
        project_id: str,
        specification_digest: bytes,
        requirement_ids: Iterable[str],
    ) -> None:
        """Register project-owned requirement identifiers."""

    def register_operation_requirements(
        self,
        project_id: str,
        specification_digest: bytes,
        links: Iterable[tuple[str, str, str]],
    ) -> None:
        """Link public operations to project-owned requirements."""

    def register_target_profile(
        self,
        project_id: str,
        specification_digest: bytes,
        profile_id: str,
        target_id: str,
        attributes: Mapping[str, DimensionValue],
    ) -> None:
        """Register one target profile and its project-declared attributes."""

    def register_operation_support(
        self,
        project_id: str,
        specification_digest: bytes,
        surface_id: str,
        operation_id: str,
        profile_id: str,
        support_state: str,
        reason: str | None,
        dimensions: Mapping[str, DimensionValue],
    ) -> None:
        """Register declared support for an operation/profile/dimension tuple."""

    def insert_cases(self, cases: Sequence[CaseRecord]) -> int:
        """Insert or verify a bounded set of typed cases and referenced assets."""

    def selected_case_digests(
        self,
        project_id: str,
        specification_digest: bytes,
        *,
        profile_id: str | None = None,
        case_ids: Sequence[str] | None = None,
    ) -> list[tuple[str, bytes]]:
        """Return the exact selected case IDs and immutable input digests."""

    def register_identity(self, identity: ImplementationIdentity) -> None:
        """Persist or verify an immutable implementation identity."""

    def create_run(
        self,
        run_id: str,
        project_id: str,
        specification_digest: bytes,
        case_set_digest: bytes,
        host_description: str,
        identities: Sequence[bytes],
        comparator_identity_digest: bytes,
        case_ids: Sequence[str],
        source_identity_digest: bytes | None = None,
    ) -> None:
        """Freeze a campaign's selected cases and execution identities."""

    def iter_case_batches(
        self,
        project_id: str,
        specification_digest: bytes,
        *,
        maximum_cases: int,
        maximum_estimated_bytes: int,
        profile_id: str,
        case_ids: Sequence[str] | None,
    ) -> Iterable[tuple[Sequence[CaseRecord], int]]:
        """Yield bounded case batches with their estimated materialization size."""

    def start_batch(
        self,
        run_id: str,
        identity_digest: bytes,
        batch_number: int,
        case_ids: Sequence[str],
        estimated_bytes: int,
    ) -> int:
        """Record a batch and its exact membership before execution."""

    def finish_batch(
        self,
        batch_id: int,
        *,
        error_kind: str | None = None,
        error_message: str | None = None,
    ) -> None:
        """Record batch completion or the harness failure that stopped it."""

    def set_run_state(self, run_id: str, run_state: str) -> None:
        """Set the terminal run state after execution or failure."""

    def verify_run_profile_complete(self, run_id: str, identity_digest: bytes) -> int:
        """Require complete, unique evidence for the frozen run selection."""

    def find_execution(
        self,
        identity_digest: bytes,
        project_id: str,
        specification_digest: bytes,
        case_id: str,
        case_digest: bytes,
    ) -> tuple[int, ExecutionRecord] | None:
        """Find evidence only for the exact implementation and case identity."""

    def record_execution(self, record: ExecutionRecord) -> int:
        """Persist an execution and return its stable store-local ID."""

    def record_comparison(self, record: ComparisonRecord) -> bytes:
        """Persist one comparator decision and return its result digest."""

    def record_run_error(
        self,
        run_id: str,
        *,
        scope: str,
        error_kind: str,
        message: str,
        external_id: str | None = None,
        batch_id: int | None = None,
    ) -> int:
        """Persist a harness or infrastructure error separately from parity."""
