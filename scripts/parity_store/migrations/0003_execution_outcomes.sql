ALTER TABLE parity_execution
    DROP CONSTRAINT parity_execution_execution_state_check;

ALTER TABLE parity_execution
    ADD CONSTRAINT parity_execution_execution_state_check
    CHECK (execution_state IN ('completed', 'not_run', 'not_applicable', 'infrastructure_error'));

ALTER TABLE parity_execution
    ADD COLUMN execution_reason TEXT;

CREATE TABLE parity_execution_details (
    execution_id BIGINT PRIMARY KEY REFERENCES parity_execution(execution_id) ON DELETE CASCADE,
    value_id BIGINT NOT NULL REFERENCES parity_value(value_id)
);
