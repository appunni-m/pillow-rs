"""PostgreSQL implementation of the project-neutral parity storage API."""

from __future__ import annotations

from collections import defaultdict, deque
from collections.abc import Iterable, Mapping, Sequence
from datetime import datetime, timezone
import hashlib
from pathlib import Path
from typing import Any

from .model import (
    AssetRecord,
    CaseRecord,
    CaseStep,
    ComparisonRecord,
    DimensionValue,
    ExecutionRecord,
    ExpectedObservation,
    FaultContract,
    ImplementationIdentity,
    ObservationSelector,
    ObservationRecord,
)
from .values import (
    FlatTree,
    canonical_value_digest,
    case_set_digest as compute_case_set_digest,
    compress_blob,
    expand_blob,
    flatten_value,
    rebuild_value_tree,
)

_MIGRATIONS = Path(__file__).parent / "migrations"


class PostgresStore:
    """Typed relational parity catalog and evidence store."""

    def __init__(self, connection: Any):
        self.connection = connection
        if hasattr(connection, "autocommit") and not connection.autocommit:
            try:
                connection.autocommit = True
            except Exception as exc:
                raise ValueError(
                    "PostgresStore requires an idle connection so it can manage transactions"
                ) from exc
        from .result_store import ResultStore

        self.results = ResultStore(self)

    @classmethod
    def connect(cls, dsn: str) -> "PostgresStore":
        try:
            import psycopg
        except ModuleNotFoundError as exc:
            raise RuntimeError(
                "PostgreSQL parity storage requires requirements-parity-store.txt"
            ) from exc
        return cls(psycopg.connect(dsn))

    def close(self) -> None:
        self.connection.close()

    def migrate(self) -> None:
        migration_files = sorted(_MIGRATIONS.glob("[0-9][0-9][0-9][0-9]_*.sql"))
        versions = [int(path.name.split("_", 1)[0]) for path in migration_files]
        if not versions or versions != list(range(1, len(versions) + 1)):
            raise RuntimeError("parity-store migrations must be numbered contiguously from 0001")
        exists = self.connection.execute(
            "SELECT to_regclass('public.parity_store_schema')"
        ).fetchone()[0]
        applied = {} if exists is None else {
            int(version): bytes(digest)
            for version, digest in self.connection.execute(
                "SELECT version, migration_sha256 FROM parity_store_schema ORDER BY version"
            ).fetchall()
        }
        unknown = sorted(set(applied) - set(versions))
        if unknown:
            raise RuntimeError(f"database has unknown parity-store migration versions: {unknown}")

        for version, path in zip(versions, migration_files, strict=True):
            sql = path.read_text(encoding="utf-8")
            digest = hashlib.sha256(sql.encode("utf-8")).digest()
            if version in applied:
                if applied[version] != digest:
                    raise RuntimeError(
                        f"parity-store migration {version} does not match its applied digest"
                    )
                continue
            if version != max(applied, default=0) + 1:
                raise RuntimeError(f"parity-store migration gap before version {version}")
            with self.connection.transaction():
                for statement in sql.split(";"):
                    if statement.strip():
                        self.connection.execute(statement)
                self.connection.execute(
                    "INSERT INTO parity_store_schema (version, migration_sha256) VALUES (%s, %s)",
                    (version, digest),
                )
            applied[version] = digest

    def register_project(self, project_id: str, display_name: str) -> None:
        self.connection.execute(
            "INSERT INTO parity_project (project_id, display_name) VALUES (%s, %s) "
            "ON CONFLICT (project_id) DO UPDATE SET display_name = EXCLUDED.display_name",
            (project_id, display_name),
        )

    def register_specification(
        self,
        project_id: str,
        specification_digest: bytes,
        schema_id: str,
        source_ref: str,
    ) -> None:
        self.connection.execute(
            "INSERT INTO parity_specification "
            "(project_id, specification_digest, schema_id, source_ref) "
            "VALUES (%s, %s, %s, %s) ON CONFLICT DO NOTHING",
            (project_id, specification_digest, schema_id, source_ref),
        )
        row = self.connection.execute(
            "SELECT schema_id, source_ref FROM parity_specification "
            "WHERE project_id = %s AND specification_digest = %s",
            (project_id, specification_digest),
        ).fetchone()
        if row != (schema_id, source_ref):
            raise ValueError("specification digest is already registered with different metadata")

    def register_specification_documents(
        self,
        project_id: str,
        specification_digest: bytes,
        documents: Sequence[tuple[str, str | None, bytes]],
    ) -> None:
        rows = [
            (project_id, specification_digest, ordinal, source_path, schema_id, digest)
            for ordinal, (source_path, schema_id, digest) in enumerate(documents)
        ]
        if any(len(digest) != 32 for _project, _spec, _ordinal, _path, _schema, digest in rows):
            raise ValueError("specification source documents require SHA-256 digests")
        existing = self.connection.execute(
            "SELECT document_ordinal, source_path, schema_id, document_sha256 "
            "FROM parity_specification_document WHERE project_id = %s "
            "AND specification_digest = %s ORDER BY document_ordinal",
            (project_id, specification_digest),
        ).fetchall()
        expected = [
            (ordinal, source_path, schema_id, digest)
            for _project, _spec, ordinal, source_path, schema_id, digest in rows
        ]
        actual = [
            (int(ordinal), source_path, schema_id, bytes(digest))
            for ordinal, source_path, schema_id, digest in existing
        ]
        if actual:
            if actual != expected:
                raise ValueError("specification digest is bound to different source documents")
            return
        if rows:
            self._executemany(
                "INSERT INTO parity_specification_document "
                "(project_id, specification_digest, document_ordinal, source_path, schema_id, document_sha256) "
                "VALUES (%s, %s, %s, %s, %s, %s)",
                rows,
            )

    def specification_documents_match(
        self,
        project_id: str,
        specification_digest: bytes,
        documents: Sequence[tuple[str, str | None, bytes]],
    ) -> bool:
        """Return whether this registered snapshot has the exact source set."""

        specification_exists = self.connection.execute(
            "SELECT 1 FROM parity_specification WHERE project_id = %s "
            "AND specification_digest = %s",
            (project_id, specification_digest),
        ).fetchone()
        if specification_exists is None:
            return False
        rows = self.connection.execute(
            "SELECT document_ordinal, source_path, schema_id, document_sha256 "
            "FROM parity_specification_document WHERE project_id = %s "
            "AND specification_digest = %s ORDER BY document_ordinal",
            (project_id, specification_digest),
        ).fetchall()
        actual = [
            (int(ordinal), path, schema_id, bytes(digest))
            for ordinal, path, schema_id, digest in rows
        ]
        expected = [
            (ordinal, path, schema_id, digest)
            for ordinal, (path, schema_id, digest) in enumerate(documents)
        ]
        return bool(actual) and actual == expected

    def existing_case_ids(
        self,
        project_id: str,
        specification_digest: bytes,
        case_ids: Sequence[str],
    ) -> set[str]:
        """Return the requested case IDs already imported for this snapshot."""

        if not case_ids:
            return set()
        rows = self.connection.execute(
            "SELECT case_id FROM parity_case WHERE project_id = %s "
            "AND specification_digest = %s AND case_id = ANY(%s)",
            (project_id, specification_digest, list(case_ids)),
        ).fetchall()
        return {str(row[0]) for row in rows}

    def register_operations(
        self,
        project_id: str,
        specification_digest: bytes,
        operations: Iterable[tuple[str, str]],
    ) -> None:
        rows = [
            (project_id, specification_digest, surface_id, operation_id)
            for surface_id, operation_id in operations
        ]
        if rows:
            self._executemany(
                "INSERT INTO parity_operation "
                "(project_id, specification_digest, surface_id, operation_id) "
                "VALUES (%s, %s, %s, %s) ON CONFLICT DO NOTHING",
                rows,
            )

    def register_requirements(
        self, project_id: str, specification_digest: bytes, requirement_ids: Iterable[str]
    ) -> None:
        rows = [(project_id, specification_digest, item) for item in requirement_ids]
        if rows:
            self._executemany(
                "INSERT INTO parity_requirement "
                "(project_id, specification_digest, requirement_id) "
                "VALUES (%s, %s, %s) ON CONFLICT DO NOTHING",
                rows,
            )

    def register_operation_requirements(
        self,
        project_id: str,
        specification_digest: bytes,
        links: Iterable[tuple[str, str, str]],
    ) -> None:
        rows = [
            (project_id, specification_digest, surface, operation, requirement)
            for surface, operation, requirement in links
        ]
        if rows:
            self._executemany(
                "INSERT INTO parity_operation_requirement "
                "(project_id, specification_digest, surface_id, operation_id, requirement_id) "
                "VALUES (%s, %s, %s, %s, %s) ON CONFLICT DO NOTHING",
                rows,
            )

    def register_target_profile(
        self,
        project_id: str,
        specification_digest: bytes,
        profile_id: str,
        target_id: str,
        attributes: Mapping[str, DimensionValue],
    ) -> None:
        self.connection.execute(
            "INSERT INTO parity_target (project_id, specification_digest, target_id, display_name, runtime) "
            "VALUES (%s, %s, %s, %s, %s) ON CONFLICT DO NOTHING",
            (project_id, specification_digest, target_id, target_id, "project-declared"),
        )
        self.connection.execute(
            "INSERT INTO parity_target_profile "
            "(project_id, specification_digest, profile_id, target_id) VALUES (%s, %s, %s, %s) "
            "ON CONFLICT DO NOTHING",
            (project_id, specification_digest, profile_id, target_id),
        )
        rows = [
            _dimension_row(project_id, specification_digest, profile_id, key, value)
            for key, value in attributes.items()
        ]
        if rows:
            self._executemany(
                "INSERT INTO parity_profile_attribute "
                "(project_id, specification_digest, profile_id, attribute_name, value_kind, "
                "text_value, integer_value, boolean_value) "
                "VALUES (%s, %s, %s, %s, %s, %s, %s, %s) ON CONFLICT DO NOTHING",
                rows,
            )

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
        support_digest = _stable_digest(
            {"dimensions": dict(dimensions), "state": support_state, "reason": reason}
        )
        self.connection.execute(
            "INSERT INTO parity_operation_support "
            "(support_digest, project_id, specification_digest, surface_id, operation_id, profile_id, support_state, reason) "
            "VALUES (%s, %s, %s, %s, %s, %s, %s, %s) ON CONFLICT DO NOTHING",
            (
                support_digest,
                project_id,
                specification_digest,
                surface_id,
                operation_id,
                profile_id,
                support_state,
                reason,
            ),
        )
        support_id = self.connection.execute(
            "SELECT support_id FROM parity_operation_support WHERE project_id = %s "
            "AND specification_digest = %s AND surface_id = %s AND operation_id = %s "
            "AND profile_id = %s AND support_digest = %s",
            (project_id, specification_digest, surface_id, operation_id, profile_id, support_digest),
        ).fetchone()[0]
        rows = [
            _support_dimension_row(support_id, name, value)
            for name, value in dimensions.items()
        ]
        if rows:
            self._executemany(
                "INSERT INTO parity_support_dimension "
                "(support_id, dimension_name, value_kind, text_value, integer_value, boolean_value) "
                "VALUES (%s, %s, %s, %s, %s, %s) ON CONFLICT DO NOTHING",
                rows,
            )

    def insert_cases(self, cases: Sequence[CaseRecord]) -> int:
        if not cases:
            return 0
        project_id = cases[0].project_id
        specification_digest = cases[0].specification_digest
        if any(
            case.project_id != project_id or case.specification_digest != specification_digest
            for case in cases
        ):
            raise ValueError("a case import batch must use one project and specification")

        existing = self._existing_case_digests(project_id, specification_digest, cases)
        new_cases = []
        for case in cases:
            prior = existing.get(case.case_id)
            if prior is None:
                new_cases.append(case)
            elif prior != case.case_digest:
                raise ValueError(f"case {case.case_id} changed under an unchanged specification digest")
        if not new_cases:
            return 0

        trees: list[FlatTree] = []
        value_references: list[dict[str, Any]] = []
        for case in new_cases:
            references: dict[str, Any] = {"payload": len(trees)}
            trees.append(flatten_value(case.payload))
            arguments = []
            for step_ordinal, step in enumerate(case.steps):
                for argument_name, value in step.arguments.items():
                    arguments.append((step_ordinal, argument_name, len(trees)))
                    trees.append(flatten_value(value))
            references["arguments"] = arguments
            expected = []
            for observation_ordinal, observation in enumerate(case.expected_observations):
                expected.append((observation_ordinal, observation.step_id, len(trees)))
                trees.append(flatten_value(observation.value))
            references["expected"] = expected
            if case.fault_contract is not None:
                references["fault"] = len(trees)
                trees.append(flatten_value(case.fault_contract.fault_value))
            value_references.append(references)

        asset_blobs: dict[bytes, tuple[int, str, bytes]] = {}
        for case in new_cases:
            if len({asset.asset_id for asset in case.assets}) != len(case.assets):
                raise ValueError(f"case {case.case_id} has duplicate asset IDs")
            for asset in case.assets:
                if asset.content is None:
                    continue
                digest = hashlib.sha256(asset.content).digest()
                if asset.expected_sha256 is not None and digest != asset.expected_sha256:
                    raise ValueError(f"asset {asset.asset_id} has an unexpected SHA-256 digest")
                codec, payload = compress_blob(asset.content)
                asset_blobs[digest] = (len(asset.content), codec, payload)
        self._insert_blobs(asset_blobs)

        value_maps = self._insert_value_trees(trees)
        self._insert_case_rows(new_cases, value_references, value_maps)
        return len(new_cases)

    def iter_case_batches(
        self,
        project_id: str,
        specification_digest: bytes,
        *,
        maximum_cases: int = 500,
        maximum_estimated_bytes: int = 64 * 1024 * 1024,
        after_case_id: str | None = None,
        profile_id: str | None = None,
        case_ids: Sequence[str] | None = None,
    ):
        if maximum_cases <= 0 or maximum_estimated_bytes <= 0:
            raise ValueError("batch limits must be positive")
        if case_ids is not None and len(case_ids) != len(set(case_ids)):
            raise ValueError("explicit case selection contains duplicate IDs")
        last_fetched = "" if after_case_id is None else after_case_id
        pending: deque[tuple[str, int]] = deque()
        while True:
            if not pending:
                query = (
                    "SELECT case_id, estimated_bytes FROM parity_case "
                    "WHERE project_id = %s AND specification_digest = %s AND case_id > %s "
                )
                parameters: list[Any] = [project_id, specification_digest, last_fetched]
                if profile_id is not None:
                    query += (
                        "AND EXISTS (SELECT 1 FROM parity_case_profile AS cp "
                        "WHERE cp.project_id = parity_case.project_id "
                        "AND cp.specification_digest = parity_case.specification_digest "
                        "AND cp.case_id = parity_case.case_id AND cp.profile_id = %s) "
                    )
                    parameters.append(profile_id)
                if case_ids is not None:
                    query += "AND case_id = ANY(%s) "
                    parameters.append(list(case_ids))
                query += "ORDER BY case_id LIMIT %s"
                parameters.append(maximum_cases)
                with self.connection.transaction():
                    rows = self.connection.execute(query, parameters).fetchall()
                if not rows:
                    break
                pending.extend((row[0], int(row[1])) for row in rows)
                last_fetched = rows[-1][0]

            selected: list[str] = []
            selected_bytes = 0
            while pending and len(selected) < maximum_cases:
                case_id, weight = pending[0]
                if selected and selected_bytes + weight > maximum_estimated_bytes:
                    break
                pending.popleft()
                selected.append(case_id)
                selected_bytes += weight
            if not selected:
                case_id, weight = pending.popleft()
                selected = [case_id]
                selected_bytes = weight
            with self.connection.transaction():
                records = self.load_cases(project_id, specification_digest, selected)
            yield records, selected_bytes

    def selected_case_digests(
        self,
        project_id: str,
        specification_digest: bytes,
        *,
        profile_id: str | None = None,
        case_ids: Sequence[str] | None = None,
    ) -> list[tuple[str, bytes]]:
        if case_ids is not None and len(case_ids) != len(set(case_ids)):
            raise ValueError("explicit case selection contains duplicate IDs")
        query = (
            "SELECT case_id, case_digest FROM parity_case "
            "WHERE project_id = %s AND specification_digest = %s "
        )
        parameters: list[Any] = [project_id, specification_digest]
        if profile_id is not None:
            query += (
                "AND EXISTS (SELECT 1 FROM parity_case_profile AS cp "
                "WHERE cp.project_id = parity_case.project_id "
                "AND cp.specification_digest = parity_case.specification_digest "
                "AND cp.case_id = parity_case.case_id AND cp.profile_id = %s) "
            )
            parameters.append(profile_id)
        if case_ids is not None:
            query += "AND case_id = ANY(%s) "
            parameters.append(list(case_ids))
        with self.connection.transaction():
            rows = self.connection.execute(query + "ORDER BY case_id", parameters).fetchall()
        return [(case_id, bytes(digest)) for case_id, digest in rows]

    def load_cases(
        self, project_id: str, specification_digest: bytes, case_ids: Sequence[str]
    ) -> list[CaseRecord]:
        if not case_ids:
            return []
        if len(case_ids) != len(set(case_ids)):
            raise ValueError("database batch request contains duplicate case IDs")
        ids = list(case_ids)
        rows = self.connection.execute(
            "SELECT case_id, surface_id, operation_id, verification_kind, source_document, "
            "source_document_digest, source_ordinal, case_digest, estimated_bytes "
            "FROM parity_case WHERE project_id = %s AND specification_digest = %s "
            "AND case_id = ANY(%s) ORDER BY case_id",
            (project_id, specification_digest, ids),
        ).fetchall()
        if len(rows) != len(ids):
            raise ValueError("database batch contains unknown or duplicate case IDs")
        case_rows = {row[0]: row for row in rows}
        roots = self.connection.execute(
            "SELECT case_id, value_id FROM parity_case_root WHERE project_id = %s "
            "AND specification_digest = %s AND case_id = ANY(%s)",
            (project_id, specification_digest, ids),
        ).fetchall()
        values = self._load_value_roots([int(row[1]) for row in roots])
        payloads = {case_id: values[int(value_id)] for case_id, value_id in roots}

        step_rows = self.connection.execute(
            "SELECT case_id, step_ordinal, step_id, surface_id, operation_id, receiver_step_id "
            "FROM parity_case_step WHERE project_id = %s AND specification_digest = %s "
            "AND case_id = ANY(%s) ORDER BY case_id, step_ordinal",
            (project_id, specification_digest, ids),
        ).fetchall()
        argument_rows = self.connection.execute(
            "SELECT case_id, step_ordinal, argument_name, value_id FROM parity_step_argument "
            "WHERE project_id = %s AND specification_digest = %s AND case_id = ANY(%s) "
            "ORDER BY case_id, step_ordinal, argument_name",
            (project_id, specification_digest, ids),
        ).fetchall()
        argument_values = self._load_value_roots([int(row[3]) for row in argument_rows])
        arguments: dict[tuple[str, int], dict[str, Any]] = defaultdict(dict)
        for case_id, ordinal, name, value_id in argument_rows:
            arguments[(case_id, int(ordinal))][name] = argument_values[int(value_id)]

        selector_rows = self.connection.execute(
            "SELECT case_id, observation_ordinal, step_id, field_path "
            "FROM parity_case_observation_selector WHERE project_id = %s "
            "AND specification_digest = %s AND case_id = ANY(%s) "
            "ORDER BY case_id, observation_ordinal",
            (project_id, specification_digest, ids),
        ).fetchall()
        selectors: dict[str, list[ObservationSelector]] = defaultdict(list)
        for case_id, _ordinal, step_id, field_path in selector_rows:
            selectors[case_id].append(ObservationSelector(step_id, field_path))

        expected_rows = self.connection.execute(
            "SELECT case_id, observation_ordinal, step_id, value_id "
            "FROM parity_case_expected_observation WHERE project_id = %s "
            "AND specification_digest = %s AND case_id = ANY(%s) "
            "ORDER BY case_id, observation_ordinal",
            (project_id, specification_digest, ids),
        ).fetchall()
        expected_values = self._load_value_roots([int(row[3]) for row in expected_rows])
        expected: dict[str, list[ExpectedObservation]] = defaultdict(list)
        for case_id, _ordinal, step_id, value_id in expected_rows:
            expected[case_id].append(
                ExpectedObservation(step_id, expected_values[int(value_id)])
            )

        fault_rows = self.connection.execute(
            "SELECT case_id, contract_id, injection_point, fault_value_id "
            "FROM parity_case_fault_contract WHERE project_id = %s "
            "AND specification_digest = %s AND case_id = ANY(%s)",
            (project_id, specification_digest, ids),
        ).fetchall()
        fault_values = self._load_value_roots([int(row[3]) for row in fault_rows])
        faults = {
            case_id: FaultContract(contract_id, injection_point, fault_values[int(value_id)])
            for case_id, contract_id, injection_point, value_id in fault_rows
        }

        dimensions: dict[str, dict[str, DimensionValue]] = defaultdict(dict)
        for case_id, name, kind, text_value, integer_value, boolean_value in self.connection.execute(
            "SELECT case_id, dimension_name, value_kind, text_value, integer_value, boolean_value "
            "FROM parity_case_dimension WHERE project_id = %s AND specification_digest = %s "
            "AND case_id = ANY(%s)",
            (project_id, specification_digest, ids),
        ).fetchall():
            dimensions[case_id][name] = _dimension_value(kind, text_value, integer_value, boolean_value)
        requirements: dict[str, list[str]] = defaultdict(list)
        for case_id, requirement_id in self.connection.execute(
            "SELECT case_id, requirement_id FROM parity_case_requirement "
            "WHERE project_id = %s AND specification_digest = %s AND case_id = ANY(%s) "
            "ORDER BY case_id, requirement_id",
            (project_id, specification_digest, ids),
        ).fetchall():
            requirements[case_id].append(requirement_id)
        profiles: dict[str, list[str]] = defaultdict(list)
        for case_id, profile_id in self.connection.execute(
            "SELECT case_id, profile_id FROM parity_case_profile "
            "WHERE project_id = %s AND specification_digest = %s AND case_id = ANY(%s) "
            "ORDER BY case_id, profile_id",
            (project_id, specification_digest, ids),
        ).fetchall():
            profiles[case_id].append(profile_id)
        assets = self._load_case_assets(project_id, specification_digest, ids)
        steps: dict[str, list[CaseStep]] = defaultdict(list)
        for case_id, ordinal, step_id, surface_id, operation_id, receiver_step_id in step_rows:
            step = CaseStep(
                step_id=step_id,
                surface_id=surface_id,
                operation_id=operation_id,
                arguments=arguments[(case_id, int(ordinal))],
                receiver_step_id=receiver_step_id,
            )
            steps[case_id].append(step)

        return [
            CaseRecord(
                project_id=project_id,
                specification_digest=specification_digest,
                case_id=case_id,
                surface_id=case_rows[case_id][1],
                operation_id=case_rows[case_id][2],
                verification_kind=case_rows[case_id][3],
                source_document=case_rows[case_id][4],
                source_document_digest=bytes(case_rows[case_id][5]),
                source_ordinal=case_rows[case_id][6],
                case_digest=bytes(case_rows[case_id][7]),
                estimated_bytes=int(case_rows[case_id][8]),
                dimensions=dimensions[case_id],
                requirements=tuple(requirements[case_id]),
                profiles=tuple(profiles[case_id]),
                payload=payloads[case_id],
                steps=tuple(steps[case_id]),
                observation_selectors=tuple(selectors[case_id]),
                expected_observations=tuple(expected[case_id]),
                fault_contract=faults.get(case_id),
                assets=tuple(assets[case_id]),
            )
            for case_id in ids
        ]

    def register_identity(self, identity: ImplementationIdentity) -> None:
        if identity.implementation_kind not in {"oracle", "target", "comparator"}:
            raise ValueError(f"invalid implementation kind: {identity.implementation_kind}")
        self.connection.execute(
            "INSERT INTO parity_implementation_identity "
            "(identity_digest, project_id, implementation_kind, implementation_id, profile_id, "
            "revision, runtime, dirty, configuration_digest) "
            "VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s) ON CONFLICT DO NOTHING",
            (
                identity.identity_digest,
                identity.project_id,
                identity.implementation_kind,
                identity.implementation_id,
                identity.profile_id,
                identity.revision,
                identity.runtime,
                identity.dirty,
                identity.configuration_digest,
            ),
        )
        row = self.connection.execute(
            "SELECT project_id, implementation_kind, implementation_id, profile_id, revision, "
            "runtime, dirty, configuration_digest FROM parity_implementation_identity "
            "WHERE identity_digest = %s",
            (identity.identity_digest,),
        ).fetchone()
        expected = (
            identity.project_id,
            identity.implementation_kind,
            identity.implementation_id,
            identity.profile_id,
            identity.revision,
            identity.runtime,
            identity.dirty,
            identity.configuration_digest,
        )
        if row != expected:
            raise ValueError("implementation identity digest is already bound to different data")
        rows = [
            _identity_attribute_row(identity.identity_digest, name, value)
            for name, value in identity.attributes.items()
        ]
        if rows:
            self._executemany(
                "INSERT INTO parity_implementation_attribute "
                "(identity_digest, attribute_name, value_kind, text_value, integer_value, boolean_value) "
                "VALUES (%s, %s, %s, %s, %s, %s) ON CONFLICT DO NOTHING",
                rows,
            )

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
        if len(case_set_digest) != 32:
            raise ValueError("case-set digest must be SHA-256")
        if not case_ids or len(case_ids) != len(set(case_ids)):
            raise ValueError("run selection must contain unique case IDs")
        comparator = self.connection.execute(
            "SELECT project_id, implementation_kind FROM parity_implementation_identity "
            "WHERE identity_digest = %s",
            (comparator_identity_digest,),
        ).fetchone()
        if comparator != (project_id, "comparator"):
            raise ValueError("run comparator identity is missing or belongs to another project")
        for identity_digest in identities:
            target = self.connection.execute(
                "SELECT project_id, implementation_kind FROM parity_implementation_identity "
                "WHERE identity_digest = %s",
                (identity_digest,),
            ).fetchone()
            if target != (project_id, "target"):
                raise ValueError("run profile identity is missing or is not a target for this project")
        if source_identity_digest is not None:
            source = self.connection.execute(
                "SELECT project_id, implementation_kind FROM parity_implementation_identity "
                "WHERE identity_digest = %s",
                (source_identity_digest,),
            ).fetchone()
            if source != (project_id, "oracle"):
                raise ValueError("run source identity is missing or is not an oracle for this project")
        self.connection.execute(
            "INSERT INTO parity_run "
            "(run_id, project_id, specification_digest, case_set_digest, started_at, run_state, host_description) "
            "VALUES (%s, %s, %s, %s, %s, 'running', %s) ON CONFLICT (run_id) DO NOTHING",
            (run_id, project_id, specification_digest, case_set_digest, datetime.now(timezone.utc), host_description),
        )
        run_row = self.connection.execute(
            "SELECT project_id, specification_digest, case_set_digest FROM parity_run WHERE run_id = %s",
            (run_id,),
        ).fetchone()
        if run_row != (project_id, specification_digest, case_set_digest):
            raise ValueError("run ID is already bound to a different selection")
        rows = [(run_id, identity) for identity in identities]
        if rows:
            self._executemany(
                "INSERT INTO parity_run_profile (run_id, identity_digest) VALUES (%s, %s) ON CONFLICT DO NOTHING",
                rows,
            )
        self.connection.execute(
            "INSERT INTO parity_run_comparator (run_id, identity_digest) VALUES (%s, %s) "
            "ON CONFLICT (run_id) DO NOTHING",
            (run_id, comparator_identity_digest),
        )
        comparator_row = self.connection.execute(
            "SELECT identity_digest FROM parity_run_comparator WHERE run_id = %s",
            (run_id,),
        ).fetchone()
        if comparator_row is None or bytes(comparator_row[0]) != comparator_identity_digest:
            raise ValueError("run ID is already bound to another comparator")
        existing_source = self.connection.execute(
            "SELECT identity_digest FROM parity_run_oracle WHERE run_id = %s",
            (run_id,),
        ).fetchone()
        if source_identity_digest is None:
            if existing_source is not None:
                raise ValueError("run ID is already bound to a source identity")
        else:
            self.connection.execute(
                "INSERT INTO parity_run_oracle (run_id, identity_digest) VALUES (%s, %s) "
                "ON CONFLICT (run_id) DO NOTHING",
                (run_id, source_identity_digest),
            )
            existing_source = self.connection.execute(
                "SELECT identity_digest FROM parity_run_oracle WHERE run_id = %s",
                (run_id,),
            ).fetchone()
            if existing_source is None or bytes(existing_source[0]) != source_identity_digest:
                raise ValueError("run ID is already bound to another source identity")
        case_rows = self.connection.execute(
            "SELECT case_id, case_digest FROM parity_case WHERE project_id = %s "
            "AND specification_digest = %s AND case_id = ANY(%s) ORDER BY case_id",
            (project_id, specification_digest, list(case_ids)),
        ).fetchall()
        if {row[0] for row in case_rows} != set(case_ids):
            raise ValueError("run selection contains cases outside the specification")
        if compute_case_set_digest([(row[0], bytes(row[1])) for row in case_rows]) != case_set_digest:
            raise ValueError("run case-set digest does not match imported cases")
        selected_rows = [
            (
                run_id,
                ordinal,
                project_id,
                specification_digest,
                case_id,
                bytes(case_digest),
            )
            for ordinal, (case_id, case_digest) in enumerate(case_rows)
        ]
        prior_count = int(
            self.connection.execute(
                "SELECT COUNT(*) FROM parity_run_case WHERE run_id = %s", (run_id,)
            ).fetchone()[0]
        )
        if prior_count:
            if prior_count != len(selected_rows):
                raise ValueError("run ID already has a different case selection")
            # Every ordinal is immutable; reject any case-set replacement.
            existing_rows = self.connection.execute(
                "SELECT case_id, case_digest FROM parity_run_case WHERE run_id = %s "
                "ORDER BY case_ordinal",
                (run_id,),
            ).fetchall()
            if [
                (case_id, bytes(case_digest)) for case_id, case_digest in existing_rows
            ] != [(case_id, bytes(digest)) for case_id, digest in case_rows]:
                raise ValueError("run ID already has a different case selection")
        else:
            for start in range(0, len(selected_rows), 5000):
                self._executemany(
                    "INSERT INTO parity_run_case "
                    "(run_id, case_ordinal, project_id, specification_digest, case_id, case_digest) "
                    "VALUES (%s, %s, %s, %s, %s, %s)",
                    selected_rows[start : start + 5000],
                )

    def set_run_state(self, run_id: str, run_state: str) -> None:
        if run_state not in {"completed", "failed", "cancelled"}:
            raise ValueError(f"invalid terminal parity run state: {run_state}")
        self.connection.execute(
            "UPDATE parity_run SET run_state = %s, finished_at = %s WHERE run_id = %s AND run_state = 'running'",
            (run_state, datetime.now(timezone.utc), run_id),
        )

    def verify_run_profile_complete(self, run_id: str, identity_digest: bytes) -> int:
        """Require exactly one completed batch and comparison per selected case."""

        target = self.connection.execute(
            "SELECT 1 FROM parity_run_profile WHERE run_id = %s AND identity_digest = %s",
            (run_id, identity_digest),
        ).fetchone()
        if target is None:
            raise ValueError("target identity is not part of the parity run")

        selected_count = int(
            self.connection.execute(
                "SELECT COUNT(*) FROM parity_run_case WHERE run_id = %s", (run_id,)
            ).fetchone()[0]
        )
        comparison_count = int(
            self.connection.execute(
                "SELECT COUNT(*) FROM parity_comparison WHERE run_id = %s "
                "AND target_identity_digest = %s",
                (run_id, identity_digest),
            ).fetchone()[0]
        )
        unfinished_batches = int(
            self.connection.execute(
                "SELECT COUNT(*) FROM parity_batch WHERE run_id = %s "
                "AND identity_digest = %s AND batch_state <> 'completed'",
                (run_id, identity_digest),
            ).fetchone()[0]
        )
        duplicate_batch_cases = self.connection.execute(
            "SELECT COUNT(*) FROM ("
            "SELECT bc.project_id, bc.specification_digest, bc.case_id "
            "FROM parity_batch_case AS bc JOIN parity_batch AS b USING (batch_id) "
            "WHERE b.run_id = %s AND b.identity_digest = %s "
            "GROUP BY bc.project_id, bc.specification_digest, bc.case_id "
            "HAVING COUNT(*) <> 1) AS duplicate_cases",
            (run_id, identity_digest),
        ).fetchone()[0]
        missing_batch_cases = self.connection.execute(
            "SELECT COUNT(*) FROM parity_run_case AS rc "
            "LEFT JOIN (SELECT bc.project_id, bc.specification_digest, bc.case_id "
            "FROM parity_batch_case AS bc JOIN parity_batch AS b USING (batch_id) "
            "WHERE b.run_id = %s AND b.identity_digest = %s) AS assigned "
            "USING (project_id, specification_digest, case_id) "
            "WHERE rc.run_id = %s AND assigned.case_id IS NULL",
            (run_id, identity_digest, run_id),
        ).fetchone()[0]
        missing_comparisons = self.connection.execute(
            "SELECT COUNT(*) FROM parity_run_case AS rc "
            "LEFT JOIN parity_comparison AS pc ON pc.run_id = rc.run_id "
            "AND pc.project_id = rc.project_id "
            "AND pc.specification_digest = rc.specification_digest "
            "AND pc.case_id = rc.case_id AND pc.target_identity_digest = %s "
            "WHERE rc.run_id = %s AND pc.case_id IS NULL",
            (identity_digest, run_id),
        ).fetchone()[0]
        if (
            selected_count == 0
            or comparison_count != selected_count
            or unfinished_batches
            or duplicate_batch_cases
            or missing_batch_cases
            or missing_comparisons
        ):
            raise ValueError(
                "parity profile is incomplete: "
                f"selected={selected_count}, comparisons={comparison_count}, "
                f"unfinished_batches={unfinished_batches}, "
                f"duplicate_batch_cases={duplicate_batch_cases}, "
                f"missing_batch_cases={missing_batch_cases}, "
                f"missing_comparisons={missing_comparisons}"
            )
        return selected_count

    def start_batch(
        self,
        run_id: str,
        identity_digest: bytes,
        batch_number: int,
        case_ids: Sequence[str],
        estimated_bytes: int,
    ) -> int:
        if not case_ids:
            raise ValueError("cannot record an empty parity batch")
        row = self.connection.execute(
            "INSERT INTO parity_batch "
            "(run_id, identity_digest, batch_number, selected_count, estimated_bytes, first_case_id, last_case_id, batch_state, started_at) "
            "VALUES (%s, %s, %s, %s, %s, %s, %s, 'started', %s) "
            "ON CONFLICT (run_id, identity_digest, batch_number) DO UPDATE "
            "SET batch_state = 'started', started_at = EXCLUDED.started_at, finished_at = NULL, error_kind = NULL, error_message = NULL "
            "RETURNING batch_id",
            (
                run_id,
                identity_digest,
                batch_number,
                len(case_ids),
                estimated_bytes,
                case_ids[0],
                case_ids[-1],
                datetime.now(timezone.utc),
            ),
        ).fetchone()
        batch_id = int(row[0])
        run = self.connection.execute(
            "SELECT project_id, specification_digest FROM parity_run WHERE run_id = %s",
            (run_id,),
        ).fetchone()
        if run is None:
            raise ValueError("unknown parity run")
        project_id, specification_digest = run
        case_rows = self.connection.execute(
            "SELECT case_id, case_digest FROM parity_run_case WHERE run_id = %s "
            "AND case_id = ANY(%s)",
            (run_id, list(case_ids)),
        ).fetchall()
        case_digests = {case_id: bytes(digest) for case_id, digest in case_rows}
        if set(case_digests) != set(case_ids):
            raise ValueError("parity batch contains unknown cases")
        prior_cases = self.connection.execute(
            "SELECT run_id, project_id, specification_digest, case_id, case_digest "
            "FROM parity_batch_case WHERE batch_id = %s ORDER BY case_ordinal",
            (batch_id,),
        ).fetchall()
        expected_cases = [
            (run_id, project_id, specification_digest, case_id, case_digests[case_id])
            for case_id in case_ids
        ]
        if prior_cases and prior_cases != expected_cases:
            raise ValueError("parity batch number is already bound to different case IDs")
        if not prior_cases:
            self._executemany(
                "INSERT INTO parity_batch_case "
                "(batch_id, run_id, case_ordinal, project_id, specification_digest, case_id, case_digest) "
                "VALUES (%s, %s, %s, %s, %s, %s, %s)",
                [
                    (
                        batch_id,
                        run_id,
                        ordinal,
                        project_id,
                        specification_digest,
                        case_id,
                        case_digests[case_id],
                    )
                    for ordinal, case_id in enumerate(case_ids)
                ],
            )
        return batch_id

    def finish_batch(self, batch_id: int, *, error_kind: str | None = None, error_message: str | None = None) -> None:
        state = "failed" if error_kind is not None else "completed"
        self.connection.execute(
            "UPDATE parity_batch SET batch_state = %s, finished_at = %s, error_kind = %s, error_message = %s "
            "WHERE batch_id = %s",
            (state, datetime.now(timezone.utc), error_kind, error_message, batch_id),
        )

    def _executemany(self, query: str, rows: Sequence[tuple[Any, ...]]) -> None:
        if not rows:
            return
        with self.connection.cursor() as cursor:
            cursor.executemany(query, rows)

    def _existing_case_digests(
        self, project_id: str, specification_digest: bytes, cases: Sequence[CaseRecord]
    ) -> dict[str, bytes]:
        ids = [case.case_id for case in cases]
        rows = self.connection.execute(
            "SELECT case_id, case_digest FROM parity_case WHERE project_id = %s "
            "AND specification_digest = %s AND case_id = ANY(%s)",
            (project_id, specification_digest, ids),
        ).fetchall()
        return {case_id: bytes(digest) for case_id, digest in rows}

    def _insert_value_trees(self, trees: Sequence[FlatTree]) -> list[dict[tuple[str | int, ...], int]]:
        if not trees:
            return []
        node_count = sum(len(tree.nodes) for tree in trees)
        global_ids = [
            int(row[0])
            for row in self.connection.execute(
                "SELECT nextval(pg_get_serial_sequence('public.parity_value', 'value_id')) "
                "FROM generate_series(1, %s)",
                (node_count,),
            ).fetchall()
        ]
        tree_offsets: list[int] = []
        offset = 0
        for tree in trees:
            tree_offsets.append(offset)
            offset += len(tree.nodes)

        blob_rows: dict[bytes, tuple[int, str, bytes]] = {}
        value_rows: list[tuple[Any, ...]] = []
        numeric_array_rows: list[tuple[Any, ...]] = []
        child_rows: list[tuple[int, int, str | None, int]] = []
        result_maps: list[dict[tuple[str | int, ...], int]] = []
        for tree_index, tree in enumerate(trees):
            base = tree_offsets[tree_index]
            result_maps.append(
                {path: global_ids[base + local_id] for path, local_id in tree.path_ids.items()}
            )
            for node in tree.nodes:
                if node.blob_sha256 is not None and node.blob_raw is not None:
                    codec, payload = compress_blob(node.blob_raw)
                    prior = blob_rows.get(node.blob_sha256)
                    current = (len(node.blob_raw), codec, payload)
                    if prior is not None and prior[0] != current[0]:
                        raise ValueError("SHA-256 collision with different binary lengths")
                    blob_rows[node.blob_sha256] = current
                value_rows.append(
                    (
                        global_ids[base + node.local_id],
                        node.kind,
                        node.text_value,
                        node.integer_value,
                        node.boolean_value,
                        node.float64_bits,
                        (
                            None
                            if node.numeric_array_dtype is not None
                            else node.blob_sha256
                        ),
                        node.reference_id,
                    )
                )
                if node.numeric_array_dtype is not None:
                    if node.blob_sha256 is None or node.numeric_array_shape is None:
                        raise ValueError("packed numeric array is missing its blob or shape")
                    numeric_array_rows.append(
                        (
                            global_ids[base + node.local_id],
                            node.blob_sha256,
                            node.numeric_array_dtype,
                            list(node.numeric_array_shape),
                        )
                    )
            for child in tree.children:
                child_rows.append(
                    (
                        global_ids[base + child.parent_local_id],
                        child.child_ordinal,
                        child.object_key,
                        global_ids[base + child.child_local_id],
                    )
                )

        self._insert_blobs(blob_rows)
        self._executemany(
            "INSERT INTO parity_value (value_id, value_kind, text_value, integer_value, boolean_value, float64_bits, blob_sha256, reference_id) "
            "OVERRIDING SYSTEM VALUE VALUES (%s, %s, %s, %s, %s, %s, %s, %s)",
            value_rows,
        )
        self._executemany(
            "INSERT INTO parity_value_child (parent_value_id, child_ordinal, object_key, child_value_id) "
            "VALUES (%s, %s, %s, %s)",
            child_rows,
        )
        self._executemany(
            "INSERT INTO parity_value_numeric_array "
            "(value_id, blob_sha256, array_dtype, array_shape) "
            "VALUES (%s, %s, %s, %s)",
            numeric_array_rows,
        )
        return result_maps

    def _insert_blobs(self, blob_rows: Mapping[bytes, tuple[int, str, bytes]]) -> None:
        rows = [(digest, *values) for digest, values in blob_rows.items()]
        if rows:
            self._executemany(
                "INSERT INTO parity_blob (sha256, raw_length, compression, payload) "
                "VALUES (%s, %s, %s, %s) ON CONFLICT (sha256) DO NOTHING",
                rows,
            )
            lengths = self.connection.execute(
                "SELECT sha256, raw_length FROM parity_blob WHERE sha256 = ANY(%s)",
                ([row[0] for row in rows],),
            ).fetchall()
            actual = {bytes(digest): int(length) for digest, length in lengths}
            if any(actual.get(digest) != values[0] for digest, values in blob_rows.items()):
                raise ValueError("existing content-addressed blob has a conflicting length")

    def _insert_case_rows(
        self,
        cases: Sequence[CaseRecord],
        value_references: Sequence[Mapping[str, Any]],
        value_maps: Sequence[Mapping[tuple[str | int, ...], int]],
    ) -> None:
        case_rows = [
            (
                case.project_id,
                case.specification_digest,
                case.case_id,
                case.surface_id,
                case.operation_id,
                case.verification_kind,
                case.source_document,
                case.source_document_digest,
                case.source_ordinal,
                case.case_digest,
                case.estimated_bytes,
            )
            for case in cases
        ]
        self._executemany(
            "INSERT INTO parity_case "
            "(project_id, specification_digest, case_id, surface_id, operation_id, verification_kind, "
            "source_document, source_document_digest, source_ordinal, case_digest, estimated_bytes) "
            "VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)",
            case_rows,
        )
        roots = []
        dimensions = []
        requirement_rows = []
        profile_rows = []
        step_rows = []
        argument_rows = []
        observation_rows = []
        expected_rows = []
        asset_rows = []
        fault_rows = []
        for case, references in zip(cases, value_references, strict=True):
            payload_values = value_maps[int(references["payload"])]
            roots.append(
                (
                    case.project_id,
                    case.specification_digest,
                    case.case_id,
                    payload_values[()],
                )
            )
            for name, value in case.dimensions.items():
                dimensions.append(_case_dimension_row(case, name, value))
            requirement_rows.extend(
                (case.project_id, case.specification_digest, case.case_id, item)
                for item in case.requirements
            )
            profile_rows.extend(
                (case.project_id, case.specification_digest, case.case_id, item)
                for item in case.profiles
            )
            for step_ordinal, step in enumerate(case.steps):
                step_rows.append(
                    (
                        case.project_id,
                        case.specification_digest,
                        case.case_id,
                        step_ordinal,
                        step.step_id,
                        step.surface_id,
                        step.operation_id,
                        step.receiver_step_id,
                    )
                )
            for step_ordinal, argument_name, tree_index in references["arguments"]:
                argument_rows.append(
                    (
                        case.project_id,
                        case.specification_digest,
                        case.case_id,
                        step_ordinal,
                        argument_name,
                        value_maps[tree_index][()],
                    )
                )
            observation_rows.extend(
                (
                    case.project_id,
                    case.specification_digest,
                    case.case_id,
                    ordinal,
                    observation.step_id,
                    observation.field_path,
                )
                for ordinal, observation in enumerate(case.observation_selectors)
            )
            for observation_ordinal, step_id, tree_index in references["expected"]:
                expected_rows.append(
                    (
                        case.project_id,
                        case.specification_digest,
                        case.case_id,
                        observation_ordinal,
                        step_id,
                        value_maps[tree_index][()],
                    )
                )
            if case.fault_contract is not None:
                fault_rows.append(
                    (
                        case.project_id,
                        case.specification_digest,
                        case.case_id,
                        case.fault_contract.contract_id,
                        case.fault_contract.injection_point,
                        value_maps[int(references["fault"])][()],
                    )
                )
            for asset in case.assets:
                content_digest = (
                    hashlib.sha256(asset.content).digest()
                    if asset.content is not None
                    else None
                )
                asset_rows.append(
                    (
                        case.project_id,
                        case.specification_digest,
                        case.case_id,
                        asset.asset_id,
                        asset.asset_kind,
                        asset.locator,
                        asset.media_type,
                        asset.expected_sha256,
                        content_digest,
                    )
                )

        self._executemany(
            "INSERT INTO parity_case_root (project_id, specification_digest, case_id, value_id) VALUES (%s, %s, %s, %s)",
            roots,
        )
        self._insert_many(
            "INSERT INTO parity_case_dimension "
            "(project_id, specification_digest, case_id, dimension_name, value_kind, text_value, integer_value, boolean_value) "
            "VALUES (%s, %s, %s, %s, %s, %s, %s, %s)",
            dimensions,
        )
        self._insert_many(
            "INSERT INTO parity_case_requirement "
            "(project_id, specification_digest, case_id, requirement_id) VALUES (%s, %s, %s, %s)",
            requirement_rows,
        )
        self._insert_many(
            "INSERT INTO parity_case_profile (project_id, specification_digest, case_id, profile_id) VALUES (%s, %s, %s, %s)",
            profile_rows,
        )
        self._insert_many(
            "INSERT INTO parity_case_step "
            "(project_id, specification_digest, case_id, step_ordinal, step_id, surface_id, operation_id, receiver_step_id) "
            "VALUES (%s, %s, %s, %s, %s, %s, %s, %s)",
            step_rows,
        )
        self._insert_many(
            "INSERT INTO parity_step_argument "
            "(project_id, specification_digest, case_id, step_ordinal, argument_name, value_id) "
            "VALUES (%s, %s, %s, %s, %s, %s)",
            argument_rows,
        )
        self._insert_many(
            "INSERT INTO parity_case_observation_selector "
            "(project_id, specification_digest, case_id, observation_ordinal, step_id, field_path) "
            "VALUES (%s, %s, %s, %s, %s, %s)",
            observation_rows,
        )
        self._insert_many(
            "INSERT INTO parity_case_expected_observation "
            "(project_id, specification_digest, case_id, observation_ordinal, step_id, value_id) "
            "VALUES (%s, %s, %s, %s, %s, %s)",
            expected_rows,
        )
        self._insert_many(
            "INSERT INTO parity_case_asset "
            "(project_id, specification_digest, case_id, asset_id, asset_kind, locator, media_type, expected_sha256, content_sha256) "
            "VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s)",
            asset_rows,
        )
        self._insert_many(
            "INSERT INTO parity_case_fault_contract "
            "(project_id, specification_digest, case_id, contract_id, injection_point, fault_value_id) "
            "VALUES (%s, %s, %s, %s, %s, %s)",
            fault_rows,
        )

    def _insert_many(self, query: str, rows: Sequence[tuple[Any, ...]]) -> None:
        if rows:
            self._executemany(query, rows)

    def _load_value_roots(self, root_ids: Sequence[int]) -> dict[int, Any]:
        if not root_ids:
            return {}
        rows = self.connection.execute(
            "WITH RECURSIVE node_tree(root_value_id, value_id, parent_value_id, child_ordinal, object_key) AS ("
            "SELECT value_id, value_id, NULL::BIGINT, NULL::INTEGER, NULL::TEXT FROM parity_value WHERE value_id = ANY(%s) "
            "UNION ALL SELECT tree.root_value_id, child.child_value_id, child.parent_value_id, child.child_ordinal, child.object_key "
            "FROM node_tree AS tree JOIN parity_value_child AS child ON child.parent_value_id = tree.value_id) "
            "SELECT tree.root_value_id, tree.value_id, tree.parent_value_id, tree.child_ordinal, tree.object_key, "
            "value.value_kind, value.text_value, value.integer_value, value.boolean_value, value.float64_bits, "
            "value.blob_sha256, value.reference_id, packed.array_dtype, packed.array_shape, packed.blob_sha256 "
            "FROM node_tree AS tree JOIN parity_value AS value ON value.value_id = tree.value_id "
            "LEFT JOIN parity_value_numeric_array AS packed ON packed.value_id = value.value_id "
            "ORDER BY tree.root_value_id, tree.parent_value_id, tree.child_ordinal",
            (list(root_ids),),
        ).fetchall()
        value_rows: list[dict[str, Any]] = []
        children: dict[int, list[tuple[int, str | None, int]]] = defaultdict(list)
        blob_digests: set[bytes] = set()
        for row in rows:
            root_id, value_id, parent_id, ordinal, key = row[:5]
            if parent_id is not None:
                children[int(parent_id)].append((int(ordinal), key, int(value_id)))
            if row[10] is not None:
                blob_digests.add(bytes(row[10]))
            if row[14] is not None:
                blob_digests.add(bytes(row[14]))
            value_rows.append(
                {
                    "value_id": int(value_id),
                    "value_kind": "numeric-array" if row[12] is not None else row[5],
                    "text_value": row[6],
                    "integer_value": row[7],
                    "boolean_value": row[8],
                    "float64_bits": row[9],
                    "blob_sha256": row[14] if row[12] is not None else row[10],
                    "reference_id": row[11],
                    "numeric_array_dtype": row[12],
                    "numeric_array_shape": row[13],
                }
            )
        blob_values = self._read_blobs(blob_digests)
        if not {int(row[1]) for row in rows}.issuperset(root_ids):
            raise ValueError("typed value references a missing root node")
        return {
            root_id: rebuild_value_tree(root_id, value_rows, children, blob_values)
            for root_id in root_ids
        }

    def _read_blobs(self, digests: Iterable[bytes]) -> dict[bytes, bytes]:
        keys = list(digests)
        if not keys:
            return {}
        rows = self.connection.execute(
            "SELECT sha256, raw_length, compression, payload FROM parity_blob WHERE sha256 = ANY(%s)",
            (keys,),
        ).fetchall()
        values = {
            bytes(digest): expand_blob(compression, bytes(payload), int(length), bytes(digest))
            for digest, length, compression, payload in rows
        }
        if set(values) != set(keys):
            raise ValueError("one or more referenced parity blobs are missing")
        return values

    def _load_case_assets(
        self, project_id: str, specification_digest: bytes, case_ids: Sequence[str]
    ) -> dict[str, list[AssetRecord]]:
        rows = self.connection.execute(
            "SELECT case_id, asset_id, asset_kind, locator, media_type, expected_sha256, content_sha256 "
            "FROM parity_case_asset WHERE project_id = %s AND specification_digest = %s "
            "AND case_id = ANY(%s) ORDER BY case_id, asset_id",
            (project_id, specification_digest, list(case_ids)),
        ).fetchall()
        by_case: dict[str, list[AssetRecord]] = defaultdict(list)
        digests = {bytes(row[6]) for row in rows if row[6] is not None}
        blobs = self._read_blobs(digests)
        for case_id, asset_id, kind, locator, media_type, expected_digest, content_digest in rows:
            by_case[case_id].append(
                AssetRecord(
                    asset_id=asset_id,
                    asset_kind=kind,
                    locator=locator,
                    media_type=media_type,
                    expected_sha256=(
                        None if expected_digest is None else bytes(expected_digest)
                    ),
                    content=(
                        None
                        if content_digest is None
                        else blobs[bytes(content_digest)]
                    ),
                )
            )
        return by_case


