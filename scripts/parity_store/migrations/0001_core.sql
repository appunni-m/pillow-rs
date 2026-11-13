CREATE TABLE parity_store_schema (
    version INTEGER PRIMARY KEY,
    migration_sha256 BYTEA NOT NULL CHECK (octet_length(migration_sha256) = 32),
    applied_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE parity_project (
    project_id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL
);

CREATE TABLE parity_specification (
    project_id TEXT NOT NULL REFERENCES parity_project(project_id),
    specification_digest BYTEA NOT NULL CHECK (octet_length(specification_digest) = 32),
    schema_id TEXT NOT NULL,
    source_ref TEXT NOT NULL,
    imported_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (project_id, specification_digest)
);

CREATE TABLE parity_operation (
    project_id TEXT NOT NULL,
    specification_digest BYTEA NOT NULL,
    surface_id TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    PRIMARY KEY (project_id, specification_digest, surface_id, operation_id),
    FOREIGN KEY (project_id, specification_digest)
        REFERENCES parity_specification(project_id, specification_digest)
);

CREATE TABLE parity_requirement (
    project_id TEXT NOT NULL,
    specification_digest BYTEA NOT NULL,
    requirement_id TEXT NOT NULL,
    PRIMARY KEY (project_id, specification_digest, requirement_id),
    FOREIGN KEY (project_id, specification_digest)
        REFERENCES parity_specification(project_id, specification_digest)
);

CREATE TABLE parity_operation_requirement (
    project_id TEXT NOT NULL,
    specification_digest BYTEA NOT NULL,
    surface_id TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    requirement_id TEXT NOT NULL,
    PRIMARY KEY (project_id, specification_digest, surface_id, operation_id, requirement_id),
    FOREIGN KEY (project_id, specification_digest, surface_id, operation_id)
        REFERENCES parity_operation(project_id, specification_digest, surface_id, operation_id),
    FOREIGN KEY (project_id, specification_digest, requirement_id)
        REFERENCES parity_requirement(project_id, specification_digest, requirement_id)
);

CREATE TABLE parity_target (
    project_id TEXT NOT NULL,
    specification_digest BYTEA NOT NULL,
    target_id TEXT NOT NULL,
    display_name TEXT NOT NULL,
    runtime TEXT NOT NULL,
    PRIMARY KEY (project_id, specification_digest, target_id),
    FOREIGN KEY (project_id, specification_digest)
        REFERENCES parity_specification(project_id, specification_digest)
);

CREATE TABLE parity_target_profile (
    project_id TEXT NOT NULL,
    specification_digest BYTEA NOT NULL,
    profile_id TEXT NOT NULL,
    target_id TEXT NOT NULL,
    PRIMARY KEY (project_id, specification_digest, profile_id),
    FOREIGN KEY (project_id, specification_digest, target_id)
        REFERENCES parity_target(project_id, specification_digest, target_id)
);

CREATE TABLE parity_profile_attribute (
    project_id TEXT NOT NULL,
    specification_digest BYTEA NOT NULL,
    profile_id TEXT NOT NULL,
    attribute_name TEXT NOT NULL,
    value_kind TEXT NOT NULL CHECK (value_kind IN ('text', 'integer', 'boolean')),
    text_value TEXT,
    integer_value NUMERIC,
    boolean_value BOOLEAN,
    PRIMARY KEY (project_id, specification_digest, profile_id, attribute_name),
    FOREIGN KEY (project_id, specification_digest, profile_id)
        REFERENCES parity_target_profile(project_id, specification_digest, profile_id),
    CHECK (
        (value_kind = 'text' AND text_value IS NOT NULL AND integer_value IS NULL AND boolean_value IS NULL)
        OR (value_kind = 'integer' AND text_value IS NULL AND integer_value IS NOT NULL AND boolean_value IS NULL)
        OR (value_kind = 'boolean' AND text_value IS NULL AND integer_value IS NULL AND boolean_value IS NOT NULL)
    )
);

CREATE TABLE parity_operation_support (
    support_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    support_digest BYTEA NOT NULL CHECK (octet_length(support_digest) = 32),
    project_id TEXT NOT NULL,
    specification_digest BYTEA NOT NULL,
    surface_id TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    profile_id TEXT NOT NULL,
    support_state TEXT NOT NULL CHECK (support_state IN ('supported', 'partial', 'unsupported')),
    reason TEXT,
    UNIQUE (project_id, specification_digest, surface_id, operation_id, profile_id, support_digest),
    FOREIGN KEY (project_id, specification_digest, surface_id, operation_id)
        REFERENCES parity_operation(project_id, specification_digest, surface_id, operation_id),
    FOREIGN KEY (project_id, specification_digest, profile_id)
        REFERENCES parity_target_profile(project_id, specification_digest, profile_id)
);

CREATE TABLE parity_support_dimension (
    support_id BIGINT NOT NULL REFERENCES parity_operation_support(support_id) ON DELETE CASCADE,
    dimension_name TEXT NOT NULL,
    value_kind TEXT NOT NULL CHECK (value_kind IN ('text', 'integer', 'boolean')),
    text_value TEXT,
    integer_value NUMERIC,
    boolean_value BOOLEAN,
    PRIMARY KEY (support_id, dimension_name),
    CHECK (
        (value_kind = 'text' AND text_value IS NOT NULL AND integer_value IS NULL AND boolean_value IS NULL)
        OR (value_kind = 'integer' AND text_value IS NULL AND integer_value IS NOT NULL AND boolean_value IS NULL)
        OR (value_kind = 'boolean' AND text_value IS NULL AND integer_value IS NULL AND boolean_value IS NOT NULL)
    )
);

CREATE TABLE parity_case (
    project_id TEXT NOT NULL,
    specification_digest BYTEA NOT NULL,
    case_id TEXT NOT NULL,
    surface_id TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    verification_kind TEXT NOT NULL,
    source_document TEXT NOT NULL,
    source_document_digest BYTEA NOT NULL CHECK (octet_length(source_document_digest) = 32),
    source_ordinal INTEGER NOT NULL CHECK (source_ordinal >= 0),
    case_digest BYTEA NOT NULL CHECK (octet_length(case_digest) = 32),
    estimated_bytes BIGINT NOT NULL DEFAULT 0 CHECK (estimated_bytes >= 0),
    PRIMARY KEY (project_id, specification_digest, case_id),
    UNIQUE (project_id, specification_digest, source_document, source_ordinal),
    UNIQUE (project_id, specification_digest, case_id, case_digest),
    FOREIGN KEY (project_id, specification_digest, surface_id, operation_id)
        REFERENCES parity_operation(project_id, specification_digest, surface_id, operation_id)
);

CREATE TABLE parity_case_dimension (
    project_id TEXT NOT NULL,
    specification_digest BYTEA NOT NULL,
    case_id TEXT NOT NULL,
    dimension_name TEXT NOT NULL,
    value_kind TEXT NOT NULL CHECK (value_kind IN ('text', 'integer', 'boolean')),
    text_value TEXT,
    integer_value NUMERIC,
    boolean_value BOOLEAN,
    PRIMARY KEY (project_id, specification_digest, case_id, dimension_name),
    FOREIGN KEY (project_id, specification_digest, case_id)
        REFERENCES parity_case(project_id, specification_digest, case_id) ON DELETE CASCADE,
    CHECK (
        (value_kind = 'text' AND text_value IS NOT NULL AND integer_value IS NULL AND boolean_value IS NULL)
        OR (value_kind = 'integer' AND text_value IS NULL AND integer_value IS NOT NULL AND boolean_value IS NULL)
        OR (value_kind = 'boolean' AND text_value IS NULL AND integer_value IS NULL AND boolean_value IS NOT NULL)
    )
);

CREATE TABLE parity_case_requirement (
    project_id TEXT NOT NULL,
    specification_digest BYTEA NOT NULL,
    case_id TEXT NOT NULL,
    requirement_id TEXT NOT NULL,
    PRIMARY KEY (project_id, specification_digest, case_id, requirement_id),
    FOREIGN KEY (project_id, specification_digest, case_id)
        REFERENCES parity_case(project_id, specification_digest, case_id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, specification_digest, requirement_id)
        REFERENCES parity_requirement(project_id, specification_digest, requirement_id)
);

CREATE TABLE parity_value (
    value_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    value_kind TEXT NOT NULL CHECK (
        value_kind IN ('null', 'boolean', 'integer', 'float64', 'text', 'bytes', 'array', 'object', 'asset-reference', 'step-binding')
    ),
    text_value TEXT,
    integer_value NUMERIC,
    boolean_value BOOLEAN,
    float64_bits BYTEA CHECK (float64_bits IS NULL OR octet_length(float64_bits) = 8),
    blob_sha256 BYTEA,
    reference_id TEXT,
    CHECK (
        (value_kind = 'null' AND text_value IS NULL AND integer_value IS NULL AND boolean_value IS NULL AND float64_bits IS NULL AND blob_sha256 IS NULL AND reference_id IS NULL)
        OR (value_kind = 'boolean' AND text_value IS NULL AND integer_value IS NULL AND boolean_value IS NOT NULL AND float64_bits IS NULL AND blob_sha256 IS NULL AND reference_id IS NULL)
        OR (value_kind = 'integer' AND text_value IS NULL AND integer_value IS NOT NULL AND boolean_value IS NULL AND float64_bits IS NULL AND blob_sha256 IS NULL AND reference_id IS NULL)
        OR (value_kind = 'float64' AND text_value IS NULL AND integer_value IS NULL AND boolean_value IS NULL AND float64_bits IS NOT NULL AND blob_sha256 IS NULL AND reference_id IS NULL)
        OR (value_kind = 'text' AND text_value IS NOT NULL AND integer_value IS NULL AND boolean_value IS NULL AND float64_bits IS NULL AND blob_sha256 IS NULL AND reference_id IS NULL)
        OR (value_kind = 'bytes' AND text_value IS NULL AND integer_value IS NULL AND boolean_value IS NULL AND float64_bits IS NULL AND blob_sha256 IS NOT NULL AND reference_id IS NULL)
        OR (value_kind IN ('array', 'object') AND text_value IS NULL AND integer_value IS NULL AND boolean_value IS NULL AND float64_bits IS NULL AND blob_sha256 IS NULL AND reference_id IS NULL)
        OR (value_kind IN ('asset-reference', 'step-binding') AND text_value IS NULL AND integer_value IS NULL AND boolean_value IS NULL AND float64_bits IS NULL AND blob_sha256 IS NULL AND reference_id IS NOT NULL)
    )
);

CREATE TABLE parity_value_child (
    parent_value_id BIGINT NOT NULL REFERENCES parity_value(value_id) ON DELETE CASCADE,
    child_ordinal INTEGER NOT NULL CHECK (child_ordinal >= 0),
    object_key TEXT,
    child_value_id BIGINT NOT NULL UNIQUE REFERENCES parity_value(value_id) ON DELETE CASCADE,
    PRIMARY KEY (parent_value_id, child_ordinal),
    UNIQUE (parent_value_id, object_key)
);

CREATE TABLE parity_blob (
    sha256 BYTEA PRIMARY KEY CHECK (octet_length(sha256) = 32),
    raw_length BIGINT NOT NULL CHECK (raw_length >= 0),
    compression TEXT NOT NULL CHECK (compression IN ('none', 'zlib')),
    payload BYTEA NOT NULL
);

ALTER TABLE parity_value
    ADD CONSTRAINT parity_value_blob_fk FOREIGN KEY (blob_sha256)
    REFERENCES parity_blob(sha256);

CREATE TABLE parity_case_root (
    project_id TEXT NOT NULL,
    specification_digest BYTEA NOT NULL,
    case_id TEXT NOT NULL,
    value_id BIGINT NOT NULL UNIQUE REFERENCES parity_value(value_id),
    PRIMARY KEY (project_id, specification_digest, case_id),
    FOREIGN KEY (project_id, specification_digest, case_id)
        REFERENCES parity_case(project_id, specification_digest, case_id) ON DELETE CASCADE
);

CREATE TABLE parity_case_step (
    project_id TEXT NOT NULL,
    specification_digest BYTEA NOT NULL,
    case_id TEXT NOT NULL,
    step_ordinal INTEGER NOT NULL CHECK (step_ordinal >= 0),
    step_id TEXT NOT NULL,
    surface_id TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    receiver_step_id TEXT,
    PRIMARY KEY (project_id, specification_digest, case_id, step_ordinal),
    UNIQUE (project_id, specification_digest, case_id, step_id),
    FOREIGN KEY (project_id, specification_digest, case_id)
        REFERENCES parity_case(project_id, specification_digest, case_id) ON DELETE CASCADE
);

ALTER TABLE parity_case_step
    ADD CONSTRAINT parity_case_step_receiver_fk
    FOREIGN KEY (project_id, specification_digest, case_id, receiver_step_id)
    REFERENCES parity_case_step(project_id, specification_digest, case_id, step_id)
    DEFERRABLE INITIALLY DEFERRED;

CREATE TABLE parity_step_argument (
    project_id TEXT NOT NULL,
    specification_digest BYTEA NOT NULL,
    case_id TEXT NOT NULL,
    step_ordinal INTEGER NOT NULL,
    argument_name TEXT NOT NULL,
    value_id BIGINT NOT NULL REFERENCES parity_value(value_id),
    PRIMARY KEY (project_id, specification_digest, case_id, step_ordinal, argument_name),
    FOREIGN KEY (project_id, specification_digest, case_id, step_ordinal)
        REFERENCES parity_case_step(project_id, specification_digest, case_id, step_ordinal) ON DELETE CASCADE
);

CREATE TABLE parity_case_observation_selector (
    project_id TEXT NOT NULL,
    specification_digest BYTEA NOT NULL,
    case_id TEXT NOT NULL,
    observation_ordinal INTEGER NOT NULL CHECK (observation_ordinal >= 0),
    step_id TEXT NOT NULL,
    field_path TEXT,
    PRIMARY KEY (project_id, specification_digest, case_id, observation_ordinal),
    FOREIGN KEY (project_id, specification_digest, case_id)
        REFERENCES parity_case(project_id, specification_digest, case_id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, specification_digest, case_id, step_id)
        REFERENCES parity_case_step(project_id, specification_digest, case_id, step_id)
        DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE parity_case_expected_observation (
    project_id TEXT NOT NULL,
    specification_digest BYTEA NOT NULL,
    case_id TEXT NOT NULL,
    observation_ordinal INTEGER NOT NULL CHECK (observation_ordinal >= 0),
    step_id TEXT NOT NULL,
    value_id BIGINT NOT NULL REFERENCES parity_value(value_id),
    PRIMARY KEY (project_id, specification_digest, case_id, observation_ordinal),
    FOREIGN KEY (project_id, specification_digest, case_id)
        REFERENCES parity_case(project_id, specification_digest, case_id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, specification_digest, case_id, step_id)
        REFERENCES parity_case_step(project_id, specification_digest, case_id, step_id)
        DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE parity_case_profile (
    project_id TEXT NOT NULL,
    specification_digest BYTEA NOT NULL,
    case_id TEXT NOT NULL,
    profile_id TEXT NOT NULL,
    PRIMARY KEY (project_id, specification_digest, case_id, profile_id),
    FOREIGN KEY (project_id, specification_digest, case_id)
        REFERENCES parity_case(project_id, specification_digest, case_id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, specification_digest, profile_id)
        REFERENCES parity_target_profile(project_id, specification_digest, profile_id)
);

CREATE TABLE parity_case_asset (
    project_id TEXT NOT NULL,
    specification_digest BYTEA NOT NULL,
    case_id TEXT NOT NULL,
    asset_id TEXT NOT NULL,
    asset_kind TEXT NOT NULL,
    locator TEXT,
    media_type TEXT,
    expected_sha256 BYTEA CHECK (expected_sha256 IS NULL OR octet_length(expected_sha256) = 32),
    content_sha256 BYTEA REFERENCES parity_blob(sha256),
    PRIMARY KEY (project_id, specification_digest, case_id, asset_id),
    FOREIGN KEY (project_id, specification_digest, case_id)
        REFERENCES parity_case(project_id, specification_digest, case_id) ON DELETE CASCADE
);

CREATE TABLE parity_case_fault_contract (
    project_id TEXT NOT NULL,
    specification_digest BYTEA NOT NULL,
    case_id TEXT NOT NULL,
    contract_id TEXT NOT NULL,
    injection_point TEXT NOT NULL,
    fault_value_id BIGINT NOT NULL REFERENCES parity_value(value_id),
    PRIMARY KEY (project_id, specification_digest, case_id),
    FOREIGN KEY (project_id, specification_digest, case_id)
        REFERENCES parity_case(project_id, specification_digest, case_id) ON DELETE CASCADE
);

CREATE TABLE parity_run (
    run_id UUID PRIMARY KEY,
    project_id TEXT NOT NULL,
    specification_digest BYTEA NOT NULL,
    case_set_digest BYTEA NOT NULL CHECK (octet_length(case_set_digest) = 32),
    started_at TIMESTAMPTZ NOT NULL,
    finished_at TIMESTAMPTZ,
    run_state TEXT NOT NULL CHECK (run_state IN ('running', 'completed', 'failed', 'cancelled')),
    host_description TEXT NOT NULL,
    FOREIGN KEY (project_id, specification_digest)
        REFERENCES parity_specification(project_id, specification_digest)
);

CREATE TABLE parity_implementation_identity (
    identity_digest BYTEA PRIMARY KEY CHECK (octet_length(identity_digest) = 32),
    project_id TEXT NOT NULL,
    implementation_kind TEXT NOT NULL CHECK (implementation_kind IN ('oracle', 'target', 'comparator')),
    implementation_id TEXT NOT NULL,
    profile_id TEXT,
    revision TEXT NOT NULL,
    runtime TEXT NOT NULL,
    dirty BOOLEAN NOT NULL,
    configuration_digest BYTEA NOT NULL CHECK (octet_length(configuration_digest) = 32),
    FOREIGN KEY (project_id) REFERENCES parity_project(project_id)
);

CREATE TABLE parity_implementation_attribute (
    identity_digest BYTEA NOT NULL REFERENCES parity_implementation_identity(identity_digest) ON DELETE CASCADE,
    attribute_name TEXT NOT NULL,
    value_kind TEXT NOT NULL CHECK (value_kind IN ('text', 'integer', 'boolean')),
    text_value TEXT,
    integer_value NUMERIC,
    boolean_value BOOLEAN,
    PRIMARY KEY (identity_digest, attribute_name),
    CHECK (
        (value_kind = 'text' AND text_value IS NOT NULL AND integer_value IS NULL AND boolean_value IS NULL)
        OR (value_kind = 'integer' AND text_value IS NULL AND integer_value IS NOT NULL AND boolean_value IS NULL)
        OR (value_kind = 'boolean' AND text_value IS NULL AND integer_value IS NULL AND boolean_value IS NOT NULL)
    )
);

CREATE TABLE parity_run_profile (
    run_id UUID NOT NULL REFERENCES parity_run(run_id) ON DELETE CASCADE,
    identity_digest BYTEA NOT NULL REFERENCES parity_implementation_identity(identity_digest),
    PRIMARY KEY (run_id, identity_digest)
);

CREATE TABLE parity_run_case (
    run_id UUID NOT NULL REFERENCES parity_run(run_id) ON DELETE CASCADE,
    case_ordinal INTEGER NOT NULL CHECK (case_ordinal >= 0),
    project_id TEXT NOT NULL,
    specification_digest BYTEA NOT NULL,
    case_id TEXT NOT NULL,
    case_digest BYTEA NOT NULL CHECK (octet_length(case_digest) = 32),
    PRIMARY KEY (run_id, case_ordinal),
    UNIQUE (run_id, project_id, specification_digest, case_id),
    UNIQUE (run_id, project_id, specification_digest, case_id, case_digest),
    FOREIGN KEY (project_id, specification_digest, case_id, case_digest)
        REFERENCES parity_case(project_id, specification_digest, case_id, case_digest)
);

CREATE TABLE parity_run_comparator (
    run_id UUID PRIMARY KEY REFERENCES parity_run(run_id) ON DELETE CASCADE,
    identity_digest BYTEA NOT NULL REFERENCES parity_implementation_identity(identity_digest),
    UNIQUE (run_id, identity_digest)
);

CREATE TABLE parity_batch (
    batch_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    run_id UUID NOT NULL REFERENCES parity_run(run_id) ON DELETE CASCADE,
    identity_digest BYTEA NOT NULL REFERENCES parity_implementation_identity(identity_digest),
    batch_number INTEGER NOT NULL CHECK (batch_number >= 0),
    selected_count INTEGER NOT NULL CHECK (selected_count > 0),
    estimated_bytes BIGINT NOT NULL CHECK (estimated_bytes >= 0),
    first_case_id TEXT NOT NULL,
    last_case_id TEXT NOT NULL,
    batch_state TEXT NOT NULL CHECK (batch_state IN ('started', 'completed', 'failed')),
    started_at TIMESTAMPTZ NOT NULL,
    finished_at TIMESTAMPTZ,
    error_kind TEXT,
    error_message TEXT,
    UNIQUE (run_id, identity_digest, batch_number),
    UNIQUE (batch_id, run_id),
    FOREIGN KEY (run_id, identity_digest)
        REFERENCES parity_run_profile(run_id, identity_digest) ON DELETE CASCADE
);

CREATE TABLE parity_batch_case (
    batch_id BIGINT NOT NULL,
    run_id UUID NOT NULL,
    case_ordinal INTEGER NOT NULL CHECK (case_ordinal >= 0),
    project_id TEXT NOT NULL,
    specification_digest BYTEA NOT NULL,
    case_id TEXT NOT NULL,
    case_digest BYTEA NOT NULL CHECK (octet_length(case_digest) = 32),
    PRIMARY KEY (batch_id, case_ordinal),
    UNIQUE (batch_id, project_id, specification_digest, case_id),
    FOREIGN KEY (batch_id, run_id)
        REFERENCES parity_batch(batch_id, run_id) ON DELETE CASCADE,
    FOREIGN KEY (run_id, project_id, specification_digest, case_id, case_digest)
        REFERENCES parity_run_case(run_id, project_id, specification_digest, case_id, case_digest)
);

CREATE TABLE parity_run_error (
    run_id UUID NOT NULL REFERENCES parity_run(run_id) ON DELETE CASCADE,
    error_ordinal INTEGER NOT NULL CHECK (error_ordinal >= 0),
    batch_id BIGINT REFERENCES parity_batch(batch_id) ON DELETE CASCADE,
    scope TEXT NOT NULL,
    external_id TEXT,
    error_kind TEXT NOT NULL,
    message TEXT NOT NULL,
    PRIMARY KEY (run_id, error_ordinal)
);

CREATE TABLE parity_execution (
    execution_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    identity_digest BYTEA NOT NULL REFERENCES parity_implementation_identity(identity_digest),
    project_id TEXT NOT NULL,
    specification_digest BYTEA NOT NULL,
    case_id TEXT NOT NULL,
    case_digest BYTEA NOT NULL CHECK (octet_length(case_digest) = 32),
    execution_state TEXT NOT NULL CHECK (execution_state IN ('completed', 'not_run', 'infrastructure_error')),
    result_digest BYTEA NOT NULL CHECK (octet_length(result_digest) = 32),
    started_at TIMESTAMPTZ NOT NULL,
    finished_at TIMESTAMPTZ NOT NULL,
    duration_ns BIGINT CHECK (duration_ns IS NULL OR duration_ns >= 0),
    UNIQUE (identity_digest, project_id, specification_digest, case_id, case_digest),
    FOREIGN KEY (project_id, specification_digest, case_id, case_digest)
        REFERENCES parity_case(project_id, specification_digest, case_id, case_digest)
);

CREATE TABLE parity_observation (
    execution_id BIGINT NOT NULL REFERENCES parity_execution(execution_id) ON DELETE CASCADE,
    observation_ordinal INTEGER NOT NULL CHECK (observation_ordinal >= 0),
    step_id TEXT NOT NULL,
    observation_state TEXT NOT NULL CHECK (observation_state IN ('ok', 'error', 'not_run')),
    value_id BIGINT REFERENCES parity_value(value_id),
    error_class TEXT,
    error_kind TEXT,
    error_message TEXT,
    error_stage TEXT,
    error_code TEXT,
    reason TEXT,
    PRIMARY KEY (execution_id, observation_ordinal),
    CHECK (
        (observation_state = 'ok' AND value_id IS NOT NULL AND error_class IS NULL AND error_kind IS NULL AND error_message IS NULL AND error_stage IS NULL AND error_code IS NULL AND reason IS NULL)
        OR (observation_state = 'error' AND value_id IS NULL AND error_class IS NOT NULL AND error_kind IS NOT NULL AND error_message IS NOT NULL AND error_stage IS NOT NULL AND reason IS NULL)
        OR (observation_state = 'not_run' AND value_id IS NULL AND error_class IS NULL AND error_kind IS NULL AND error_message IS NULL AND error_stage IS NULL AND error_code IS NULL AND reason IS NOT NULL)
    )
);

CREATE TABLE parity_comparison (
    run_id UUID NOT NULL REFERENCES parity_run(run_id) ON DELETE CASCADE,
    target_identity_digest BYTEA NOT NULL REFERENCES parity_implementation_identity(identity_digest),
    comparator_identity_digest BYTEA NOT NULL REFERENCES parity_implementation_identity(identity_digest),
    project_id TEXT NOT NULL,
    specification_digest BYTEA NOT NULL,
    case_id TEXT NOT NULL,
    source_execution_id BIGINT REFERENCES parity_execution(execution_id),
    target_execution_id BIGINT NOT NULL REFERENCES parity_execution(execution_id),
    outcome TEXT NOT NULL CHECK (outcome IN ('pass', 'fail', 'not_run', 'not_applicable', 'infrastructure_error')),
    difference_count INTEGER NOT NULL CHECK (difference_count >= 0),
    comparison_digest BYTEA NOT NULL CHECK (octet_length(comparison_digest) = 32),
    compared_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (run_id, target_identity_digest, project_id, specification_digest, case_id),
    FOREIGN KEY (project_id, specification_digest, case_id)
        REFERENCES parity_case(project_id, specification_digest, case_id),
    FOREIGN KEY (run_id, target_identity_digest)
        REFERENCES parity_run_profile(run_id, identity_digest),
    FOREIGN KEY (run_id, comparator_identity_digest)
        REFERENCES parity_run_comparator(run_id, identity_digest)
);

CREATE TABLE parity_difference (
    run_id UUID NOT NULL,
    target_identity_digest BYTEA NOT NULL,
    project_id TEXT NOT NULL,
    specification_digest BYTEA NOT NULL,
    case_id TEXT NOT NULL,
    difference_ordinal INTEGER NOT NULL CHECK (difference_ordinal >= 0),
    step_id TEXT NOT NULL,
    value_path TEXT NOT NULL,
    difference_kind TEXT NOT NULL,
    message TEXT NOT NULL,
    source_value_present BOOLEAN NOT NULL,
    source_value_id BIGINT REFERENCES parity_value(value_id),
    target_value_present BOOLEAN NOT NULL,
    target_value_id BIGINT REFERENCES parity_value(value_id),
    PRIMARY KEY (run_id, target_identity_digest, project_id, specification_digest, case_id, difference_ordinal),
    FOREIGN KEY (run_id, target_identity_digest, project_id, specification_digest, case_id)
        REFERENCES parity_comparison(run_id, target_identity_digest, project_id, specification_digest, case_id) ON DELETE CASCADE,
    CHECK (
        (source_value_present AND source_value_id IS NOT NULL)
        OR (NOT source_value_present AND source_value_id IS NULL)
    ),
    CHECK (
        (target_value_present AND target_value_id IS NOT NULL)
        OR (NOT target_value_present AND target_value_id IS NULL)
    )
);

CREATE INDEX parity_case_operation_idx
    ON parity_case(project_id, specification_digest, surface_id, operation_id, case_id);
CREATE INDEX parity_case_verification_idx
    ON parity_case(project_id, specification_digest, verification_kind, case_id);
CREATE INDEX parity_case_dimension_text_idx
    ON parity_case_dimension(project_id, specification_digest, dimension_name, text_value, case_id)
    WHERE value_kind = 'text';
CREATE INDEX parity_case_dimension_integer_idx
    ON parity_case_dimension(project_id, specification_digest, dimension_name, integer_value, case_id)
    WHERE value_kind = 'integer';
CREATE INDEX parity_comparison_outcome_idx
    ON parity_comparison(project_id, specification_digest, outcome, target_identity_digest, case_id);
