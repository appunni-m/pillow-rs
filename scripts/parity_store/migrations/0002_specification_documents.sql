CREATE TABLE parity_specification_document (
    project_id TEXT NOT NULL,
    specification_digest BYTEA NOT NULL,
    document_ordinal INTEGER NOT NULL CHECK (document_ordinal >= 0),
    source_path TEXT NOT NULL,
    schema_id TEXT,
    document_sha256 BYTEA NOT NULL CHECK (octet_length(document_sha256) = 32),
    PRIMARY KEY (project_id, specification_digest, document_ordinal),
    UNIQUE (project_id, specification_digest, source_path),
    FOREIGN KEY (project_id, specification_digest)
        REFERENCES parity_specification(project_id, specification_digest) ON DELETE CASCADE
);
