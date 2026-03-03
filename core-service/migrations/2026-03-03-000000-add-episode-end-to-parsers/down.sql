ALTER TABLE title_parsers
  DROP COLUMN IF EXISTS episode_end_source,
  DROP COLUMN IF EXISTS episode_end_value;
