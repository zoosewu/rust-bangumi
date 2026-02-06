-- Auto-Download Dispatch System Migration

-- 1. Add download_type to anime_links
ALTER TABLE anime_links ADD COLUMN download_type VARCHAR(20);
CREATE INDEX idx_anime_links_download_type ON anime_links(download_type);

-- 2. Expand downloads table: add module_id, torrent_hash, expand status constraint
ALTER TABLE downloads DROP CONSTRAINT IF EXISTS downloads_status_check;
ALTER TABLE downloads ADD COLUMN module_id INT REFERENCES service_modules(module_id);
ALTER TABLE downloads ADD COLUMN torrent_hash VARCHAR(255);
ALTER TABLE downloads ADD CONSTRAINT downloads_status_check
    CHECK (status IN ('pending', 'downloading', 'completed', 'failed', 'cancelled', 'downloader_error', 'no_downloader'));
CREATE INDEX idx_downloads_module_status ON downloads(module_id, status);
CREATE INDEX idx_downloads_torrent_hash ON downloads(torrent_hash);

-- 3. Create downloader_capabilities junction table
CREATE TABLE downloader_capabilities (
    module_id INT REFERENCES service_modules(module_id) ON DELETE CASCADE,
    download_type VARCHAR(20) NOT NULL,
    PRIMARY KEY (module_id, download_type)
);
