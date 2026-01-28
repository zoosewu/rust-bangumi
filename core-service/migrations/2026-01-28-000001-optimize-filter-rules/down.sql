-- Recreate rule_type column
ALTER TABLE filter_rules
ADD COLUMN rule_type VARCHAR(20);

UPDATE filter_rules
SET rule_type = CASE
    WHEN is_positive = TRUE THEN 'Positive'
    ELSE 'Negative'
END;

ALTER TABLE filter_rules
ALTER COLUMN rule_type SET NOT NULL;

-- Add CHECK constraint back
ALTER TABLE filter_rules
ADD CONSTRAINT filter_rules_rule_type_check
CHECK (rule_type IN ('Positive', 'Negative'));

-- Drop is_positive column
ALTER TABLE filter_rules
DROP COLUMN is_positive;

-- Drop updated_at column
ALTER TABLE filter_rules
DROP COLUMN updated_at;
