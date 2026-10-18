-- 重新同步中的記錄檔案仍在媒體庫，收斂回 synced 後才能加回舊約束
UPDATE downloads SET status = 'synced' WHERE status = 'resyncing';

ALTER TABLE downloads
    DROP CONSTRAINT IF EXISTS downloads_status_check;

ALTER TABLE downloads
    ADD CONSTRAINT downloads_status_check CHECK (status IN (
        'pending', 'downloading', 'completed', 'failed', 'cancelled',
        'downloader_error', 'no_downloader', 'syncing', 'synced', 'sync_failed',
        'batch_unmatched', 'batch_failed'
    ));
