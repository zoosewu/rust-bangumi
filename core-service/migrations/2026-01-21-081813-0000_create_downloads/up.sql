CREATE TABLE downloads (
  download_id SERIAL PRIMARY KEY,
  link_id INTEGER NOT NULL REFERENCES anime_links(link_id) ON DELETE CASCADE,
  downloader_type VARCHAR(50) NOT NULL,
  status VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'downloading', 'completed', 'failed')),
  progress DECIMAL(5, 2) DEFAULT 0.0,
  downloaded_bytes BIGINT DEFAULT 0,
  total_bytes BIGINT DEFAULT 0,
  error_message TEXT,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_downloads_link_id ON downloads(link_id);
CREATE INDEX idx_downloads_status ON downloads(status);
