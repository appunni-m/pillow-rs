CREATE TABLE parity_completed_case_import (
    project_id TEXT NOT NULL,
    specification_digest BYTEA NOT NULL,
    importer_identity TEXT NOT NULL,
    case_count BIGINT NOT NULL CHECK (case_count >= 0),
    document_count INTEGER NOT NULL CHECK (document_count >= 0),
    completed_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (project_id, specification_digest, importer_identity),
    FOREIGN KEY (project_id, specification_digest)
        REFERENCES parity_specification(project_id, specification_digest) ON DELETE CASCADE
);

CREATE TABLE parity_completed_case_import_document (
    project_id TEXT NOT NULL,
    specification_digest BYTEA NOT NULL,
    importer_identity TEXT NOT NULL,
    source_path TEXT NOT NULL,
    document_sha256 BYTEA NOT NULL CHECK (octet_length(document_sha256) = 32),
    case_count BIGINT NOT NULL CHECK (case_count >= 0),
    PRIMARY KEY (project_id, specification_digest, importer_identity, source_path),
    FOREIGN KEY (project_id, specification_digest, importer_identity)
        REFERENCES parity_completed_case_import(
            project_id, specification_digest, importer_identity
        ) ON DELETE CASCADE,
    FOREIGN KEY (project_id, specification_digest, source_path)
        REFERENCES parity_specification_document(
            project_id, specification_digest, source_path
        ) ON DELETE CASCADE
);
