ALTER TABLE pending_ai_results
  ADD COLUMN confirm_level VARCHAR,
  ADD COLUMN confirm_target_id INTEGER;
