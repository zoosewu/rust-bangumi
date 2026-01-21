CREATE TABLE filter_rules (
  rule_id SERIAL PRIMARY KEY,
  series_id INTEGER NOT NULL REFERENCES anime_series(series_id) ON DELETE CASCADE,
  group_id INTEGER NOT NULL REFERENCES subtitle_groups(group_id) ON DELETE CASCADE,
  rule_order INTEGER NOT NULL,
  rule_type VARCHAR(20) NOT NULL CHECK (rule_type IN ('Positive', 'Negative')),
  regex_pattern TEXT NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(series_id, group_id, rule_order)
);

CREATE INDEX idx_filter_rules_series_group ON filter_rules(series_id, group_id);
