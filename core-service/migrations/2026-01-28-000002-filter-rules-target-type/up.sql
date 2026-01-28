-- Create target_type ENUM
CREATE TYPE filter_target_type AS ENUM ('global', 'anime', 'subtitle_group', 'anime_series', 'fetcher');

-- Add new columns
ALTER TABLE filter_rules
ADD COLUMN target_type filter_target_type,
ADD COLUMN target_id INT;

-- Migrate existing data: series_id -> anime_series type
UPDATE filter_rules
SET target_type = 'anime_series'::filter_target_type,
    target_id = series_id;

-- Set NOT NULL after migration
ALTER TABLE filter_rules
ALTER COLUMN target_type SET NOT NULL;

-- Drop old unique constraint
ALTER TABLE filter_rules
DROP CONSTRAINT IF EXISTS filter_rules_series_id_group_id_rule_order_key;

-- Drop old foreign keys
ALTER TABLE filter_rules
DROP CONSTRAINT IF EXISTS filter_rules_series_id_fkey,
DROP CONSTRAINT IF EXISTS filter_rules_group_id_fkey;

-- Drop old columns
ALTER TABLE filter_rules
DROP COLUMN series_id,
DROP COLUMN group_id;

-- Create index on target_type and target_id
CREATE INDEX idx_filter_rules_target ON filter_rules(target_type, target_id);

-- Create new unique constraint
ALTER TABLE filter_rules
ADD CONSTRAINT filter_rules_target_order_key UNIQUE (target_type, target_id, rule_order);
