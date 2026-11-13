CREATE TABLE parity_value_numeric_array (
    value_id BIGINT PRIMARY KEY REFERENCES parity_value(value_id) ON DELETE CASCADE,
    blob_sha256 BYTEA NOT NULL REFERENCES parity_blob(sha256),
    array_dtype TEXT NOT NULL CHECK (
        array_dtype IN ('u8', 'u16', 'u32', 'u64', 'i8', 'i16', 'i32', 'i64', 'f64le')
    ),
    array_shape INTEGER[] NOT NULL CHECK (
        cardinality(array_shape) > 0 AND 0 < ALL(array_shape)
    )
);
