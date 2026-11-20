-- 一集只留一個版本：衝突改以（季, 集數）判斷，不再分字幕組。
-- 同一集的多筆紀錄合併成一筆（優先保留未解決的，再來是最新的）。
DELETE FROM anime_link_conflicts c
USING anime_link_conflicts k
WHERE c.anime_id = k.anime_id AND c.episode_no = k.episode_no AND c.conflict_id <> k.conflict_id
  AND (k.resolution_status = 'unresolved', k.created_at, k.conflict_id)
      > (c.resolution_status = 'unresolved', c.created_at, c.conflict_id);

ALTER TABLE anime_link_conflicts
  DROP CONSTRAINT anime_link_conflicts_series_id_group_id_episode_no_key,
  DROP COLUMN group_id,
  ADD CONSTRAINT anime_link_conflicts_anime_id_episode_no_key UNIQUE (anime_id, episode_no);
