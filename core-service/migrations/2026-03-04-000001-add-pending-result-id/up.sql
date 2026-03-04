ALTER TABLE title_parsers
    ADD COLUMN pending_result_id INT REFERENCES pending_ai_results(id) ON DELETE SET NULL;

ALTER TABLE filter_rules
    ADD COLUMN pending_result_id INT REFERENCES pending_ai_results(id) ON DELETE SET NULL;
