-- Add file_path and sync_retry_count to downloads
ALTER TABLE downloads ADD COLUMN file_path TEXT;
ALTER TABLE downloads ADD COLUMN sync_retry_count INT NOT NULL DEFAULT 0;

-- Expand status constraint to include sync statuses
ALTER TABLE downloads DROP CONSTRAINT IF EXISTS downloads_status_check;
ALTER TABLE downloads ADD CONSTRAINT downloads_status_check
    CHECK (status IN ('pending', 'downloading', 'completed', 'failed', 'cancelled', 'downloader_error', 'no_downloader', 'syncing', 'synced', 'sync_failed'));
