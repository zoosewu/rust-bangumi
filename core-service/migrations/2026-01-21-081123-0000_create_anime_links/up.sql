CREATE TABLE anime_links (
  link_id SERIAL PRIMARY KEY,
  series_id INTEGER NOT NULL REFERENCES anime_series(series_id) ON DELETE CASCADE,
  group_id INTEGER NOT NULL REFERENCES subtitle_groups(group_id) ON DELETE CASCADE,
  episode_no INTEGER NOT NULL,
  title VARCHAR(255),
  url TEXT NOT NULL,
  source_hash VARCHAR(255) NOT NULL,
  filtered_flag BOOLEAN NOT NULL DEFAULT FALSE,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(series_id, group_id, episode_no, source_hash)
);

CREATE INDEX idx_anime_links_series_id ON anime_links(series_id);
CREATE INDEX idx_anime_links_group_id ON anime_links(group_id);
CREATE INDEX idx_anime_links_filtered ON anime_links(filtered_flag);
