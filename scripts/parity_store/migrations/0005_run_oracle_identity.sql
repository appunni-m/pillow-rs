CREATE TABLE parity_run_oracle (
    run_id UUID PRIMARY KEY REFERENCES parity_run(run_id) ON DELETE CASCADE,
    identity_digest BYTEA NOT NULL REFERENCES parity_implementation_identity(identity_digest),
    UNIQUE (run_id, identity_digest)
);
