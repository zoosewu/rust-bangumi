-- Drop old unique constraint
ALTER TABLE anime_links
DROP CONSTRAINT IF EXISTS anime_links_series_id_group_id_episode_no_key;

-- Add new unique constraint including source_hash
ALTER TABLE anime_links
ADD CONSTRAINT anime_links_series_group_episode_hash_key
UNIQUE (series_id, group_id, episode_no, source_hash);
