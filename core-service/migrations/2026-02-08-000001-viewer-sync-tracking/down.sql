ALTER TABLE downloads DROP COLUMN IF EXISTS file_path;
ALTER TABLE downloads DROP COLUMN IF EXISTS sync_retry_count;

ALTER TABLE downloads DROP CONSTRAINT IF EXISTS downloads_status_check;
ALTER TABLE downloads ADD CONSTRAINT downloads_status_check
    CHECK (status IN ('pending', 'downloading', 'completed', 'failed', 'cancelled', 'downloader_error', 'no_downloader'));
