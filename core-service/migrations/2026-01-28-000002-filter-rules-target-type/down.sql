-- Drop new unique constraint
ALTER TABLE filter_rules
DROP CONSTRAINT IF EXISTS filter_rules_target_order_key;

-- Drop index
DROP INDEX IF EXISTS idx_filter_rules_target;

-- Add back old columns
ALTER TABLE filter_rules
ADD COLUMN series_id INT,
ADD COLUMN group_id INT;

-- Migrate data back (only anime_series type)
UPDATE filter_rules
SET series_id = target_id,
    group_id = 1  -- Default to first group since we lost this info
WHERE target_type = 'anime_series'::filter_target_type;

-- Set NOT NULL (will fail if non-anime_series rules exist)
ALTER TABLE filter_rules
ALTER COLUMN series_id SET NOT NULL,
ALTER COLUMN group_id SET NOT NULL;

-- Add back foreign keys
ALTER TABLE filter_rules
ADD CONSTRAINT filter_rules_series_id_fkey
    FOREIGN KEY (series_id) REFERENCES anime_series(series_id) ON DELETE CASCADE,
ADD CONSTRAINT filter_rules_group_id_fkey
    FOREIGN KEY (group_id) REFERENCES subtitle_groups(group_id) ON DELETE CASCADE;

-- Add back unique constraint
ALTER TABLE filter_rules
ADD CONSTRAINT filter_rules_series_id_group_id_rule_order_key
    UNIQUE (series_id, group_id, rule_order);

-- Drop new columns
ALTER TABLE filter_rules
DROP COLUMN target_type,
DROP COLUMN target_id;

-- Drop enum type
DROP TYPE filter_target_type;
