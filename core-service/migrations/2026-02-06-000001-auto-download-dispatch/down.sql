-- Revert Auto-Download Dispatch System Migration

DROP TABLE IF EXISTS downloader_capabilities;

DROP INDEX IF EXISTS idx_downloads_torrent_hash;
DROP INDEX IF EXISTS idx_downloads_module_status;
ALTER TABLE downloads DROP CONSTRAINT IF EXISTS downloads_status_check;
ALTER TABLE downloads DROP COLUMN IF EXISTS torrent_hash;
ALTER TABLE downloads DROP COLUMN IF EXISTS module_id;
ALTER TABLE downloads ADD CONSTRAINT downloads_status_check
    CHECK (status IN ('pending', 'downloading', 'completed', 'failed'));

DROP INDEX IF EXISTS idx_anime_links_download_type;
ALTER TABLE anime_links DROP COLUMN IF EXISTS download_type;
