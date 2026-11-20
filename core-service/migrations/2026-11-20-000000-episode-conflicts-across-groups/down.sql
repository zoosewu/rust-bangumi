ALTER TABLE anime_link_conflicts
  DROP CONSTRAINT anime_link_conflicts_anime_id_episode_no_key,
  ADD COLUMN group_id INTEGER REFERENCES subtitle_groups(group_id) ON DELETE CASCADE;

-- 字幕組取選定的版本，沒有選定時取該集最早的連結
UPDATE anime_link_conflicts c
SET group_id = COALESCE(
  (SELECT l.group_id FROM anime_links l WHERE l.link_id = c.chosen_link_id),
  (SELECT l.group_id FROM anime_links l
   WHERE l.anime_id = c.anime_id AND l.episode_no = c.episode_no
   ORDER BY l.link_id LIMIT 1));
DELETE FROM anime_link_conflicts WHERE group_id IS NULL;

ALTER TABLE anime_link_conflicts
  ALTER COLUMN group_id SET NOT NULL,
  ADD CONSTRAINT anime_link_conflicts_series_id_group_id_episode_no_key UNIQUE (anime_id, group_id, episode_no);
