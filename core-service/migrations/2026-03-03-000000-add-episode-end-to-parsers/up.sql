ALTER TABLE title_parsers
  ADD COLUMN episode_end_source parser_source_type,
  ADD COLUMN episode_end_value  VARCHAR(255);
