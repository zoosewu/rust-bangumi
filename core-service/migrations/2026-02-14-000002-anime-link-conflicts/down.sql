DROP INDEX IF EXISTS idx_anime_link_conflicts_unresolved;
DROP INDEX IF EXISTS idx_anime_links_conflict;
DROP TABLE IF EXISTS anime_link_conflicts;
ALTER TABLE anime_links
  DROP COLUMN IF EXISTS link_status,
  DROP COLUMN IF EXISTS conflict_flag;
ALTER TABLE anime_links DROP CONSTRAINT IF EXISTS anime_links_source_hash_unique;
ALTER TABLE anime_links ADD CONSTRAINT anime_links_series_group_episode_hash_key
  UNIQUE (series_id, group_id, episode_no, source_hash);
