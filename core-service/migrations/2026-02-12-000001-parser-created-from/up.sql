-- Add created_from tracking to title_parsers (management only, not used for execution)
ALTER TABLE title_parsers
  ADD COLUMN created_from_type filter_target_type DEFAULT NULL,
  ADD COLUMN created_from_id INTEGER DEFAULT NULL;
