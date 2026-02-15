-- 1) 移除舊 UNIQUE，改為 source_hash only
ALTER TABLE anime_links DROP CONSTRAINT IF EXISTS anime_links_series_group_episode_hash_key;
ALTER TABLE anime_links ADD CONSTRAINT anime_links_source_hash_unique UNIQUE (source_hash);

-- 2) anime_links 新增欄位
ALTER TABLE anime_links
  ADD COLUMN conflict_flag BOOLEAN NOT NULL DEFAULT FALSE,
  ADD COLUMN link_status VARCHAR(20) NOT NULL DEFAULT 'active'
    CHECK (link_status IN ('active', 'resolved'));

-- 3) 新建 anime_link_conflicts 表
CREATE TABLE anime_link_conflicts (
  conflict_id SERIAL PRIMARY KEY,
  series_id INTEGER NOT NULL REFERENCES anime_series(series_id) ON DELETE CASCADE,
  group_id INTEGER NOT NULL REFERENCES subtitle_groups(group_id) ON DELETE CASCADE,
  episode_no INTEGER NOT NULL,
  resolution_status VARCHAR(20) NOT NULL DEFAULT 'unresolved'
    CHECK (resolution_status IN ('unresolved', 'resolved')),
  chosen_link_id INTEGER REFERENCES anime_links(link_id) ON DELETE SET NULL,
  created_at TIMESTAMP NOT NULL DEFAULT NOW(),
  resolved_at TIMESTAMP,
  UNIQUE (series_id, group_id, episode_no)
);

-- 4) 索引
CREATE INDEX idx_anime_links_conflict ON anime_links(series_id, group_id, episode_no)
  WHERE link_status = 'active';
CREATE INDEX idx_anime_link_conflicts_unresolved ON anime_link_conflicts(resolution_status)
  WHERE resolution_status = 'unresolved';
