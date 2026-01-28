-- Add updated_at column
ALTER TABLE filter_rules
ADD COLUMN updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP;

-- Add is_positive column and migrate data
ALTER TABLE filter_rules
ADD COLUMN is_positive BOOLEAN;

UPDATE filter_rules
SET is_positive = CASE
    WHEN rule_type = 'Positive' THEN TRUE
    ELSE FALSE
END;

ALTER TABLE filter_rules
ALTER COLUMN is_positive SET NOT NULL;

-- Drop old rule_type column
ALTER TABLE filter_rules
DROP COLUMN rule_type;
