CREATE TABLE anime_series (
  series_id SERIAL PRIMARY KEY,
  anime_id INTEGER NOT NULL REFERENCES animes(anime_id) ON DELETE CASCADE,
  series_no INTEGER NOT NULL,
  season_id INTEGER NOT NULL REFERENCES seasons(season_id) ON DELETE CASCADE,
  description TEXT,
  aired_date DATE,
  end_date DATE,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(anime_id, series_no)
);

CREATE INDEX idx_anime_series_anime_id ON anime_series(anime_id);
CREATE INDEX idx_anime_series_season_id ON anime_series(season_id);