def _dimension_fields(value: DimensionValue) -> tuple[str, str | None, int | None, bool | None]:
    if isinstance(value, bool):
        return "boolean", None, None, value
    if isinstance(value, int):
        return "integer", None, value, None
    if isinstance(value, str):
        return "text", value, None, None
    raise TypeError(f"unsupported dimension value: {type(value).__name__}")


def _dimension_row(
    project_id: str,
    specification_digest: bytes,
    profile_id: str,
    name: str,
    value: DimensionValue,
) -> tuple[Any, ...]:
    kind, text_value, integer_value, boolean_value = _dimension_fields(value)
    return (
        project_id,
        specification_digest,
        profile_id,
        name,
        kind,
        text_value,
        integer_value,
        boolean_value,
    )


def _case_dimension_row(
    case: CaseRecord, name: str, value: DimensionValue
) -> tuple[Any, ...]:
    kind, text_value, integer_value, boolean_value = _dimension_fields(value)
    return (
        case.project_id,
        case.specification_digest,
        case.case_id,
        name,
        kind,
        text_value,
        integer_value,
        boolean_value,
    )


def _support_dimension_row(
    support_id: int, name: str, value: DimensionValue
) -> tuple[Any, ...]:
    kind, text_value, integer_value, boolean_value = _dimension_fields(value)
    return (support_id, name, kind, text_value, integer_value, boolean_value)


def _dimension_value(
    kind: str,
    text_value: str | None,
    integer_value: Any,
    boolean_value: bool | None,
) -> DimensionValue:
    if kind == "text" and text_value is not None:
        return text_value
    if kind == "integer" and integer_value is not None:
        return int(integer_value)
    if kind == "boolean" and boolean_value is not None:
        return bool(boolean_value)
    raise ValueError(f"invalid stored dimension shape for kind {kind!r}")


def _identity_attribute_row(
    identity_digest: bytes, name: str, value: DimensionValue
) -> tuple[Any, ...]:
    kind, text_value, integer_value, boolean_value = _dimension_fields(value)
    return (
        identity_digest,
        name,
        kind,
        text_value,
        integer_value,
        boolean_value,
    )


def _stable_digest(value: Any) -> bytes:
    return canonical_value_digest(value)
