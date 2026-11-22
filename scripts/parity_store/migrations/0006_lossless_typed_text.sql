ALTER TABLE parity_value
    ALTER COLUMN text_value TYPE BYTEA
    USING CASE WHEN text_value IS NULL THEN NULL ELSE convert_to(text_value, 'UTF8') END;

ALTER TABLE parity_value_child
    ALTER COLUMN object_key TYPE BYTEA
    USING CASE WHEN object_key IS NULL THEN NULL ELSE convert_to(object_key, 'UTF8') END;

ALTER TABLE parity_case_dimension
    ALTER COLUMN text_value TYPE BYTEA
    USING CASE WHEN text_value IS NULL THEN NULL ELSE convert_to(text_value, 'UTF8') END;

ALTER TABLE parity_profile_attribute
    ALTER COLUMN text_value TYPE BYTEA
    USING CASE WHEN text_value IS NULL THEN NULL ELSE convert_to(text_value, 'UTF8') END;

ALTER TABLE parity_support_dimension
    ALTER COLUMN text_value TYPE BYTEA
    USING CASE WHEN text_value IS NULL THEN NULL ELSE convert_to(text_value, 'UTF8') END;

ALTER TABLE parity_implementation_attribute
    ALTER COLUMN text_value TYPE BYTEA
    USING CASE WHEN text_value IS NULL THEN NULL ELSE convert_to(text_value, 'UTF8') END;
